use rem6_checkpoint::{
    CheckpointChunk, CheckpointComponentId, CheckpointManifest, CheckpointRegistry, CheckpointState,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, SystemActionExecutor,
    SystemActionOutcome, SystemError, VirtioPciDeviceConfigCheckpointBank,
    VirtioPciDeviceConfigCheckpointError, VirtioPciDeviceConfigCheckpointPort,
    VirtioPciDeviceConfigCheckpointRecord,
};
use rem6_virtio::{VirtioError, VirtioPciDeviceConfigDevice, VirtioPciDeviceConfigSpec};

fn device_config(bytes: Vec<u8>, writable: Vec<bool>) -> VirtioPciDeviceConfigDevice {
    VirtioPciDeviceConfigDevice::new(
        VirtioPciDeviceConfigSpec::new(bytes, ByteMask::from_bits(writable).unwrap()).unwrap(),
    )
}

fn mutate_config(config: &VirtioPciDeviceConfigDevice, address: u64, value: u8) {
    config
        .write_local(
            Address::new(address),
            vec![value],
            ByteMask::from_bits(vec![true]).unwrap(),
        )
        .unwrap();
}

fn corrupt_device_config_snapshot(
    manifest: &CheckpointManifest,
    component: &CheckpointComponentId,
) -> CheckpointManifest {
    CheckpointManifest::new(
        manifest.label().to_string(),
        manifest.tick(),
        manifest
            .states()
            .iter()
            .map(|state| {
                let chunks = state
                    .chunks()
                    .iter()
                    .map(|chunk| {
                        let mut payload = chunk.payload().to_vec();
                        if state.component() == component && chunk.name() == "device-config" {
                            payload[0] ^= 0xff;
                        }
                        CheckpointChunk::new(chunk.name().to_string(), payload)
                    })
                    .collect();
                CheckpointState::new(state.component().clone(), chunks)
            })
            .collect(),
    )
}

#[test]
fn virtio_pci_device_config_checkpoint_captures_and_restores_device_state() {
    let config = device_config(vec![0x11, 0x22, 0x33, 0x44], vec![false, true, true, false]);
    assert_eq!(
        config.read_local(Address::new(0), AccessSize::new(4).unwrap()),
        Ok(vec![0x11, 0x22, 0x33, 0x44])
    );
    config
        .write_local(
            Address::new(1),
            vec![0xaa, 0xbb],
            ByteMask::from_bits(vec![true, false]).unwrap(),
        )
        .unwrap();
    let expected = config.snapshot();
    let component = CheckpointComponentId::new("virtio.block0.device-config").unwrap();
    let port = VirtioPciDeviceConfigCheckpointPort::new(component.clone(), config.clone());
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        VirtioPciDeviceConfigCheckpointRecord::new(component.clone(), expected.clone())
    );
    assert_eq!(
        registry.chunk(&component, "device-config").unwrap(),
        expected.to_bytes()
    );

    mutate_config(&config, 2, 0xcc);
    assert_ne!(config.snapshot(), expected);

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(config.snapshot(), expected);
}

#[test]
fn system_action_executor_rejects_malformed_virtio_pci_device_config_snapshot() {
    let config = device_config(vec![0x10, 0x20, 0x30], vec![true, true, true]);
    mutate_config(&config, 1, 0xaa);
    let component = CheckpointComponentId::new("virtio.block0.device-config").unwrap();
    let bank =
        VirtioPciDeviceConfigCheckpointBank::new([VirtioPciDeviceConfigCheckpointPort::new(
            component.clone(),
            config.clone(),
        )])
        .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor
        .attach_virtio_pci_device_config_checkpoint_bank(bank)
        .unwrap();
    let checkpoint = HostActionRecord::new(
        17,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(4),
        GuestSourceId::new(6),
        HostAction::Checkpoint {
            label: "virtio-device-config".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let original_payload = executor
        .checkpoints()
        .chunk(&component, "device-config")
        .unwrap()
        .to_vec();
    mutate_config(&config, 2, 0xbb);
    let live_after_mutation = config.snapshot();
    let corrupted = corrupt_device_config_snapshot(&manifest, &component);
    let restore = HostActionRecord::new(
        29,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(5),
        GuestSourceId::new(6),
        HostAction::RestoreCheckpoint {
            manifest: corrupted,
        },
    );

    assert_eq!(
        executor.apply(&restore).unwrap_err(),
        SystemError::VirtioPciDeviceConfigCheckpoint(
            VirtioPciDeviceConfigCheckpointError::InvalidChunk {
                component: component.clone(),
                reason: VirtioError::InvalidDeviceConfigSnapshot.to_string(),
            }
        )
    );
    assert_eq!(config.snapshot(), live_after_mutation);
    assert_eq!(
        executor
            .checkpoints()
            .chunk(&component, "device-config")
            .unwrap(),
        original_payload
    );
}

#[test]
fn virtio_pci_device_config_bank_rejects_bad_second_snapshot_without_partial_restore() {
    let config0 = device_config(vec![0x11, 0x22], vec![true, true]);
    let config1 = device_config(vec![0x33, 0x44], vec![true, true]);
    mutate_config(&config0, 0, 0xaa);
    mutate_config(&config1, 0, 0xbb);
    let component0 = CheckpointComponentId::new("virtio.block0.device-config").unwrap();
    let component1 = CheckpointComponentId::new("virtio.net0.device-config").unwrap();
    let bank = VirtioPciDeviceConfigCheckpointBank::new([
        VirtioPciDeviceConfigCheckpointPort::new(component0.clone(), config0.clone()),
        VirtioPciDeviceConfigCheckpointPort::new(component1.clone(), config1.clone()),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();
    let mut bad_payload = registry
        .chunk(&component1, "device-config")
        .unwrap()
        .to_vec();
    bad_payload[0] ^= 0xff;
    registry
        .write_chunk(&component1, "device-config", bad_payload)
        .unwrap();

    mutate_config(&config0, 1, 0xcc);
    mutate_config(&config1, 1, 0xdd);
    let live0_after_mutation = config0.snapshot();
    let live1_after_mutation = config1.snapshot();

    assert_eq!(
        bank.restore_all_from(&registry).unwrap_err(),
        VirtioPciDeviceConfigCheckpointError::InvalidChunk {
            component: component1.clone(),
            reason: VirtioError::InvalidDeviceConfigSnapshot.to_string(),
        }
    );
    assert_eq!(config0.snapshot(), live0_after_mutation);
    assert_eq!(config1.snapshot(), live1_after_mutation);
}

use rem6_checkpoint::{
    CheckpointChunk, CheckpointComponentId, CheckpointManifest, CheckpointRegistry, CheckpointState,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, SystemActionExecutor,
    SystemActionOutcome, SystemError, VirtioPciCommonCheckpointBank,
    VirtioPciCommonCheckpointError, VirtioPciCommonCheckpointPort, VirtioPciCommonCheckpointRecord,
};
use rem6_virtio::{
    VirtioError, VirtioPciCommonConfigDevice, VirtioQueueSpec,
    VIRTIO_PCI_CONFIG_MSIX_VECTOR_OFFSET, VIRTIO_PCI_DEVICE_STATUS_OFFSET,
    VIRTIO_PCI_DRIVER_FEATURE_OFFSET, VIRTIO_PCI_DRIVER_FEATURE_SELECT_OFFSET,
    VIRTIO_PCI_QUEUE_DESC_OFFSET, VIRTIO_PCI_QUEUE_DEVICE_OFFSET, VIRTIO_PCI_QUEUE_DRIVER_OFFSET,
    VIRTIO_PCI_QUEUE_ENABLE_OFFSET, VIRTIO_PCI_QUEUE_MSIX_VECTOR_OFFSET,
    VIRTIO_PCI_QUEUE_SELECT_OFFSET, VIRTIO_PCI_QUEUE_SIZE_OFFSET, VIRTIO_STATUS_ACKNOWLEDGE,
    VIRTIO_STATUS_DRIVER, VIRTIO_STATUS_FEATURES_OK,
};

fn common_device() -> VirtioPciCommonConfigDevice {
    VirtioPciCommonConfigDevice::new(
        [(0, 0x0000_0005), (1, 0x0000_0002)],
        [
            VirtioQueueSpec::available(256, 0),
            VirtioQueueSpec::available(128, 1),
        ],
    )
    .unwrap()
}

fn write_u8(device: &VirtioPciCommonConfigDevice, offset: u64, value: u8) {
    device
        .write_local(
            Address::new(offset),
            vec![value],
            ByteMask::from_bits(vec![true]).unwrap(),
        )
        .unwrap();
}

fn write_u16(device: &VirtioPciCommonConfigDevice, offset: u64, value: u16) {
    device
        .write_local(
            Address::new(offset),
            value.to_le_bytes().to_vec(),
            ByteMask::from_bits(vec![true, true]).unwrap(),
        )
        .unwrap();
}

fn write_u32(device: &VirtioPciCommonConfigDevice, offset: u64, value: u32) {
    device
        .write_local(
            Address::new(offset),
            value.to_le_bytes().to_vec(),
            ByteMask::from_bits(vec![true, true, true, true]).unwrap(),
        )
        .unwrap();
}

fn write_u64(device: &VirtioPciCommonConfigDevice, offset: u64, value: u64) {
    device
        .write_local(
            Address::new(offset),
            value.to_le_bytes().to_vec(),
            ByteMask::from_bits(vec![true, true, true, true, true, true, true, true]).unwrap(),
        )
        .unwrap();
}

fn configure_common(device: &VirtioPciCommonConfigDevice) {
    write_u32(device, VIRTIO_PCI_DRIVER_FEATURE_SELECT_OFFSET, 1);
    write_u32(device, VIRTIO_PCI_DRIVER_FEATURE_OFFSET, 0x0000_0002);
    write_u16(device, VIRTIO_PCI_CONFIG_MSIX_VECTOR_OFFSET, 7);
    write_u16(device, VIRTIO_PCI_QUEUE_SELECT_OFFSET, 1);
    write_u16(device, VIRTIO_PCI_QUEUE_MSIX_VECTOR_OFFSET, 9);
    write_u16(device, VIRTIO_PCI_QUEUE_SIZE_OFFSET, 64);
    write_u64(device, VIRTIO_PCI_QUEUE_DESC_OFFSET, 0x0000_1000);
    write_u64(device, VIRTIO_PCI_QUEUE_DRIVER_OFFSET, 0x0000_2000);
    write_u64(device, VIRTIO_PCI_QUEUE_DEVICE_OFFSET, 0x0000_3000);
    write_u16(device, VIRTIO_PCI_QUEUE_ENABLE_OFFSET, 1);
    write_u8(
        device,
        VIRTIO_PCI_DEVICE_STATUS_OFFSET,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_FEATURES_OK,
    );
}

fn reset_common(device: &VirtioPciCommonConfigDevice) {
    write_u8(device, VIRTIO_PCI_DEVICE_STATUS_OFFSET, 0);
}

fn read_queue_size(device: &VirtioPciCommonConfigDevice) -> u16 {
    u16::from_le_bytes(
        device
            .read_local(
                Address::new(VIRTIO_PCI_QUEUE_SIZE_OFFSET),
                AccessSize::new(2).unwrap(),
            )
            .unwrap()
            .try_into()
            .unwrap(),
    )
}

fn corrupt_common_snapshot(
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
                        if state.component() == component && chunk.name() == "pci-common" {
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
fn virtio_pci_common_checkpoint_captures_and_restores_device_state() {
    let common = common_device();
    configure_common(&common);
    let expected = common.snapshot();
    let component = CheckpointComponentId::new("virtio.block0.pci-common").unwrap();
    let port = VirtioPciCommonCheckpointPort::new(component.clone(), common.clone());
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        VirtioPciCommonCheckpointRecord::new(component.clone(), expected.clone())
    );
    assert_eq!(
        registry.chunk(&component, "pci-common").unwrap(),
        expected.to_bytes()
    );

    reset_common(&common);
    assert_ne!(common.snapshot(), expected);

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(common.snapshot(), expected);
    assert_eq!(read_queue_size(&common), 64);
}

#[test]
fn system_action_executor_rejects_malformed_virtio_pci_common_snapshot() {
    let common = common_device();
    configure_common(&common);
    let component = CheckpointComponentId::new("virtio.block0.pci-common").unwrap();
    let bank = VirtioPciCommonCheckpointBank::new([VirtioPciCommonCheckpointPort::new(
        component.clone(),
        common.clone(),
    )])
    .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor
        .attach_virtio_pci_common_checkpoint_bank(bank)
        .unwrap();
    let checkpoint = HostActionRecord::new(
        17,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(4),
        GuestSourceId::new(6),
        HostAction::Checkpoint {
            label: "virtio-common".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let original_payload = executor
        .checkpoints()
        .chunk(&component, "pci-common")
        .unwrap()
        .to_vec();
    reset_common(&common);
    let live_after_mutation = common.snapshot();
    let corrupted = corrupt_common_snapshot(&manifest, &component);
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
        SystemError::VirtioPciCommonCheckpoint(VirtioPciCommonCheckpointError::InvalidChunk {
            component: component.clone(),
            reason: VirtioError::InvalidCommonConfigSnapshot.to_string(),
        })
    );
    assert_eq!(common.snapshot(), live_after_mutation);
    assert_eq!(
        executor
            .checkpoints()
            .chunk(&component, "pci-common")
            .unwrap(),
        original_payload
    );
}

#[test]
fn virtio_pci_common_bank_rejects_bad_second_snapshot_without_partial_restore() {
    let common0 = common_device();
    let common1 = common_device();
    configure_common(&common0);
    configure_common(&common1);
    let component0 = CheckpointComponentId::new("virtio.block0.pci-common").unwrap();
    let component1 = CheckpointComponentId::new("virtio.net0.pci-common").unwrap();
    let bank = VirtioPciCommonCheckpointBank::new([
        VirtioPciCommonCheckpointPort::new(component0.clone(), common0.clone()),
        VirtioPciCommonCheckpointPort::new(component1.clone(), common1.clone()),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();
    let mut bad_payload = registry.chunk(&component1, "pci-common").unwrap().to_vec();
    bad_payload[0] ^= 0xff;
    registry
        .write_chunk(&component1, "pci-common", bad_payload)
        .unwrap();

    reset_common(&common0);
    reset_common(&common1);
    let live0_after_mutation = common0.snapshot();
    let live1_after_mutation = common1.snapshot();

    assert_eq!(
        bank.restore_all_from(&registry).unwrap_err(),
        VirtioPciCommonCheckpointError::InvalidChunk {
            component: component1.clone(),
            reason: VirtioError::InvalidCommonConfigSnapshot.to_string(),
        }
    );
    assert_eq!(common0.snapshot(), live0_after_mutation);
    assert_eq!(common1.snapshot(), live1_after_mutation);
}

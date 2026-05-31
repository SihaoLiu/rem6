use rem6_checkpoint::{
    CheckpointChunk, CheckpointComponentId, CheckpointManifest, CheckpointRegistry, CheckpointState,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, SystemActionExecutor,
    SystemActionOutcome, SystemError, VirtioPciIsrCheckpointBank, VirtioPciIsrCheckpointError,
    VirtioPciIsrCheckpointPort, VirtioPciIsrCheckpointRecord,
};
use rem6_virtio::{VirtioError, VirtioPciIsrDevice};

fn clear_status(isr: &VirtioPciIsrDevice) {
    isr.read_local(Address::new(0), AccessSize::new(1).unwrap())
        .unwrap();
}

fn corrupt_isr_status(
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
                        if state.component() == component && chunk.name() == "pci-isr" {
                            payload[10] = 0x80;
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
fn virtio_pci_isr_checkpoint_captures_and_restores_device_state() {
    let isr = VirtioPciIsrDevice::new();
    isr.raise_queue_interrupt(5);
    isr.raise_configuration_change_interrupt(8);
    let expected = isr.snapshot();
    let component = CheckpointComponentId::new("virtio.block0.pci-isr").unwrap();
    let port = VirtioPciIsrCheckpointPort::new(component.clone(), isr.clone());
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        VirtioPciIsrCheckpointRecord::new(component.clone(), expected.clone())
    );
    assert_eq!(
        registry.chunk(&component, "pci-isr").unwrap(),
        expected.to_bytes()
    );

    clear_status(&isr);
    isr.raise_queue_interrupt(13);
    assert_ne!(isr.snapshot(), expected);

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(isr.snapshot(), expected);
}

#[test]
fn system_action_executor_rejects_malformed_virtio_pci_isr_snapshot() {
    let isr = VirtioPciIsrDevice::new();
    isr.raise_queue_interrupt(11);
    let component = CheckpointComponentId::new("virtio.block0.pci-isr").unwrap();
    let bank = VirtioPciIsrCheckpointBank::new([VirtioPciIsrCheckpointPort::new(
        component.clone(),
        isr.clone(),
    )])
    .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor
        .attach_virtio_pci_isr_checkpoint_bank(bank)
        .unwrap();
    let checkpoint = HostActionRecord::new(
        17,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(4),
        GuestSourceId::new(6),
        HostAction::Checkpoint {
            label: "virtio-isr".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let original_payload = executor
        .checkpoints()
        .chunk(&component, "pci-isr")
        .unwrap()
        .to_vec();
    clear_status(&isr);
    isr.raise_configuration_change_interrupt(23);
    let live_after_mutation = isr.snapshot();
    let corrupted = corrupt_isr_status(&manifest, &component);
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
        SystemError::VirtioPciIsrCheckpoint(VirtioPciIsrCheckpointError::InvalidChunk {
            component: component.clone(),
            reason: VirtioError::InvalidPciIsrSnapshot.to_string(),
        })
    );
    assert_eq!(isr.snapshot(), live_after_mutation);
    assert_eq!(
        executor.checkpoints().chunk(&component, "pci-isr").unwrap(),
        original_payload
    );
}

#[test]
fn virtio_pci_isr_bank_rejects_bad_second_snapshot_without_partial_restore() {
    let isr0 = VirtioPciIsrDevice::new();
    let isr1 = VirtioPciIsrDevice::new();
    isr0.raise_queue_interrupt(3);
    isr1.raise_configuration_change_interrupt(7);
    let component0 = CheckpointComponentId::new("virtio.net0.pci-isr").unwrap();
    let component1 = CheckpointComponentId::new("virtio.block0.pci-isr").unwrap();
    let bank = VirtioPciIsrCheckpointBank::new([
        VirtioPciIsrCheckpointPort::new(component0.clone(), isr0.clone()),
        VirtioPciIsrCheckpointPort::new(component1.clone(), isr1.clone()),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();
    let mut bad_payload = registry.chunk(&component1, "pci-isr").unwrap().to_vec();
    bad_payload[10] = 0x80;
    registry
        .write_chunk(&component1, "pci-isr", bad_payload)
        .unwrap();

    clear_status(&isr0);
    isr0.raise_configuration_change_interrupt(19);
    clear_status(&isr1);
    isr1.raise_queue_interrupt(31);
    let live0_after_mutation = isr0.snapshot();
    let live1_after_mutation = isr1.snapshot();

    assert_eq!(
        bank.restore_all_from(&registry).unwrap_err(),
        VirtioPciIsrCheckpointError::InvalidChunk {
            component: component1.clone(),
            reason: VirtioError::InvalidPciIsrSnapshot.to_string(),
        }
    );
    assert_eq!(isr0.snapshot(), live0_after_mutation);
    assert_eq!(isr1.snapshot(), live1_after_mutation);
}

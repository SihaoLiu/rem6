use rem6_checkpoint::{
    CheckpointChunk, CheckpointComponentId, CheckpointManifest, CheckpointRegistry, CheckpointState,
};
use rem6_kernel::PartitionId;
use rem6_memory::{Address, ByteMask};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, SystemActionExecutor,
    SystemActionOutcome, SystemError, VirtioPciNotifyCheckpointBank,
    VirtioPciNotifyCheckpointError, VirtioPciNotifyCheckpointPort, VirtioPciNotifyCheckpointRecord,
};
use rem6_virtio::{VirtioError, VirtioPciNotifyDevice, VirtioQueueIndex, VirtioQueueNotifySpec};

fn notify_device() -> VirtioPciNotifyDevice {
    VirtioPciNotifyDevice::new(
        4,
        [
            VirtioQueueNotifySpec::new(VirtioQueueIndex::new(0).unwrap(), 0),
            VirtioQueueNotifySpec::new(VirtioQueueIndex::new(1).unwrap(), 3),
        ],
    )
    .unwrap()
}

fn notify_queue(notify: &VirtioPciNotifyDevice, address: u64, value: u16, tick: u64) {
    notify
        .write_local(
            Address::new(address),
            value.to_le_bytes().to_vec(),
            ByteMask::from_bits(vec![true, true]).unwrap(),
            tick,
        )
        .unwrap();
}

fn corrupt_notify_snapshot(
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
                        if state.component() == component && chunk.name() == "pci-notify" {
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
fn virtio_pci_notify_checkpoint_captures_and_restores_device_state() {
    let notify = notify_device();
    notify_queue(&notify, 0, 0, 7);
    let expected = notify.snapshot();
    let component = CheckpointComponentId::new("virtio.block0.pci-notify").unwrap();
    let port = VirtioPciNotifyCheckpointPort::new(component.clone(), notify.clone());
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        VirtioPciNotifyCheckpointRecord::new(component.clone(), expected.clone())
    );
    assert_eq!(
        registry.chunk(&component, "pci-notify").unwrap(),
        expected.to_bytes()
    );

    notify_queue(&notify, 12, 1, 12);
    assert_ne!(notify.snapshot(), expected);

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(notify.snapshot(), expected);
}

#[test]
fn system_action_executor_rejects_malformed_virtio_pci_notify_snapshot() {
    let notify = notify_device();
    notify_queue(&notify, 0, 0, 3);
    let component = CheckpointComponentId::new("virtio.block0.pci-notify").unwrap();
    let bank = VirtioPciNotifyCheckpointBank::new([VirtioPciNotifyCheckpointPort::new(
        component.clone(),
        notify.clone(),
    )])
    .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor
        .attach_virtio_pci_notify_checkpoint_bank(bank)
        .unwrap();
    let checkpoint = HostActionRecord::new(
        17,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(4),
        GuestSourceId::new(6),
        HostAction::Checkpoint {
            label: "virtio-notify".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let original_payload = executor
        .checkpoints()
        .chunk(&component, "pci-notify")
        .unwrap()
        .to_vec();
    notify_queue(&notify, 12, 1, 11);
    let live_after_mutation = notify.snapshot();
    let corrupted = corrupt_notify_snapshot(&manifest, &component);
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
        SystemError::VirtioPciNotifyCheckpoint(VirtioPciNotifyCheckpointError::InvalidChunk {
            component: component.clone(),
            reason: VirtioError::InvalidNotifySnapshot.to_string(),
        })
    );
    assert_eq!(notify.snapshot(), live_after_mutation);
    assert_eq!(
        executor
            .checkpoints()
            .chunk(&component, "pci-notify")
            .unwrap(),
        original_payload
    );
}

#[test]
fn virtio_pci_notify_bank_rejects_bad_second_snapshot_without_partial_restore() {
    let notify0 = notify_device();
    let notify1 = notify_device();
    notify_queue(&notify0, 0, 0, 3);
    notify_queue(&notify1, 0, 0, 5);
    let component0 = CheckpointComponentId::new("virtio.block0.pci-notify").unwrap();
    let component1 = CheckpointComponentId::new("virtio.net0.pci-notify").unwrap();
    let bank = VirtioPciNotifyCheckpointBank::new([
        VirtioPciNotifyCheckpointPort::new(component0.clone(), notify0.clone()),
        VirtioPciNotifyCheckpointPort::new(component1.clone(), notify1.clone()),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();
    let mut bad_payload = registry.chunk(&component1, "pci-notify").unwrap().to_vec();
    bad_payload[0] ^= 0xff;
    registry
        .write_chunk(&component1, "pci-notify", bad_payload)
        .unwrap();

    notify_queue(&notify0, 12, 1, 13);
    notify_queue(&notify1, 12, 1, 17);
    let live0_after_mutation = notify0.snapshot();
    let live1_after_mutation = notify1.snapshot();

    assert_eq!(
        bank.restore_all_from(&registry).unwrap_err(),
        VirtioPciNotifyCheckpointError::InvalidChunk {
            component: component1.clone(),
            reason: VirtioError::InvalidNotifySnapshot.to_string(),
        }
    );
    assert_eq!(notify0.snapshot(), live0_after_mutation);
    assert_eq!(notify1.snapshot(), live1_after_mutation);
}

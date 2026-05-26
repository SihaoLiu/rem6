use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, SystemActionExecutor,
    SystemActionOutcome, VirtioSplitQueueCheckpointBank, VirtioSplitQueueCheckpointPort,
    VirtioSplitQueueCheckpointRecord,
};
use rem6_virtio::VirtioSplitQueue;

fn queue(last_available_index: u16, event_index: bool) -> VirtioSplitQueue {
    VirtioSplitQueue::new(
        8,
        Address::new(0x1000),
        Address::new(0x2000),
        Address::new(0x3000),
        last_available_index,
    )
    .unwrap()
    .with_event_index(event_index)
}

#[test]
fn virtio_split_queue_checkpoint_captures_and_restores_queue_state() {
    let live = Arc::new(Mutex::new(queue(5, true)));
    let component = CheckpointComponentId::new("virtio.block0.queue0").unwrap();
    let port = VirtioSplitQueueCheckpointPort::new(component.clone(), Arc::clone(&live));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        VirtioSplitQueueCheckpointRecord::new(component.clone(), live.lock().unwrap().snapshot())
    );
    assert_eq!(registry.chunk(&component, "split-queue").unwrap().len(), 29);

    *live.lock().unwrap() = queue(0, false);

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(live.lock().unwrap().snapshot(), captured.snapshot().clone());
}

#[test]
fn system_action_executor_checkpoints_and_restores_virtio_split_queue() {
    let live = Arc::new(Mutex::new(queue(6, true)));
    let expected = live.lock().unwrap().snapshot();
    let component = CheckpointComponentId::new("virtio.block0.queue0").unwrap();
    let bank = VirtioSplitQueueCheckpointBank::new([VirtioSplitQueueCheckpointPort::new(
        component.clone(),
        Arc::clone(&live),
    )])
    .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor
        .attach_virtio_split_queue_checkpoint_bank(bank)
        .unwrap();

    let checkpoint = HostActionRecord::new(
        18,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(1),
        GuestSourceId::new(9),
        HostAction::Checkpoint {
            label: "virtio-ready".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    assert!(manifest.states().iter().any(|state| {
        state.component() == &component
            && state
                .chunks()
                .iter()
                .any(|chunk| chunk.name() == "split-queue")
    }));

    *live.lock().unwrap() = queue(0, false);

    let restore = HostActionRecord::new(
        24,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(2),
        GuestSourceId::new(9),
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    assert_eq!(
        executor.apply(&restore).unwrap(),
        SystemActionOutcome::CheckpointRestored {
            tick: 24,
            event: GuestEventId::new(2),
            source: GuestSourceId::new(9),
            manifest,
        }
    );
    assert_eq!(live.lock().unwrap().snapshot(), expected);
}

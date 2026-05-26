use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_kernel::PartitionId;
use rem6_memory::Address;
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, SystemActionExecutor,
    SystemActionOutcome, SystemError, VirtioSplitQueueCheckpointBank,
    VirtioSplitQueueCheckpointError, VirtioSplitQueueCheckpointPort,
    VirtioSplitQueueCheckpointRecord,
};
use rem6_virtio::VirtioSplitQueue;

fn queue(last_available_index: u16, event_index: bool) -> VirtioSplitQueue {
    queue_at(0x1000, 0x2000, 0x3000, last_available_index, event_index)
}

fn queue_at(
    descriptor_table: u64,
    available_ring: u64,
    used_ring: u64,
    last_available_index: u16,
    event_index: bool,
) -> VirtioSplitQueue {
    VirtioSplitQueue::new(
        8,
        Address::new(descriptor_table),
        Address::new(available_ring),
        Address::new(used_ring),
        last_available_index,
    )
    .unwrap()
    .with_event_index(event_index)
}

fn split_queue_payload(
    descriptor_table: u64,
    available_ring: u64,
    used_ring: u64,
    last_available_index: u16,
    event_index: bool,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend(8_u16.to_le_bytes());
    payload.extend(descriptor_table.to_le_bytes());
    payload.extend(available_ring.to_le_bytes());
    payload.extend(used_ring.to_le_bytes());
    payload.extend(last_available_index.to_le_bytes());
    payload.push(u8::from(event_index));
    payload
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

#[test]
fn system_action_executor_rejects_virtio_split_queue_shape_mismatch_without_partial_restore() {
    let queue0 = Arc::new(Mutex::new(queue_at(0x1000, 0x2000, 0x3000, 0, false)));
    let queue1 = Arc::new(Mutex::new(queue_at(0x4000, 0x5000, 0x6000, 0, false)));
    let component0 = CheckpointComponentId::new("virtio.block0.queue0").unwrap();
    let component1 = CheckpointComponentId::new("virtio.block0.queue1").unwrap();
    let bank = VirtioSplitQueueCheckpointBank::new([
        VirtioSplitQueueCheckpointPort::new(component0.clone(), Arc::clone(&queue0)),
        VirtioSplitQueueCheckpointPort::new(component1.clone(), Arc::clone(&queue1)),
    ])
    .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor
        .attach_virtio_split_queue_checkpoint_bank(bank)
        .unwrap();
    executor
        .checkpoints_mut()
        .write_chunk(
            &component0,
            "split-queue",
            split_queue_payload(0x1000, 0x2000, 0x3000, 5, true),
        )
        .unwrap();
    executor
        .checkpoints_mut()
        .write_chunk(
            &component1,
            "split-queue",
            split_queue_payload(0x4000, 0x5000, 0x7000, 7, true),
        )
        .unwrap();
    let manifest = executor.checkpoints().capture("bad-virtio", 31).unwrap();
    let before0 = queue0.lock().unwrap().snapshot();
    let before1 = queue1.lock().unwrap().snapshot();
    let restore = HostActionRecord::new(
        37,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(3),
        GuestSourceId::new(9),
        HostAction::RestoreCheckpoint { manifest },
    );

    let error = executor.apply(&restore).unwrap_err();

    match error {
        SystemError::VirtioCheckpoint(VirtioSplitQueueCheckpointError::Virtio {
            component,
            error,
        }) => {
            assert_eq!(component, component1);
            assert!(error.to_string().contains("snapshot shape mismatch"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(queue0.lock().unwrap().snapshot(), before0);
    assert_eq!(queue1.lock().unwrap().snapshot(), before1);
}

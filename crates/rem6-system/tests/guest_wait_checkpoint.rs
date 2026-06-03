use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_kernel::PartitionId;
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestChildStatus, GuestEventId, GuestProcessGroupId, GuestProcessId, GuestSignal,
    GuestSourceId, GuestWaitCheckpointBank, GuestWaitCheckpointError, GuestWaitCheckpointPort,
    GuestWaitOptions, GuestWaitOutcome, GuestWaitQueue, GuestWaitSelector, GuestWaitStatus,
    HostAction, HostActionRecord, SystemActionExecutor, SystemActionOutcome, SystemError,
};

const GUEST_WAIT_CHUNK: &str = "guest-wait";
const GUEST_WAIT_CHECKPOINT_VERSION: u64 = 1;
const WAIT_STATUS_EXITED: u32 = 0;
const WAIT_STATUS_SIGNALED: u32 = 1;
const WAIT_STATUS_STOPPED: u32 = 2;
const WAIT_STATUS_CONTINUED: u32 = 3;

fn checkpoint_component(name: &str) -> CheckpointComponentId {
    CheckpointComponentId::new(name).unwrap()
}

fn push_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn guest_wait_checkpoint_payload(
    current_pgid: u32,
    children: &[(u32, u32, u32, u32, bool)],
) -> Vec<u8> {
    let mut payload = Vec::new();
    push_u64(&mut payload, GUEST_WAIT_CHECKPOINT_VERSION);
    push_u32(&mut payload, current_pgid);
    push_u64(&mut payload, children.len() as u64);
    for (pid, pgid, status_tag, status_value, core_dumped) in children {
        push_u32(&mut payload, *pid);
        push_u32(&mut payload, *pgid);
        push_u32(&mut payload, *status_tag);
        push_u32(&mut payload, *status_value);
        push_u64(&mut payload, u64::from(*core_dumped));
    }
    payload
}

fn populated_guest_wait_queue() -> Arc<Mutex<GuestWaitQueue>> {
    let queue = Arc::new(Mutex::new(GuestWaitQueue::new(
        GuestProcessGroupId::new(10).unwrap(),
    )));
    {
        let mut locked = queue.lock().unwrap();
        locked.push(GuestChildStatus::new(
            GuestProcessId::new(100).unwrap(),
            GuestProcessGroupId::new(11).unwrap(),
            GuestWaitStatus::exited(1),
        ));
        locked.push(GuestChildStatus::new(
            GuestProcessId::new(101).unwrap(),
            GuestProcessGroupId::new(10).unwrap(),
            GuestWaitStatus::signaled(GuestSignal::new(6).unwrap(), true),
        ));
        locked.push(GuestChildStatus::new(
            GuestProcessId::new(102).unwrap(),
            GuestProcessGroupId::new(11).unwrap(),
            GuestWaitStatus::stopped(GuestSignal::new(19).unwrap()),
        ));
    }
    queue
}

fn host_record(tick: u64, event: u64, action: HostAction) -> HostActionRecord {
    let host = PartitionId::new(1);
    HostActionRecord::new(
        tick,
        host,
        host,
        GuestEventId::new(event),
        GuestSourceId::new(7),
        action,
    )
}

#[test]
fn guest_wait_checkpoint_bank_round_trips_pending_children_and_selectors() {
    let component = checkpoint_component("guest.wait0");
    let source_queue = populated_guest_wait_queue();
    let expected = source_queue.lock().unwrap().snapshot();
    let capture_bank = GuestWaitCheckpointBank::new([GuestWaitCheckpointPort::new(
        component.clone(),
        source_queue.clone(),
    )])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    capture_bank.register_all(&mut registry).unwrap();

    let captured = capture_bank.capture_all_into(&mut registry).unwrap();

    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].component(), &component);
    assert_eq!(captured[0].snapshot(), &expected);
    assert!(registry.chunk(&component, GUEST_WAIT_CHUNK).is_some());

    let restore_queue = Arc::new(Mutex::new(GuestWaitQueue::new(
        GuestProcessGroupId::new(99).unwrap(),
    )));
    restore_queue.lock().unwrap().push(GuestChildStatus::new(
        GuestProcessId::new(900).unwrap(),
        GuestProcessGroupId::new(99).unwrap(),
        GuestWaitStatus::continued(),
    ));
    let restore_bank = GuestWaitCheckpointBank::new([GuestWaitCheckpointPort::new(
        component.clone(),
        restore_queue.clone(),
    )])
    .unwrap();

    let restored = restore_bank.restore_all_from(&registry).unwrap();

    assert_eq!(restored[0].snapshot(), &expected);
    assert_eq!(restore_queue.lock().unwrap().snapshot(), expected);
    assert_eq!(
        restore_queue.lock().unwrap().wait(
            GuestWaitSelector::from_wait4_pid(0).unwrap(),
            GuestWaitOptions::blocking()
        ),
        GuestWaitOutcome::Ready(GuestChildStatus::new(
            GuestProcessId::new(101).unwrap(),
            GuestProcessGroupId::new(10).unwrap(),
            GuestWaitStatus::signaled(GuestSignal::new(6).unwrap(), true),
        ))
    );
    assert_eq!(
        restore_queue.lock().unwrap().wait(
            GuestWaitSelector::from_wait4_pid(-11).unwrap(),
            GuestWaitOptions::blocking()
        ),
        GuestWaitOutcome::Ready(GuestChildStatus::new(
            GuestProcessId::new(100).unwrap(),
            GuestProcessGroupId::new(11).unwrap(),
            GuestWaitStatus::exited(1),
        ))
    );
}

#[test]
fn guest_wait_checkpoint_restore_rejects_malformed_payload_without_mutating_queue() {
    let component = checkpoint_component("guest.wait.bad");
    let queue = populated_guest_wait_queue();
    let before = queue.lock().unwrap().snapshot();
    let bank = GuestWaitCheckpointBank::new([GuestWaitCheckpointPort::new(
        component.clone(),
        queue.clone(),
    )])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    registry
        .write_chunk(
            &component,
            GUEST_WAIT_CHUNK,
            guest_wait_checkpoint_payload(10, &[(0, 10, WAIT_STATUS_EXITED, 1, false)]),
        )
        .unwrap();

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert!(matches!(
        error,
        GuestWaitCheckpointError::InvalidChunk {
            component: error_component,
            ..
        } if error_component == component
    ));
    assert_eq!(queue.lock().unwrap().snapshot(), before);
}

#[test]
fn guest_wait_checkpoint_restore_rejects_status_shape_errors_without_mutating_queue() {
    let component = checkpoint_component("guest.wait.shape");
    let queue = populated_guest_wait_queue();
    let before = queue.lock().unwrap().snapshot();
    let bank = GuestWaitCheckpointBank::new([GuestWaitCheckpointPort::new(
        component.clone(),
        queue.clone(),
    )])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    let mut unsupported = Vec::new();
    push_u64(&mut unsupported, GUEST_WAIT_CHECKPOINT_VERSION + 1);
    push_u32(&mut unsupported, 10);
    push_u64(&mut unsupported, 0);
    registry
        .write_chunk(&component, GUEST_WAIT_CHUNK, unsupported)
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestWaitCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(queue.lock().unwrap().snapshot(), before);

    registry
        .write_chunk(&component, GUEST_WAIT_CHUNK, vec![1])
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestWaitCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(queue.lock().unwrap().snapshot(), before);

    registry
        .write_chunk(
            &component,
            GUEST_WAIT_CHUNK,
            guest_wait_checkpoint_payload(10, &[(101, 10, WAIT_STATUS_STOPPED, 19, true)]),
        )
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestWaitCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(queue.lock().unwrap().snapshot(), before);

    registry
        .write_chunk(
            &component,
            GUEST_WAIT_CHUNK,
            guest_wait_checkpoint_payload(10, &[(101, 10, WAIT_STATUS_SIGNALED, 128, false)]),
        )
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestWaitCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(queue.lock().unwrap().snapshot(), before);

    registry
        .write_chunk(
            &component,
            GUEST_WAIT_CHUNK,
            guest_wait_checkpoint_payload(10, &[(101, 10, WAIT_STATUS_EXITED, 1, true)]),
        )
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestWaitCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(queue.lock().unwrap().snapshot(), before);

    let mut trailing =
        guest_wait_checkpoint_payload(10, &[(101, 10, WAIT_STATUS_CONTINUED, 0, false)]);
    trailing.push(0xaa);
    registry
        .write_chunk(&component, GUEST_WAIT_CHUNK, trailing)
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestWaitCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(queue.lock().unwrap().snapshot(), before);
}

#[test]
fn guest_wait_checkpoint_restore_rejects_unbounded_counts_without_mutating_queue() {
    let component = checkpoint_component("guest.wait.counts");
    let queue = populated_guest_wait_queue();
    let before = queue.lock().unwrap().snapshot();
    let bank = GuestWaitCheckpointBank::new([GuestWaitCheckpointPort::new(
        component.clone(),
        queue.clone(),
    )])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    let mut huge_children = Vec::new();
    push_u64(&mut huge_children, GUEST_WAIT_CHECKPOINT_VERSION);
    push_u32(&mut huge_children, 10);
    push_u64(&mut huge_children, u64::MAX);
    registry
        .write_chunk(&component, GUEST_WAIT_CHUNK, huge_children)
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestWaitCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(queue.lock().unwrap().snapshot(), before);
}

#[test]
fn guest_wait_checkpoint_bank_prevalidates_all_ports_before_restore() {
    let valid_component = checkpoint_component("guest.wait.valid-port");
    let invalid_component = checkpoint_component("guest.wait.invalid-port");
    let valid_queue = populated_guest_wait_queue();
    let valid_before = valid_queue.lock().unwrap().snapshot();
    let invalid_queue = populated_guest_wait_queue();
    let invalid_before = invalid_queue.lock().unwrap().snapshot();
    let bank = GuestWaitCheckpointBank::new([
        GuestWaitCheckpointPort::new(valid_component.clone(), valid_queue.clone()),
        GuestWaitCheckpointPort::new(invalid_component.clone(), invalid_queue.clone()),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    registry
        .write_chunk(
            &valid_component,
            GUEST_WAIT_CHUNK,
            guest_wait_checkpoint_payload(10, &[(101, 10, WAIT_STATUS_EXITED, 1, false)]),
        )
        .unwrap();
    registry
        .write_chunk(
            &invalid_component,
            GUEST_WAIT_CHUNK,
            guest_wait_checkpoint_payload(10, &[(102, 0, WAIT_STATUS_EXITED, 2, false)]),
        )
        .unwrap();

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert!(matches!(
        error,
        GuestWaitCheckpointError::InvalidChunk {
            component: error_component,
            ..
        } if error_component == invalid_component
    ));
    assert_eq!(valid_queue.lock().unwrap().snapshot(), valid_before);
    assert_eq!(invalid_queue.lock().unwrap().snapshot(), invalid_before);
}

#[test]
fn system_action_executor_checkpoints_and_restores_guest_wait_queues() {
    let component = checkpoint_component("guest.wait.host");
    let queue = populated_guest_wait_queue();
    let expected = queue.lock().unwrap().snapshot();
    let bank = GuestWaitCheckpointBank::new([GuestWaitCheckpointPort::new(
        component.clone(),
        queue.clone(),
    )])
    .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor.attach_guest_wait_checkpoint_bank(bank).unwrap();

    let checkpoint = host_record(
        20,
        1,
        HostAction::Checkpoint {
            label: "guest-waits".to_string(),
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
                .any(|chunk| chunk.name() == GUEST_WAIT_CHUNK)
    }));

    queue
        .lock()
        .unwrap()
        .wait(GuestWaitSelector::AnyChild, GuestWaitOptions::blocking());
    let restore = host_record(
        40,
        2,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );

    assert_eq!(
        executor.apply(&restore).unwrap(),
        SystemActionOutcome::CheckpointRestored {
            tick: 40,
            event: GuestEventId::new(2),
            source: GuestSourceId::new(7),
            manifest,
        }
    );
    assert_eq!(queue.lock().unwrap().snapshot(), expected);
}

#[test]
fn system_action_executor_rejects_malformed_guest_wait_checkpoint_without_mutation() {
    let component = checkpoint_component("guest.wait.host.bad");
    let queue = populated_guest_wait_queue();
    let before = queue.lock().unwrap().snapshot();
    let bank = GuestWaitCheckpointBank::new([GuestWaitCheckpointPort::new(
        component.clone(),
        queue.clone(),
    )])
    .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor.attach_guest_wait_checkpoint_bank(bank).unwrap();
    let manifest = rem6_checkpoint::CheckpointManifest::new(
        "bad-guest-waits",
        50,
        vec![rem6_checkpoint::CheckpointState::new(
            component.clone(),
            vec![rem6_checkpoint::CheckpointChunk::new(
                GUEST_WAIT_CHUNK,
                guest_wait_checkpoint_payload(10, &[(102, 0, WAIT_STATUS_EXITED, 2, false)]),
            )],
        )],
    );
    let restore = host_record(
        60,
        3,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );

    let error = executor.apply(&restore).unwrap_err();

    assert!(matches!(
        error,
        SystemError::GuestWaitCheckpoint(GuestWaitCheckpointError::InvalidChunk {
            component: error_component,
            ..
        }) if error_component == component
    ));
    assert_eq!(queue.lock().unwrap().snapshot(), before);
}

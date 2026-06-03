use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_kernel::PartitionId;
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestFutexAddress, GuestFutexCheckpointBank, GuestFutexCheckpointError,
    GuestFutexCheckpointPort, GuestFutexKey, GuestFutexTable, GuestFutexWaitRequest, GuestSourceId,
    GuestThreadGroupId, GuestThreadId, HostAction, HostActionRecord, SystemActionExecutor,
    SystemActionOutcome, SystemError,
};

const GUEST_FUTEX_CHUNK: &str = "guest-futex";
const GUEST_FUTEX_CHECKPOINT_VERSION: u64 = 1;

fn checkpoint_component(name: &str) -> CheckpointComponentId {
    CheckpointComponentId::new(name).unwrap()
}

fn push_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn guest_futex_checkpoint_payload(waiters: &[(u64, u64, u64, u32, u64, u32)]) -> Vec<u8> {
    let mut payload = Vec::new();
    push_u64(&mut payload, GUEST_FUTEX_CHECKPOINT_VERSION);
    push_u64(&mut payload, waiters.len() as u64);
    for (address, thread_group, thread, partition, tick, bitset) in waiters {
        push_u64(&mut payload, *address);
        push_u64(&mut payload, *thread_group);
        push_u64(&mut payload, *thread);
        push_u32(&mut payload, *partition);
        push_u64(&mut payload, *tick);
        push_u32(&mut payload, *bitset);
    }
    payload
}

fn populated_guest_futex_table() -> Arc<Mutex<GuestFutexTable>> {
    let table = Arc::new(Mutex::new(GuestFutexTable::new()));
    let address = GuestFutexAddress::new(0x181c08);
    let other_address = GuestFutexAddress::new(0x181d00);
    let thread_group = GuestThreadGroupId::new(42);
    let key = GuestFutexKey::new(address, thread_group);
    let other_key = GuestFutexKey::new(other_address, thread_group);
    {
        let mut locked = table.lock().unwrap();
        locked
            .wait(
                GuestFutexWaitRequest::new(
                    key,
                    GuestThreadId::new(1),
                    PartitionId::new(1),
                    100,
                    7,
                    7,
                )
                .with_bitset(0x1),
            )
            .unwrap();
        locked
            .wait(
                GuestFutexWaitRequest::new(
                    key,
                    GuestThreadId::new(2),
                    PartitionId::new(2),
                    101,
                    7,
                    7,
                )
                .with_bitset(0x4),
            )
            .unwrap();
        locked
            .wait(
                GuestFutexWaitRequest::new(
                    other_key,
                    GuestThreadId::new(3),
                    PartitionId::new(3),
                    102,
                    9,
                    9,
                )
                .with_bitset(0x8),
            )
            .unwrap();
    }
    table
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
fn guest_futex_checkpoint_bank_round_trips_waiters_and_bitsets() {
    let component = checkpoint_component("guest.futex0");
    let source_table = populated_guest_futex_table();
    let expected = source_table.lock().unwrap().snapshot();
    let capture_bank = GuestFutexCheckpointBank::new([GuestFutexCheckpointPort::new(
        component.clone(),
        source_table.clone(),
    )])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    capture_bank.register_all(&mut registry).unwrap();

    let captured = capture_bank.capture_all_into(&mut registry).unwrap();

    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].component(), &component);
    assert_eq!(captured[0].snapshot(), &expected);
    assert!(registry.chunk(&component, GUEST_FUTEX_CHUNK).is_some());

    let restore_table = Arc::new(Mutex::new(GuestFutexTable::new()));
    restore_table
        .lock()
        .unwrap()
        .wait(GuestFutexWaitRequest::new(
            GuestFutexKey::new(GuestFutexAddress::new(0x9000), GuestThreadGroupId::new(90)),
            GuestThreadId::new(90),
            PartitionId::new(9),
            10,
            1,
            1,
        ))
        .unwrap();
    let restore_bank = GuestFutexCheckpointBank::new([GuestFutexCheckpointPort::new(
        component.clone(),
        restore_table.clone(),
    )])
    .unwrap();

    let restored = restore_bank.restore_all_from(&registry).unwrap();

    assert_eq!(restored[0].snapshot(), &expected);
    assert_eq!(restore_table.lock().unwrap().snapshot(), expected);
    let wake = restore_table
        .lock()
        .unwrap()
        .wake_bitset(
            GuestFutexAddress::new(0x181c08),
            GuestThreadGroupId::new(42),
            10,
            0x4,
            200,
        )
        .unwrap();
    assert_eq!(wake.woken_threads(), vec![GuestThreadId::new(2)]);
    assert_eq!(
        restore_table.lock().unwrap().waiter_threads(
            GuestFutexAddress::new(0x181c08),
            GuestThreadGroupId::new(42)
        ),
        vec![GuestThreadId::new(1)]
    );
}

#[test]
fn guest_futex_checkpoint_restore_rejects_malformed_payload_without_mutating_table() {
    let component = checkpoint_component("guest.futex.bad");
    let table = populated_guest_futex_table();
    let before = table.lock().unwrap().snapshot();
    let bank = GuestFutexCheckpointBank::new([GuestFutexCheckpointPort::new(
        component.clone(),
        table.clone(),
    )])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    registry
        .write_chunk(
            &component,
            GUEST_FUTEX_CHUNK,
            guest_futex_checkpoint_payload(&[
                (0x1000, 1, 11, 0, 10, u32::MAX),
                (0x2000, 1, 11, 1, 11, u32::MAX),
            ]),
        )
        .unwrap();

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert!(matches!(
        error,
        GuestFutexCheckpointError::InvalidChunk {
            component: error_component,
            ..
        } if error_component == component
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);
}

#[test]
fn guest_futex_checkpoint_restore_rejects_shape_errors_without_mutating_table() {
    let component = checkpoint_component("guest.futex.shape");
    let table = populated_guest_futex_table();
    let before = table.lock().unwrap().snapshot();
    let bank = GuestFutexCheckpointBank::new([GuestFutexCheckpointPort::new(
        component.clone(),
        table.clone(),
    )])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    let mut unsupported = Vec::new();
    push_u64(&mut unsupported, GUEST_FUTEX_CHECKPOINT_VERSION + 1);
    push_u64(&mut unsupported, 0);
    registry
        .write_chunk(&component, GUEST_FUTEX_CHUNK, unsupported)
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestFutexCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);

    registry
        .write_chunk(&component, GUEST_FUTEX_CHUNK, vec![1])
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestFutexCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);

    let mut trailing = guest_futex_checkpoint_payload(&[(0x3000, 2, 12, 0, 12, u32::MAX)]);
    trailing.push(0xaa);
    registry
        .write_chunk(&component, GUEST_FUTEX_CHUNK, trailing)
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestFutexCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);

    registry
        .write_chunk(
            &component,
            GUEST_FUTEX_CHUNK,
            guest_futex_checkpoint_payload(&[(0x4000, 2, 13, 0, 13, 0)]),
        )
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestFutexCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);
}

#[test]
fn guest_futex_checkpoint_restore_rejects_unbounded_counts_without_mutating_table() {
    let component = checkpoint_component("guest.futex.counts");
    let table = populated_guest_futex_table();
    let before = table.lock().unwrap().snapshot();
    let bank = GuestFutexCheckpointBank::new([GuestFutexCheckpointPort::new(
        component.clone(),
        table.clone(),
    )])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    let mut huge_waiters = Vec::new();
    push_u64(&mut huge_waiters, GUEST_FUTEX_CHECKPOINT_VERSION);
    push_u64(&mut huge_waiters, u64::MAX);
    registry
        .write_chunk(&component, GUEST_FUTEX_CHUNK, huge_waiters)
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestFutexCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);
}

#[test]
fn guest_futex_checkpoint_bank_prevalidates_all_ports_before_restore() {
    let valid_component = checkpoint_component("guest.futex.valid-port");
    let invalid_component = checkpoint_component("guest.futex.invalid-port");
    let valid_table = populated_guest_futex_table();
    let valid_before = valid_table.lock().unwrap().snapshot();
    let invalid_table = populated_guest_futex_table();
    let invalid_before = invalid_table.lock().unwrap().snapshot();
    let bank = GuestFutexCheckpointBank::new([
        GuestFutexCheckpointPort::new(valid_component.clone(), valid_table.clone()),
        GuestFutexCheckpointPort::new(invalid_component.clone(), invalid_table.clone()),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    registry
        .write_chunk(
            &valid_component,
            GUEST_FUTEX_CHUNK,
            guest_futex_checkpoint_payload(&[(0x5000, 5, 21, 1, 50, u32::MAX)]),
        )
        .unwrap();
    registry
        .write_chunk(
            &invalid_component,
            GUEST_FUTEX_CHUNK,
            guest_futex_checkpoint_payload(&[(0x6000, 6, 22, 2, 60, 0)]),
        )
        .unwrap();

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert!(matches!(
        error,
        GuestFutexCheckpointError::InvalidChunk {
            component: error_component,
            ..
        } if error_component == invalid_component
    ));
    assert_eq!(valid_table.lock().unwrap().snapshot(), valid_before);
    assert_eq!(invalid_table.lock().unwrap().snapshot(), invalid_before);
}

#[test]
fn system_action_executor_checkpoints_and_restores_guest_futex_tables() {
    let component = checkpoint_component("guest.futex.host");
    let table = populated_guest_futex_table();
    let expected = table.lock().unwrap().snapshot();
    let bank = GuestFutexCheckpointBank::new([GuestFutexCheckpointPort::new(
        component.clone(),
        table.clone(),
    )])
    .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor.attach_guest_futex_checkpoint_bank(bank).unwrap();

    let checkpoint = host_record(
        20,
        1,
        HostAction::Checkpoint {
            label: "guest-futexes".to_string(),
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
                .any(|chunk| chunk.name() == GUEST_FUTEX_CHUNK)
    }));

    table
        .lock()
        .unwrap()
        .wake(
            GuestFutexAddress::new(0x181c08),
            GuestThreadGroupId::new(42),
            usize::MAX,
            30,
        )
        .unwrap();
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
    assert_eq!(table.lock().unwrap().snapshot(), expected);
}

#[test]
fn system_action_executor_rejects_malformed_guest_futex_checkpoint_without_mutation() {
    let component = checkpoint_component("guest.futex.host.bad");
    let table = populated_guest_futex_table();
    let before = table.lock().unwrap().snapshot();
    let bank = GuestFutexCheckpointBank::new([GuestFutexCheckpointPort::new(
        component.clone(),
        table.clone(),
    )])
    .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor.attach_guest_futex_checkpoint_bank(bank).unwrap();
    let manifest = rem6_checkpoint::CheckpointManifest::new(
        "bad-guest-futexes",
        50,
        vec![rem6_checkpoint::CheckpointState::new(
            component.clone(),
            vec![rem6_checkpoint::CheckpointChunk::new(
                GUEST_FUTEX_CHUNK,
                guest_futex_checkpoint_payload(&[(0x7000, 7, 31, 3, 70, 0)]),
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
        SystemError::GuestFutexCheckpoint(GuestFutexCheckpointError::InvalidChunk {
            component: error_component,
            ..
        }) if error_component == component
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);
}

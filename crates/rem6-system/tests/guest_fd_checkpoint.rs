use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_kernel::PartitionId;
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestFd, GuestFdCheckpointBank, GuestFdCheckpointError, GuestFdCheckpointPort,
    GuestFdEntry, GuestFdTable, GuestFileDescription, GuestFileDescriptionId, GuestFileOffset,
    GuestFileSignalOwner, GuestFileStatusFlags, GuestHostFd, GuestSourceId, HostAction,
    HostActionRecord, SystemActionExecutor, SystemActionOutcome, SystemError,
};

const GUEST_FD_CHUNK: &str = "guest-fd";
const GUEST_FD_CHECKPOINT_VERSION: u64 = 4;
const GUEST_FD_SIGNAL_OWNER_PROCESS_KIND: u32 = 1;

fn checkpoint_component(name: &str) -> CheckpointComponentId {
    CheckpointComponentId::new(name).unwrap()
}

fn push_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn guest_fd_checkpoint_payload(
    descriptions: &[(u64, Option<i32>, u32, u64)],
    entries: &[(u32, u64, bool)],
) -> Vec<u8> {
    let mut payload = Vec::new();
    push_u64(&mut payload, GUEST_FD_CHECKPOINT_VERSION);
    push_u64(&mut payload, descriptions.len() as u64);
    for (description, host_fd, status_flags, file_offset) in descriptions {
        push_u64(&mut payload, *description);
        match host_fd {
            Some(host_fd) => {
                push_u64(&mut payload, 1);
                push_u64(&mut payload, *host_fd as u64);
            }
            None => {
                push_u64(&mut payload, 0);
                push_u64(&mut payload, 0);
            }
        }
        push_u32(&mut payload, *status_flags);
        push_u64(&mut payload, *file_offset);
        push_u64(&mut payload, 0);
        push_u32(&mut payload, GUEST_FD_SIGNAL_OWNER_PROCESS_KIND);
        push_u32(&mut payload, 0);
    }
    push_u64(&mut payload, entries.len() as u64);
    for (fd, description, close_on_exec) in entries {
        push_u32(&mut payload, *fd);
        push_u64(&mut payload, *description);
        push_u64(&mut payload, u64::from(*close_on_exec));
    }
    payload
}

fn populated_guest_fd_table() -> (Arc<Mutex<GuestFdTable>>, GuestFd, GuestFd) {
    let table = Arc::new(Mutex::new(GuestFdTable::new()));
    let original = GuestFd::new(50).unwrap();
    let description = GuestFileDescriptionId::new(500);
    {
        let mut locked = table.lock().unwrap();
        locked
            .insert_description(GuestFileDescription::host_backed(
                description,
                GuestHostFd::new(60).unwrap(),
                GuestFileStatusFlags::new(0x02),
            ))
            .unwrap();
        locked
            .insert_description(GuestFileDescription::guest_backed(
                GuestFileDescriptionId::new(501),
                GuestFileStatusFlags::new(0x40),
            ))
            .unwrap();
        locked
            .insert(
                original,
                GuestFdEntry::new(description).with_close_on_exec(true),
            )
            .unwrap();
        locked
            .set_file_offset(original, GuestFileOffset::new(4096))
            .unwrap();
        locked.set_signal_owner(original, -123).unwrap();
        locked.set_signal_number(original, 10).unwrap();
        let duplicate = locked.dup(original).unwrap();
        (table.clone(), original, duplicate)
    }
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
fn guest_fd_checkpoint_bank_round_trips_shared_description_aliases() {
    let component = checkpoint_component("guest.fd0");
    let (source_table, original, duplicate) = populated_guest_fd_table();
    let expected = source_table.lock().unwrap().snapshot();
    let capture_bank = GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(
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
    assert!(registry.chunk(&component, GUEST_FD_CHUNK).is_some());

    let restore_table = Arc::new(Mutex::new(GuestFdTable::new()));
    restore_table
        .lock()
        .unwrap()
        .insert(
            GuestFd::new(99).unwrap(),
            GuestFdEntry::new(GuestFileDescriptionId::new(999)),
        )
        .unwrap();
    let restore_bank = GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(
        component.clone(),
        restore_table.clone(),
    )])
    .unwrap();

    let restored = restore_bank.restore_all_from(&registry).unwrap();

    assert_eq!(restored[0].snapshot(), &expected);
    assert_eq!(restore_table.lock().unwrap().snapshot(), expected);
    restore_table
        .lock()
        .unwrap()
        .advance_file_offset(duplicate, 32)
        .unwrap();
    assert_eq!(
        restore_table
            .lock()
            .unwrap()
            .file_offset(original)
            .unwrap()
            .get(),
        4128
    );
    assert_eq!(
        restore_table
            .lock()
            .unwrap()
            .signal_owner(duplicate)
            .unwrap(),
        -123
    );
    assert_eq!(
        restore_table
            .lock()
            .unwrap()
            .signal_number(duplicate)
            .unwrap(),
        10
    );
    restore_table
        .lock()
        .unwrap()
        .set_signal_owner(duplicate, 77)
        .unwrap();
    restore_table
        .lock()
        .unwrap()
        .set_signal_number(duplicate, 12)
        .unwrap();
    assert_eq!(
        restore_table
            .lock()
            .unwrap()
            .signal_owner(original)
            .unwrap(),
        77
    );
    assert_eq!(
        restore_table
            .lock()
            .unwrap()
            .signal_number(original)
            .unwrap(),
        12
    );
}

#[test]
fn guest_fd_checkpoint_round_trips_typed_signal_owner_kinds() {
    let owners = [
        (GuestFileSignalOwner::thread(42).unwrap(), 42),
        (GuestFileSignalOwner::process(41).unwrap(), 41),
        (GuestFileSignalOwner::process_group(77).unwrap(), -77),
    ];

    for (index, (owner, legacy_owner)) in owners.into_iter().enumerate() {
        let component = checkpoint_component(&format!("guest.fd.owner-kind.{index}"));
        let table = Arc::new(Mutex::new(GuestFdTable::new()));
        let fd = GuestFd::new(10 + index as i32).unwrap();
        let description = GuestFileDescriptionId::new(900 + index as u64);
        {
            let mut locked = table.lock().unwrap();
            locked
                .insert_description(GuestFileDescription::guest_backed(
                    description,
                    GuestFileStatusFlags::new(0x02),
                ))
                .unwrap();
            locked.insert(fd, GuestFdEntry::new(description)).unwrap();
            locked.set_typed_signal_owner(fd, owner).unwrap();
        }

        let capture_bank =
            GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(component.clone(), table)])
                .unwrap();
        let mut registry = CheckpointRegistry::new();
        capture_bank.register_all(&mut registry).unwrap();
        capture_bank.capture_all_into(&mut registry).unwrap();

        let restored_table = Arc::new(Mutex::new(GuestFdTable::new()));
        let restore_bank = GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(
            component,
            restored_table.clone(),
        )])
        .unwrap();

        restore_bank.restore_all_from(&registry).unwrap();
        assert_eq!(
            restored_table
                .lock()
                .unwrap()
                .typed_signal_owner(fd)
                .unwrap(),
            owner
        );
        assert_eq!(
            restored_table.lock().unwrap().signal_owner(fd).unwrap(),
            legacy_owner
        );
    }
}

#[test]
fn guest_file_signal_owner_rejects_negative_ids() {
    assert!(GuestFileSignalOwner::thread(-1).is_err());
    assert!(GuestFileSignalOwner::process(-1).is_err());
    assert!(GuestFileSignalOwner::process_group(-1).is_err());
}

#[test]
fn guest_fd_checkpoint_restore_rejects_malformed_payload_without_mutating_table() {
    let component = checkpoint_component("guest.fd.bad");
    let (table, _, _) = populated_guest_fd_table();
    let before = table.lock().unwrap().snapshot();
    let bank =
        GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(component.clone(), table.clone())])
            .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    registry
        .write_chunk(
            &component,
            GUEST_FD_CHUNK,
            guest_fd_checkpoint_payload(&[], &[(3, 700, false)]),
        )
        .unwrap();

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert!(matches!(
        error,
        GuestFdCheckpointError::InvalidChunk {
            component: error_component,
            ..
        } if error_component == component
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);
}

#[test]
fn guest_fd_checkpoint_restore_reports_missing_chunk_without_mutating_table() {
    let component = checkpoint_component("guest.fd.missing");
    let (table, _, _) = populated_guest_fd_table();
    let before = table.lock().unwrap().snapshot();
    let bank =
        GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(component.clone(), table.clone())])
            .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();

    assert_eq!(
        bank.restore_all_from(&registry),
        Err(GuestFdCheckpointError::MissingChunk {
            component,
            name: GUEST_FD_CHUNK.to_string()
        })
    );
    assert_eq!(table.lock().unwrap().snapshot(), before);
}

#[test]
fn guest_fd_checkpoint_restore_rejects_version_and_truncated_payloads() {
    let component = checkpoint_component("guest.fd.version");
    let (table, _, _) = populated_guest_fd_table();
    let before = table.lock().unwrap().snapshot();
    let bank =
        GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(component.clone(), table.clone())])
            .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    let mut unsupported = Vec::new();
    push_u64(&mut unsupported, GUEST_FD_CHECKPOINT_VERSION + 1);
    push_u64(&mut unsupported, 0);
    push_u64(&mut unsupported, 0);
    registry
        .write_chunk(&component, GUEST_FD_CHUNK, unsupported)
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestFdCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);

    registry
        .write_chunk(
            &component,
            GUEST_FD_CHUNK,
            vec![GUEST_FD_CHECKPOINT_VERSION as u8],
        )
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestFdCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);
}

#[test]
fn guest_fd_checkpoint_decode_canonicalizes_valid_payload_order() {
    let component = checkpoint_component("guest.fd.canonical");
    let table = Arc::new(Mutex::new(GuestFdTable::new()));
    let bank =
        GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(component.clone(), table.clone())])
            .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    registry
        .write_chunk(
            &component,
            GUEST_FD_CHUNK,
            guest_fd_checkpoint_payload(
                &[(902, None, 0x02, 32), (901, None, 0x01, 16)],
                &[(8, 902, false), (7, 901, true)],
            ),
        )
        .unwrap();

    let restored = bank.restore_all_from(&registry).unwrap();
    let snapshot = restored[0].snapshot();

    assert_eq!(
        snapshot.descriptions()[0].id(),
        GuestFileDescriptionId::new(901)
    );
    assert_eq!(
        snapshot.descriptions()[1].id(),
        GuestFileDescriptionId::new(902)
    );
    assert_eq!(snapshot.entries()[0].fd(), GuestFd::new(7).unwrap());
    assert_eq!(snapshot.entries()[1].fd(), GuestFd::new(8).unwrap());
    assert_eq!(table.lock().unwrap().snapshot(), *snapshot);
}

#[test]
fn guest_fd_checkpoint_restore_rejects_unexpected_absent_host_fd_payload() {
    let component = checkpoint_component("guest.fd.host-field");
    let (table, _, _) = populated_guest_fd_table();
    let before = table.lock().unwrap().snapshot();
    let bank =
        GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(component.clone(), table.clone())])
            .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    let mut malformed = Vec::new();
    push_u64(&mut malformed, GUEST_FD_CHECKPOINT_VERSION);
    push_u64(&mut malformed, 1);
    push_u64(&mut malformed, 900);
    push_u64(&mut malformed, 0);
    push_u64(&mut malformed, 12);
    push_u32(&mut malformed, 0x01);
    push_u64(&mut malformed, 0);
    push_u64(&mut malformed, 0);
    push_u32(&mut malformed, GUEST_FD_SIGNAL_OWNER_PROCESS_KIND);
    push_u32(&mut malformed, 0);
    registry
        .write_chunk(&component, GUEST_FD_CHUNK, malformed)
        .unwrap();

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert!(matches!(
        error,
        GuestFdCheckpointError::InvalidChunk {
            component: error_component,
            ..
        } if error_component == component
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);
}

#[test]
fn guest_fd_checkpoint_restore_rejects_present_host_fd_outside_i32() {
    let component = checkpoint_component("guest.fd.host-range");
    let (table, _, _) = populated_guest_fd_table();
    let before = table.lock().unwrap().snapshot();
    let bank =
        GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(component.clone(), table.clone())])
            .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    let mut malformed = Vec::new();
    push_u64(&mut malformed, GUEST_FD_CHECKPOINT_VERSION);
    push_u64(&mut malformed, 1);
    push_u64(&mut malformed, 930);
    push_u64(&mut malformed, 1);
    push_u64(&mut malformed, i32::MAX as u64 + 1);
    push_u32(&mut malformed, 0x01);
    push_u64(&mut malformed, 0);
    push_u64(&mut malformed, 0);
    push_u32(&mut malformed, GUEST_FD_SIGNAL_OWNER_PROCESS_KIND);
    push_u32(&mut malformed, 0);
    registry
        .write_chunk(&component, GUEST_FD_CHUNK, malformed)
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestFdCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);
}

#[test]
fn guest_fd_checkpoint_restore_rejects_invalid_signal_owner_kind() {
    let component = checkpoint_component("guest.fd.owner-kind");
    let (table, _, _) = populated_guest_fd_table();
    let before = table.lock().unwrap().snapshot();
    let bank =
        GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(component.clone(), table.clone())])
            .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    let mut malformed = guest_fd_checkpoint_payload(&[(920, None, 0x01, 0)], &[]);
    let owner_kind_offset = 8 + 8 + 8 + 8 + 8 + 4 + 8 + 8;
    malformed[owner_kind_offset..owner_kind_offset + 4].copy_from_slice(&99_u32.to_le_bytes());
    registry
        .write_chunk(&component, GUEST_FD_CHUNK, malformed)
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestFdCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);
}

#[test]
fn guest_fd_checkpoint_restore_rejects_invalid_flags_and_trailing_bytes() {
    let component = checkpoint_component("guest.fd.payload-shape");
    let (table, _, _) = populated_guest_fd_table();
    let before = table.lock().unwrap().snapshot();
    let bank =
        GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(component.clone(), table.clone())])
            .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    let mut invalid_bool = guest_fd_checkpoint_payload(&[(910, None, 0x01, 0)], &[]);
    let host_presence_offset = 8 + 8 + 8;
    invalid_bool[host_presence_offset..host_presence_offset + 8]
        .copy_from_slice(&2_u64.to_le_bytes());
    registry
        .write_chunk(&component, GUEST_FD_CHUNK, invalid_bool)
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestFdCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);

    let mut trailing = guest_fd_checkpoint_payload(&[(911, None, 0x01, 0)], &[]);
    trailing.push(0xaa);
    registry
        .write_chunk(&component, GUEST_FD_CHUNK, trailing)
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestFdCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);
}

#[test]
fn guest_fd_checkpoint_restore_rejects_unbounded_counts_without_mutating_table() {
    let component = checkpoint_component("guest.fd.counts");
    let (table, _, _) = populated_guest_fd_table();
    let before = table.lock().unwrap().snapshot();
    let bank =
        GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(component.clone(), table.clone())])
            .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    let mut huge_descriptions = Vec::new();
    push_u64(&mut huge_descriptions, GUEST_FD_CHECKPOINT_VERSION);
    push_u64(&mut huge_descriptions, u64::MAX);
    registry
        .write_chunk(&component, GUEST_FD_CHUNK, huge_descriptions)
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestFdCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);

    let mut huge_entries = Vec::new();
    push_u64(&mut huge_entries, GUEST_FD_CHECKPOINT_VERSION);
    push_u64(&mut huge_entries, 0);
    push_u64(&mut huge_entries, u64::MAX);
    registry
        .write_chunk(&component, GUEST_FD_CHUNK, huge_entries)
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestFdCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);
}

#[test]
fn guest_fd_checkpoint_bank_prevalidates_all_ports_before_restore() {
    let valid_component = checkpoint_component("guest.fd.valid-port");
    let invalid_component = checkpoint_component("guest.fd.invalid-port");
    let (valid_table, _, _) = populated_guest_fd_table();
    let valid_before = valid_table.lock().unwrap().snapshot();
    let (invalid_table, _, _) = populated_guest_fd_table();
    let invalid_before = invalid_table.lock().unwrap().snapshot();
    let bank = GuestFdCheckpointBank::new([
        GuestFdCheckpointPort::new(valid_component.clone(), valid_table.clone()),
        GuestFdCheckpointPort::new(invalid_component.clone(), invalid_table.clone()),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    registry
        .write_chunk(
            &valid_component,
            GUEST_FD_CHUNK,
            guest_fd_checkpoint_payload(&[(920, None, 0x01, 0)], &[(3, 920, false)]),
        )
        .unwrap();
    registry
        .write_chunk(
            &invalid_component,
            GUEST_FD_CHUNK,
            guest_fd_checkpoint_payload(&[], &[(4, 921, false)]),
        )
        .unwrap();

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert!(matches!(
        error,
        GuestFdCheckpointError::InvalidChunk {
            component: error_component,
            ..
        } if error_component == invalid_component
    ));
    assert_eq!(valid_table.lock().unwrap().snapshot(), valid_before);
    assert_eq!(invalid_table.lock().unwrap().snapshot(), invalid_before);
}

#[test]
fn guest_fd_checkpoint_restore_rejects_duplicate_fds_and_descriptions() {
    let component = checkpoint_component("guest.fd.duplicates");
    let (table, _, _) = populated_guest_fd_table();
    let before = table.lock().unwrap().snapshot();
    let bank =
        GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(component.clone(), table.clone())])
            .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    registry
        .write_chunk(
            &component,
            GUEST_FD_CHUNK,
            guest_fd_checkpoint_payload(&[(800, None, 0x01, 0), (800, None, 0x02, 16)], &[]),
        )
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestFdCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);

    registry
        .write_chunk(
            &component,
            GUEST_FD_CHUNK,
            guest_fd_checkpoint_payload(
                &[(801, None, 0x01, 0)],
                &[(5, 801, false), (5, 801, true)],
            ),
        )
        .unwrap();

    assert!(matches!(
        bank.restore_all_from(&registry).unwrap_err(),
        GuestFdCheckpointError::InvalidChunk { .. }
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);
}

#[test]
fn system_action_executor_checkpoints_and_restores_guest_fd_tables() {
    let component = checkpoint_component("guest.fd.host");
    let (table, _, _) = populated_guest_fd_table();
    let expected = table.lock().unwrap().snapshot();
    let bank =
        GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(component.clone(), table.clone())])
            .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor.attach_guest_fd_checkpoint_bank(bank).unwrap();

    let checkpoint = host_record(
        20,
        1,
        HostAction::Checkpoint {
            label: "guest-fds".to_string(),
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
                .any(|chunk| chunk.name() == GUEST_FD_CHUNK)
    }));

    table
        .lock()
        .unwrap()
        .set_status_flags(GuestFd::new(50).unwrap(), GuestFileStatusFlags::new(0x802))
        .unwrap();
    let restore = host_record(
        30,
        2,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );

    assert_eq!(
        executor.apply(&restore).unwrap(),
        SystemActionOutcome::CheckpointRestored {
            tick: 30,
            event: GuestEventId::new(2),
            source: GuestSourceId::new(7),
            manifest,
        }
    );
    assert_eq!(table.lock().unwrap().snapshot(), expected);
}

#[test]
fn system_action_executor_rejects_malformed_guest_fd_checkpoint_without_mutation() {
    let component = checkpoint_component("guest.fd.host.bad");
    let (table, _, _) = populated_guest_fd_table();
    let before = table.lock().unwrap().snapshot();
    let bank =
        GuestFdCheckpointBank::new([GuestFdCheckpointPort::new(component.clone(), table.clone())])
            .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor.attach_guest_fd_checkpoint_bank(bank).unwrap();
    let manifest = rem6_checkpoint::CheckpointManifest::new(
        "bad-guest-fds",
        40,
        vec![rem6_checkpoint::CheckpointState::new(
            component.clone(),
            vec![rem6_checkpoint::CheckpointChunk::new(
                GUEST_FD_CHUNK,
                guest_fd_checkpoint_payload(&[], &[(4, 900, false)]),
            )],
        )],
    );
    let restore = host_record(
        50,
        3,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );

    let error = executor.apply(&restore).unwrap_err();

    assert!(matches!(
        error,
        SystemError::GuestFdCheckpoint(GuestFdCheckpointError::InvalidChunk {
            component: error_component,
            ..
        }) if error_component == component
    ));
    assert_eq!(table.lock().unwrap().snapshot(), before);
}

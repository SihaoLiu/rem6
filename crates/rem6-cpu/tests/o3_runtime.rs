use rem6_cpu::{
    O3DependencyScopeId, O3IssueOpClass, O3IssueQueueId, O3LoadStoreQueueEntry,
    O3PendingStateCheckpointPayload, O3PendingStateSnapshot, O3PhysicalRegisterId, O3PipelineStage,
    O3RegisterClass, O3RenameMapEntry, O3ReorderBufferEntry, O3RuntimeCheckpointPayload,
    O3ScopedReadyInstruction, O3WritebackCompletion, O3WritebackTransferPolicy,
    O3WritebackTransferSnapshot,
};
use rem6_memory::Address;

const O3_RUNTIME_CHECKPOINT_MAGIC_BYTES: usize = 4;
const O3_RUNTIME_CHECKPOINT_VERSION_BYTES: usize = 1;
const O3_RUNTIME_CHECKPOINT_PENDING_LEN_OFFSET: usize =
    O3_RUNTIME_CHECKPOINT_MAGIC_BYTES + O3_RUNTIME_CHECKPOINT_VERSION_BYTES;
const O3_RUNTIME_CHECKPOINT_HEADER_BYTES: usize =
    O3_RUNTIME_CHECKPOINT_MAGIC_BYTES + O3_RUNTIME_CHECKPOINT_VERSION_BYTES + 4 * 4;
const O3_RUNTIME_ROB_DESTINATION_PRESENT_OFFSET: usize = 8 + 8;
const O3_RUNTIME_ROB_READY_OFFSET: usize = O3_RUNTIME_ROB_DESTINATION_PRESENT_OFFSET + 1 + 4;

#[test]
fn o3_runtime_checkpoint_round_trips_rob_lsq_rename_and_pending_state() {
    let pending_scope = O3DependencyScopeId::new(0x44);
    let produced_scope = O3DependencyScopeId::new(0x55);
    let snapshot = rem6_cpu::O3RuntimeSnapshot::new(
        [
            O3ReorderBufferEntry::new(
                10,
                Address::new(0x8000),
                Some(O3PhysicalRegisterId::new(40)),
            )
            .with_ready(true),
            O3ReorderBufferEntry::new(11, Address::new(0x8004), None),
        ],
        [
            O3LoadStoreQueueEntry::load(10, Some(Address::new(0x9000)), 8).with_completed(true),
            O3LoadStoreQueueEntry::store(11, None, 4),
        ],
        [
            O3RenameMapEntry::new(O3RegisterClass::Integer, 1, O3PhysicalRegisterId::new(40)),
            O3RenameMapEntry::new(
                O3RegisterClass::FloatingPoint,
                2,
                O3PhysicalRegisterId::new(80),
            ),
        ],
        O3PendingStateSnapshot::new(
            [pending_scope],
            [
                O3ScopedReadyInstruction::new(12, O3IssueQueueId::new(0), O3IssueOpClass::IntAlu)
                    .with_waits_on([pending_scope])
                    .with_produces([produced_scope]),
            ],
            O3WritebackTransferSnapshot::new(
                O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap(),
                [O3WritebackCompletion::new(13)],
            ),
        )
        .unwrap(),
    )
    .unwrap();
    let payload = O3RuntimeCheckpointPayload::from_snapshot(snapshot.clone()).unwrap();
    let decoded = O3RuntimeCheckpointPayload::decode(payload.encode().as_slice()).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(decoded.snapshot().reorder_buffer()[0].sequence(), 10);
    assert_eq!(
        decoded.snapshot().load_store_queue()[0].address(),
        Some(Address::new(0x9000))
    );
    assert_eq!(
        decoded.snapshot().rename_map()[0].physical(),
        O3PhysicalRegisterId::new(40)
    );
    let pending_payload =
        O3PendingStateCheckpointPayload::from_snapshot(decoded.snapshot().pending_state().clone())
            .unwrap();
    assert_eq!(
        pending_payload.snapshot().resolved_dependency_scopes(),
        &[pending_scope]
    );
}

#[test]
fn o3_runtime_checkpoint_rejects_invalid_bool_bytes() {
    let payload = O3RuntimeCheckpointPayload::from_snapshot(
        rem6_cpu::O3RuntimeSnapshot::new(
            [O3ReorderBufferEntry::new(
                1,
                Address::new(0x8000),
                Some(O3PhysicalRegisterId::new(40)),
            )
            .with_ready(true)],
            [],
            [],
            O3PendingStateSnapshot::new(
                [],
                [],
                O3WritebackTransferSnapshot::new(
                    O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 1, 0).unwrap(),
                    [],
                ),
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
    .encode();
    let rob_offset = o3_runtime_rob_payload_offset(&payload);

    let mut invalid_destination_present = payload.clone();
    invalid_destination_present[rob_offset + O3_RUNTIME_ROB_DESTINATION_PRESENT_OFFSET] = 2;
    assert!(matches!(
        O3RuntimeCheckpointPayload::decode(&invalid_destination_present),
        Err(rem6_cpu::O3RuntimeError::InvalidCheckpointBool {
            field: "ROB destination-present",
            value: 2
        })
    ));

    let mut invalid_ready = payload;
    invalid_ready[rob_offset + O3_RUNTIME_ROB_READY_OFFSET] = 2;
    assert!(matches!(
        O3RuntimeCheckpointPayload::decode(&invalid_ready),
        Err(rem6_cpu::O3RuntimeError::InvalidCheckpointBool {
            field: "ROB ready",
            value: 2
        })
    ));
}

#[test]
fn o3_runtime_snapshot_rejects_duplicate_rob_lsq_and_rename_entries() {
    let pending = O3PendingStateSnapshot::new(
        [],
        [],
        O3WritebackTransferSnapshot::new(
            O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 1, 0).unwrap(),
            [],
        ),
    )
    .unwrap();

    assert!(rem6_cpu::O3RuntimeSnapshot::new(
        [
            O3ReorderBufferEntry::new(1, Address::new(0x8000), None),
            O3ReorderBufferEntry::new(1, Address::new(0x8004), None),
        ],
        [],
        [],
        pending.clone(),
    )
    .is_err());

    assert!(rem6_cpu::O3RuntimeSnapshot::new(
        [],
        [
            O3LoadStoreQueueEntry::load(2, Some(Address::new(0x9000)), 4),
            O3LoadStoreQueueEntry::store(2, Some(Address::new(0x9008)), 8),
        ],
        [],
        pending.clone(),
    )
    .is_err());

    assert!(rem6_cpu::O3RuntimeSnapshot::new(
        [],
        [],
        [
            O3RenameMapEntry::new(O3RegisterClass::Integer, 1, O3PhysicalRegisterId::new(10)),
            O3RenameMapEntry::new(O3RegisterClass::Integer, 1, O3PhysicalRegisterId::new(11)),
        ],
        pending,
    )
    .is_err());
}

fn o3_runtime_rob_payload_offset(payload: &[u8]) -> usize {
    let pending_len_bytes = payload
        [O3_RUNTIME_CHECKPOINT_PENDING_LEN_OFFSET..O3_RUNTIME_CHECKPOINT_PENDING_LEN_OFFSET + 4]
        .try_into()
        .unwrap();
    O3_RUNTIME_CHECKPOINT_HEADER_BYTES + u32::from_le_bytes(pending_len_bytes) as usize
}

use rem6_cpu::{
    BranchTargetKind, CpuCore, CpuFetchConfig, CpuFetchEvent, CpuFetchRecord, CpuId, CpuResetState,
    O3DependencyScopeId, O3IssueOpClass, O3IssueQueueId, O3LoadStoreQueueEntry,
    O3PendingStateCheckpointPayload, O3PendingStateSnapshot, O3PhysicalRegisterId, O3PipelineStage,
    O3RegisterClass, O3RenameMapEntry, O3ReorderBufferEntry, O3RuntimeCheckpointPayload,
    O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering, O3RuntimeStats,
    O3ScopedReadyInstruction, O3WritebackTransferPolicy, O3WritebackTransferSnapshot, RiscvCore,
    RiscvCpuExecutionEvent,
};
use rem6_isa_riscv::{
    AtomicMemoryOp, FloatRegister, Immediate, MemoryAccessKind, MemoryWidth, Register,
    RegisterWrite, RiscvCounterCsr, RiscvCounterInhibitCsr, RiscvCounterInhibitCsrInstruction,
    RiscvCsrOp, RiscvExecutionRecord, RiscvInstruction, RiscvVectorFloatInstruction,
    RiscvVectorMaskMode, RiscvVectorMemoryInstruction, VectorRegister,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

const O3_RUNTIME_CHECKPOINT_MAGIC_BYTES: usize = 4;
const O3_RUNTIME_CHECKPOINT_VERSION_BYTES: usize = 1;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_ISSUE_ARBITRATION_STATS: u8 = 22;
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS: u8 = 23;
const O3_RUNTIME_CHECKPOINT_PENDING_LEN_OFFSET: usize =
    O3_RUNTIME_CHECKPOINT_MAGIC_BYTES + O3_RUNTIME_CHECKPOINT_VERSION_BYTES;
const O3_RUNTIME_CHECKPOINT_HEADER_BYTES: usize =
    O3_RUNTIME_CHECKPOINT_MAGIC_BYTES + O3_RUNTIME_CHECKPOINT_VERSION_BYTES + 4 * 4;
const O3_RUNTIME_CHECKPOINT_ROB_COUNT_OFFSET: usize = O3_RUNTIME_CHECKPOINT_PENDING_LEN_OFFSET + 4;
const O3_RUNTIME_CHECKPOINT_LSQ_COUNT_OFFSET: usize = O3_RUNTIME_CHECKPOINT_ROB_COUNT_OFFSET + 4;
const O3_RUNTIME_CHECKPOINT_RENAME_COUNT_OFFSET: usize = O3_RUNTIME_CHECKPOINT_LSQ_COUNT_OFFSET + 4;
const O3_RUNTIME_CHECKPOINT_ROB_ENTRY_BYTES_LEGACY: usize = 8 + 8 + 1 + 4 + 1;
const O3_RUNTIME_CHECKPOINT_ROB_ENTRY_BYTES_WITH_READY_TICK: usize =
    O3_RUNTIME_CHECKPOINT_ROB_ENTRY_BYTES_LEGACY + 8;
const O3_RUNTIME_CHECKPOINT_ROB_ENTRY_BYTES: usize =
    O3_RUNTIME_CHECKPOINT_ROB_ENTRY_BYTES_WITH_READY_TICK + 1 + 1 + 1 + 4;
const O3_RUNTIME_CHECKPOINT_LSQ_ENTRY_BYTES: usize = 8 + 1 + 8 + 4 + 1 + 1;
const O3_RUNTIME_CHECKPOINT_RENAME_ENTRY_BYTES: usize = 1 + 4 + 4;
const O3_RUNTIME_CHECKPOINT_RENAME_ENTRY_BYTES_WITH_DEPENDENCY: usize =
    O3_RUNTIME_CHECKPOINT_RENAME_ENTRY_BYTES + 1;
const O3_RUNTIME_CHECKPOINT_BASE_AND_FU_STATS_BYTES: usize =
    (12 + O3RuntimeFuLatencyClass::COUNT * 2) * 8;
const O3_RUNTIME_CHECKPOINT_CURRENT_BASE_AND_FU_STATS_BYTES: usize =
    O3_RUNTIME_CHECKPOINT_BASE_AND_FU_STATS_BYTES + 8;
const O3_RUNTIME_CHECKPOINT_FU_LATENCY_CLASS_EXTREMA_STATS_BYTES: usize =
    O3RuntimeFuLatencyClass::COUNT * 2 * 8;
const O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_STATS_BYTES: usize =
    O3RuntimeLsqOperation::TRACKED.len() * 8;
const O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_BYTE_STATS_BYTES: usize =
    O3RuntimeLsqOperation::TRACKED.len() * 2 * 8;
const O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_FORWARDING_STATS_BYTES: usize =
    O3RuntimeLsqOperation::TRACKED.len() * 2 * 8;
const O3_RUNTIME_CHECKPOINT_LSQ_FORWARDING_SUPPRESSION_STATS_BYTES: usize =
    (1 + O3RuntimeLsqOperation::TRACKED.len()) * 8;
const O3_RUNTIME_CHECKPOINT_LSQ_FORWARDING_SUPPRESSION_REASON_STATS_BYTES: usize =
    (2 + O3RuntimeLsqOperation::TRACKED.len() * 2) * 8;
const O3_RUNTIME_CHECKPOINT_LSQ_ORDERING_STATS_BYTES: usize =
    (O3RuntimeLsqOrdering::TRACKED.len() + 1) * 8;
const O3_RUNTIME_CHECKPOINT_LSQ_MATRIX_STATS_BYTES: usize =
    O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_ORDERING_STATS_BYTES;
const O3_RUNTIME_CHECKPOINT_LSQ_LATENCY_STATS_BYTES: usize =
    O3RuntimeLsqOperation::TRACKED.len() * 4 * 8;
const O3_RUNTIME_CHECKPOINT_LSQ_DATA_LATENCY_STATS_BYTES: usize = 4 * 8;
const O3_RUNTIME_CHECKPOINT_BRANCH_REPAIR_STATS_BYTES: usize =
    (3 + BranchTargetKind::COUNT * 3) * 8;
const O3_RUNTIME_CHECKPOINT_IEW_BRANCH_MISPREDICT_SPLIT_STATS_BYTES: usize = 2 * 8;
const O3_RUNTIME_CHECKPOINT_IEW_DEPENDENCY_STATS_BYTES: usize = 2 * 8;
const O3_RUNTIME_CHECKPOINT_IQ_BRANCH_ISSUED_STATS_BYTES: usize = 8;
const O3_RUNTIME_CHECKPOINT_BRANCH_EVENT_STATS_BYTES: usize = BranchTargetKind::COUNT * 6 * 8;
const O3_RUNTIME_CHECKPOINT_BRANCH_EVENT_PREDICTION_STATS_BYTES: usize =
    BranchTargetKind::COUNT * 4 * 8;
const O3_RUNTIME_CHECKPOINT_BRANCH_MISMATCH_STATS_BYTES: usize = BranchTargetKind::COUNT * 16 * 8;
const O3_RUNTIME_CHECKPOINT_LIVE_RETIRE_GATE_STATS_BYTES: usize = 3 * 8;
const O3_RUNTIME_CHECKPOINT_ISSUE_ARBITRATION_STATS_BYTES: usize = 5 * 8;
const O3_RUNTIME_CHECKPOINT_WRITEBACK_PORT_STATS_BYTES: usize = 6 * 8;
const O3_RUNTIME_CHECKPOINT_LIVE_RETIRE_GATE_PAYLOAD_BYTES: usize = 1 + 4 + 8 + 8;
const O3_RUNTIME_CHECKPOINT_STATS_BYTES_WITHOUT_FORWARDING_SUPPRESSION: usize =
    (15 + O3RuntimeFuLatencyClass::COUNT * 2) * 8
        + O3_RUNTIME_CHECKPOINT_LSQ_MATRIX_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_FORWARDING_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_LATENCY_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_DATA_LATENCY_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_BRANCH_REPAIR_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_IEW_BRANCH_MISPREDICT_SPLIT_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_IEW_DEPENDENCY_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_IQ_BRANCH_ISSUED_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_BRANCH_EVENT_STATS_BYTES;
const O3_RUNTIME_CHECKPOINT_STATS_BYTES: usize =
    O3_RUNTIME_CHECKPOINT_STATS_BYTES_WITHOUT_FORWARDING_SUPPRESSION
        + O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_BYTE_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_FORWARDING_SUPPRESSION_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_FORWARDING_SUPPRESSION_REASON_STATS_BYTES;
const O3_RUNTIME_CHECKPOINT_V21_STATS_BYTES: usize = O3_RUNTIME_CHECKPOINT_STATS_BYTES
    + O3_RUNTIME_CHECKPOINT_FU_LATENCY_CLASS_EXTREMA_STATS_BYTES
    + O3_RUNTIME_CHECKPOINT_BRANCH_EVENT_PREDICTION_STATS_BYTES
    + O3_RUNTIME_CHECKPOINT_BRANCH_MISMATCH_STATS_BYTES;
const O3_RUNTIME_CHECKPOINT_STATS_BYTES_WITHOUT_BRANCH_MISMATCH: usize =
    O3_RUNTIME_CHECKPOINT_V21_STATS_BYTES
        - O3_RUNTIME_CHECKPOINT_FU_LATENCY_CLASS_EXTREMA_STATS_BYTES
        - O3_RUNTIME_CHECKPOINT_BRANCH_MISMATCH_STATS_BYTES;
const O3_RUNTIME_CHECKPOINT_STATS_BYTES_WITHOUT_FORWARDING_SUPPRESSION_REASON: usize =
    O3_RUNTIME_CHECKPOINT_STATS_BYTES
        - O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_BYTE_STATS_BYTES
        - O3_RUNTIME_CHECKPOINT_LSQ_FORWARDING_SUPPRESSION_REASON_STATS_BYTES;
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
            .with_ready(true)
            .with_ready_tick(123),
            O3ReorderBufferEntry::new(11, Address::new(0x8004), None).with_ready_tick(456),
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
                [],
            ),
        )
        .unwrap(),
    )
    .unwrap();
    let payload = O3RuntimeCheckpointPayload::from_snapshot(snapshot.clone()).unwrap();
    let decoded = O3RuntimeCheckpointPayload::decode(payload.encode().as_slice()).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(decoded.snapshot().reorder_buffer()[0].sequence(), 10);
    assert_eq!(decoded.snapshot().reorder_buffer()[0].ready_tick(), 123);
    assert_eq!(decoded.snapshot().reorder_buffer()[1].ready_tick(), 456);
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
fn o3_runtime_checkpoint_decodes_v1_payloads_without_stats() {
    let payload = RiscvCore::default_o3_runtime_checkpoint_payload();
    let mut encoded = strip_current_live_retire_gate(&payload.encode());
    let v1_len = encoded
        .len()
        .checked_sub(O3_RUNTIME_CHECKPOINT_V21_STATS_BYTES)
        .unwrap();
    encoded.truncate(v1_len);
    encoded[O3_RUNTIME_CHECKPOINT_MAGIC_BYTES] = 1;

    let decoded = O3RuntimeCheckpointPayload::decode(&encoded).unwrap();

    assert_eq!(decoded.snapshot(), payload.snapshot());
    assert_eq!(decoded.stats(), O3RuntimeStats::default());
    assert_eq!(
        decoded
            .stats()
            .lsq_operation_count(O3RuntimeLsqOperation::Atomic),
        0
    );
    assert_eq!(
        decoded
            .stats()
            .lsq_ordering_count(O3RuntimeLsqOrdering::AcquireRelease),
        0
    );
}

#[test]
fn o3_runtime_checkpoint_decodes_v2_scalar_fu_stats_into_class_arrays() {
    let payload = RiscvCore::default_o3_runtime_checkpoint_payload();
    let mut encoded = strip_current_live_retire_gate(&payload.encode());
    let stats_offset = encoded
        .len()
        .checked_sub(O3_RUNTIME_CHECKPOINT_V21_STATS_BYTES)
        .unwrap();
    encoded.truncate(stats_offset);
    encoded[O3_RUNTIME_CHECKPOINT_MAGIC_BYTES] = 2;
    for value in [7, 7, 6, 5, 4, 3, 16, 12, 2, 1, 5, 44, 2, 4, 3, 40, 9, 8, 7] as [u64; 19] {
        encoded.extend_from_slice(&value.to_le_bytes());
    }

    let decoded = O3RuntimeCheckpointPayload::decode(&encoded).unwrap();
    let stats = decoded.stats();

    assert_eq!(decoded.snapshot(), payload.snapshot());
    assert_eq!(stats.instructions(), 7);
    assert_eq!(stats.fu_latency_instructions(), 5);
    assert_eq!(stats.fu_latency_cycles(), 44);
    assert_eq!(
        stats.fu_latency_class_instructions(O3RuntimeFuLatencyClass::ScalarIntegerMul),
        2
    );
    assert_eq!(
        stats.fu_latency_class_cycles(O3RuntimeFuLatencyClass::ScalarIntegerMul),
        4
    );
    assert_eq!(
        stats.fu_latency_class_instructions(O3RuntimeFuLatencyClass::ScalarIntegerDiv),
        3
    );
    assert_eq!(
        stats.fu_latency_class_cycles(O3RuntimeFuLatencyClass::ScalarIntegerDiv),
        40
    );
    assert_eq!(
        stats.fu_latency_class_instructions(O3RuntimeFuLatencyClass::ScalarFloatMisc),
        0
    );
    assert_eq!(stats.max_rob_occupancy(), 9);
    assert_eq!(stats.max_lsq_occupancy(), 8);
    assert_eq!(stats.rename_map_entries(), 7);
    assert_eq!(stats.lsq_operation_count(O3RuntimeLsqOperation::Atomic), 0);
    assert_eq!(
        stats.lsq_ordering_count(O3RuntimeLsqOrdering::AcquireRelease),
        0
    );
}

#[test]
fn o3_runtime_checkpoint_decodes_legacy_rob_entries_with_nonzero_ready_ticks() {
    let snapshot = rem6_cpu::O3RuntimeSnapshot::new(
        [O3ReorderBufferEntry::new(
            10,
            Address::new(0x8000),
            Some(O3PhysicalRegisterId::new(40)),
        )
        .with_ready(true)
        .with_ready_tick(123)],
        [],
        [],
        O3PendingStateSnapshot::new(
            [],
            [],
            O3WritebackTransferSnapshot::new(
                O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap(),
                [],
            ),
        )
        .unwrap(),
    )
    .unwrap();
    let payload = O3RuntimeCheckpointPayload::from_snapshot(snapshot).unwrap();
    let mut encoded = strip_current_rob_ready_tick_bytes(&payload.encode());
    encoded[O3_RUNTIME_CHECKPOINT_MAGIC_BYTES] = 18;

    let decoded = O3RuntimeCheckpointPayload::decode(&encoded).unwrap();

    assert_eq!(decoded.snapshot().reorder_buffer()[0].ready_tick(), 0x8000);
}

#[test]
fn o3_runtime_checkpoint_decodes_v19_without_live_retire_gate() {
    let payload = RiscvCore::default_o3_runtime_checkpoint_payload();
    let mut encoded = strip_current_live_retire_gate(&payload.encode());
    encoded[O3_RUNTIME_CHECKPOINT_MAGIC_BYTES] = 19;

    let decoded = O3RuntimeCheckpointPayload::decode(&encoded).unwrap();

    assert_eq!(decoded.snapshot(), payload.snapshot());
    assert_eq!(decoded.pending_live_retire_gate(), None);
    assert_eq!(decoded.stats().live_retire_gate_scheduled_waits(), 0);
    assert_eq!(decoded.stats().live_retire_gate_wait_ticks(), 0);
    assert_eq!(decoded.stats().live_retire_gate_max_wait_ticks(), 0);
}

#[test]
fn o3_runtime_checkpoint_decodes_v20_without_live_staged_rob_metadata() {
    let snapshot = rem6_cpu::O3RuntimeSnapshot::new(
        [O3ReorderBufferEntry::new(
            10,
            Address::new(0x8000),
            Some(O3PhysicalRegisterId::new(40)),
        )
        .with_ready(true)
        .with_ready_tick(123)],
        [],
        [],
        O3PendingStateSnapshot::new(
            [],
            [],
            O3WritebackTransferSnapshot::new(
                O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap(),
                [],
            ),
        )
        .unwrap(),
    )
    .unwrap();
    let payload = O3RuntimeCheckpointPayload::from_snapshot(snapshot).unwrap();
    let mut encoded = strip_current_live_staged_rob_bytes(&payload.encode());
    encoded[O3_RUNTIME_CHECKPOINT_MAGIC_BYTES] = 20;

    let decoded = O3RuntimeCheckpointPayload::decode(&encoded).unwrap();
    let entry = decoded.snapshot().reorder_buffer()[0];
    assert_eq!(entry.ready_tick(), 123);
    assert!(entry.is_ready());
    assert!(!entry.is_live_staged());
}

#[test]
fn o3_runtime_checkpoint_rejects_inconsistent_live_staged_rob_metadata() {
    let snapshot = rem6_cpu::O3RuntimeSnapshot::new(
        [O3ReorderBufferEntry::new(
            10,
            Address::new(0x8000),
            Some(O3PhysicalRegisterId::new(40)),
        )
        .with_ready_tick(123)],
        [],
        [],
        O3PendingStateSnapshot::new(
            [],
            [],
            O3WritebackTransferSnapshot::new(
                O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap(),
                [],
            ),
        )
        .unwrap(),
    )
    .unwrap();
    let encoded = O3RuntimeCheckpointPayload::from_snapshot(snapshot)
        .unwrap()
        .encode();
    let rob_offset = o3_runtime_rob_payload_offset(&encoded);
    let destination_present_offset = rob_offset + 8 + 8;
    let live_staged_offset = rob_offset + O3_RUNTIME_CHECKPOINT_ROB_ENTRY_BYTES_WITH_READY_TICK;
    let rename_destination_present_offset = live_staged_offset + 1;

    for (name, destination_present, live_staged, rename_destination_present) in [
        ("non-staged rename destination", 1, 0, 1),
        ("staged destination without rename identity", 1, 1, 0),
        ("staged rename identity without destination", 0, 1, 1),
    ] {
        let mut malformed = encoded.clone();
        malformed[destination_present_offset] = destination_present;
        malformed[live_staged_offset] = live_staged;
        malformed[rename_destination_present_offset] = rename_destination_present;

        let error = O3RuntimeCheckpointPayload::decode(&malformed)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("live-staged ROB metadata"),
            "{name} should be rejected explicitly, got: {error}"
        );
    }
}

#[test]
fn o3_runtime_checkpoint_rejects_legacy_public_live_overlay_as_committed_mapping() {
    let physical = O3PhysicalRegisterId::new(40);
    let snapshot = rem6_cpu::O3RuntimeSnapshot::new(
        [
            O3ReorderBufferEntry::new(10, Address::new(0x8000), Some(physical))
                .with_ready_tick(123),
        ],
        [],
        [O3RenameMapEntry::new(O3RegisterClass::Integer, 3, physical)],
        O3PendingStateSnapshot::new(
            [],
            [],
            O3WritebackTransferSnapshot::new(
                O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap(),
                [],
            ),
        )
        .unwrap(),
    )
    .unwrap();
    let mut encoded = O3RuntimeCheckpointPayload::from_snapshot(snapshot)
        .unwrap()
        .encode();
    let rob_offset = o3_runtime_rob_payload_offset(&encoded);
    let live_staged_offset = rob_offset + O3_RUNTIME_CHECKPOINT_ROB_ENTRY_BYTES_WITH_READY_TICK;
    encoded[live_staged_offset] = 1;
    encoded[live_staged_offset + 1] = 1;
    encoded[live_staged_offset + 3..live_staged_offset + 7].copy_from_slice(&3_u32.to_le_bytes());

    let error = O3RuntimeCheckpointPayload::decode(&encoded)
        .unwrap_err()
        .to_string();

    assert!(
        error.contains("already committed"),
        "legacy public-overlay payload should be rejected explicitly, got: {error}"
    );
}

#[test]
fn o3_runtime_checkpoint_rejects_invalid_live_staged_physical_register() {
    let snapshot = rem6_cpu::O3RuntimeSnapshot::new(
        [O3ReorderBufferEntry::new(
            10,
            Address::new(0x8000),
            Some(O3PhysicalRegisterId::new(40)),
        )
        .with_ready_tick(123)],
        [],
        [],
        O3PendingStateSnapshot::new(
            [],
            [],
            O3WritebackTransferSnapshot::new(
                O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap(),
                [],
            ),
        )
        .unwrap(),
    )
    .unwrap();
    let mut encoded = O3RuntimeCheckpointPayload::from_snapshot(snapshot)
        .unwrap()
        .encode();
    let rob_offset = o3_runtime_rob_payload_offset(&encoded);
    let physical_offset = rob_offset + 8 + 8 + 1;
    let live_staged_offset = rob_offset + O3_RUNTIME_CHECKPOINT_ROB_ENTRY_BYTES_WITH_READY_TICK;
    encoded[physical_offset..physical_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    encoded[live_staged_offset] = 1;
    encoded[live_staged_offset + 1] = 1;
    encoded[live_staged_offset + 3..live_staged_offset + 7].copy_from_slice(&3_u32.to_le_bytes());

    let error = O3RuntimeCheckpointPayload::decode(&encoded)
        .unwrap_err()
        .to_string();

    assert!(
        error.contains("invalid physical register"),
        "invalid staged physical register should be rejected explicitly, got: {error}"
    );
}

#[test]
fn o3_runtime_checkpoint_decodes_v3_non_integer_fu_class_stats() {
    let core = RiscvCore::new(core(0x8000));
    for (sequence, instruction) in [
        (
            1,
            RiscvInstruction::FloatClassS {
                rd: reg(12),
                rs1: freg(1),
            },
        ),
        (
            2,
            RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::ClassV {
                vd: vreg(3),
                vs2: vreg(2),
            }),
        ),
    ] {
        let pc = 0x8000 + sequence * 4;
        core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
            fetch_event(pc, sequence),
            instruction,
            RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), None),
        ));
    }

    let encoded = strip_current_lsq_forwarding_suppression_stats(
        &strip_current_rob_ready_tick_bytes(&core.o3_runtime_checkpoint_payload().encode()),
    );
    let stats_offset = encoded
        .len()
        .checked_sub(O3_RUNTIME_CHECKPOINT_STATS_BYTES_WITHOUT_FORWARDING_SUPPRESSION)
        .unwrap();
    let newer_stats_offset = stats_offset + O3_RUNTIME_CHECKPOINT_BASE_AND_FU_STATS_BYTES;
    let branch_event_offset = encoded
        .len()
        .checked_sub(O3_RUNTIME_CHECKPOINT_BRANCH_EVENT_STATS_BYTES)
        .unwrap();
    let max_occupancy_offset = newer_stats_offset
        + O3_RUNTIME_CHECKPOINT_LSQ_MATRIX_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_FORWARDING_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_LATENCY_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_DATA_LATENCY_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_BRANCH_REPAIR_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_IEW_BRANCH_MISPREDICT_SPLIT_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_IEW_DEPENDENCY_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_IQ_BRANCH_ISSUED_STATS_BYTES;
    let mut encoded = [
        &encoded[..newer_stats_offset],
        &encoded[max_occupancy_offset..branch_event_offset],
    ]
    .concat();
    encoded[O3_RUNTIME_CHECKPOINT_MAGIC_BYTES] = 3;
    let decoded = O3RuntimeCheckpointPayload::decode(&encoded).unwrap();
    let stats = decoded.stats();

    assert_eq!(stats.fu_latency_instructions(), 2);
    assert_eq!(stats.fu_latency_cycles(), 4);
    assert_eq!(
        stats.fu_latency_class_instructions(O3RuntimeFuLatencyClass::ScalarFloatMisc),
        1
    );
    assert_eq!(
        stats.fu_latency_class_cycles(O3RuntimeFuLatencyClass::ScalarFloatMisc),
        2
    );
    assert_eq!(
        stats.fu_latency_class_instructions(O3RuntimeFuLatencyClass::VectorFloatMisc),
        1
    );
    assert_eq!(
        stats.fu_latency_class_cycles(O3RuntimeFuLatencyClass::VectorFloatMisc),
        2
    );
    assert_eq!(stats.fu_integer_mul_instructions(), 0);
    assert_eq!(stats.fu_integer_div_instructions(), 0);
    assert_eq!(stats.max_rob_occupancy(), 2);
    assert_eq!(stats.max_lsq_occupancy(), 0);
    assert_eq!(stats.rename_map_entries(), 0);
    assert_eq!(stats.lsq_operation_count(O3RuntimeLsqOperation::Atomic), 0);
    assert_eq!(
        stats.lsq_ordering_count(O3RuntimeLsqOrdering::AcquireRelease),
        0
    );
    assert_eq!(stats.lsq_store_conditional_failures(), 0);
}

#[test]
fn o3_runtime_checkpoint_decodes_v4_lsq_matrix_stats_without_branch_repair_stats() {
    let core = RiscvCore::new(core(0x8000));
    for (sequence, instruction, access) in [
        (
            1,
            RiscvInstruction::LoadReserved {
                rd: reg(7),
                rs1: reg(5),
                width: MemoryWidth::Doubleword,
                acquire: true,
                release: false,
            },
            MemoryAccessKind::LoadReserved {
                rd: reg(7),
                address: 0x9000,
                width: MemoryWidth::Doubleword,
                acquire: true,
                release: false,
            },
        ),
        (
            2,
            RiscvInstruction::StoreConditional {
                rd: reg(8),
                rs1: reg(5),
                rs2: reg(6),
                width: MemoryWidth::Doubleword,
                acquire: false,
                release: true,
            },
            MemoryAccessKind::StoreConditional {
                rd: reg(8),
                address: 0x9000,
                width: MemoryWidth::Doubleword,
                value: 3,
                acquire: false,
                release: true,
            },
        ),
        (
            3,
            RiscvInstruction::AtomicMemory {
                rd: reg(9),
                rs1: reg(5),
                rs2: reg(6),
                width: MemoryWidth::Doubleword,
                op: AtomicMemoryOp::Swap,
                acquire: true,
                release: true,
            },
            MemoryAccessKind::AtomicMemory {
                rd: reg(9),
                address: 0x9000,
                width: MemoryWidth::Doubleword,
                op: AtomicMemoryOp::Swap,
                value: 4,
                acquire: true,
                release: true,
            },
        ),
    ] {
        let pc = 0x8000 + sequence * 4;
        core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
            fetch_event(pc, sequence),
            instruction,
            RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
        ));
    }

    let payload = core.o3_runtime_checkpoint_payload();
    let encoded =
        strip_current_lsq_forwarding_suppression_stats(&strip_current_rob_ready_tick_bytes(
            &strip_current_rename_dependency_bytes(&payload.encode()),
        ));
    let stats_offset = encoded
        .len()
        .checked_sub(O3_RUNTIME_CHECKPOINT_STATS_BYTES_WITHOUT_FORWARDING_SUPPRESSION)
        .unwrap();
    let lsq_latency_offset = stats_offset
        + O3_RUNTIME_CHECKPOINT_BASE_AND_FU_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_STATS_BYTES;
    let lsq_ordering_offset = lsq_latency_offset
        + O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_FORWARDING_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_LATENCY_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_DATA_LATENCY_STATS_BYTES;
    let branch_repair_offset = lsq_ordering_offset + O3_RUNTIME_CHECKPOINT_LSQ_ORDERING_STATS_BYTES;
    let branch_event_offset = encoded
        .len()
        .checked_sub(O3_RUNTIME_CHECKPOINT_BRANCH_EVENT_STATS_BYTES)
        .unwrap();
    let max_occupancy_offset = branch_repair_offset
        + O3_RUNTIME_CHECKPOINT_BRANCH_REPAIR_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_IEW_BRANCH_MISPREDICT_SPLIT_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_IEW_DEPENDENCY_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_IQ_BRANCH_ISSUED_STATS_BYTES;
    let mut encoded = [
        &encoded[..lsq_latency_offset],
        &encoded[lsq_ordering_offset..branch_repair_offset],
        &encoded[max_occupancy_offset..branch_event_offset],
    ]
    .concat();
    encoded[O3_RUNTIME_CHECKPOINT_MAGIC_BYTES] = 4;
    let decoded = O3RuntimeCheckpointPayload::decode(&encoded).unwrap();
    let stats = decoded.stats();

    assert_eq!(decoded.snapshot(), payload.snapshot());
    assert_eq!(
        stats.lsq_operation_count(O3RuntimeLsqOperation::LoadReserved),
        1
    );
    assert_eq!(
        stats.lsq_operation_count(O3RuntimeLsqOperation::StoreConditional),
        1
    );
    assert_eq!(stats.lsq_operation_count(O3RuntimeLsqOperation::Atomic), 1);
    assert_eq!(stats.lsq_ordering_count(O3RuntimeLsqOrdering::Acquire), 1);
    assert_eq!(stats.lsq_ordering_count(O3RuntimeLsqOrdering::Release), 1);
    assert_eq!(
        stats.lsq_ordering_count(O3RuntimeLsqOrdering::AcquireRelease),
        1
    );
    assert_eq!(stats.lsq_store_conditional_failures(), 0);
    assert_eq!(stats.branch_repair_targetless_mismatches(), 0);
    assert_eq!(stats.branch_repair_wrong_targets(), 0);
    assert_eq!(stats.branch_repair_direction_only_mismatches(), 0);
}

#[test]
fn o3_runtime_checkpoint_decodes_v5_branch_repair_stats_without_lsq_latency_stats() {
    let core = RiscvCore::new(core(0x8000));
    for (sequence, instruction, access) in [
        (
            1,
            RiscvInstruction::LoadReserved {
                rd: reg(7),
                rs1: reg(5),
                width: MemoryWidth::Doubleword,
                acquire: true,
                release: false,
            },
            MemoryAccessKind::LoadReserved {
                rd: reg(7),
                address: 0x9000,
                width: MemoryWidth::Doubleword,
                acquire: true,
                release: false,
            },
        ),
        (
            2,
            RiscvInstruction::StoreConditional {
                rd: reg(8),
                rs1: reg(5),
                rs2: reg(6),
                width: MemoryWidth::Doubleword,
                acquire: false,
                release: true,
            },
            MemoryAccessKind::StoreConditional {
                rd: reg(8),
                address: 0x9000,
                width: MemoryWidth::Doubleword,
                value: 3,
                acquire: false,
                release: true,
            },
        ),
    ] {
        let pc = 0x8000 + sequence * 4;
        core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
            fetch_event(pc, sequence),
            instruction,
            RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
        ));
    }

    let payload = core.o3_runtime_checkpoint_payload();
    let encoded =
        strip_current_lsq_forwarding_suppression_stats(&strip_current_rob_ready_tick_bytes(
            &strip_current_rename_dependency_bytes(&payload.encode()),
        ));
    let stats_offset = encoded
        .len()
        .checked_sub(O3_RUNTIME_CHECKPOINT_STATS_BYTES_WITHOUT_FORWARDING_SUPPRESSION)
        .unwrap();
    let lsq_latency_offset = stats_offset
        + O3_RUNTIME_CHECKPOINT_BASE_AND_FU_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_STATS_BYTES;
    let lsq_ordering_offset = lsq_latency_offset
        + O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_FORWARDING_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_LATENCY_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_DATA_LATENCY_STATS_BYTES;
    let iew_split_offset = lsq_ordering_offset
        + O3_RUNTIME_CHECKPOINT_LSQ_ORDERING_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_BRANCH_REPAIR_STATS_BYTES;
    let branch_event_offset = encoded
        .len()
        .checked_sub(O3_RUNTIME_CHECKPOINT_BRANCH_EVENT_STATS_BYTES)
        .unwrap();
    let max_occupancy_offset = iew_split_offset
        + O3_RUNTIME_CHECKPOINT_IEW_BRANCH_MISPREDICT_SPLIT_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_IEW_DEPENDENCY_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_IQ_BRANCH_ISSUED_STATS_BYTES;
    let mut encoded = [
        &encoded[..lsq_latency_offset],
        &encoded[lsq_ordering_offset..iew_split_offset],
        &encoded[max_occupancy_offset..branch_event_offset],
    ]
    .concat();
    encoded[O3_RUNTIME_CHECKPOINT_MAGIC_BYTES] = 5;
    let decoded = O3RuntimeCheckpointPayload::decode(&encoded).unwrap();
    let stats = decoded.stats();

    assert_eq!(decoded.snapshot(), payload.snapshot());
    assert_eq!(
        stats.lsq_operation_count(O3RuntimeLsqOperation::LoadReserved),
        1
    );
    assert_eq!(
        stats.lsq_operation_count(O3RuntimeLsqOperation::StoreConditional),
        1
    );
    assert_eq!(stats.lsq_ordering_count(O3RuntimeLsqOrdering::Acquire), 1);
    assert_eq!(stats.lsq_ordering_count(O3RuntimeLsqOrdering::Release), 1);
    assert_eq!(
        stats.lsq_operation_latency_ticks(O3RuntimeLsqOperation::LoadReserved),
        0
    );
    assert_eq!(
        stats.lsq_operation_latency_avg_ticks(O3RuntimeLsqOperation::StoreConditional),
        0
    );
    assert_eq!(stats.branch_repair_targetless_mismatches(), 0);
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

#[test]
fn o3_runtime_stats_count_grouped_vector_segment_load_rename_destinations() {
    let core = RiscvCore::new(core(0x8000));
    let instruction =
        RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::LoadSegmentUnitStride {
            vd: vreg(8),
            rs1: reg(10),
            width: MemoryWidth::Word,
            fields: 3,
            mask: RiscvVectorMaskMode::Unmasked,
        });
    let access = MemoryAccessKind::VectorLoadSegmentUnitStride {
        vd: vreg(8),
        address: 0x9000,
        width: MemoryWidth::Word,
        fields: 3,
        element_count: 2,
        byte_len: 48,
        byte_mask: None,
        group_registers: 2,
    };
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 1),
        instruction,
        RiscvExecutionRecord::new(instruction, 0x8000, 0x8004, Vec::new(), Some(access)),
    ));

    let stats = core.o3_runtime_stats();
    assert_eq!(stats.instructions(), 1);
    assert_eq!(stats.rob_allocations(), 1);
    assert_eq!(stats.rob_commits(), 1);
    assert_eq!(stats.rename_writes(), 6);
    assert_eq!(stats.lsq_loads(), 1);
    assert_eq!(stats.lsq_stores(), 0);
}

#[test]
fn o3_runtime_trace_records_only_when_enabled() {
    let core = RiscvCore::new(core(0x8000));
    let instruction = RiscvInstruction::Addi {
        rd: reg(5),
        rs1: reg(0),
        imm: Immediate::new(7),
    };
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 1),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            0x8000,
            0x8004,
            vec![RegisterWrite::new(reg(5), 7)],
            None,
        ),
    ));

    assert_eq!(core.o3_runtime_stats().instructions(), 1);
    assert!(core.o3_runtime_trace_records().is_empty());

    core.reset_o3_runtime_stats();
    core.record_o3_retired_instruction_with_trace(
        &RiscvCpuExecutionEvent::new(
            fetch_event(0x8004, 2),
            instruction,
            RiscvExecutionRecord::new(
                instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(reg(5), 8)],
                None,
            ),
        ),
        true,
    );

    let trace = core.o3_runtime_trace_records();
    assert_eq!(trace.len(), 1);
    assert_eq!(trace[0].sequence(), 1);
    assert_eq!(trace[0].tick(), 12);
    assert_eq!(trace[0].pc(), Address::new(0x8004));
    assert!(trace[0].rob_allocated());
    assert!(trace[0].rob_committed());
    assert_eq!(trace[0].rename_writes(), 1);
}

#[test]
fn o3_runtime_trace_dates_partial_rob_drain_at_committed_ready_tick() {
    let core = RiscvCore::new(core(0x8000));
    let older = RiscvInstruction::Mul {
        rd: reg(3),
        rs1: reg(1),
        rs2: reg(2),
    };
    core.record_o3_retired_instruction_with_trace(
        &RiscvCpuExecutionEvent::new(
            fetch_event(0x8000, 1),
            older,
            RiscvExecutionRecord::new(
                older,
                0x8000,
                0x8004,
                vec![RegisterWrite::new(reg(3), 42)],
                None,
            ),
        ),
        true,
    );

    let younger = RiscvInstruction::Div {
        rd: reg(4),
        rs1: reg(2),
        rs2: reg(1),
    };
    core.record_o3_retired_instruction_with_trace(
        &RiscvCpuExecutionEvent::new(
            fetch_event(0x8004, 2),
            younger,
            RiscvExecutionRecord::new(
                younger,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(reg(4), 1)],
                None,
            ),
        ),
        true,
    );

    let trace = core.o3_runtime_trace_records();
    assert_eq!(trace.len(), 2);
    let older_ready_tick = trace[0].writeback_tick();
    assert!(trace[1].writeback_tick() > older_ready_tick);
    assert_eq!(trace[1].rob_commits_at_tick(), 1);
    assert!(trace[1].rob_commit_blocked());
    assert_eq!(trace[1].commit_tick(), trace[1].writeback_tick());
    assert!(trace[1].commit_tick() >= older_ready_tick);
    assert_eq!(trace[1].writeback_to_commit_ticks(), 0);
}

#[test]
fn o3_runtime_stats_ignore_x0_memory_destinations_for_rename_writes() {
    let core = RiscvCore::new(core(0x8000));
    let cases = [
        (
            RiscvInstruction::Load {
                rd: reg(0),
                rs1: reg(10),
                offset: Immediate::new(0),
                width: MemoryWidth::Word,
                signed: false,
            },
            MemoryAccessKind::Load {
                rd: reg(0),
                address: 0x9000,
                width: MemoryWidth::Word,
                signed: false,
            },
        ),
        (
            RiscvInstruction::LoadReserved {
                rd: reg(0),
                rs1: reg(10),
                width: MemoryWidth::Word,
                acquire: false,
                release: false,
            },
            MemoryAccessKind::LoadReserved {
                rd: reg(0),
                address: 0x9004,
                width: MemoryWidth::Word,
                acquire: false,
                release: false,
            },
        ),
        (
            RiscvInstruction::StoreConditional {
                rd: reg(0),
                rs1: reg(10),
                rs2: reg(11),
                width: MemoryWidth::Word,
                acquire: false,
                release: false,
            },
            MemoryAccessKind::StoreConditional {
                rd: reg(0),
                address: 0x9008,
                width: MemoryWidth::Word,
                value: 0x1234,
                acquire: false,
                release: false,
            },
        ),
        (
            RiscvInstruction::AtomicMemory {
                rd: reg(0),
                rs1: reg(10),
                rs2: reg(11),
                width: MemoryWidth::Word,
                op: AtomicMemoryOp::Add,
                acquire: false,
                release: false,
            },
            MemoryAccessKind::AtomicMemory {
                rd: reg(0),
                address: 0x900c,
                width: MemoryWidth::Word,
                op: AtomicMemoryOp::Add,
                value: 0x5678,
                acquire: false,
                release: false,
            },
        ),
    ];

    for (index, (instruction, access)) in cases.into_iter().enumerate() {
        let pc = 0x8000 + u64::try_from(index).unwrap() * 4;
        core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
            fetch_event(pc, 10 + u64::try_from(index).unwrap()),
            instruction,
            RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
        ));
    }

    let stats = core.o3_runtime_stats();
    assert_eq!(stats.instructions(), 4);
    assert_eq!(stats.rob_allocations(), 4);
    assert_eq!(stats.rob_commits(), 4);
    assert_eq!(stats.rename_writes(), 0);
    assert_eq!(stats.lsq_loads(), 3);
    assert_eq!(stats.lsq_stores(), 2);
}

#[test]
fn o3_runtime_stats_do_not_claim_unowned_store_load_forwarding_match() {
    let core = RiscvCore::new(core(0x8000));
    let store = RiscvInstruction::Store {
        rs1: reg(10),
        rs2: reg(11),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
    };
    let load = RiscvInstruction::Load {
        rd: reg(12),
        rs1: reg(10),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 20),
        store,
        RiscvExecutionRecord::new(
            store,
            0x8000,
            0x8004,
            Vec::new(),
            Some(MemoryAccessKind::Store {
                address: 0x9000,
                width: MemoryWidth::Word,
                value: 0x5a,
            }),
        ),
    ));
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8004, 21),
        load,
        RiscvExecutionRecord::new(
            load,
            0x8004,
            0x8008,
            Vec::new(),
            Some(MemoryAccessKind::Load {
                rd: reg(12),
                address: 0x9000,
                width: MemoryWidth::Word,
                signed: false,
            }),
        ),
    ));
    let stats = core.o3_runtime_stats();
    assert_eq!(stats.lsq_loads(), 1);
    assert_eq!(stats.lsq_stores(), 1);
    assert_eq!(stats.lsq_store_to_load_forwarding_candidates(), 0);
    assert_eq!(stats.lsq_store_to_load_forwarding_matches(), 0);
}

#[test]
fn o3_runtime_reset_stats_clears_store_forwarding_window() {
    let core = RiscvCore::new(core(0x8000));
    let store = scalar_store_instruction();
    let load = scalar_load_instruction();
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 20),
        store,
        RiscvExecutionRecord::new(
            store,
            0x8000,
            0x8004,
            Vec::new(),
            Some(scalar_store_access(0x5a)),
        ),
    ));

    core.reset_o3_runtime_stats();
    let mismatched_load = MemoryAccessKind::Load {
        rd: reg(12),
        address: 0x9004,
        width: MemoryWidth::Word,
        signed: false,
    };
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8004, 21),
        load,
        RiscvExecutionRecord::new(load, 0x8004, 0x8008, Vec::new(), Some(mismatched_load)),
    ));

    let stats = core.o3_runtime_stats();
    assert_eq!(stats.lsq_loads(), 1);
    assert_eq!(stats.lsq_stores(), 0);
    assert_eq!(stats.lsq_store_to_load_forwarding_candidates(), 0);
    assert_eq!(stats.lsq_store_to_load_forwarding_matches(), 0);
    assert_eq!(stats.lsq_store_to_load_forwarding_suppressed(), 0);
}

#[test]
fn o3_runtime_checkpoint_encoding_is_stable_after_out_of_order_rename_retire() {
    let core = RiscvCore::new(core(0x8000));
    for (index, register) in [reg(11), reg(5)].into_iter().enumerate() {
        let pc = 0x8000 + u64::try_from(index).unwrap() * 4;
        let instruction = RiscvInstruction::Addi {
            rd: register,
            rs1: reg(0),
            imm: Immediate::new(i64::try_from(index).unwrap()),
        };
        core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
            fetch_event(pc, 20 + u64::try_from(index).unwrap()),
            instruction,
            RiscvExecutionRecord::new(
                instruction,
                pc,
                pc + 4,
                vec![RegisterWrite::new(register, u64::try_from(index).unwrap())],
                None,
            ),
        ));
    }

    let encoded = core.o3_runtime_checkpoint_payload().encode();
    let decoded = O3RuntimeCheckpointPayload::decode(&encoded).unwrap();

    assert_eq!(decoded.encode(), encoded);
    assert_eq!(decoded.snapshot().rename_map()[0].architectural(), 5);
    assert_eq!(decoded.snapshot().rename_map()[1].architectural(), 11);
}

#[test]
fn o3_runtime_checkpoint_restore_preserves_counted_dependency_producers() {
    let core = RiscvCore::new(core(0x8000));
    for (pc, sequence, rd, rs1, value) in [
        (0x8000, 20, reg(5), reg(0), 7),
        (0x8004, 21, reg(6), reg(5), 8),
    ] {
        let instruction = RiscvInstruction::Addi {
            rd,
            rs1,
            imm: Immediate::new(i64::try_from(value).unwrap()),
        };
        core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
            fetch_event(pc, sequence),
            instruction,
            RiscvExecutionRecord::new(
                instruction,
                pc,
                pc + 4,
                vec![RegisterWrite::new(rd, value)],
                None,
            ),
        ));
    }
    assert_eq!(core.o3_runtime_stats().iew_producer_insts(), 1);
    assert_eq!(core.o3_runtime_stats().iew_consumer_insts(), 1);

    let payload = core.o3_runtime_checkpoint_payload();
    core.restore_o3_runtime_checkpoint_payload(payload).unwrap();

    let instruction = RiscvInstruction::Addi {
        rd: reg(7),
        rs1: reg(5),
        imm: Immediate::new(9),
    };
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8008, 22),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            0x8008,
            0x800c,
            vec![RegisterWrite::new(reg(7), 9)],
            None,
        ),
    ));

    let stats = core.o3_runtime_stats();
    assert_eq!(stats.iew_producer_insts(), 1);
    assert_eq!(stats.iew_consumer_insts(), 2);
}

#[test]
fn o3_runtime_dependency_fanout_counts_csr_register_sources() {
    let core = RiscvCore::new(core(0x8000));
    let producer = RiscvInstruction::Addi {
        rd: reg(5),
        rs1: reg(0),
        imm: Immediate::new(7),
    };
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 20),
        producer,
        RiscvExecutionRecord::new(
            producer,
            0x8000,
            0x8004,
            vec![RegisterWrite::new(reg(5), 7)],
            None,
        ),
    ));
    let direct_csr_source = RiscvInstruction::WriteCounterCsr {
        rd: reg(0),
        csr: RiscvCounterCsr::Cycle,
        rs1: reg(5),
    };
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8004, 21),
        direct_csr_source,
        RiscvExecutionRecord::new(direct_csr_source, 0x8004, 0x8008, Vec::new(), None),
    ));
    let wrapped_csr_source =
        RiscvInstruction::CounterInhibitCsr(RiscvCounterInhibitCsrInstruction::register(
            reg(0),
            RiscvCounterInhibitCsr::Mcountinhibit,
            RiscvCsrOp::Write,
            reg(5),
        ));
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8008, 22),
        wrapped_csr_source,
        RiscvExecutionRecord::new(wrapped_csr_source, 0x8008, 0x800c, Vec::new(), None),
    ));

    let stats = core.o3_runtime_stats();
    assert_eq!(stats.iew_producer_insts(), 1);
    assert_eq!(stats.iew_consumer_insts(), 2);
}

#[test]
fn o3_runtime_dependency_fanout_counts_sfence_vma_sources() {
    let core = RiscvCore::new(core(0x8000));
    for (pc, sequence, rd, value) in [(0x8000, 20, reg(5), 7), (0x8004, 21, reg(6), 8)] {
        let producer = RiscvInstruction::Addi {
            rd,
            rs1: reg(0),
            imm: Immediate::new(value),
        };
        core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
            fetch_event(pc, sequence),
            producer,
            RiscvExecutionRecord::new(
                producer,
                pc,
                pc + 4,
                vec![RegisterWrite::new(rd, value as u64)],
                None,
            ),
        ));
    }
    let sfence = RiscvInstruction::SfenceVma {
        rs1: reg(5),
        rs2: reg(6),
    };
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8008, 22),
        sfence,
        RiscvExecutionRecord::new(sfence, 0x8008, 0x800c, Vec::new(), None),
    ));

    let stats = core.o3_runtime_stats();
    assert_eq!(stats.iew_producer_insts(), 2);
    assert_eq!(stats.iew_consumer_insts(), 2);
}

fn o3_runtime_rob_payload_offset(payload: &[u8]) -> usize {
    let pending_len_bytes = payload
        [O3_RUNTIME_CHECKPOINT_PENDING_LEN_OFFSET..O3_RUNTIME_CHECKPOINT_PENDING_LEN_OFFSET + 4]
        .try_into()
        .unwrap();
    O3_RUNTIME_CHECKPOINT_HEADER_BYTES + u32::from_le_bytes(pending_len_bytes) as usize
}

fn strip_current_live_retire_gate(payload: &[u8]) -> Vec<u8> {
    let payload = strip_current_live_staged_rob_bytes(payload);
    let trailer_offset = payload
        .len()
        .checked_sub(O3_RUNTIME_CHECKPOINT_LIVE_RETIRE_GATE_PAYLOAD_BYTES)
        .unwrap();
    let trailing_branch_stats = O3_RUNTIME_CHECKPOINT_BRANCH_EVENT_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_BRANCH_EVENT_PREDICTION_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_BRANCH_MISMATCH_STATS_BYTES;
    let live_stats_offset = trailer_offset
        .checked_sub(trailing_branch_stats + O3_RUNTIME_CHECKPOINT_LIVE_RETIRE_GATE_STATS_BYTES)
        .unwrap();
    [
        &payload[..live_stats_offset],
        &payload[live_stats_offset + O3_RUNTIME_CHECKPOINT_LIVE_RETIRE_GATE_STATS_BYTES
            ..trailer_offset],
    ]
    .concat()
}

fn strip_current_issue_arbitration_stats(payload: &[u8]) -> Vec<u8> {
    let payload = strip_current_writeback_port_stats(payload);
    if payload[O3_RUNTIME_CHECKPOINT_MAGIC_BYTES]
        < O3_RUNTIME_CHECKPOINT_VERSION_WITH_ISSUE_ARBITRATION_STATS
    {
        return payload;
    }
    let trailer_offset = payload
        .len()
        .checked_sub(O3_RUNTIME_CHECKPOINT_LIVE_RETIRE_GATE_PAYLOAD_BYTES)
        .unwrap();
    let issue_offset = trailer_offset
        .checked_sub(O3_RUNTIME_CHECKPOINT_ISSUE_ARBITRATION_STATS_BYTES)
        .unwrap();
    [&payload[..issue_offset], &payload[trailer_offset..]].concat()
}

fn strip_current_writeback_port_stats(payload: &[u8]) -> Vec<u8> {
    if payload[O3_RUNTIME_CHECKPOINT_MAGIC_BYTES]
        < O3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS
    {
        return payload.to_vec();
    }
    let trailer_offset = payload
        .len()
        .checked_sub(O3_RUNTIME_CHECKPOINT_LIVE_RETIRE_GATE_PAYLOAD_BYTES)
        .unwrap();
    let writeback_offset = trailer_offset
        .checked_sub(O3_RUNTIME_CHECKPOINT_WRITEBACK_PORT_STATS_BYTES)
        .unwrap();
    [&payload[..writeback_offset], &payload[trailer_offset..]].concat()
}

fn strip_current_live_staged_rob_bytes(payload: &[u8]) -> Vec<u8> {
    let payload = strip_current_issue_arbitration_stats(payload);
    let pending_len = checkpoint_u32(&payload, O3_RUNTIME_CHECKPOINT_PENDING_LEN_OFFSET) as usize;
    let rob_count = checkpoint_u32(&payload, O3_RUNTIME_CHECKPOINT_ROB_COUNT_OFFSET) as usize;
    let rob_offset = O3_RUNTIME_CHECKPOINT_HEADER_BYTES + pending_len;
    let mut offset = rob_offset;
    let live_metadata_bytes = O3_RUNTIME_CHECKPOINT_ROB_ENTRY_BYTES
        - O3_RUNTIME_CHECKPOINT_ROB_ENTRY_BYTES_WITH_READY_TICK;
    let mut legacy = Vec::with_capacity(
        payload
            .len()
            .saturating_sub(rob_count * live_metadata_bytes),
    );
    legacy.extend_from_slice(&payload[..rob_offset]);
    for _ in 0..rob_count {
        legacy.extend_from_slice(
            &payload[offset..offset + O3_RUNTIME_CHECKPOINT_ROB_ENTRY_BYTES_WITH_READY_TICK],
        );
        offset += O3_RUNTIME_CHECKPOINT_ROB_ENTRY_BYTES;
    }
    legacy.extend_from_slice(&payload[offset..]);
    legacy
}

fn strip_current_rob_ready_tick_bytes(payload: &[u8]) -> Vec<u8> {
    let payload = strip_current_live_retire_gate(payload);
    let pending_len = checkpoint_u32(&payload, O3_RUNTIME_CHECKPOINT_PENDING_LEN_OFFSET) as usize;
    let rob_count = checkpoint_u32(&payload, O3_RUNTIME_CHECKPOINT_ROB_COUNT_OFFSET) as usize;
    let rob_offset = O3_RUNTIME_CHECKPOINT_HEADER_BYTES + pending_len;
    let mut offset = rob_offset;
    let mut legacy = Vec::with_capacity(payload.len().saturating_sub(rob_count * 8));
    legacy.extend_from_slice(&payload[..rob_offset]);
    for _ in 0..rob_count {
        legacy.extend_from_slice(
            &payload[offset..offset + O3_RUNTIME_CHECKPOINT_ROB_ENTRY_BYTES_LEGACY],
        );
        offset += O3_RUNTIME_CHECKPOINT_ROB_ENTRY_BYTES_WITH_READY_TICK;
    }
    legacy.extend_from_slice(&payload[offset..]);
    legacy
}

fn strip_current_rename_dependency_bytes(payload: &[u8]) -> Vec<u8> {
    let pending_len = checkpoint_u32(payload, O3_RUNTIME_CHECKPOINT_PENDING_LEN_OFFSET) as usize;
    let rob_count = checkpoint_u32(payload, O3_RUNTIME_CHECKPOINT_ROB_COUNT_OFFSET) as usize;
    let lsq_count = checkpoint_u32(payload, O3_RUNTIME_CHECKPOINT_LSQ_COUNT_OFFSET) as usize;
    let rename_count = checkpoint_u32(payload, O3_RUNTIME_CHECKPOINT_RENAME_COUNT_OFFSET) as usize;
    let mut offset = O3_RUNTIME_CHECKPOINT_HEADER_BYTES
        + pending_len
        + rob_count * O3_RUNTIME_CHECKPOINT_ROB_ENTRY_BYTES
        + lsq_count * O3_RUNTIME_CHECKPOINT_LSQ_ENTRY_BYTES;
    let mut legacy = Vec::with_capacity(payload.len().saturating_sub(rename_count));
    legacy.extend_from_slice(&payload[..offset]);
    for _ in 0..rename_count {
        legacy
            .extend_from_slice(&payload[offset..offset + O3_RUNTIME_CHECKPOINT_RENAME_ENTRY_BYTES]);
        offset += O3_RUNTIME_CHECKPOINT_RENAME_ENTRY_BYTES_WITH_DEPENDENCY;
    }
    legacy.extend_from_slice(&payload[offset..]);
    legacy
}

fn strip_current_lsq_forwarding_suppression_stats(payload: &[u8]) -> Vec<u8> {
    let payload = strip_current_lsq_forwarding_suppression_reason_stats(payload);
    let stats_offset = payload
        .len()
        .checked_sub(O3_RUNTIME_CHECKPOINT_STATS_BYTES_WITHOUT_FORWARDING_SUPPRESSION_REASON)
        .unwrap();
    let operation_suppression_offset = stats_offset
        + O3_RUNTIME_CHECKPOINT_CURRENT_BASE_AND_FU_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_FORWARDING_STATS_BYTES;
    let operation_suppression_bytes =
        O3_RUNTIME_CHECKPOINT_LSQ_FORWARDING_SUPPRESSION_STATS_BYTES - 8;
    let aggregate_suppression_offset = stats_offset + 10 * 8;
    let without_operation_suppression = [
        &payload[..operation_suppression_offset],
        &payload[operation_suppression_offset + operation_suppression_bytes..],
    ]
    .concat();
    [
        &without_operation_suppression[..aggregate_suppression_offset],
        &without_operation_suppression[aggregate_suppression_offset + 8..],
    ]
    .concat()
}

fn strip_current_lsq_forwarding_suppression_reason_stats(payload: &[u8]) -> Vec<u8> {
    let payload = strip_current_branch_event_prediction_stats(payload);
    let stats_offset = payload
        .len()
        .checked_sub(
            O3_RUNTIME_CHECKPOINT_STATS_BYTES
                - O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_BYTE_STATS_BYTES,
        )
        .unwrap();
    let operation_suppression_bytes =
        O3_RUNTIME_CHECKPOINT_LSQ_FORWARDING_SUPPRESSION_STATS_BYTES - 8;
    let reason_offset = stats_offset
        + O3_RUNTIME_CHECKPOINT_CURRENT_BASE_AND_FU_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_FORWARDING_STATS_BYTES
        + operation_suppression_bytes;
    [
        &payload[..reason_offset],
        &payload
            [reason_offset + O3_RUNTIME_CHECKPOINT_LSQ_FORWARDING_SUPPRESSION_REASON_STATS_BYTES..],
    ]
    .concat()
}

fn strip_current_branch_event_prediction_stats(payload: &[u8]) -> Vec<u8> {
    let payload = strip_current_branch_mismatch_stats(payload);
    let prediction_offset = payload
        .len()
        .checked_sub(O3_RUNTIME_CHECKPOINT_BRANCH_EVENT_PREDICTION_STATS_BYTES)
        .unwrap();
    strip_current_lsq_operation_byte_stats(
        &payload[..prediction_offset],
        O3_RUNTIME_CHECKPOINT_STATS_BYTES_WITHOUT_BRANCH_MISMATCH
            - O3_RUNTIME_CHECKPOINT_BRANCH_EVENT_PREDICTION_STATS_BYTES,
    )
}

fn strip_current_branch_mismatch_stats(payload: &[u8]) -> Vec<u8> {
    let payload = strip_current_fu_latency_class_extrema_stats(payload);
    let mismatch_offset = payload
        .len()
        .checked_sub(O3_RUNTIME_CHECKPOINT_BRANCH_MISMATCH_STATS_BYTES)
        .unwrap();
    payload[..mismatch_offset].to_vec()
}

fn strip_current_fu_latency_class_extrema_stats(payload: &[u8]) -> Vec<u8> {
    let stats_offset = payload
        .len()
        .checked_sub(O3_RUNTIME_CHECKPOINT_V21_STATS_BYTES)
        .unwrap();
    let extrema_offset = stats_offset + O3_RUNTIME_CHECKPOINT_CURRENT_BASE_AND_FU_STATS_BYTES;
    [
        &payload[..extrema_offset],
        &payload[extrema_offset + O3_RUNTIME_CHECKPOINT_FU_LATENCY_CLASS_EXTREMA_STATS_BYTES..],
    ]
    .concat()
}

fn strip_current_lsq_operation_byte_stats(
    payload: &[u8],
    stats_bytes_with_lsq_operation_byte_stats: usize,
) -> Vec<u8> {
    let stats_offset = payload
        .len()
        .checked_sub(stats_bytes_with_lsq_operation_byte_stats)
        .unwrap();
    let byte_offset = stats_offset
        + O3_RUNTIME_CHECKPOINT_CURRENT_BASE_AND_FU_STATS_BYTES
        + O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_STATS_BYTES;
    [
        &payload[..byte_offset],
        &payload[byte_offset + O3_RUNTIME_CHECKPOINT_LSQ_OPERATION_BYTE_STATS_BYTES..],
    ]
    .concat()
}

fn checkpoint_u32(payload: &[u8], offset: usize) -> u32 {
    let bytes = payload[offset..offset + 4].try_into().unwrap();
    u32::from_le_bytes(bytes)
}

fn core(entry: u64) -> CpuCore {
    CpuCore::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(7),
            Address::new(entry),
        ),
        CpuFetchConfig::new(
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            MemoryRouteId::new(0),
            rem6_memory::CacheLineLayout::new(64).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap()
}

fn fetch_event(pc: u64, sequence: u64) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            memory_request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        0x0000_0073u32.to_le_bytes().to_vec(),
    )
}

fn memory_request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn scalar_store_instruction() -> RiscvInstruction {
    RiscvInstruction::Store {
        rs1: reg(10),
        rs2: reg(11),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
    }
}

fn scalar_load_instruction() -> RiscvInstruction {
    RiscvInstruction::Load {
        rd: reg(12),
        rs1: reg(10),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    }
}

fn scalar_store_access(value: u64) -> MemoryAccessKind {
    MemoryAccessKind::Store {
        address: 0x9000,
        width: MemoryWidth::Word,
        value,
    }
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn freg(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

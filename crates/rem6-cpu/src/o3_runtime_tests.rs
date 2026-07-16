use rem6_isa_riscv::{MemoryWidth, RiscvExecutionRecord, RiscvInstruction};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, AgentId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

use super::*;
use crate::{CpuFetchEvent, CpuFetchRecord};

#[path = "o3_runtime/tests/pending_data.rs"]
mod pending_data;

#[test]
fn o3_issue_width_defaults_to_shared_cpu_default() {
    let runtime = O3RuntimeState::default();

    assert_eq!(runtime.issue_width(), DEFAULT_RISCV_O3_ISSUE_WIDTH);
}

#[test]
fn o3_issue_width_setter_accepts_shared_range() {
    let mut runtime = O3RuntimeState::default();

    for width in [
        MIN_RISCV_O3_ISSUE_WIDTH,
        DEFAULT_RISCV_O3_ISSUE_WIDTH,
        MAX_RISCV_O3_ISSUE_WIDTH,
    ] {
        assert!(runtime.set_issue_width(width), "{width}");
        assert_eq!(runtime.issue_width(), width);
    }
}

#[test]
fn o3_issue_width_setter_rejects_out_of_range_without_changing_existing_width() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));

    for width in [MIN_RISCV_O3_ISSUE_WIDTH - 1, MAX_RISCV_O3_ISSUE_WIDTH + 1] {
        assert!(!runtime.set_issue_width(width), "{width}");
        assert_eq!(runtime.issue_width(), 2);
    }
}

#[test]
fn o3_issue_width_survives_snapshot_restore() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(3));
    let snapshot = O3RuntimeState::default().snapshot();

    runtime.restore(snapshot).unwrap();

    assert_eq!(runtime.issue_width(), 3);
}

#[test]
fn branch_link_write_uses_actual_coroutine_register_write() {
    let coroutine = RiscvInstruction::Jalr {
        rd: rem6_isa_riscv::Register::new(5).unwrap(),
        rs1: rem6_isa_riscv::Register::new(1).unwrap(),
        offset: rem6_isa_riscv::Immediate::new(0),
    };
    let record = RiscvExecutionRecord::new(
        coroutine,
        0x800c,
        0x8008,
        vec![rem6_isa_riscv::RegisterWrite::new(
            rem6_isa_riscv::Register::new(5).unwrap(),
            0x8010,
        )],
        None,
    );

    assert!(o3_branch_link_register_write(&record));
}

#[test]
fn branch_link_write_keeps_plain_return_false() {
    let plain_return = RiscvInstruction::Jalr {
        rd: rem6_isa_riscv::Register::new(0).unwrap(),
        rs1: rem6_isa_riscv::Register::new(1).unwrap(),
        offset: rem6_isa_riscv::Immediate::new(0),
    };
    let record = RiscvExecutionRecord::new(plain_return, 0x800c, 0x8008, vec![], None);

    assert!(!o3_branch_link_register_write(&record));
}

#[test]
fn failed_store_conditional_stats_count_failed_operation() {
    let mut runtime = O3RuntimeState::default();
    let mut event = store_conditional_event(0x8000, 10);

    runtime.record_retired_instruction(&event);
    event.set_data_access_event_kind(RiscvDataAccessEventKind::ConditionalFailed);
    runtime.record_data_access_outcome(&event, 41, 7);

    let stats = runtime.stats();

    assert_eq!(
        stats.lsq_operation_count(O3RuntimeLsqOperation::StoreConditional),
        1
    );
    assert_eq!(
        stats.lsq_operation_latency_ticks(O3RuntimeLsqOperation::StoreConditional),
        7
    );
    assert_eq!(stats.lsq_data_latency_samples(), 1);
    assert_eq!(stats.lsq_data_latency_ticks(), 7);
    assert_eq!(stats.lsq_data_latency_min_ticks(), 7);
    assert_eq!(stats.lsq_data_latency_max_ticks(), 7);
    assert_eq!(stats.lsq_data_latency_avg_ticks(), 7);
    assert_eq!(
        stats.lsq_operation_latency_min_ticks(O3RuntimeLsqOperation::StoreConditional),
        7
    );
    assert_eq!(
        stats.lsq_operation_latency_max_ticks(O3RuntimeLsqOperation::StoreConditional),
        7
    );
    assert_eq!(
        stats.lsq_operation_latency_avg_ticks(O3RuntimeLsqOperation::StoreConditional),
        7
    );
    assert_eq!(stats.lsq_store_conditional_failures(), 1);
}

#[test]
fn lsq_operation_latency_min_keeps_zero_tick_sample() {
    let mut runtime = O3RuntimeState::default();
    let first = store_conditional_event(0x8000, 10);
    let second = store_conditional_event(0x8004, 11);

    runtime.record_retired_instruction(&first);
    runtime.record_retired_instruction(&second);
    runtime.record_data_access_outcome(&first, 41, 0);
    runtime.record_data_access_outcome(&second, 46, 5);

    let stats = runtime.stats();

    assert_eq!(
        stats.lsq_operation_count(O3RuntimeLsqOperation::StoreConditional),
        2
    );
    assert_eq!(
        stats.lsq_operation_latency_ticks(O3RuntimeLsqOperation::StoreConditional),
        5
    );
    assert_eq!(stats.lsq_data_latency_samples(), 2);
    assert_eq!(stats.lsq_data_latency_ticks(), 5);
    assert_eq!(stats.lsq_data_latency_min_ticks(), 0);
    assert_eq!(stats.lsq_data_latency_max_ticks(), 5);
    assert_eq!(stats.lsq_data_latency_avg_ticks(), 2);
    assert_eq!(
        stats.lsq_operation_latency_min_ticks(O3RuntimeLsqOperation::StoreConditional),
        0
    );
    assert_eq!(
        stats.lsq_operation_latency_max_ticks(O3RuntimeLsqOperation::StoreConditional),
        5
    );
    assert_eq!(
        stats.lsq_operation_latency_avg_ticks(O3RuntimeLsqOperation::StoreConditional),
        2
    );
}

#[test]
fn failed_store_conditional_checkpoint_round_trips_failure_count() {
    let mut runtime = O3RuntimeState::default();
    let mut event = store_conditional_event(0x8000, 10);

    runtime.record_retired_instruction(&event);
    event.set_data_access_event_kind(RiscvDataAccessEventKind::ConditionalFailed);
    runtime.record_data_access_outcome(&event, 41, 7);

    let payload =
        O3RuntimeCheckpointPayload::from_snapshot_with_stats(runtime.snapshot(), runtime.stats())
            .unwrap();
    let encoded = payload.encode();
    let decoded = O3RuntimeCheckpointPayload::decode(&encoded).unwrap();

    assert_eq!(decoded.encode(), encoded);
    assert_eq!(decoded.stats().lsq_store_conditional_failures(), 1);
    assert_eq!(
        decoded
            .stats()
            .lsq_operation_count(O3RuntimeLsqOperation::StoreConditional),
        1
    );
    assert_eq!(
        decoded
            .stats()
            .lsq_operation_latency_ticks(O3RuntimeLsqOperation::StoreConditional),
        7
    );
    assert_eq!(decoded.stats().lsq_data_latency_samples(), 1);
    assert_eq!(decoded.stats().lsq_data_latency_ticks(), 7);
    assert_eq!(decoded.stats().lsq_data_latency_min_ticks(), 7);
    assert_eq!(decoded.stats().lsq_data_latency_max_ticks(), 7);
    assert_eq!(decoded.stats().lsq_data_latency_avg_ticks(), 7);
    assert_eq!(
        decoded
            .stats()
            .lsq_operation_latency_min_ticks(O3RuntimeLsqOperation::StoreConditional),
        7
    );
    assert_eq!(
        decoded
            .stats()
            .lsq_operation_latency_max_ticks(O3RuntimeLsqOperation::StoreConditional),
        7
    );
    assert_eq!(
        decoded
            .stats()
            .lsq_operation_latency_avg_ticks(O3RuntimeLsqOperation::StoreConditional),
        7
    );
}

#[test]
fn branch_repair_stats_checkpoint_round_trips_current_payload() {
    let mut targetless_kinds = [0; BranchTargetKind::COUNT];
    let mut wrong_target_kinds = [0; BranchTargetKind::COUNT];
    let mut direction_only_kinds = [0; BranchTargetKind::COUNT];
    let mut branch_event_kinds = [0; BranchTargetKind::COUNT];
    let mut branch_event_taken_kinds = [0; BranchTargetKind::COUNT];
    let mut branch_event_predicted_taken_kinds = [0; BranchTargetKind::COUNT];
    let mut branch_event_predicted_target_kinds = [0; BranchTargetKind::COUNT];
    let mut branch_event_predicted_target_match_kinds = [0; BranchTargetKind::COUNT];
    let mut branch_event_predicted_target_mismatch_kinds = [0; BranchTargetKind::COUNT];
    let mut branch_event_resolved_target_kinds = [0; BranchTargetKind::COUNT];
    let mut branch_event_link_write_kinds = [0; BranchTargetKind::COUNT];
    let mut branch_event_squash_kinds = [0; BranchTargetKind::COUNT];
    let mut branch_event_squashed_target_without_link_write_kinds = [0; BranchTargetKind::COUNT];
    targetless_kinds[BranchTargetKind::DirectConditional.index()] = 2;
    wrong_target_kinds[BranchTargetKind::CallIndirect.index()] = 3;
    direction_only_kinds[BranchTargetKind::DirectUnconditional.index()] = 4;
    branch_event_kinds[BranchTargetKind::Return.index()] = 1;
    branch_event_taken_kinds[BranchTargetKind::Return.index()] = 1;
    branch_event_predicted_taken_kinds[BranchTargetKind::Return.index()] = 1;
    branch_event_predicted_target_kinds[BranchTargetKind::Return.index()] = 1;
    branch_event_predicted_target_match_kinds[BranchTargetKind::Return.index()] = 1;
    branch_event_predicted_target_mismatch_kinds[BranchTargetKind::CallIndirect.index()] = 2;
    branch_event_resolved_target_kinds[BranchTargetKind::Return.index()] = 1;
    branch_event_link_write_kinds[BranchTargetKind::Return.index()] = 0;
    branch_event_squash_kinds[BranchTargetKind::Return.index()] = 1;
    branch_event_squashed_target_without_link_write_kinds[BranchTargetKind::Return.index()] = 1;
    let stats = O3RuntimeStats {
        branch_repair_targetless_mismatches: 2,
        branch_repair_wrong_targets: 3,
        branch_repair_direction_only_mismatches: 4,
        branch_repair_targetless_mismatch_kinds: targetless_kinds,
        branch_repair_wrong_target_kinds: wrong_target_kinds,
        branch_repair_direction_only_kinds: direction_only_kinds,
        branch_event_kinds,
        branch_event_taken_kinds,
        branch_event_predicted_taken_kinds,
        branch_event_predicted_target_kinds,
        branch_event_predicted_target_match_kinds,
        branch_event_predicted_target_mismatch_kinds,
        branch_event_resolved_target_kinds,
        branch_event_link_write_kinds,
        branch_event_squash_kinds,
        branch_event_squashed_target_without_link_write_kinds,
        iew_predicted_taken_incorrect: 5,
        iew_predicted_not_taken_incorrect: 6,
        iq_branch_insts_issued: 8,
        ..O3RuntimeStats::default()
    };
    let payload =
        O3RuntimeCheckpointPayload::from_snapshot_with_stats(default_o3_runtime_snapshot(), stats)
            .unwrap();
    let encoded = payload.encode();
    let decoded = O3RuntimeCheckpointPayload::decode(&encoded).unwrap();

    assert_eq!(decoded.encode(), encoded);
    assert_eq!(decoded.stats().branch_repair_targetless_mismatches(), 2);
    assert_eq!(decoded.stats().branch_repair_wrong_targets(), 3);
    assert_eq!(decoded.stats().branch_repair_direction_only_mismatches(), 4);
    assert_eq!(
        decoded
            .stats()
            .branch_repair_targetless_mismatch_kind(BranchTargetKind::DirectConditional),
        2
    );
    assert_eq!(
        decoded
            .stats()
            .branch_repair_wrong_target_kind(BranchTargetKind::CallIndirect),
        3
    );
    assert_eq!(
        decoded
            .stats()
            .branch_repair_direction_only_kind(BranchTargetKind::DirectUnconditional),
        4
    );
    assert_eq!(
        decoded.stats().branch_event_kind(BranchTargetKind::Return),
        1
    );
    assert_eq!(
        decoded
            .stats()
            .branch_event_taken_kind(BranchTargetKind::Return),
        1
    );
    assert_eq!(
        decoded
            .stats()
            .branch_event_predicted_taken_kind(BranchTargetKind::Return),
        1
    );
    assert_eq!(
        decoded
            .stats()
            .branch_event_predicted_not_taken_kind(BranchTargetKind::Return),
        0
    );
    assert_eq!(
        decoded
            .stats()
            .branch_event_predicted_target_kind(BranchTargetKind::Return),
        1
    );
    assert_eq!(
        decoded
            .stats()
            .branch_event_predicted_target_match_kind(BranchTargetKind::Return),
        1
    );
    assert_eq!(
        decoded
            .stats()
            .branch_event_predicted_target_mismatch_kind(BranchTargetKind::CallIndirect),
        2
    );
    assert_eq!(
        decoded
            .stats()
            .branch_event_resolved_target_kind(BranchTargetKind::Return),
        1
    );
    assert_eq!(
        decoded
            .stats()
            .branch_event_link_write_kind(BranchTargetKind::Return),
        0
    );
    assert_eq!(
        decoded
            .stats()
            .branch_event_squash_kind(BranchTargetKind::Return),
        1
    );
    assert_eq!(
        decoded
            .stats()
            .branch_event_squashed_target_without_link_write_kind(BranchTargetKind::Return),
        1
    );
    assert_eq!(decoded.stats().iew_predicted_taken_incorrect(), 5);
    assert_eq!(decoded.stats().iew_predicted_not_taken_incorrect(), 6);
    assert_eq!(decoded.stats().iq_branch_insts_issued(), 8);
}

fn store_conditional_event(pc: u64, sequence: u64) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::StoreConditional {
        rd: Register::new(7).unwrap(),
        rs1: Register::new(5).unwrap(),
        rs2: Register::new(6).unwrap(),
        width: MemoryWidth::Doubleword,
        acquire: false,
        release: false,
    };
    let access = MemoryAccessKind::StoreConditional {
        rd: Register::new(7).unwrap(),
        address: 0x9000,
        width: MemoryWidth::Doubleword,
        value: 0x2a,
        acquire: false,
        release: false,
    };
    RiscvCpuExecutionEvent::new(
        fetch_event(pc, sequence),
        instruction,
        RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
    )
}

fn fetch_event(pc: u64, sequence: u64) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            MemoryRequestId::new(AgentId::new(7), sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        0x0000_0073u32.to_le_bytes().to_vec(),
    )
}

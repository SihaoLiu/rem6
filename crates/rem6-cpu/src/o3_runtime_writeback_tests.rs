use super::*;
use rem6_isa_riscv::{
    Immediate, MemoryAccessKind, MemoryWidth, Register, RegisterWrite, RiscvExecutionRecord,
    RiscvInstruction,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

use crate::o3_pipeline::{
    O3DependencyScopeId, O3IssueOpClass, O3IssueQueueId, O3PendingStateSnapshot, O3PipelineStage,
    O3ScopedReadyInstruction, O3WritebackCompletion, O3WritebackTransferPolicy,
    O3WritebackTransferSnapshot,
};
use crate::riscv_defaults::{
    DEFAULT_RISCV_O3_WRITEBACK_WIDTH, MAX_RISCV_O3_WRITEBACK_WIDTH, MIN_RISCV_O3_WRITEBACK_WIDTH,
};
use crate::{
    CpuCore, CpuFetchConfig, CpuFetchEvent, CpuFetchRecord, CpuId, CpuResetState, RiscvCore,
    RiscvCpuError, RiscvCpuExecutionEvent, RiscvDataAccessEventKind,
};

#[test]
fn o3_writeback_width_defaults_to_one_and_rejects_out_of_range_updates() {
    let mut runtime = O3RuntimeState::default();
    assert_eq!(runtime.writeback_width(), DEFAULT_RISCV_O3_WRITEBACK_WIDTH);

    let resolved = [O3DependencyScopeId::new(11)];
    let ready = [
        O3ScopedReadyInstruction::new(17, O3IssueQueueId::new(2), O3IssueOpClass::IntAlu)
            .with_waits_on(resolved)
            .with_produces([O3DependencyScopeId::new(19)]),
    ];
    let deferred = [O3WritebackCompletion::new(23)];
    runtime.snapshot.pending_state = O3PendingStateSnapshot::new(
        resolved,
        ready.clone(),
        O3WritebackTransferSnapshot::new(
            O3WritebackTransferPolicy::new(
                O3PipelineStage::Iew,
                DEFAULT_RISCV_O3_WRITEBACK_WIDTH,
                0,
            )
            .unwrap(),
            deferred,
        ),
    )
    .unwrap();
    let original_pending = runtime.snapshot.pending_state().clone();

    assert!(runtime.set_writeback_width(MAX_RISCV_O3_WRITEBACK_WIDTH));
    assert_eq!(runtime.writeback_width(), MAX_RISCV_O3_WRITEBACK_WIDTH);
    assert_eq!(
        runtime
            .snapshot
            .pending_state()
            .resolved_dependency_scopes(),
        original_pending.resolved_dependency_scopes()
    );
    assert_eq!(
        runtime.snapshot.pending_state().ready(),
        original_pending.ready()
    );
    assert_eq!(
        runtime.snapshot.pending_state().writeback().deferred(),
        original_pending.writeback().deferred()
    );
    assert_eq!(
        runtime
            .snapshot
            .pending_state()
            .writeback()
            .policy()
            .source(),
        O3PipelineStage::Iew
    );
    assert_eq!(
        runtime
            .snapshot
            .pending_state()
            .writeback()
            .policy()
            .future_cycles(),
        0
    );

    for width in [
        MIN_RISCV_O3_WRITEBACK_WIDTH - 1,
        MAX_RISCV_O3_WRITEBACK_WIDTH + 1,
    ] {
        assert!(!runtime.set_writeback_width(width), "{width}");
        assert_eq!(runtime.writeback_width(), MAX_RISCV_O3_WRITEBACK_WIDTH);
    }

    assert!(runtime.set_writeback_width(MIN_RISCV_O3_WRITEBACK_WIDTH));
    assert_eq!(runtime.writeback_width(), MIN_RISCV_O3_WRITEBACK_WIDTH);
}

#[test]
fn writeback_width_one_reserves_oldest_same_cycle_row_first() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));

    let reservations = runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(4, 20),
            O3LiveWritebackReady::fixed_fu(5, 20),
        ])
        .unwrap();

    assert_eq!(
        reservation_rows(&reservations),
        vec![(4, 20, 20, 0, true), (5, 20, 21, 0, true)]
    );
}

#[test]
fn writeback_width_two_admits_exact_fit_same_cycle_rows() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(2));

    let reservations = runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(4, 20),
            O3LiveWritebackReady::fixed_fu(5, 20),
        ])
        .unwrap();

    assert_eq!(
        reservation_rows(&reservations),
        vec![(4, 20, 20, 0, true), (5, 20, 20, 1, true)]
    );
}

#[test]
fn writeback_planner_does_not_introduce_future_raw_ready_rows_early() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));

    let reservations = runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(1, 10),
            O3LiveWritebackReady::fixed_fu(2, 30),
        ])
        .unwrap();

    assert_eq!(
        reservation_rows(&reservations),
        vec![(1, 10, 10, 0, true), (2, 30, 30, 0, true)]
    );
    let stats = runtime.stats();
    assert_eq!(stats.writeback_port_cycles(), 2);
    assert_eq!(stats.writeback_port_admitted_rows(), 2);
    assert_eq!(stats.writeback_port_deferred_rows(), 0);
    assert_eq!(stats.writeback_port_deferred_row_cycles(), 0);
    assert_eq!(stats.writeback_port_max_ready_rows_per_cycle(), 1);
    assert_eq!(stats.writeback_port_max_deferred_rows(), 0);
}

#[test]
fn writeback_reentry_returns_identical_reservation_without_recounting() {
    let mut runtime = O3RuntimeState::default();

    let first = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(9, 12)])
        .unwrap();
    let stats = runtime.stats();
    let second = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(9, 12)])
        .unwrap();

    assert_eq!(first, second);
    assert_eq!(runtime.stats(), stats);
    assert_eq!(
        reservation_rows(&runtime.writeback_reservations()),
        vec![(9, 12, 12, 0, true)]
    );
}

#[test]
fn partial_reentry_cannot_overbook_or_recount_writeback() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    let first = runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(4, 20),
            O3LiveWritebackReady::fixed_fu(5, 20),
        ])
        .unwrap();
    let stats = runtime.stats();

    let second = runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(5, 20),
            O3LiveWritebackReady::fixed_fu(4, 20),
        ])
        .unwrap();

    assert_eq!(reservation_rows(&second), reservation_rows(&first));
    assert_eq!(runtime.stats(), stats);
    assert_eq!(runtime.writeback_calendar.occupied_slots(20), vec![0]);
    assert_eq!(runtime.writeback_calendar.occupied_slots(21), vec![0]);
}

#[test]
fn reentry_rejects_changed_raw_ready_tick() {
    let mut runtime = O3RuntimeState::default();
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(4, 20)])
        .unwrap();

    let error = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(4, 21)])
        .unwrap_err();

    assert_eq!(
        error,
        O3RuntimeError::WritebackReservationMismatch {
            sequence: 4,
            existing_raw_ready_tick: 20,
            requested_raw_ready_tick: 21,
        }
    );
}

#[test]
fn writeback_width_change_is_rejected_while_reservations_are_live() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(4, 20)])
        .unwrap();

    assert!(!runtime.set_writeback_width(2));
    assert_eq!(runtime.writeback_width(), 1);
}

#[test]
fn rollback_discards_future_writeback_reservation() {
    let mut runtime = runtime_with_reserved_sequences([(4, 20), (5, 21), (6, 22)]);

    runtime.discard_live_staged_window_from_at(5, 19);

    assert!(runtime.writeback_reservation(4).is_some());
    assert!(runtime.writeback_reservation(5).is_none());
    assert!(runtime.writeback_reservation(6).is_none());
}

#[test]
fn stats_reset_preserves_writeback_calendar_without_recounting_reservations() {
    let mut runtime = runtime_with_reserved_sequences([(4, 20), (5, 21)]);
    let reservations = runtime.writeback_reservations();

    runtime.reset_stats();

    assert_eq!(runtime.writeback_reservations(), reservations);
    assert_eq!(runtime.stats().writeback_port_admitted_rows(), 0);
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(4, 20)])
        .unwrap();
    assert_eq!(runtime.stats().writeback_port_admitted_rows(), 0);
}

#[test]
fn writeback_calendar_prunes_only_before_current_tick() {
    let mut runtime = runtime_with_reserved_sequences([(4, 20), (5, 21)]);

    runtime.prune_writeback_calendar_before(21);

    assert!(runtime.writeback_reservation(4).is_none());
    assert!(runtime.writeback_reservation(5).is_some());
    runtime.prune_writeback_calendar_before(22);
    assert!(runtime.writeback_reservation(5).is_none());
}

#[test]
fn discarded_future_slot_can_be_reused_without_replaying_history() {
    let mut runtime = runtime_with_reserved_sequences([(4, 20), (5, 21)]);

    runtime.discard_future_writeback_sequence(5, 20);
    let replacement = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(6, 21)])
        .unwrap()[0];

    assert_eq!(replacement.admitted_tick(), 21);
    assert_eq!(
        runtime.writeback_reservation(4).unwrap().admitted_tick(),
        20
    );
}

#[test]
fn retry_cleanup_discards_younger_future_writeback_reservations() {
    assert_scalar_memory_suffix_cleanup(RiscvDataAccessEventKind::Retry);
}

#[test]
fn failure_cleanup_discards_younger_future_writeback_reservations() {
    assert_scalar_memory_suffix_cleanup(RiscvDataAccessEventKind::Failed);
}

#[test]
fn full_lifecycle_cleanup_discards_all_writeback_authority() {
    let (mut runtime, sequence, admitted_tick) = runtime_with_live_speculative_writeback();
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(99, admitted_tick + 4)])
        .unwrap();

    runtime.discard_live_staged_instructions();

    assert!(runtime.live_speculative_executions.is_empty());
    assert!(runtime.writeback_reservation(sequence).is_none());
    assert!(runtime.writeback_reservation(99).is_none());
    assert!(runtime.writeback_reservations().is_empty());
}

#[test]
fn pc_redirect_discards_all_writeback_authority_but_keeps_callback_error_sticky() {
    let (mut runtime, sequence, admitted_tick) = runtime_with_live_speculative_writeback();
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(99, admitted_tick + 4)])
        .unwrap();
    let core = core_with_runtime(runtime);
    let callback_error =
        RiscvCpuError::O3Runtime(O3RuntimeError::WritebackTickOverflow { tick: 7 });
    core.state
        .lock()
        .expect("riscv core lock")
        .pending_callback_error = Some(callback_error.clone());

    core.redirect_pc(Address::new(0x9000));

    core.with_o3_runtime(|runtime| {
        assert!(runtime.live_speculative_executions.is_empty());
        assert!(runtime.writeback_reservation(sequence).is_none());
        assert!(runtime.writeback_reservation(99).is_none());
    });
    assert_eq!(core.pending_callback_error(), Some(callback_error));
}

#[test]
fn explicit_hart_reset_discards_writeback_authority_and_callback_error() {
    let (mut runtime, sequence, admitted_tick) = runtime_with_live_speculative_writeback();
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(99, admitted_tick + 4)])
        .unwrap();
    let core = core_with_runtime(runtime);
    core.state
        .lock()
        .expect("riscv core lock")
        .pending_callback_error = Some(RiscvCpuError::O3Runtime(
        O3RuntimeError::WritebackTickOverflow { tick: 7 },
    ));

    core.resume_nonretentive_supervisor_hart(Address::new(0x9000), 0x1234);

    core.with_o3_runtime(|runtime| {
        assert!(runtime.live_speculative_executions.is_empty());
        assert!(runtime.writeback_reservation(sequence).is_none());
        assert!(runtime.writeback_reservation(99).is_none());
    });
    assert_eq!(core.pending_callback_error(), None);
}

#[test]
fn checkpoint_finalization_keeps_live_writeback_owner_nonquiescent() {
    let (runtime, sequence, admitted_tick) = runtime_with_live_speculative_writeback();
    let core = core_with_runtime(runtime);

    core.finalize_quiescent_o3_writeback_for_checkpoint();

    core.with_o3_runtime(|runtime| {
        assert_eq!(
            runtime
                .writeback_reservation(sequence)
                .map(O3WritebackReservation::admitted_tick),
            Some(admitted_tick)
        );
        assert!(runtime.has_pending_retirement_authority());
    });
    assert!(!core.data_access_lifecycle_is_quiescent());
}

fn assert_scalar_memory_suffix_cleanup(kind: RiscvDataAccessEventKind) {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let oldest = scalar_load_event(0x8000, 10, 12, 0x9000);
    let boundary = scalar_load_event(0x8004, 11, 13, 0x9040);
    let discarded = scalar_load_event(0x8008, 12, 14, 0x9080);
    let oldest_request = memory_request(20);
    let boundary_request = memory_request(21);
    let discarded_request = memory_request(22);
    assert!(runtime.stage_live_data_access_issue(&oldest, oldest_request, 31));
    assert!(runtime.stage_live_data_access_issue(&boundary, boundary_request, 32));
    assert!(runtime.stage_live_data_access_issue(&discarded, discarded_request, 33));
    let oldest_sequence = runtime.live_data_accesses[0].sequence;
    let discarded_sequence = runtime.live_data_accesses[2].sequence;

    let mut oldest_completed = oldest.clone();
    oldest_completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &oldest_completed,
            oldest_request,
            39,
            8,
            Some(&[0x2a, 0, 0, 0]),
        )
        .unwrap());
    let mut discarded_completed = discarded.clone();
    discarded_completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &discarded_completed,
            discarded_request,
            40,
            7,
            Some(&[0x63, 0, 0, 0]),
        )
        .unwrap());
    assert_eq!(
        runtime.live_data_accesses[0].admitted_writeback_tick,
        Some(40)
    );
    assert_eq!(
        runtime.live_data_accesses[2].admitted_writeback_tick,
        Some(41)
    );

    let mut boundary_outcome = boundary.clone();
    boundary_outcome.set_data_access_event_kind(kind);
    assert!(runtime
        .complete_live_data_access_response(&boundary_outcome, boundary_request, 40, 8, None,)
        .unwrap());

    assert_eq!(runtime.live_data_accesses.len(), 2);
    assert_eq!(
        runtime.live_data_accesses[0].admitted_writeback_tick,
        Some(40)
    );
    assert!(runtime
        .live_data_accesses
        .iter()
        .all(|live| live.sequence != discarded_sequence));
    assert_eq!(
        runtime
            .writeback_reservation(oldest_sequence)
            .map(O3WritebackReservation::admitted_tick),
        Some(40)
    );
    assert!(runtime.writeback_reservation(discarded_sequence).is_none());
}

fn runtime_with_live_speculative_writeback() -> (O3RuntimeState, u64, u64) {
    let mut runtime = O3RuntimeState::default();
    let head = addi(1, 0, 1);
    let younger = addi(3, 0, 42);
    runtime.stage_live_retire_window(
        Address::new(0x8000),
        head,
        20,
        [(Address::new(0x8004), younger)],
    );
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), younger)
        .unwrap();
    let sequence = candidate.sequence();
    runtime
        .record_live_speculative_execution(
            candidate,
            &[memory_request(11)],
            20,
            RiscvExecutionRecord::new(
                younger,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(reg(3), 42)],
                None,
            ),
        )
        .unwrap();
    let admitted_tick = runtime.live_speculative_executions[0].admitted_writeback_tick;
    assert_eq!(
        runtime
            .writeback_reservation(sequence)
            .map(O3WritebackReservation::admitted_tick),
        Some(admitted_tick)
    );
    (runtime, sequence, admitted_tick)
}

fn scalar_load_event(
    pc: u64,
    fetch_sequence: u64,
    destination: u8,
    address: u64,
) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Load {
        rd: reg(destination),
        rs1: reg(10),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    RiscvCpuExecutionEvent::new(
        fetch_event(pc, fetch_sequence),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            pc,
            pc + 4,
            Vec::new(),
            Some(MemoryAccessKind::Load {
                rd: reg(destination),
                address,
                width: MemoryWidth::Word,
                signed: false,
            }),
        ),
    )
}

fn addi(rd: u8, rs1: u8, immediate: i64) -> RiscvInstruction {
    RiscvInstruction::Addi {
        rd: reg(rd),
        rs1: reg(rs1),
        imm: Immediate::new(immediate),
    }
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
        0x0000_0013_u32.to_le_bytes().to_vec(),
    )
}

fn memory_request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn core_with_runtime(runtime: O3RuntimeState) -> RiscvCore {
    let core = RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                MemoryRouteId::new(0),
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    );
    core.state.lock().expect("riscv core lock").o3_runtime = runtime;
    core
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn runtime_with_reserved_sequences<const N: usize>(rows: [(u64, u64); N]) -> O3RuntimeState {
    let mut runtime = O3RuntimeState::default();
    runtime
        .reserve_writeback_completions(
            rows.map(|(sequence, tick)| O3LiveWritebackReady::fixed_fu(sequence, tick)),
        )
        .unwrap();
    runtime
}

fn reservation_rows(reservations: &[O3WritebackReservation]) -> Vec<(u64, u64, u64, usize, bool)> {
    reservations
        .iter()
        .map(|reservation| {
            (
                reservation.sequence(),
                reservation.raw_ready_tick(),
                reservation.admitted_tick(),
                reservation.slot(),
                reservation.decision_counted(),
            )
        })
        .collect()
}

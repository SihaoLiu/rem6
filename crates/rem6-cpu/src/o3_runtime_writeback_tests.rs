use super::*;
use crate::o3_pipeline::{
    O3DependencyScopeId, O3IssueOpClass, O3IssueQueueId, O3PendingStateSnapshot, O3PipelineStage,
    O3ScopedReadyInstruction, O3WritebackCompletion, O3WritebackTransferPolicy,
    O3WritebackTransferSnapshot,
};
use crate::riscv_defaults::{
    DEFAULT_RISCV_O3_WRITEBACK_WIDTH, MAX_RISCV_O3_WRITEBACK_WIDTH, MIN_RISCV_O3_WRITEBACK_WIDTH,
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

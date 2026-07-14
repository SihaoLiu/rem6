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

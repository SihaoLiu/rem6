use rem6_cpu::{
    InOrderBranchRedirect, InOrderPipelineConfig, InOrderPipelineError, InOrderPipelineInstruction,
    InOrderPipelineRunSummary, InOrderPipelineScheduler, InOrderPipelineSnapshot,
    InOrderPipelineStage, InOrderPipelineStageWidth, InOrderPipelineState,
};

fn instruction(sequence: u64, stage: InOrderPipelineStage) -> InOrderPipelineInstruction {
    InOrderPipelineInstruction::new(sequence, stage)
}

fn stage_width(
    stage: InOrderPipelineStage,
    slots: usize,
) -> Result<InOrderPipelineStageWidth, InOrderPipelineError> {
    InOrderPipelineStageWidth::new(stage, slots)
}

fn scheduler_with_decode_width(decode_width: usize) -> InOrderPipelineScheduler {
    InOrderPipelineScheduler::new(config_with_decode_width(decode_width))
}

fn config_with_decode_width(decode_width: usize) -> InOrderPipelineConfig {
    InOrderPipelineConfig::new([
        stage_width(InOrderPipelineStage::Fetch1, 1).unwrap(),
        stage_width(InOrderPipelineStage::Fetch2, 1).unwrap(),
        stage_width(InOrderPipelineStage::Decode, decode_width).unwrap(),
        stage_width(InOrderPipelineStage::Execute, 1).unwrap(),
        stage_width(InOrderPipelineStage::Commit, 1).unwrap(),
    ])
    .unwrap()
}

#[test]
fn in_order_pipeline_blocks_younger_ready_work_after_oldest_resource_block() {
    let scheduler = scheduler_with_decode_width(1);

    let plan = scheduler.plan([
        instruction(10, InOrderPipelineStage::Decode),
        instruction(11, InOrderPipelineStage::Decode),
        instruction(12, InOrderPipelineStage::Execute),
    ]);

    assert_eq!(plan.advanced_sequences().collect::<Vec<_>>(), vec![10]);
    assert_eq!(
        plan.advanced()[0].destination_stage(),
        Some(InOrderPipelineStage::Execute)
    );
    assert_eq!(plan.resource_blocked()[0].sequence(), 11);
    assert_eq!(plan.ordering_blocked()[0].sequence(), 12);
    assert!(plan.has_blocked_work());
}

#[test]
fn in_order_pipeline_commit_width_marks_retirements_in_program_order() {
    let config = InOrderPipelineConfig::new([
        stage_width(InOrderPipelineStage::Fetch1, 1).unwrap(),
        stage_width(InOrderPipelineStage::Fetch2, 1).unwrap(),
        stage_width(InOrderPipelineStage::Decode, 1).unwrap(),
        stage_width(InOrderPipelineStage::Execute, 1).unwrap(),
        stage_width(InOrderPipelineStage::Commit, 2).unwrap(),
    ])
    .unwrap();
    let scheduler = InOrderPipelineScheduler::new(config);

    let plan = scheduler.plan([
        instruction(20, InOrderPipelineStage::Commit),
        instruction(21, InOrderPipelineStage::Commit),
        instruction(22, InOrderPipelineStage::Commit),
    ]);

    assert_eq!(plan.advanced_sequences().collect::<Vec<_>>(), vec![20, 21]);
    assert!(plan.advanced().iter().all(|advance| advance.retires()));
    assert!(plan
        .advanced()
        .iter()
        .all(|advance| advance.destination_stage().is_none()));
    assert_eq!(plan.resource_blocked()[0].sequence(), 22);
    assert!(plan.ordering_blocked().is_empty());
}

#[test]
fn in_order_pipeline_branch_redirect_flushes_younger_pipeline_work() {
    let scheduler = scheduler_with_decode_width(1);
    let redirect = InOrderBranchRedirect::new(30, InOrderPipelineStage::Execute, 0x2000);

    let plan = scheduler
        .plan_with_redirect(
            [
                instruction(29, InOrderPipelineStage::Commit),
                instruction(30, InOrderPipelineStage::Execute),
                instruction(31, InOrderPipelineStage::Decode),
                instruction(32, InOrderPipelineStage::Fetch2),
            ],
            Some(redirect),
        )
        .unwrap();

    assert_eq!(plan.redirect(), Some(&redirect));
    assert_eq!(plan.advanced_sequences().collect::<Vec<_>>(), vec![29, 30]);
    assert_eq!(plan.flushed_sequences().collect::<Vec<_>>(), vec![31, 32]);
    assert!(plan
        .flushed()
        .iter()
        .all(|instruction| instruction.sequence() > redirect.sequence()));
    assert!(!plan.has_blocked_work());
}

#[test]
fn in_order_pipeline_rejects_redirect_for_absent_instruction() {
    let scheduler = scheduler_with_decode_width(1);
    let redirect = InOrderBranchRedirect::new(30, InOrderPipelineStage::Execute, 0x2000);

    assert_eq!(
        scheduler
            .plan_with_redirect(
                [
                    instruction(29, InOrderPipelineStage::Commit),
                    instruction(31, InOrderPipelineStage::Decode),
                ],
                Some(redirect),
            )
            .unwrap_err(),
        InOrderPipelineError::MissingBranchRedirectInstruction { sequence: 30 }
    );
}

#[test]
fn in_order_pipeline_rejects_redirect_stage_mismatch() {
    let scheduler = scheduler_with_decode_width(1);
    let redirect = InOrderBranchRedirect::new(30, InOrderPipelineStage::Execute, 0x2000);

    assert_eq!(
        scheduler
            .plan_with_redirect(
                [
                    instruction(29, InOrderPipelineStage::Commit),
                    instruction(30, InOrderPipelineStage::Decode),
                ],
                Some(redirect),
            )
            .unwrap_err(),
        InOrderPipelineError::BranchRedirectStageMismatch {
            sequence: 30,
            expected: InOrderPipelineStage::Execute,
            actual: InOrderPipelineStage::Decode,
        }
    );
}

#[test]
fn in_order_pipeline_snapshot_restore_preserves_in_flight_plan() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state.advance_cycle();
    state
        .replace_in_flight([
            instruction(10, InOrderPipelineStage::Decode),
            instruction(11, InOrderPipelineStage::Decode),
            instruction(12, InOrderPipelineStage::Execute),
        ])
        .unwrap();

    let snapshot = state.snapshot();
    assert_eq!(snapshot.cycle(), 1);
    assert_eq!(
        snapshot
            .in_flight()
            .iter()
            .map(|instruction| instruction.sequence())
            .collect::<Vec<_>>(),
        vec![10, 11, 12]
    );

    let restored = InOrderPipelineState::restore(snapshot).unwrap();
    assert_eq!(restored.cycle(), 1);
    assert_eq!(
        restored
            .in_flight()
            .iter()
            .map(|instruction| instruction.sequence())
            .collect::<Vec<_>>(),
        vec![10, 11, 12]
    );

    let plan = restored.plan_cycle();
    assert_eq!(plan.advanced_sequences().collect::<Vec<_>>(), vec![10]);
    assert_eq!(plan.resource_blocked()[0].sequence(), 11);
    assert_eq!(plan.ordering_blocked()[0].sequence(), 12);
}

#[test]
fn in_order_pipeline_advance_cycle_increments_cycle_cursor() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    assert_eq!(state.cycle(), 0);
    state
        .replace_in_flight([instruction(1, InOrderPipelineStage::Decode)])
        .unwrap();

    state.advance_cycle();
    assert_eq!(state.cycle(), 1);

    state.advance_cycle();
    assert_eq!(state.cycle(), 2);
}

#[test]
fn in_order_pipeline_try_advance_rejects_cycle_cursor_overflow() {
    let snapshot = InOrderPipelineSnapshot::with_cycle(config_with_decode_width(1), u64::MAX, []);
    let mut state = InOrderPipelineState::restore(snapshot).unwrap();

    assert_eq!(
        state.try_advance_cycle().unwrap_err(),
        InOrderPipelineError::CycleCursorOverflow { cycle: u64::MAX }
    );
    assert_eq!(state.cycle(), u64::MAX);
}

#[test]
fn in_order_pipeline_advance_cycle_updates_in_flight_state() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state
        .replace_in_flight([
            instruction(9, InOrderPipelineStage::Commit),
            instruction(10, InOrderPipelineStage::Decode),
            instruction(11, InOrderPipelineStage::Decode),
            instruction(12, InOrderPipelineStage::Execute),
        ])
        .unwrap();

    let plan = state.advance_cycle();

    assert_eq!(plan.advanced_sequences().collect::<Vec<_>>(), vec![9, 10]);
    assert!(plan.advanced()[0].retires());
    assert_eq!(
        plan.advanced()[1].destination_stage(),
        Some(InOrderPipelineStage::Execute)
    );
    assert_eq!(plan.resource_blocked()[0].sequence(), 11);
    assert_eq!(plan.ordering_blocked()[0].sequence(), 12);
    assert_eq!(
        state
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![
            (10, InOrderPipelineStage::Execute),
            (11, InOrderPipelineStage::Decode),
            (12, InOrderPipelineStage::Execute),
        ]
    );
}

#[test]
fn in_order_pipeline_advance_cycle_with_redirect_removes_flushed_work() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state
        .replace_in_flight([
            instruction(29, InOrderPipelineStage::Commit),
            instruction(30, InOrderPipelineStage::Execute),
            instruction(31, InOrderPipelineStage::Decode),
            instruction(32, InOrderPipelineStage::Fetch2),
        ])
        .unwrap();
    let redirect = InOrderBranchRedirect::new(30, InOrderPipelineStage::Execute, 0x2000);

    let plan = state.advance_cycle_with_redirect(Some(redirect)).unwrap();

    assert_eq!(plan.advanced_sequences().collect::<Vec<_>>(), vec![29, 30]);
    assert_eq!(plan.flushed_sequences().collect::<Vec<_>>(), vec![31, 32]);
    assert_eq!(
        state
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(30, InOrderPipelineStage::Commit)]
    );
}

#[test]
fn in_order_pipeline_restore_rejects_duplicate_in_flight_sequences() {
    let snapshot = InOrderPipelineSnapshot::new(
        config_with_decode_width(1),
        [
            instruction(7, InOrderPipelineStage::Decode),
            instruction(7, InOrderPipelineStage::Execute),
        ],
    );

    assert_eq!(
        InOrderPipelineState::restore(snapshot).unwrap_err(),
        InOrderPipelineError::DuplicateInFlightInstruction { sequence: 7 }
    );
}

#[test]
fn in_order_pipeline_recorded_cycle_preserves_before_plan_and_after_snapshots() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state
        .replace_in_flight([
            instruction(9, InOrderPipelineStage::Commit),
            instruction(10, InOrderPipelineStage::Decode),
        ])
        .unwrap();

    let record = state.advance_cycle_recorded();

    assert_eq!(record.cycle(), 0);
    assert_eq!(record.before().cycle(), 0);
    assert_eq!(
        record
            .before()
            .in_flight()
            .iter()
            .map(|instruction| instruction.sequence())
            .collect::<Vec<_>>(),
        vec![9, 10]
    );
    assert_eq!(
        record.plan().advanced_sequences().collect::<Vec<_>>(),
        vec![9, 10]
    );
    assert_eq!(record.after().cycle(), 1);
    assert_eq!(
        record
            .after()
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(10, InOrderPipelineStage::Execute)]
    );
    assert_eq!(state.snapshot(), *record.after());
}

#[test]
fn in_order_pipeline_cycle_summary_counts_recorded_work() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state
        .replace_in_flight([
            instruction(9, InOrderPipelineStage::Commit),
            instruction(10, InOrderPipelineStage::Decode),
            instruction(11, InOrderPipelineStage::Decode),
            instruction(12, InOrderPipelineStage::Execute),
        ])
        .unwrap();

    let summary = state.advance_cycle_recorded().summary();

    assert_eq!(summary.cycle(), 0);
    assert_eq!(summary.advanced_count(), 2);
    assert_eq!(summary.retired_count(), 1);
    assert_eq!(summary.flushed_count(), 0);
    assert_eq!(summary.resource_blocked_count(), 1);
    assert_eq!(summary.ordering_blocked_count(), 1);
    assert!(summary.state_changed());
    assert_eq!(summary.redirect_target_pc(), None);
}

#[test]
fn in_order_pipeline_cycle_summary_records_redirect_target() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state
        .replace_in_flight([
            instruction(29, InOrderPipelineStage::Commit),
            instruction(30, InOrderPipelineStage::Execute),
            instruction(31, InOrderPipelineStage::Decode),
            instruction(32, InOrderPipelineStage::Fetch2),
        ])
        .unwrap();
    let redirect = InOrderBranchRedirect::new(30, InOrderPipelineStage::Execute, 0x2000);

    let summary = state
        .try_advance_cycle_recorded_with_redirect(Some(redirect))
        .unwrap()
        .summary();

    assert_eq!(summary.cycle(), 0);
    assert_eq!(summary.advanced_count(), 2);
    assert_eq!(summary.retired_count(), 1);
    assert_eq!(summary.flushed_count(), 2);
    assert_eq!(summary.resource_blocked_count(), 0);
    assert_eq!(summary.ordering_blocked_count(), 0);
    assert!(summary.state_changed());
    assert_eq!(summary.redirect_target_pc(), Some(0x2000));
}

#[test]
fn in_order_pipeline_run_summary_aggregates_cycle_records() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state
        .replace_in_flight([
            instruction(9, InOrderPipelineStage::Commit),
            instruction(10, InOrderPipelineStage::Decode),
            instruction(11, InOrderPipelineStage::Decode),
        ])
        .unwrap();

    let first = state.advance_cycle_recorded();
    let second = state.advance_cycle_recorded();

    let summary = InOrderPipelineRunSummary::from_cycle_records([first, second]);

    assert_eq!(summary.cycle_count(), 2);
    assert_eq!(summary.first_cycle(), Some(0));
    assert_eq!(summary.last_cycle(), Some(1));
    assert_eq!(summary.advanced_count(), 4);
    assert_eq!(summary.retired_count(), 1);
    assert_eq!(summary.flushed_count(), 0);
    assert_eq!(summary.resource_blocked_count(), 1);
    assert_eq!(summary.ordering_blocked_count(), 0);
    assert_eq!(summary.redirect_count(), 0);
    assert_eq!(summary.state_changed_cycle_count(), 2);
}

#[test]
fn in_order_pipeline_empty_run_summary_has_no_cycle_window() {
    let summary = InOrderPipelineRunSummary::from_cycle_summaries([]);

    assert!(summary.is_empty());
    assert_eq!(summary.cycle_count(), 0);
    assert_eq!(summary.first_cycle(), None);
    assert_eq!(summary.last_cycle(), None);
    assert_eq!(summary.advanced_count(), 0);
    assert_eq!(summary.state_changed_cycle_count(), 0);
}

#[test]
fn in_order_pipeline_run_summary_merges_partial_summaries() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state
        .replace_in_flight([
            instruction(9, InOrderPipelineStage::Commit),
            instruction(10, InOrderPipelineStage::Decode),
            instruction(11, InOrderPipelineStage::Decode),
        ])
        .unwrap();

    let first = InOrderPipelineRunSummary::from_cycle_records([state.advance_cycle_recorded()]);
    let second = InOrderPipelineRunSummary::from_cycle_records([state.advance_cycle_recorded()]);
    let merged = InOrderPipelineRunSummary::from_cycle_summaries([])
        .merge(first)
        .merge(second);

    assert!(!merged.is_empty());
    assert_eq!(merged.cycle_count(), 2);
    assert_eq!(merged.first_cycle(), Some(0));
    assert_eq!(merged.last_cycle(), Some(1));
    assert_eq!(merged.advanced_count(), 4);
    assert_eq!(merged.retired_count(), 1);
    assert_eq!(merged.resource_blocked_count(), 1);
    assert_eq!(merged.state_changed_cycle_count(), 2);
}

#[test]
fn in_order_pipeline_config_rejects_zero_missing_and_duplicate_widths() {
    assert_eq!(
        InOrderPipelineStageWidth::new(InOrderPipelineStage::Execute, 0).unwrap_err(),
        InOrderPipelineError::ZeroStageWidth {
            stage: InOrderPipelineStage::Execute,
        }
    );

    assert_eq!(
        InOrderPipelineConfig::new([
            stage_width(InOrderPipelineStage::Fetch1, 1).unwrap(),
            stage_width(InOrderPipelineStage::Fetch2, 1).unwrap(),
            stage_width(InOrderPipelineStage::Decode, 1).unwrap(),
            stage_width(InOrderPipelineStage::Execute, 1).unwrap(),
        ])
        .unwrap_err(),
        InOrderPipelineError::MissingStageWidth {
            stage: InOrderPipelineStage::Commit,
        }
    );

    assert_eq!(
        InOrderPipelineConfig::new([
            stage_width(InOrderPipelineStage::Fetch1, 1).unwrap(),
            stage_width(InOrderPipelineStage::Fetch1, 1).unwrap(),
            stage_width(InOrderPipelineStage::Fetch2, 1).unwrap(),
            stage_width(InOrderPipelineStage::Decode, 1).unwrap(),
            stage_width(InOrderPipelineStage::Execute, 1).unwrap(),
            stage_width(InOrderPipelineStage::Commit, 1).unwrap(),
        ])
        .unwrap_err(),
        InOrderPipelineError::DuplicateStageWidth {
            stage: InOrderPipelineStage::Fetch1,
        }
    );
}

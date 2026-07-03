use rem6_cpu::{
    InOrderBranchPrediction, InOrderBranchRedirect, InOrderPipelineCheckpointPayload,
    InOrderPipelineConfig, InOrderPipelineError, InOrderPipelineInstruction,
    InOrderPipelineRunSummary, InOrderPipelineScheduler, InOrderPipelineSnapshot,
    InOrderPipelineStage, InOrderPipelineStageWidth, InOrderPipelineState,
};

const IN_ORDER_CHECKPOINT_INSTRUCTION_COUNT_OFFSET: usize = 33;
const IN_ORDER_CHECKPOINT_FIRST_INSTRUCTION_OFFSET: usize = 37;
const IN_ORDER_CHECKPOINT_INSTRUCTION_RECORD_BYTES: usize = 9;
const SINGLE_IN_ORDER_CHECKPOINT_BYTES: &[u8] = &[
    b'R', b'I', b'O', b'P', 1, 4, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0,
    0, 1, 0, 0, 0, 1, 0, 0, 0, 0x5b, 0, 0, 0, 0, 0, 0, 0, 3,
];

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
    let redirect =
        InOrderBranchRedirect::branch_prediction(30, InOrderPipelineStage::Execute, 0x2000);

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
fn in_order_pipeline_records_pc_only_branch_prediction() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state
        .replace_in_flight([instruction(30, InOrderPipelineStage::Execute)])
        .unwrap();
    let prediction = InOrderBranchPrediction::new(
        30,
        InOrderPipelineStage::Execute,
        0x1000,
        true,
        true,
        Some(0x2000),
        true,
        Some(0x2000),
    );

    let record = state
        .try_advance_cycle_recorded_with_prediction(Some(prediction))
        .unwrap();

    assert_eq!(record.branch_predictions().len(), 1);
    let evidence = &record.branch_predictions()[0];
    assert_eq!(evidence.sequence(), 30);
    assert_eq!(evidence.resolved_stage(), InOrderPipelineStage::Execute);
    assert_eq!(evidence.fetch_pc(), 0x1000);
    assert!(evidence.is_conditional());
    assert!(evidence.predicted_taken());
    assert_eq!(evidence.predicted_target_pc(), Some(0x2000));
    assert!(evidence.resolved_taken());
    assert_eq!(evidence.resolved_target_pc(), Some(0x2000));
    assert!(!evidence.mispredicted());
    assert_eq!(evidence.repair_target_pc(), None);
    assert!(evidence.flushed().is_empty());

    let summary = record.summary();
    assert_eq!(summary.branch_prediction_count(), 1);
    assert_eq!(summary.correct_branch_prediction_count(), 1);
    assert_eq!(summary.branch_misprediction_count(), 0);
    assert_eq!(summary.conditional_branch_prediction_count(), 1);
    assert_eq!(summary.conditional_branch_predicted_taken_count(), 1);
    assert_eq!(summary.conditional_branch_misprediction_count(), 0);
    assert_eq!(summary.branch_prediction_flushed_count(), 0);
}

#[test]
fn in_order_pipeline_prediction_mispredict_flushes_younger_work() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state
        .replace_in_flight([
            instruction(29, InOrderPipelineStage::Commit),
            instruction(30, InOrderPipelineStage::Execute),
            instruction(31, InOrderPipelineStage::Decode),
            instruction(32, InOrderPipelineStage::Fetch2),
        ])
        .unwrap();
    let prediction = InOrderBranchPrediction::new(
        30,
        InOrderPipelineStage::Execute,
        0x1000,
        true,
        false,
        None,
        true,
        Some(0x2000),
    );

    let record = state
        .try_advance_cycle_recorded_with_prediction(Some(prediction))
        .unwrap();

    assert_eq!(
        record
            .plan()
            .redirect()
            .map(|redirect| redirect.target_pc()),
        Some(0x2000)
    );
    let evidence = &record.branch_predictions()[0];
    assert!(evidence.mispredicted());
    assert_eq!(evidence.repair_target_pc(), Some(0x2000));
    assert_eq!(
        evidence
            .flushed()
            .iter()
            .map(|instruction| instruction.sequence())
            .collect::<Vec<_>>(),
        vec![31, 32]
    );
    assert_eq!(
        state
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(30, InOrderPipelineStage::Commit)]
    );

    let summary = record.summary();
    assert_eq!(summary.branch_prediction_count(), 1);
    assert_eq!(summary.branch_misprediction_count(), 1);
    assert_eq!(summary.conditional_branch_prediction_count(), 1);
    assert_eq!(summary.conditional_branch_predicted_taken_count(), 0);
    assert_eq!(summary.conditional_branch_misprediction_count(), 1);
    assert_eq!(summary.branch_prediction_flushed_count(), 2);
    let run_summary = InOrderPipelineRunSummary::from_cycle_records([record]);
    assert_eq!(run_summary.flushed_count(), 2);
    assert_eq!(run_summary.flush_cycle_count(), 1);
    assert_eq!(run_summary.branch_prediction_flushed_count(), 2);
    assert_eq!(run_summary.branch_prediction_flush_cycle_count(), 1);
}

#[test]
fn in_order_pipeline_prediction_mispredict_requires_repair_target() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state
        .replace_in_flight([instruction(30, InOrderPipelineStage::Execute)])
        .unwrap();
    let prediction = InOrderBranchPrediction::new(
        30,
        InOrderPipelineStage::Execute,
        0x1000,
        true,
        true,
        Some(0x2000),
        false,
        None,
    );

    assert_eq!(
        state
            .try_advance_cycle_recorded_with_prediction(Some(prediction))
            .unwrap_err(),
        InOrderPipelineError::MissingBranchPredictionRepairTarget { sequence: 30 }
    );
    assert_eq!(state.cycle(), 0);
    assert_eq!(
        state
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(30, InOrderPipelineStage::Execute)]
    );
}

#[test]
fn in_order_pipeline_correct_prediction_rejects_absent_instruction() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state
        .replace_in_flight([instruction(31, InOrderPipelineStage::Execute)])
        .unwrap();
    let prediction = InOrderBranchPrediction::new(
        30,
        InOrderPipelineStage::Execute,
        0x1000,
        true,
        false,
        None,
        false,
        None,
    );

    assert_eq!(
        state
            .try_advance_cycle_recorded_with_prediction(Some(prediction))
            .unwrap_err(),
        InOrderPipelineError::MissingBranchPredictionInstruction { sequence: 30 }
    );
    assert_eq!(state.cycle(), 0);
    assert_eq!(
        state
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(31, InOrderPipelineStage::Execute)]
    );
}

#[test]
fn in_order_pipeline_correct_prediction_rejects_wrong_stage() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state
        .replace_in_flight([instruction(30, InOrderPipelineStage::Decode)])
        .unwrap();
    let prediction = InOrderBranchPrediction::new(
        30,
        InOrderPipelineStage::Execute,
        0x1000,
        true,
        true,
        Some(0x2000),
        true,
        Some(0x2000),
    );

    assert_eq!(
        state
            .try_advance_cycle_recorded_with_prediction(Some(prediction))
            .unwrap_err(),
        InOrderPipelineError::BranchPredictionStageMismatch {
            sequence: 30,
            expected: InOrderPipelineStage::Execute,
            actual: InOrderPipelineStage::Decode,
        }
    );
    assert_eq!(state.cycle(), 0);
    assert_eq!(
        state
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(30, InOrderPipelineStage::Decode)]
    );
}

#[test]
fn in_order_pipeline_rejects_redirect_for_absent_instruction() {
    let scheduler = scheduler_with_decode_width(1);
    let redirect =
        InOrderBranchRedirect::branch_prediction(30, InOrderPipelineStage::Execute, 0x2000);

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
    let redirect =
        InOrderBranchRedirect::branch_prediction(30, InOrderPipelineStage::Execute, 0x2000);

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
fn in_order_pipeline_resource_stall_cycle_preserves_in_flight_work() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state
        .replace_in_flight([
            instruction(7, InOrderPipelineStage::Fetch1),
            instruction(8, InOrderPipelineStage::Decode),
        ])
        .unwrap();

    let record = state.try_record_resource_stall_cycle().unwrap();

    assert_eq!(record.cycle(), 0);
    assert_eq!(record.stall_cycle_count(), 1);
    assert_eq!(record.before().cycle(), 0);
    assert_eq!(record.after().cycle(), 1);
    assert_eq!(record.before().in_flight(), record.after().in_flight());
    assert_eq!(state.cycle(), 1);
    assert_eq!(
        state
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![
            (7, InOrderPipelineStage::Fetch1),
            (8, InOrderPipelineStage::Decode)
        ]
    );

    let summary = record.summary();
    assert_eq!(summary.advanced_count(), 0);
    assert_eq!(summary.retired_count(), 0);
    assert_eq!(summary.resource_blocked_count(), 2);
    assert_eq!(summary.ordering_blocked_count(), 0);
    assert!(!summary.state_changed());
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
    let redirect =
        InOrderBranchRedirect::branch_prediction(30, InOrderPipelineStage::Execute, 0x2000);

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
    let redirect =
        InOrderBranchRedirect::branch_prediction(30, InOrderPipelineStage::Execute, 0x2000);

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
    assert_eq!(summary.branch_prediction_redirect_count(), 1);
    assert_eq!(summary.trap_redirect_count(), 0);
}

#[test]
fn in_order_pipeline_cycle_summary_records_trap_redirect_flush_counts() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state
        .replace_in_flight([
            instruction(29, InOrderPipelineStage::Commit),
            instruction(30, InOrderPipelineStage::Commit),
            instruction(31, InOrderPipelineStage::Decode),
            instruction(32, InOrderPipelineStage::Fetch2),
        ])
        .unwrap();
    let redirect = InOrderBranchRedirect::trap(30, InOrderPipelineStage::Commit, 0x0);

    let summary = state
        .try_advance_cycle_recorded_with_redirect(Some(redirect))
        .unwrap()
        .summary();

    assert_eq!(summary.redirect_target_pc(), Some(0x0));
    assert_eq!(summary.branch_prediction_redirect_count(), 0);
    assert_eq!(summary.trap_redirect_count(), 1);
    assert_eq!(summary.flushed_count(), 2);
    assert_eq!(summary.trap_redirect_flushed_count(), 2);
    assert_eq!(summary.trap_redirect_flush_cycle_count(), 1);
    assert_eq!(summary.branch_prediction_flushed_count(), 0);
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
fn in_order_pipeline_run_summary_aggregates_redirect_flush_and_ordering_counts() {
    let mut redirect_state = InOrderPipelineState::new(config_with_decode_width(1));
    redirect_state
        .replace_in_flight([
            instruction(29, InOrderPipelineStage::Commit),
            instruction(30, InOrderPipelineStage::Execute),
            instruction(31, InOrderPipelineStage::Decode),
            instruction(32, InOrderPipelineStage::Fetch2),
        ])
        .unwrap();
    let redirect =
        InOrderBranchRedirect::branch_prediction(30, InOrderPipelineStage::Execute, 0x2000);
    let redirect_record = redirect_state
        .try_advance_cycle_recorded_with_redirect(Some(redirect))
        .unwrap();

    let blocked_snapshot = InOrderPipelineSnapshot::with_cycle(
        config_with_decode_width(1),
        10,
        [
            instruction(40, InOrderPipelineStage::Decode),
            instruction(41, InOrderPipelineStage::Decode),
            instruction(42, InOrderPipelineStage::Execute),
        ],
    );
    let mut blocked_state = InOrderPipelineState::restore(blocked_snapshot).unwrap();
    let blocked_record = blocked_state.advance_cycle_recorded();

    let summary = InOrderPipelineRunSummary::from_cycle_records([redirect_record])
        .merge_disjoint(InOrderPipelineRunSummary::from_cycle_records([
            blocked_record,
        ]))
        .unwrap();

    assert_eq!(summary.cycle_count(), 2);
    assert_eq!(summary.first_cycle(), Some(0));
    assert_eq!(summary.last_cycle(), Some(10));
    assert_eq!(summary.redirect_count(), 1);
    assert_eq!(summary.flushed_count(), 2);
    assert_eq!(summary.flush_cycle_count(), 1);
    assert_eq!(summary.branch_prediction_flush_cycle_count(), 0);
    assert_eq!(summary.branch_prediction_redirect_count(), 1);
    assert_eq!(summary.trap_redirect_count(), 0);
    assert_eq!(summary.resource_blocked_count(), 1);
    assert_eq!(summary.ordering_blocked_count(), 1);
}

#[test]
fn in_order_pipeline_empty_run_summary_has_no_cycle_window() {
    let summary = InOrderPipelineRunSummary::from_cycle_summaries([]);

    assert!(summary.is_empty());
    assert_eq!(summary.cycle_count(), 0);
    assert_eq!(summary.first_cycle(), None);
    assert_eq!(summary.last_cycle(), None);
    assert_eq!(summary.advanced_count(), 0);
    assert_eq!(summary.flush_cycle_count(), 0);
    assert_eq!(summary.branch_prediction_flush_cycle_count(), 0);
    assert_eq!(summary.state_changed_cycle_count(), 0);
}

#[test]
fn in_order_pipeline_run_summary_merges_disjoint_partial_summaries() {
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
        .merge_disjoint(first)
        .unwrap()
        .merge_disjoint(second)
        .unwrap();

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
fn in_order_pipeline_prediction_summary_merges_disjoint_windows() {
    let first_snapshot = InOrderPipelineSnapshot::with_cycle(
        config_with_decode_width(1),
        4,
        [instruction(50, InOrderPipelineStage::Execute)],
    );
    let mut first_state = InOrderPipelineState::restore(first_snapshot).unwrap();
    let correct_prediction = InOrderBranchPrediction::new(
        50,
        InOrderPipelineStage::Execute,
        0x4000,
        true,
        false,
        None,
        false,
        None,
    );
    let first = InOrderPipelineRunSummary::from_cycle_records([first_state
        .try_advance_cycle_recorded_with_prediction(Some(correct_prediction))
        .unwrap()]);

    let second_snapshot = InOrderPipelineSnapshot::with_cycle(
        config_with_decode_width(1),
        10,
        [
            instruction(60, InOrderPipelineStage::Execute),
            instruction(61, InOrderPipelineStage::Decode),
            instruction(62, InOrderPipelineStage::Fetch2),
        ],
    );
    let mut second_state = InOrderPipelineState::restore(second_snapshot).unwrap();
    let misprediction = InOrderBranchPrediction::new(
        60,
        InOrderPipelineStage::Execute,
        0x5000,
        false,
        false,
        None,
        true,
        Some(0x5800),
    );
    let second = InOrderPipelineRunSummary::from_cycle_records([second_state
        .try_advance_cycle_recorded_with_prediction(Some(misprediction))
        .unwrap()]);

    let merged = first.merge_disjoint(second).unwrap();

    assert_eq!(merged.cycle_count(), 2);
    assert_eq!(merged.first_cycle(), Some(4));
    assert_eq!(merged.last_cycle(), Some(10));
    assert_eq!(merged.branch_prediction_count(), 2);
    assert_eq!(merged.correct_branch_prediction_count(), 1);
    assert_eq!(merged.branch_misprediction_count(), 1);
    assert_eq!(merged.conditional_branch_prediction_count(), 1);
    assert_eq!(merged.conditional_branch_predicted_taken_count(), 0);
    assert_eq!(merged.conditional_branch_misprediction_count(), 0);
    assert_eq!(merged.branch_prediction_flushed_count(), 2);
    assert_eq!(merged.branch_prediction_flush_cycle_count(), 1);
}

#[test]
fn in_order_pipeline_run_summary_rejects_overlapping_partial_summaries() {
    let left = InOrderPipelineRunSummary::from_cycle_summaries([InOrderPipelineState::new(
        config_with_decode_width(1),
    )
    .advance_cycle_recorded()
    .summary()]);
    let right = InOrderPipelineRunSummary::from_cycle_summaries([InOrderPipelineState::new(
        config_with_decode_width(1),
    )
    .advance_cycle_recorded()
    .summary()]);

    assert_eq!(
        left.merge_disjoint(right).unwrap_err(),
        InOrderPipelineError::OverlappingRunSummaryMerge {
            left_first_cycle: 0,
            left_last_cycle: 0,
            right_first_cycle: 0,
            right_last_cycle: 0,
        }
    );
}

#[test]
fn in_order_pipeline_checkpoint_payload_round_trips_state() {
    let config = InOrderPipelineConfig::new([
        stage_width(InOrderPipelineStage::Fetch1, 2).unwrap(),
        stage_width(InOrderPipelineStage::Fetch2, 1).unwrap(),
        stage_width(InOrderPipelineStage::Decode, 2).unwrap(),
        stage_width(InOrderPipelineStage::Execute, 1).unwrap(),
        stage_width(InOrderPipelineStage::Commit, 2).unwrap(),
    ])
    .unwrap();
    let snapshot = InOrderPipelineSnapshot::with_cycle(
        config.clone(),
        19,
        [
            instruction(80, InOrderPipelineStage::Fetch1),
            instruction(81, InOrderPipelineStage::Decode),
            instruction(82, InOrderPipelineStage::Commit),
        ],
    );
    let payload = InOrderPipelineCheckpointPayload::from_snapshot(snapshot.clone()).unwrap();

    let decoded = InOrderPipelineCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = InOrderPipelineState::restore(decoded.into_snapshot()).unwrap();

    assert_eq!(restored.cycle(), 19);
    assert_eq!(restored.config(), &config);
    assert_eq!(restored.in_flight(), snapshot.in_flight());
}

#[test]
fn in_order_pipeline_prediction_flush_checkpoint_round_trips_remaining_state() {
    let mut state = InOrderPipelineState::new(config_with_decode_width(1));
    state
        .replace_in_flight([
            instruction(30, InOrderPipelineStage::Execute),
            instruction(31, InOrderPipelineStage::Decode),
            instruction(32, InOrderPipelineStage::Fetch2),
        ])
        .unwrap();
    let prediction = InOrderBranchPrediction::new(
        30,
        InOrderPipelineStage::Execute,
        0x1000,
        true,
        false,
        None,
        true,
        Some(0x2000),
    );

    state
        .try_advance_cycle_recorded_with_prediction(Some(prediction))
        .unwrap();
    let payload = InOrderPipelineCheckpointPayload::from_state(&state).encode();
    let decoded = InOrderPipelineCheckpointPayload::decode(&payload).unwrap();
    let restored = InOrderPipelineState::restore(decoded.into_snapshot()).unwrap();

    assert_eq!(restored.cycle(), 1);
    assert_eq!(
        restored
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(30, InOrderPipelineStage::Commit)]
    );
}

#[test]
fn in_order_pipeline_checkpoint_payload_has_stable_single_instruction_bytes() {
    let snapshot = InOrderPipelineSnapshot::with_cycle(
        config_with_decode_width(1),
        4,
        [instruction(91, InOrderPipelineStage::Execute)],
    );
    let payload = InOrderPipelineCheckpointPayload::from_snapshot(snapshot.clone())
        .unwrap()
        .encode();

    assert_eq!(payload, SINGLE_IN_ORDER_CHECKPOINT_BYTES);
    let decoded = InOrderPipelineCheckpointPayload::decode(SINGLE_IN_ORDER_CHECKPOINT_BYTES)
        .unwrap()
        .into_snapshot();

    assert_eq!(decoded, snapshot);
}

#[test]
fn in_order_pipeline_checkpoint_payload_rejects_duplicate_sequences() {
    let snapshot = InOrderPipelineSnapshot::with_cycle(
        config_with_decode_width(1),
        3,
        [
            instruction(90, InOrderPipelineStage::Fetch2),
            instruction(90, InOrderPipelineStage::Decode),
        ],
    );

    assert_eq!(
        InOrderPipelineCheckpointPayload::from_snapshot(snapshot).unwrap_err(),
        InOrderPipelineError::DuplicateInFlightInstruction { sequence: 90 }
    );
}

#[test]
fn in_order_pipeline_checkpoint_payload_rejects_count_size_mismatches() {
    let mut missing_instruction_payload = SINGLE_IN_ORDER_CHECKPOINT_BYTES.to_vec();
    missing_instruction_payload[IN_ORDER_CHECKPOINT_INSTRUCTION_COUNT_OFFSET
        ..IN_ORDER_CHECKPOINT_INSTRUCTION_COUNT_OFFSET + 4]
        .copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        InOrderPipelineCheckpointPayload::decode(&missing_instruction_payload).unwrap_err(),
        InOrderPipelineError::InvalidCheckpointPayloadSize {
            expected: IN_ORDER_CHECKPOINT_FIRST_INSTRUCTION_OFFSET
                + 2 * IN_ORDER_CHECKPOINT_INSTRUCTION_RECORD_BYTES,
            actual: SINGLE_IN_ORDER_CHECKPOINT_BYTES.len(),
        }
    );

    let mut trailing_payload = SINGLE_IN_ORDER_CHECKPOINT_BYTES.to_vec();
    trailing_payload.push(0);
    assert_eq!(
        InOrderPipelineCheckpointPayload::decode(&trailing_payload).unwrap_err(),
        InOrderPipelineError::InvalidCheckpointPayloadSize {
            expected: SINGLE_IN_ORDER_CHECKPOINT_BYTES.len(),
            actual: SINGLE_IN_ORDER_CHECKPOINT_BYTES.len() + 1,
        }
    );
}

#[test]
fn in_order_pipeline_checkpoint_payload_rejects_duplicate_sequences_from_decode() {
    let snapshot = InOrderPipelineSnapshot::with_cycle(
        config_with_decode_width(1),
        4,
        [
            instruction(91, InOrderPipelineStage::Execute),
            instruction(92, InOrderPipelineStage::Commit),
        ],
    );
    let mut payload = InOrderPipelineCheckpointPayload::from_snapshot(snapshot)
        .unwrap()
        .encode();
    let second_sequence_offset =
        IN_ORDER_CHECKPOINT_FIRST_INSTRUCTION_OFFSET + IN_ORDER_CHECKPOINT_INSTRUCTION_RECORD_BYTES;
    payload[second_sequence_offset..second_sequence_offset + 8]
        .copy_from_slice(&91_u64.to_le_bytes());

    assert_eq!(
        InOrderPipelineCheckpointPayload::decode(&payload).unwrap_err(),
        InOrderPipelineError::DuplicateInFlightInstruction { sequence: 91 }
    );
}

#[test]
fn in_order_pipeline_checkpoint_payload_rejects_malformed_payloads() {
    assert_eq!(
        InOrderPipelineCheckpointPayload::decode(b"bad").unwrap_err(),
        InOrderPipelineError::InvalidCheckpointPayloadSize {
            expected: IN_ORDER_CHECKPOINT_FIRST_INSTRUCTION_OFFSET,
            actual: 3,
        }
    );

    let payload =
        InOrderPipelineCheckpointPayload::from_snapshot(InOrderPipelineSnapshot::with_cycle(
            config_with_decode_width(1),
            4,
            [instruction(91, InOrderPipelineStage::Execute)],
        ))
        .unwrap()
        .encode();
    let mut invalid_stage_payload = payload.clone();
    *invalid_stage_payload.last_mut().unwrap() = 99;
    assert_eq!(
        InOrderPipelineCheckpointPayload::decode(&invalid_stage_payload).unwrap_err(),
        InOrderPipelineError::InvalidCheckpointStageCode { code: 99 }
    );

    let mut invalid_magic_payload = payload.clone();
    invalid_magic_payload[0] = b'X';
    assert_eq!(
        InOrderPipelineCheckpointPayload::decode(&invalid_magic_payload).unwrap_err(),
        InOrderPipelineError::InvalidCheckpointMagic
    );

    let mut unsupported_version_payload = payload.clone();
    unsupported_version_payload[4] = 2;
    assert_eq!(
        InOrderPipelineCheckpointPayload::decode(&unsupported_version_payload).unwrap_err(),
        InOrderPipelineError::UnsupportedCheckpointVersion { version: 2 }
    );

    let mut invalid_width_payload = payload;
    invalid_width_payload[13..17].copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        InOrderPipelineCheckpointPayload::decode(&invalid_width_payload).unwrap_err(),
        InOrderPipelineError::ZeroStageWidth {
            stage: InOrderPipelineStage::Fetch1,
        }
    );
}

#[test]
fn in_order_pipeline_checkpoint_payload_rejects_widths_too_large_to_encode() {
    let config = InOrderPipelineConfig::new([
        stage_width(
            InOrderPipelineStage::Fetch1,
            usize::try_from(u32::MAX).unwrap() + 1,
        )
        .unwrap(),
        stage_width(InOrderPipelineStage::Fetch2, 1).unwrap(),
        stage_width(InOrderPipelineStage::Decode, 1).unwrap(),
        stage_width(InOrderPipelineStage::Execute, 1).unwrap(),
        stage_width(InOrderPipelineStage::Commit, 1).unwrap(),
    ])
    .unwrap();
    let payload =
        InOrderPipelineCheckpointPayload::from_snapshot(InOrderPipelineSnapshot::new(config, []))
            .unwrap();

    assert_eq!(
        payload.try_encode().unwrap_err(),
        InOrderPipelineError::CheckpointValueTooLarge {
            field: "stage width",
            value: usize::try_from(u32::MAX).unwrap() + 1,
            maximum: usize::try_from(u32::MAX).unwrap(),
        }
    );
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

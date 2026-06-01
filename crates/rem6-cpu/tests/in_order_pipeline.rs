use rem6_cpu::{
    InOrderPipelineConfig, InOrderPipelineError, InOrderPipelineInstruction,
    InOrderPipelineScheduler, InOrderPipelineStage, InOrderPipelineStageWidth,
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
    let config = InOrderPipelineConfig::new([
        stage_width(InOrderPipelineStage::Fetch1, 1).unwrap(),
        stage_width(InOrderPipelineStage::Fetch2, 1).unwrap(),
        stage_width(InOrderPipelineStage::Decode, decode_width).unwrap(),
        stage_width(InOrderPipelineStage::Execute, 1).unwrap(),
        stage_width(InOrderPipelineStage::Commit, 1).unwrap(),
    ])
    .unwrap();
    InOrderPipelineScheduler::new(config)
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

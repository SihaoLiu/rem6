use rem6_cpu::{
    O3DistributedIssueScheduler, O3IssueOpClass, O3IssueQueueCapacity, O3IssueQueueId,
    O3PipelineError, O3PipelineStage, O3ReadyInstruction, O3UnblockDecisionReason, O3UnblockPolicy,
    O3WritebackTransferPolicy,
};

#[test]
fn o3_unblock_policy_signals_before_skid_buffer_is_empty() {
    let policy =
        O3UnblockPolicy::new(O3PipelineStage::Fetch, O3PipelineStage::Decode, 1, 2).unwrap();

    assert_eq!(policy.upstream(), O3PipelineStage::Fetch);
    assert_eq!(policy.downstream(), O3PipelineStage::Decode);
    assert_eq!(policy.backward_signal_delay_cycles(), 1);
    assert_eq!(policy.downstream_width(), 2);
    assert_eq!(policy.early_unblock_threshold_entries(), 2);

    let still_draining = policy.decision(3);
    assert!(!still_draining.should_signal_unblock());
    assert_eq!(
        still_draining.reason(),
        O3UnblockDecisionReason::SkidBufferAboveEarlyThreshold
    );
    assert_eq!(still_draining.cycles_to_drain(), 2);

    let early = policy.decision(2);
    assert!(early.should_signal_unblock());
    assert_eq!(early.reason(), O3UnblockDecisionReason::SignalDelayCovered);
    assert_eq!(early.cycles_to_drain(), 1);
    assert!(!policy.empty_only_would_signal(2));

    let empty = policy.decision(0);
    assert!(empty.should_signal_unblock());
    assert_eq!(empty.reason(), O3UnblockDecisionReason::SkidBufferEmpty);
}

#[test]
fn o3_unblock_policy_validates_zero_width_and_zero_delay_boundaries() {
    assert_eq!(
        O3UnblockPolicy::new(O3PipelineStage::Rename, O3PipelineStage::Iew, 1, 0).unwrap_err(),
        O3PipelineError::ZeroDownstreamWidth {
            downstream: O3PipelineStage::Iew,
        }
    );

    let zero_delay =
        O3UnblockPolicy::new(O3PipelineStage::Decode, O3PipelineStage::Rename, 0, 4).unwrap();
    assert_eq!(zero_delay.early_unblock_threshold_entries(), 0);
    assert!(!zero_delay.decision(1).should_signal_unblock());
    assert!(zero_delay.decision(0).should_signal_unblock());
    assert!(zero_delay.empty_only_would_signal(0));
}

#[test]
fn o3_writeback_transfer_policy_defers_overfull_same_tick_completions() {
    let policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 4, 2).unwrap();

    assert_eq!(policy.source(), O3PipelineStage::Iew);
    assert_eq!(policy.writeback_width(), 4);
    assert_eq!(policy.future_cycles(), 2);
    assert_eq!(policy.capacity_entries(), 12);

    let plan = policy.plan_ready_count(14);
    assert_eq!(plan.ready_count(), 14);
    assert_eq!(plan.admitted_count(), 12);
    assert_eq!(plan.deferred_count(), 2);
    assert!(plan.has_deferred());

    let first = plan.admissions().first().unwrap();
    assert_eq!(first.ready_index(), 0);
    assert_eq!(first.cycle_offset(), 0);
    assert_eq!(first.slot(), 0);

    let last = plan.admissions().last().unwrap();
    assert_eq!(last.ready_index(), 11);
    assert_eq!(last.cycle_offset(), 2);
    assert_eq!(last.slot(), 3);
    assert!(plan
        .admissions()
        .iter()
        .all(|admission| admission.cycle_offset() <= policy.future_cycles()));
}

#[test]
fn o3_writeback_transfer_policy_keeps_exact_fit_completions_in_window() {
    let policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 3, 1).unwrap();

    let plan = policy.plan_ready_count(6);
    assert_eq!(plan.ready_count(), 6);
    assert_eq!(plan.admitted_count(), 6);
    assert_eq!(plan.deferred_count(), 0);
    assert!(!plan.has_deferred());
    assert_eq!(plan.admissions()[3].ready_index(), 3);
    assert_eq!(plan.admissions()[3].cycle_offset(), 1);
    assert_eq!(plan.admissions()[3].slot(), 0);
}

#[test]
fn o3_writeback_transfer_policy_rejects_unrepresentable_windows() {
    assert_eq!(
        O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 0, 1).unwrap_err(),
        O3PipelineError::ZeroWritebackWidth {
            source: O3PipelineStage::Iew,
        }
    );

    assert_eq!(
        O3WritebackTransferPolicy::new(O3PipelineStage::Iew, usize::MAX, 1).unwrap_err(),
        O3PipelineError::WritebackWindowOverflow {
            source: O3PipelineStage::Iew,
            writeback_width: usize::MAX,
            future_cycles: 1,
        }
    );
}

#[test]
fn o3_distributed_issue_scheduler_skips_blocked_queue_without_starving_peer_queue() {
    let queue_a = O3IssueQueueId::new(0);
    let queue_b = O3IssueQueueId::new(1);
    let scheduler = O3DistributedIssueScheduler::new(
        2,
        [
            O3IssueQueueCapacity::new(queue_a, O3IssueOpClass::IntAlu, 1).unwrap(),
            O3IssueQueueCapacity::new(queue_b, O3IssueOpClass::IntAlu, 1).unwrap(),
        ],
    )
    .unwrap();

    let plan = scheduler.plan([
        O3ReadyInstruction::new(10, queue_a, O3IssueOpClass::IntAlu),
        O3ReadyInstruction::new(11, queue_a, O3IssueOpClass::IntAlu),
        O3ReadyInstruction::new(12, queue_b, O3IssueOpClass::IntAlu),
        O3ReadyInstruction::new(13, queue_b, O3IssueOpClass::IntAlu),
    ]);

    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![10, 12]);
    assert_eq!(plan.issued()[0].queue(), queue_a);
    assert_eq!(plan.issued()[1].queue(), queue_b);
    assert_eq!(plan.blocked()[0].sequence(), 11);
    assert_eq!(plan.blocked()[0].queue(), queue_a);
}

#[test]
fn o3_distributed_issue_scheduler_validates_width_and_queue_capacity() {
    assert_eq!(
        O3DistributedIssueScheduler::new(
            0,
            [
                O3IssueQueueCapacity::new(O3IssueQueueId::new(0), O3IssueOpClass::IntAlu, 1,)
                    .unwrap()
            ],
        )
        .unwrap_err(),
        O3PipelineError::ZeroIssueWidth
    );
    assert_eq!(
        O3IssueQueueCapacity::new(O3IssueQueueId::new(0), O3IssueOpClass::IntAlu, 0).unwrap_err(),
        O3PipelineError::ZeroIssueQueueCapacity {
            queue: O3IssueQueueId::new(0),
            op_class: O3IssueOpClass::IntAlu,
        }
    );
}

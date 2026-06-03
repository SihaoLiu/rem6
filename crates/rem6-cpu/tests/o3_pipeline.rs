use rem6_cpu::{
    O3DependencyScopeId, O3DistributedIssueScheduler, O3IssueOpClass, O3IssueQueueCapacity,
    O3IssueQueueId, O3PipelineError, O3PipelineStage, O3ReadyInstruction, O3ScopedIssueScheduler,
    O3ScopedReadyInstruction, O3UnblockDecisionReason, O3UnblockPolicy,
    O3VectorReductionDependencyPlan, O3VectorReductionGroupId, O3VectorReductionOrdering,
    O3WritebackCompletion, O3WritebackTransferBuffer, O3WritebackTransferPolicy,
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
fn o3_writeback_transfer_buffer_replays_deferred_before_new_ready_work() {
    let policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap();
    let mut buffer = O3WritebackTransferBuffer::new(policy);

    let first = buffer.plan_cycle([
        O3WritebackCompletion::new(10),
        O3WritebackCompletion::new(11),
        O3WritebackCompletion::new(12),
    ]);

    assert_eq!(first.new_ready_count(), 3);
    assert_eq!(first.deferred_before_count(), 0);
    assert_eq!(first.admitted_sequences().collect::<Vec<_>>(), vec![10, 11]);
    assert_eq!(first.deferred_sequences().collect::<Vec<_>>(), vec![12]);
    assert_eq!(buffer.pending_deferred_count(), 1);

    let second = buffer.plan_cycle([O3WritebackCompletion::new(13)]);

    assert_eq!(second.new_ready_count(), 1);
    assert_eq!(second.deferred_before_count(), 1);
    assert_eq!(
        second.admitted_sequences().collect::<Vec<_>>(),
        vec![12, 13]
    );
    assert!(second.deferred_sequences().next().is_none());
    assert_eq!(buffer.pending_deferred_count(), 0);
}

#[test]
fn o3_writeback_transfer_buffer_preserves_window_offsets_for_admitted_work() {
    let policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 1).unwrap();
    let mut buffer = O3WritebackTransferBuffer::new(policy);

    let first = buffer.plan_cycle([
        O3WritebackCompletion::new(20),
        O3WritebackCompletion::new(21),
        O3WritebackCompletion::new(22),
        O3WritebackCompletion::new(23),
        O3WritebackCompletion::new(24),
    ]);

    assert_eq!(
        first
            .admissions()
            .iter()
            .map(|admission| {
                (
                    admission.completion().sequence(),
                    admission.cycle_offset(),
                    admission.slot(),
                )
            })
            .collect::<Vec<_>>(),
        vec![(20, 0, 0), (21, 0, 1), (22, 1, 0), (23, 1, 1)]
    );
    assert_eq!(first.deferred_sequences().collect::<Vec<_>>(), vec![24]);

    let second = buffer.plan_cycle([]);

    assert_eq!(second.admitted_sequences().collect::<Vec<_>>(), vec![24]);
    assert_eq!(second.admissions()[0].cycle_offset(), 0);
    assert_eq!(second.admissions()[0].slot(), 0);
    assert_eq!(buffer.pending_deferred_count(), 0);
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

#[test]
fn o3_unordered_vector_reduction_uses_scoped_dependencies_without_serializing() {
    let group = O3VectorReductionGroupId::new(7);
    let reduction =
        O3VectorReductionDependencyPlan::new(group, 100, 3, O3VectorReductionOrdering::Unordered)
            .unwrap();

    assert_eq!(reduction.micro_ops().len(), 4);
    assert!(reduction
        .micro_ops()
        .iter()
        .all(|micro_op| !micro_op.requires_serialize_after()));
    assert!(reduction
        .partial_micro_ops()
        .iter()
        .all(|micro_op| micro_op.waits_on().is_empty()));

    let publish = reduction.publish_micro_op();
    assert_eq!(publish.sequence(), 103);
    assert_eq!(publish.waits_on().len(), 3);
    assert_eq!(
        publish.produces(),
        &[reduction.architectural_result_scope()]
    );

    let scheduler = O3ScopedIssueScheduler::new(
        2,
        [O3IssueQueueCapacity::new(O3IssueQueueId::new(0), O3IssueOpClass::IntAlu, 2).unwrap()],
    )
    .unwrap();
    let plan = scheduler.plan(
        [],
        [
            O3ScopedReadyInstruction::new(
                publish.sequence(),
                O3IssueQueueId::new(0),
                O3IssueOpClass::IntAlu,
            )
            .with_waits_on(publish.waits_on().iter().copied()),
            O3ScopedReadyInstruction::new(104, O3IssueQueueId::new(0), O3IssueOpClass::IntAlu),
        ],
    );

    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![104]);
    assert_eq!(plan.dependency_blocked()[0].sequence(), publish.sequence());
}

#[test]
fn o3_ordered_vector_reduction_chains_only_reduction_local_scopes() {
    let reduction = O3VectorReductionDependencyPlan::new(
        O3VectorReductionGroupId::new(9),
        200,
        3,
        O3VectorReductionOrdering::Ordered,
    )
    .unwrap();
    let partials = reduction.partial_micro_ops();

    assert!(partials[0].waits_on().is_empty());
    assert_eq!(partials[1].waits_on(), partials[0].produces());
    assert_eq!(partials[2].waits_on(), partials[1].produces());
    assert_eq!(
        reduction.publish_micro_op().waits_on(),
        partials[2].produces()
    );
    assert!(reduction
        .micro_ops()
        .iter()
        .all(|micro_op| !micro_op.requires_serialize_after()));
}

#[test]
fn o3_scoped_issue_scheduler_rejects_duplicate_producers() {
    let queue = O3IssueQueueId::new(0);
    let scheduler = O3ScopedIssueScheduler::new(
        2,
        [O3IssueQueueCapacity::new(queue, O3IssueOpClass::IntAlu, 2).unwrap()],
    )
    .unwrap();
    let scope = O3DependencyScopeId::new(1);

    assert_eq!(
        scheduler
            .try_plan(
                [],
                [
                    O3ScopedReadyInstruction::new(10, queue, O3IssueOpClass::IntAlu)
                        .with_produces([scope]),
                    O3ScopedReadyInstruction::new(11, queue, O3IssueOpClass::IntAlu)
                        .with_produces([scope]),
                ],
            )
            .unwrap_err(),
        O3PipelineError::DuplicateDependencyProducer { scope }
    );
}

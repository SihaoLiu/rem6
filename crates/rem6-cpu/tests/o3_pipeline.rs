use rem6_cpu::{
    O3DependencyScopeId, O3DistributedIssueScheduler, O3IssueOpClass, O3IssueQueueCapacity,
    O3IssueQueueId, O3PendingStateCheckpointPayload, O3PendingStateSnapshot, O3PipelineError,
    O3PipelineStage, O3ReadyInstruction, O3ScopedIssueScheduler, O3ScopedReadyInstruction,
    O3UnblockDecisionReason, O3UnblockPolicy, O3VectorReductionDependencyPlan,
    O3VectorReductionGroupId, O3VectorReductionOrdering, O3WritebackCompletion,
    O3WritebackTransferBuffer, O3WritebackTransferCheckpointPayload, O3WritebackTransferPolicy,
};

const O3_WRITEBACK_CHECKPOINT_MAGIC_OFFSET: usize = 0;
const O3_WRITEBACK_CHECKPOINT_VERSION_OFFSET: usize = 4;
const O3_WRITEBACK_CHECKPOINT_STAGE_OFFSET: usize = 5;
const O3_WRITEBACK_CHECKPOINT_WIDTH_OFFSET: usize = 6;
const O3_WRITEBACK_CHECKPOINT_DEFERRED_COUNT_OFFSET: usize = 18;
const O3_WRITEBACK_CHECKPOINT_HEADER_BYTES: usize = 22;
const O3_PENDING_CHECKPOINT_MAGIC_OFFSET: usize = 0;
const O3_PENDING_CHECKPOINT_VERSION_OFFSET: usize = 4;
const O3_PENDING_CHECKPOINT_HEADER_BYTES: usize = 17;

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
fn o3_writeback_transfer_buffer_skips_occupied_slots() {
    let policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap();
    let mut buffer = O3WritebackTransferBuffer::new(policy);

    let cycle = buffer
        .plan_cycle_with_occupied_slots([0], [O3WritebackCompletion::new(7)])
        .unwrap();

    assert_eq!(cycle.new_ready_count(), 1);
    assert_eq!(cycle.deferred_before_count(), 0);
    assert_eq!(
        cycle
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
        vec![(7, 0, 1)]
    );
    assert!(cycle.deferred_sequences().next().is_none());
    assert_eq!(buffer.pending_deferred_count(), 0);
}

#[test]
fn o3_writeback_transfer_buffer_preserves_deferred_before_new_ready_with_occupancy() {
    let policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 1, 0).unwrap();
    let mut buffer = O3WritebackTransferBuffer::new(policy);

    let first = buffer
        .plan_cycle_with_occupied_slots([0], [O3WritebackCompletion::new(1)])
        .unwrap();

    assert_eq!(first.new_ready_count(), 1);
    assert_eq!(first.deferred_before_count(), 0);
    assert!(first.admitted_sequences().next().is_none());
    assert_eq!(first.deferred_sequences().collect::<Vec<_>>(), vec![1]);
    assert_eq!(buffer.pending_deferred_count(), 1);

    let second = buffer
        .plan_cycle_with_occupied_slots([], [O3WritebackCompletion::new(2)])
        .unwrap();

    assert_eq!(second.new_ready_count(), 1);
    assert_eq!(second.deferred_before_count(), 1);
    assert_eq!(second.admitted_sequences().collect::<Vec<_>>(), vec![1]);
    assert_eq!(second.deferred_sequences().collect::<Vec<_>>(), vec![2]);
    assert_eq!(buffer.pending_deferred_count(), 1);
}

#[test]
fn o3_writeback_transfer_buffer_uses_future_policy_slots_after_occupied_current_cycle() {
    let policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 1, 1).unwrap();
    let mut buffer = O3WritebackTransferBuffer::new(policy);

    let cycle = buffer
        .plan_cycle_with_occupied_slots([0], [O3WritebackCompletion::new(7)])
        .unwrap();

    assert_eq!(cycle.new_ready_count(), 1);
    assert_eq!(cycle.deferred_before_count(), 0);
    assert_eq!(
        cycle
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
        vec![(7, 1, 0)]
    );
    assert!(cycle.deferred_sequences().next().is_none());
    assert_eq!(buffer.pending_deferred_count(), 0);
}

#[test]
fn o3_writeback_transfer_buffer_rejects_duplicate_occupied_slots() {
    let policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 1, 0).unwrap();
    let mut buffer = O3WritebackTransferBuffer::new(policy);
    buffer.plan_cycle([O3WritebackCompletion::new(1), O3WritebackCompletion::new(2)]);

    let error = buffer
        .plan_cycle_with_occupied_slots([0, 0], [O3WritebackCompletion::new(3)])
        .unwrap_err();

    assert_eq!(
        error,
        O3PipelineError::DuplicateWritebackOccupiedSlot {
            source: O3PipelineStage::Iew,
            slot: 0,
        }
    );
    assert_eq!(
        error.to_string(),
        "O3 IEW writeback occupied slot 0 appears more than once"
    );
    assert_eq!(buffer.pending_deferred_count(), 1);
    let next = buffer.plan_cycle([]);
    assert_eq!(next.admitted_sequences().collect::<Vec<_>>(), vec![2]);
}

#[test]
fn o3_writeback_transfer_buffer_rejects_out_of_range_occupied_slots() {
    let policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap();
    let mut buffer = O3WritebackTransferBuffer::new(policy);
    buffer.plan_cycle([
        O3WritebackCompletion::new(1),
        O3WritebackCompletion::new(2),
        O3WritebackCompletion::new(3),
    ]);

    let error = buffer
        .plan_cycle_with_occupied_slots([2], [O3WritebackCompletion::new(4)])
        .unwrap_err();

    assert_eq!(
        error,
        O3PipelineError::WritebackOccupiedSlotOutOfRange {
            source: O3PipelineStage::Iew,
            slot: 2,
            writeback_width: 2,
        }
    );
    assert_eq!(
        error.to_string(),
        "O3 IEW writeback occupied slot 2 is out of range for width 2"
    );
    assert_eq!(buffer.pending_deferred_count(), 1);
    let next = buffer.plan_cycle([]);
    assert_eq!(next.admitted_sequences().collect::<Vec<_>>(), vec![3]);
}

#[test]
fn o3_writeback_transfer_checkpoint_payload_round_trips_deferred_state() {
    let policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap();
    let mut buffer = O3WritebackTransferBuffer::new(policy.clone());

    let first = buffer.plan_cycle([
        O3WritebackCompletion::new(30),
        O3WritebackCompletion::new(31),
        O3WritebackCompletion::new(32),
        O3WritebackCompletion::new(33),
    ]);

    assert_eq!(first.admitted_sequences().collect::<Vec<_>>(), vec![30, 31]);
    assert_eq!(first.deferred_sequences().collect::<Vec<_>>(), vec![32, 33]);
    assert_eq!(buffer.pending_deferred_count(), 2);

    let payload = O3WritebackTransferCheckpointPayload::from_buffer(&buffer).unwrap();
    let decoded =
        O3WritebackTransferCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let mut restored = O3WritebackTransferBuffer::from_snapshot(decoded.into_snapshot()).unwrap();

    assert_eq!(restored.policy(), &policy);
    assert_eq!(restored.pending_deferred_count(), 2);

    let second = restored.plan_cycle([O3WritebackCompletion::new(34)]);

    assert_eq!(second.deferred_before_count(), 2);
    assert_eq!(
        second.admitted_sequences().collect::<Vec<_>>(),
        vec![32, 33]
    );
    assert_eq!(second.deferred_sequences().collect::<Vec<_>>(), vec![34]);
    assert_eq!(restored.pending_deferred_count(), 1);
}

#[test]
fn o3_writeback_transfer_checkpoint_payload_rejects_malformed_bytes() {
    let policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap();
    let payload = O3WritebackTransferCheckpointPayload::from_snapshot(
        rem6_cpu::O3WritebackTransferSnapshot::new(policy, [O3WritebackCompletion::new(41)]),
    )
    .unwrap()
    .encode();

    assert_eq!(
        O3WritebackTransferCheckpointPayload::decode(b"bad").unwrap_err(),
        O3PipelineError::InvalidCheckpointPayloadSize {
            expected: O3_WRITEBACK_CHECKPOINT_HEADER_BYTES,
            actual: 3,
        }
    );

    let mut invalid_magic = payload.clone();
    invalid_magic[O3_WRITEBACK_CHECKPOINT_MAGIC_OFFSET] = b'X';
    assert_eq!(
        O3WritebackTransferCheckpointPayload::decode(&invalid_magic).unwrap_err(),
        O3PipelineError::InvalidCheckpointMagic
    );

    let mut unsupported_version = payload.clone();
    unsupported_version[O3_WRITEBACK_CHECKPOINT_VERSION_OFFSET] = 2;
    assert_eq!(
        O3WritebackTransferCheckpointPayload::decode(&unsupported_version).unwrap_err(),
        O3PipelineError::UnsupportedCheckpointVersion { version: 2 }
    );

    let mut invalid_stage = payload.clone();
    invalid_stage[O3_WRITEBACK_CHECKPOINT_STAGE_OFFSET] = 99;
    assert_eq!(
        O3WritebackTransferCheckpointPayload::decode(&invalid_stage).unwrap_err(),
        O3PipelineError::InvalidCheckpointStageCode { code: 99 }
    );

    let mut invalid_width = payload.clone();
    invalid_width[O3_WRITEBACK_CHECKPOINT_WIDTH_OFFSET..O3_WRITEBACK_CHECKPOINT_WIDTH_OFFSET + 4]
        .copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        O3WritebackTransferCheckpointPayload::decode(&invalid_width).unwrap_err(),
        O3PipelineError::ZeroWritebackWidth {
            source: O3PipelineStage::Iew,
        }
    );

    let mut missing_completion = payload.clone();
    missing_completion.truncate(O3_WRITEBACK_CHECKPOINT_HEADER_BYTES);
    assert_eq!(
        O3WritebackTransferCheckpointPayload::decode(&missing_completion).unwrap_err(),
        O3PipelineError::InvalidCheckpointPayloadSize {
            expected: O3_WRITEBACK_CHECKPOINT_HEADER_BYTES + 8,
            actual: O3_WRITEBACK_CHECKPOINT_HEADER_BYTES,
        }
    );

    let mut trailing_payload = payload.clone();
    trailing_payload.push(0);
    assert_eq!(
        O3WritebackTransferCheckpointPayload::decode(&trailing_payload).unwrap_err(),
        O3PipelineError::InvalidCheckpointPayloadSize {
            expected: O3_WRITEBACK_CHECKPOINT_HEADER_BYTES + 8,
            actual: O3_WRITEBACK_CHECKPOINT_HEADER_BYTES + 9,
        }
    );
}

#[test]
fn o3_writeback_transfer_checkpoint_payload_rejects_deferred_count_that_cannot_fit_payload() {
    let policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap();
    let mut payload = O3WritebackTransferCheckpointPayload::from_snapshot(
        rem6_cpu::O3WritebackTransferSnapshot::new(policy, []),
    )
    .unwrap()
    .encode();
    payload[O3_WRITEBACK_CHECKPOINT_DEFERRED_COUNT_OFFSET
        ..O3_WRITEBACK_CHECKPOINT_DEFERRED_COUNT_OFFSET + 4]
        .copy_from_slice(&u32::MAX.to_le_bytes());

    let expected = (u32::MAX as usize)
        .checked_mul(8)
        .and_then(|bytes| O3_WRITEBACK_CHECKPOINT_HEADER_BYTES.checked_add(bytes))
        .unwrap_or(O3_WRITEBACK_CHECKPOINT_HEADER_BYTES);
    assert_eq!(
        O3WritebackTransferCheckpointPayload::decode(&payload).unwrap_err(),
        O3PipelineError::InvalidCheckpointPayloadSize {
            expected,
            actual: O3_WRITEBACK_CHECKPOINT_HEADER_BYTES,
        }
    );
}

#[test]
fn o3_pending_state_checkpoint_payload_round_trips_issue_dependencies_and_writeback() {
    let queue = O3IssueQueueId::new(0);
    let resolved_scope = O3DependencyScopeId::new(0xfeed);
    let produced_scope = O3DependencyScopeId::new(0xbeef);
    let writeback_policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap();
    let mut writeback = O3WritebackTransferBuffer::new(writeback_policy);

    let first_writeback = writeback.plan_cycle([
        O3WritebackCompletion::new(50),
        O3WritebackCompletion::new(51),
        O3WritebackCompletion::new(52),
    ]);

    assert_eq!(
        first_writeback.admitted_sequences().collect::<Vec<_>>(),
        vec![50, 51]
    );
    assert_eq!(
        first_writeback.deferred_sequences().collect::<Vec<_>>(),
        vec![52]
    );

    let snapshot = O3PendingStateSnapshot::new(
        [resolved_scope],
        [
            O3ScopedReadyInstruction::new(21, queue, O3IssueOpClass::IntAlu)
                .with_waits_on([resolved_scope])
                .with_produces([produced_scope]),
            O3ScopedReadyInstruction::new(22, queue, O3IssueOpClass::IntAlu)
                .with_waits_on([produced_scope]),
        ],
        writeback.snapshot(),
    )
    .unwrap();
    let payload = O3PendingStateCheckpointPayload::from_snapshot(snapshot).unwrap();
    let decoded = O3PendingStateCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = decoded.snapshot();

    assert_eq!(restored.resolved_dependency_scopes(), &[resolved_scope]);
    assert_eq!(
        restored
            .ready()
            .iter()
            .map(|instruction| instruction.sequence())
            .collect::<Vec<_>>(),
        vec![21, 22]
    );
    assert_eq!(restored.ready()[0].waits_on(), &[resolved_scope]);
    assert_eq!(restored.ready()[0].produces(), &[produced_scope]);

    let scheduler = O3ScopedIssueScheduler::new(
        2,
        [O3IssueQueueCapacity::new(queue, O3IssueOpClass::IntAlu, 2).unwrap()],
    )
    .unwrap();
    let issue_plan = scheduler.plan(
        restored.resolved_dependency_scopes().iter().copied(),
        restored.ready().iter().cloned(),
    );

    assert_eq!(issue_plan.issued_sequences().collect::<Vec<_>>(), vec![21]);
    assert_eq!(issue_plan.dependency_blocked()[0].sequence(), 22);

    let mut restored_writeback =
        O3WritebackTransferBuffer::from_snapshot(restored.writeback().clone()).unwrap();
    let second_writeback = restored_writeback.plan_cycle([O3WritebackCompletion::new(53)]);

    assert_eq!(second_writeback.deferred_before_count(), 1);
    assert_eq!(
        second_writeback.admitted_sequences().collect::<Vec<_>>(),
        vec![52, 53]
    );
}

#[test]
fn o3_pending_state_checkpoint_payload_rejects_malformed_bytes() {
    let payload = O3PendingStateCheckpointPayload::from_snapshot(
        O3PendingStateSnapshot::new(
            [],
            [O3ScopedReadyInstruction::new(
                10,
                O3IssueQueueId::new(0),
                O3IssueOpClass::IntAlu,
            )],
            rem6_cpu::O3WritebackTransferSnapshot::new(
                O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 1, 0).unwrap(),
                [O3WritebackCompletion::new(11)],
            ),
        )
        .unwrap(),
    )
    .unwrap()
    .encode();

    assert_eq!(
        O3PendingStateCheckpointPayload::decode(b"bad").unwrap_err(),
        O3PipelineError::InvalidCheckpointPayloadSize {
            expected: O3_PENDING_CHECKPOINT_HEADER_BYTES,
            actual: 3,
        }
    );

    let mut invalid_magic = payload.clone();
    invalid_magic[O3_PENDING_CHECKPOINT_MAGIC_OFFSET] = b'X';
    assert_eq!(
        O3PendingStateCheckpointPayload::decode(&invalid_magic).unwrap_err(),
        O3PipelineError::InvalidCheckpointMagic
    );

    let mut unsupported_version = payload.clone();
    unsupported_version[O3_PENDING_CHECKPOINT_VERSION_OFFSET] = 2;
    assert_eq!(
        O3PendingStateCheckpointPayload::decode(&unsupported_version).unwrap_err(),
        O3PipelineError::UnsupportedCheckpointVersion { version: 2 }
    );

    let mut truncated = payload.clone();
    truncated.pop();
    assert_eq!(
        O3PendingStateCheckpointPayload::decode(&truncated).unwrap_err(),
        O3PipelineError::InvalidCheckpointPayloadSize {
            expected: payload.len(),
            actual: payload.len() - 1,
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

#[test]
fn scoped_issue_full_reservation_still_classifies_dependency_state() {
    let queue = O3IssueQueueId::new(1);
    let scope = O3DependencyScopeId::new(10);
    let scheduler = O3ScopedIssueScheduler::new(
        2,
        [O3IssueQueueCapacity::new(queue, O3IssueOpClass::IntAlu, 2).unwrap()],
    )
    .unwrap();
    let resolved = O3ScopedReadyInstruction::new(1, queue, O3IssueOpClass::IntAlu);
    let unresolved =
        O3ScopedReadyInstruction::new(2, queue, O3IssueOpClass::IntAlu).with_waits_on([scope]);
    let plan = scheduler
        .try_plan_with_reserved_width(2, [], [resolved.clone(), unresolved.clone()])
        .unwrap();
    assert_eq!(plan.issue_width(), 2);
    assert_eq!(plan.reserved_width(), 2);
    assert_eq!(plan.available_width(), 0);
    assert!(plan.issued().is_empty());
    assert_eq!(plan.resource_blocked(), &[resolved]);
    assert_eq!(plan.dependency_blocked(), &[unresolved]);
}

#[test]
fn scoped_issue_partial_reservation_limits_selected_rows() {
    let queue = O3IssueQueueId::new(1);
    let scheduler = O3ScopedIssueScheduler::new(
        4,
        [O3IssueQueueCapacity::new(queue, O3IssueOpClass::IntAlu, 4).unwrap()],
    )
    .unwrap();
    let ready = [3, 1, 2]
        .into_iter()
        .map(|sequence| O3ScopedReadyInstruction::new(sequence, queue, O3IssueOpClass::IntAlu));
    let plan = scheduler.plan_with_reserved_width(2, [], ready);
    assert_eq!(plan.available_width(), 2);
    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(
        plan.resource_blocked()
            .iter()
            .map(|row| row.sequence())
            .collect::<Vec<_>>(),
        vec![3]
    );
}

#[test]
fn scoped_issue_rejects_reservation_above_configured_width() {
    let scheduler =
        O3ScopedIssueScheduler::new(2, std::iter::empty::<O3IssueQueueCapacity>()).unwrap();
    let error = scheduler
        .try_plan_with_reserved_width(
            3,
            std::iter::empty::<O3DependencyScopeId>(),
            std::iter::empty::<O3ScopedReadyInstruction>(),
        )
        .unwrap_err();
    assert_eq!(
        error,
        O3PipelineError::ReservedIssueWidthExceedsConfigured {
            reserved_width: 3,
            issue_width: 2,
        }
    );
    assert_eq!(
        error.to_string(),
        "O3 reserved issue width 3 exceeds configured width 2"
    );
}

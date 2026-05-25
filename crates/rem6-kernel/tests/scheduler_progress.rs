use rem6_kernel::{
    LivelockTransitionKind, PartitionId, PartitionedScheduler, SchedulerError, WaitForNode,
};

fn component(name: &str) -> WaitForNode {
    WaitForNode::component(name).unwrap()
}

#[test]
fn scheduler_parallel_run_records_worker_progress_transitions() {
    let subject = component("coherence-ring");
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(3, 4, 3).unwrap();
    let core0 = PartitionId::new(0);
    let core1 = PartitionId::new(1);
    let core2 = PartitionId::new(2);

    let core0_subject = subject.clone();
    scheduler
        .schedule_parallel_at(core0, 0, move |context| {
            context.record_progress_transition(
                core0_subject.clone(),
                LivelockTransitionKind::ProtocolRetry,
            );
            context.record_progress_transition(
                core0_subject,
                LivelockTransitionKind::ResourceArbitration,
            );
        })
        .unwrap();
    let core1_subject = subject.clone();
    scheduler
        .schedule_parallel_at(core1, 0, move |context| {
            context
                .record_progress_transition(core1_subject, LivelockTransitionKind::ProtocolRetry);
        })
        .unwrap();
    scheduler.schedule_parallel_at(core2, 0, |_| {}).unwrap();

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();

    assert_eq!(run.progress_transition_count(), 3);
    assert_eq!(
        run.progress_transition_count_by_kind(LivelockTransitionKind::ProtocolRetry),
        2,
    );
    assert_eq!(
        run.progress_transition_count_by_kind(LivelockTransitionKind::ResourceArbitration),
        1,
    );
    assert_eq!(run.epochs()[0].progress_transition_count(), 3);
    assert_eq!(
        run.batches()[0].progress_transition_count_for_partition(core0),
        2
    );
    assert_eq!(
        run.batches()[0].progress_transition_count_for_partition(core1),
        1
    );
    assert_eq!(
        run.batches()[0].progress_transition_count_for_partition(core2),
        0
    );

    let transitions = run.progress_transitions();
    assert_eq!(transitions.len(), 3);
    assert_eq!(transitions[0].partition(), core0);
    assert_eq!(transitions[0].subject(), &subject);
    assert_eq!(transitions[0].kind(), LivelockTransitionKind::ProtocolRetry);
    assert_eq!(transitions[0].tick(), 0);
    assert_eq!(transitions[0].order(), 0);
    assert_eq!(transitions[1].partition(), core0);
    assert_eq!(
        transitions[1].kind(),
        LivelockTransitionKind::ResourceArbitration,
    );
    assert_eq!(transitions[1].order(), 1);
    assert_eq!(transitions[2].partition(), core1);
    assert_eq!(transitions[2].kind(), LivelockTransitionKind::ProtocolRetry);

    let snapshot = run.progress_monitor_snapshot(3).unwrap();
    assert!(snapshot.has_livelock());
    assert_eq!(snapshot.diagnostics().len(), 1);
    let diagnostic = &snapshot.diagnostics()[0];
    assert_eq!(diagnostic.subject(), &subject);
    assert_eq!(diagnostic.transition_count(), 3);
    assert_eq!(
        diagnostic.transition_count_by_kind(LivelockTransitionKind::ProtocolRetry),
        2,
    );
    assert_eq!(
        diagnostic.transition_count_by_kind(LivelockTransitionKind::ResourceArbitration),
        1,
    );
}

#[test]
fn scheduler_parallel_progress_snapshot_rejects_zero_threshold() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(1, 4).unwrap();
    let core = PartitionId::new(0);
    let subject = component("core0");

    scheduler
        .schedule_parallel_at(core, 0, move |context| {
            context.record_progress_transition(subject, LivelockTransitionKind::SchedulerEpoch);
        })
        .unwrap();

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();

    assert_eq!(
        run.progress_monitor_snapshot(0),
        Err(rem6_kernel::ProgressMonitorError::ZeroTransitionThreshold),
    );
}

#[test]
fn scheduler_progress_transition_order_is_source_local_across_epochs() {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(1, 4, 1).unwrap();
    let core = PartitionId::new(0);
    let first_subject = component("core0-retry");
    let second_subject = component("core0-arbitration");

    scheduler
        .schedule_parallel_at(core, 0, move |context| {
            context
                .record_progress_transition(first_subject, LivelockTransitionKind::ProtocolRetry);
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(core, 5, move |context| {
            context.record_progress_transition(
                second_subject,
                LivelockTransitionKind::ResourceArbitration,
            );
        })
        .unwrap();

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();
    let transitions = run.progress_transitions();

    assert_eq!(transitions.len(), 2);
    assert_eq!(transitions[0].partition(), core);
    assert_eq!(transitions[0].tick(), 0);
    assert_eq!(transitions[0].order(), 0);
    assert_eq!(transitions[1].partition(), core);
    assert_eq!(transitions[1].tick(), 5);
    assert_eq!(transitions[1].order(), 1);
}

#[test]
fn quiescent_snapshot_preserves_progress_transition_order_identity() {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(1, 4, 1).unwrap();
    let core = PartitionId::new(0);
    let first_subject = component("core0-retry");
    let second_subject = component("core0-arbitration");

    scheduler
        .schedule_parallel_at(core, 0, move |context| {
            context
                .record_progress_transition(first_subject, LivelockTransitionKind::ProtocolRetry);
        })
        .unwrap();
    scheduler.run_until_idle_parallel_recorded().unwrap();

    let snapshot = scheduler.quiescent_snapshot().unwrap();
    let mut restored = PartitionedScheduler::with_parallel_worker_limit(1, 4, 1).unwrap();
    restored.restore_quiescent(&snapshot).unwrap();
    restored
        .schedule_parallel_at(core, restored.now(), move |context| {
            context.record_progress_transition(
                second_subject,
                LivelockTransitionKind::ResourceArbitration,
            );
        })
        .unwrap();

    let run = restored.run_until_idle_parallel_recorded().unwrap();
    let transitions = run.progress_transitions();

    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].partition(), core);
    assert_eq!(transitions[0].order(), 1);
}

#[test]
fn scheduler_parallel_worker_panic_returns_typed_error_after_progress_record() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(1, 4).unwrap();
    let core = PartitionId::new(0);
    let subject = component("core0");

    scheduler
        .schedule_parallel_at(core, 0, move |context| {
            context.record_progress_transition(subject, LivelockTransitionKind::SchedulerEpoch);
            panic!("worker panic sentinel");
        })
        .unwrap();

    assert_eq!(
        scheduler.run_next_epoch_parallel_recorded(),
        Err(SchedulerError::ParallelWorkerPanicked { partition: core }),
    );
}

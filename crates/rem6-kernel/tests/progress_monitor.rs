use rem6_kernel::{LivelockTransitionKind, ProgressMonitor, ProgressMonitorError, WaitForNode};

fn component(name: &str) -> WaitForNode {
    WaitForNode::component(name).unwrap()
}

#[test]
fn progress_monitor_reports_livelock_after_repeated_transitions_without_work() {
    let cache = component("l1d0");
    let directory = component("dir0");
    let mut monitor = ProgressMonitor::with_transition_threshold(3).unwrap();

    monitor
        .record_transition(cache.clone(), LivelockTransitionKind::ProtocolRetry, 10)
        .unwrap();
    monitor
        .record_transition(directory.clone(), LivelockTransitionKind::QueueRotation, 11)
        .unwrap();
    monitor
        .record_transition(cache.clone(), LivelockTransitionKind::QueueRotation, 12)
        .unwrap();
    assert_eq!(monitor.diagnostic(&cache), None);
    monitor
        .record_transition(cache.clone(), LivelockTransitionKind::ProtocolRetry, 14)
        .unwrap();

    let diagnostic = monitor.diagnostic(&cache).unwrap();
    assert_eq!(diagnostic.subject(), &cache);
    assert_eq!(diagnostic.threshold(), 3);
    assert_eq!(diagnostic.transition_count(), 3);
    assert_eq!(diagnostic.first_transition_tick(), 10);
    assert_eq!(diagnostic.last_transition_tick(), 14);
    assert_eq!(
        diagnostic.transition_kinds(),
        &[
            LivelockTransitionKind::ProtocolRetry,
            LivelockTransitionKind::QueueRotation,
        ],
    );
    assert_eq!(
        diagnostic.transition_count_by_kind(LivelockTransitionKind::ProtocolRetry),
        2,
    );
    assert_eq!(
        diagnostic.transition_count_by_kind(LivelockTransitionKind::QueueRotation),
        1,
    );
    let kind_counts = diagnostic.transition_kind_counts();
    assert_eq!(kind_counts[0].kind(), LivelockTransitionKind::ProtocolRetry);
    assert_eq!(kind_counts[0].count(), 2);
    assert_eq!(kind_counts[0].first_transition_tick(), 10);
    assert_eq!(kind_counts[0].last_transition_tick(), 14);
    assert_eq!(kind_counts[1].kind(), LivelockTransitionKind::QueueRotation);
    assert_eq!(kind_counts[1].count(), 1);
    assert_eq!(kind_counts[1].first_transition_tick(), 12);
    assert_eq!(kind_counts[1].last_transition_tick(), 12);
    assert_eq!(monitor.diagnostics(), vec![diagnostic.clone()]);

    let snapshot = monitor.snapshot();
    assert!(snapshot.has_livelock());
    assert_eq!(snapshot.diagnostics(), &[diagnostic]);
    assert_eq!(snapshot.window_count(), 2);
    assert_eq!(snapshot.transition_count(&cache), Some(3));
    assert_eq!(snapshot.transition_count(&directory), Some(1));
}

#[test]
fn progress_monitor_useful_work_resets_progress_free_window() {
    let core = component("core0");
    let mut monitor = ProgressMonitor::with_transition_threshold(2).unwrap();

    monitor
        .record_transition(core.clone(), LivelockTransitionKind::SchedulerEpoch, 2)
        .unwrap();
    assert!(monitor.record_useful_work(&core, 3));
    monitor
        .record_transition(core.clone(), LivelockTransitionKind::SchedulerEpoch, 4)
        .unwrap();
    assert_eq!(monitor.diagnostic(&core), None);

    monitor
        .record_transition(core.clone(), LivelockTransitionKind::MessageRetry, 5)
        .unwrap();
    let diagnostic = monitor.diagnostic(&core).unwrap();
    assert_eq!(diagnostic.transition_count(), 2);
    assert_eq!(diagnostic.first_transition_tick(), 4);
    assert_eq!(diagnostic.last_useful_tick(), Some(3));
}

#[test]
fn progress_monitor_rejects_zero_threshold_without_state_mutation() {
    assert_eq!(
        ProgressMonitor::with_transition_threshold(0),
        Err(ProgressMonitorError::ZeroTransitionThreshold),
    );
}

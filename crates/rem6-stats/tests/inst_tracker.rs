use rem6_stats::{
    GlobalInstTracker, GlobalInstTrackerSnapshot, InstTrackerUpdate, LocalInstTracker,
    ProbePayload, ProbeRegistry, StatsError,
};

#[test]
fn global_inst_tracker_counts_retired_events_and_reports_thresholds_once() {
    let mut tracker = GlobalInstTracker::new(vec![2, 3, 2]);

    assert_eq!(tracker.counter(), 0);
    assert_eq!(tracker.thresholds(), &[2, 3]);

    assert_eq!(tracker.record_retired_inst().unwrap(), None);
    assert_eq!(tracker.counter(), 1);

    assert_eq!(
        tracker.record_retired_inst().unwrap(),
        Some(InstTrackerUpdate::new(2, true))
    );
    assert_eq!(tracker.thresholds(), &[3]);

    assert_eq!(
        tracker.record_retired_inst().unwrap(),
        Some(InstTrackerUpdate::new(3, false))
    );
    assert_eq!(tracker.thresholds(), &[]);

    assert_eq!(tracker.record_retired_inst().unwrap(), None);
    assert_eq!(tracker.counter(), 4);

    tracker.reset_counter();
    assert_eq!(tracker.counter(), 0);
    assert_eq!(tracker.thresholds(), &[]);

    tracker.add_threshold(2);
    assert_eq!(tracker.record_retired_inst().unwrap(), None);
    assert_eq!(
        tracker.record_retired_inst().unwrap(),
        Some(InstTrackerUpdate::new(2, false))
    );

    tracker.add_threshold(4);
    tracker.reset_thresholds();
    assert_eq!(tracker.thresholds(), &[]);
    assert_eq!(tracker.record_retired_inst().unwrap(), None);
}

#[test]
fn local_inst_tracker_consumes_matching_probe_events_only_while_listening() {
    let mut probes = ProbeRegistry::new();
    let retired = probes.register_point("cpu0", "RetiredInsts").unwrap();
    let committed = probes.register_point("cpu0", "CommittedInsts").unwrap();
    probes.add_listener(retired, "inst_tracker").unwrap();
    probes.add_listener(committed, "inst_tracker").unwrap();

    let mut global = GlobalInstTracker::new(vec![2]);
    let mut local = LocalInstTracker::new(false);

    let ignored_while_stopped = probes
        .emit(1, retired, ProbePayload::Counter { amount: 8 })
        .unwrap();
    assert_eq!(
        local
            .observe_retired_insts_probe_event(ignored_while_stopped, retired, &mut global)
            .unwrap(),
        None
    );
    assert_eq!(global.counter(), 0);

    local.start_listening();
    let wrong_point = probes
        .emit(2, committed, ProbePayload::Counter { amount: 1 })
        .unwrap();
    assert_eq!(
        local
            .observe_retired_insts_probe_event(wrong_point, retired, &mut global)
            .unwrap(),
        None
    );
    assert_eq!(global.counter(), 0);

    let wrong_payload = probes.emit(3, retired, ProbePayload::Unit).unwrap();
    assert_eq!(
        local
            .observe_retired_insts_probe_event(wrong_payload, retired, &mut global)
            .unwrap(),
        None
    );
    assert_eq!(global.counter(), 0);

    let first_retired = probes
        .emit(4, retired, ProbePayload::Counter { amount: 8 })
        .unwrap();
    assert_eq!(
        local
            .observe_retired_insts_probe_event(first_retired, retired, &mut global)
            .unwrap(),
        None
    );
    assert_eq!(global.counter(), 1);

    let second_retired = probes
        .emit(5, retired, ProbePayload::Counter { amount: 8 })
        .unwrap();
    assert_eq!(
        local
            .observe_retired_insts_probe_event(second_retired, retired, &mut global)
            .unwrap(),
        Some(InstTrackerUpdate::new(2, false))
    );
    assert_eq!(global.counter(), 2);

    local.stop_listening();
    let stopped_again = probes
        .emit(6, retired, ProbePayload::Counter { amount: 1 })
        .unwrap();
    assert_eq!(
        local
            .observe_retired_insts_probe_event(stopped_again, retired, &mut global)
            .unwrap(),
        None
    );
    assert_eq!(global.counter(), 2);
}

#[test]
fn global_inst_tracker_snapshot_round_trips_pending_thresholds() {
    let mut tracker = GlobalInstTracker::new(vec![2, 4]);

    assert_eq!(tracker.record_retired_inst().unwrap(), None);
    assert_eq!(
        tracker.record_retired_inst().unwrap(),
        Some(InstTrackerUpdate::new(2, true))
    );

    let snapshot = tracker.snapshot();
    assert_eq!(snapshot, GlobalInstTrackerSnapshot::new(2, vec![4]));

    let mut restored = GlobalInstTracker::from_snapshot(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    assert_eq!(restored.record_retired_inst().unwrap(), None);
    assert_eq!(
        restored.record_retired_inst().unwrap(),
        Some(InstTrackerUpdate::new(4, false))
    );
}

#[test]
fn global_inst_tracker_snapshot_rejects_ambiguous_threshold_state() {
    assert_eq!(
        GlobalInstTracker::from_snapshot(&GlobalInstTrackerSnapshot::new(3, vec![4, 4]))
            .unwrap_err(),
        StatsError::DuplicateInstThreshold { threshold: 4 }
    );
    assert_eq!(
        GlobalInstTracker::from_snapshot(&GlobalInstTrackerSnapshot::new(3, vec![2])).unwrap_err(),
        StatsError::UnreachableInstThreshold {
            threshold: 2,
            counter: 3,
        }
    );
    assert_eq!(
        GlobalInstTracker::from_snapshot(&GlobalInstTrackerSnapshot::new(3, vec![3])).unwrap_err(),
        StatsError::UnreachableInstThreshold {
            threshold: 3,
            counter: 3,
        }
    );

    assert_eq!(
        GlobalInstTracker::from_snapshot(&GlobalInstTrackerSnapshot::new(u64::MAX, Vec::new()))
            .unwrap()
            .record_retired_inst()
            .unwrap_err(),
        StatsError::InstTrackerCounterOverflow
    );
}

use rem6_stats::{
    PcCountPair, PcCountTracker, PcCountTrackerManager, PcCountTrackerSnapshot,
    PcCountTrackerUpdate, ProbePayload, ProbeRegistry, StatsError,
};

fn targets() -> Vec<PcCountPair> {
    vec![
        PcCountPair::new(0x1000, 2),
        PcCountPair::new(0x1000, 3),
        PcCountPair::new(0x2000, 1),
    ]
}

#[test]
fn pc_count_tracker_filters_retired_pcs_and_reports_target_pairs_once() {
    let mut manager = PcCountTrackerManager::new(targets());
    let tracker = PcCountTracker::new(targets());

    assert_eq!(tracker.observe_retired_pc(0x3000, &mut manager), None);
    assert_eq!(manager.pc_count(0x3000), None);
    assert_eq!(manager.pc_count(0x1000), Some(0));

    assert_eq!(
        tracker.observe_retired_pc(0x2000, &mut manager),
        Some(PcCountTrackerUpdate::new(PcCountPair::new(0x2000, 1), true))
    );
    assert_eq!(manager.current_pair(), PcCountPair::new(0x2000, 1));

    assert_eq!(tracker.observe_retired_pc(0x1000, &mut manager), None);
    assert_eq!(manager.pc_count(0x1000), Some(1));
    assert_eq!(
        tracker.observe_retired_pc(0x1000, &mut manager),
        Some(PcCountTrackerUpdate::new(PcCountPair::new(0x1000, 2), true))
    );
    assert_eq!(
        tracker.observe_retired_pc(0x1000, &mut manager),
        Some(PcCountTrackerUpdate::new(
            PcCountPair::new(0x1000, 3),
            false
        ))
    );
    assert!(!manager.is_armed());

    assert_eq!(tracker.observe_retired_pc(0x1000, &mut manager), None);
    assert_eq!(manager.pc_count(0x1000), Some(3));
}

#[test]
fn pc_count_tracker_snapshot_round_trips_counter_state_and_pending_targets() {
    let mut manager = PcCountTrackerManager::new(targets());
    let tracker = PcCountTracker::new(targets());

    assert_eq!(tracker.observe_retired_pc(0x1000, &mut manager), None);
    assert_eq!(
        tracker.observe_retired_pc(0x2000, &mut manager),
        Some(PcCountTrackerUpdate::new(PcCountPair::new(0x2000, 1), true))
    );

    let snapshot = manager.snapshot();
    assert_eq!(
        snapshot,
        PcCountTrackerSnapshot::new(
            vec![(0x1000, 1), (0x2000, 1)],
            vec![PcCountPair::new(0x1000, 2), PcCountPair::new(0x1000, 3)],
            PcCountPair::new(0x2000, 1),
            true,
        )
    );

    let mut restored = PcCountTrackerManager::from_snapshot(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(
        tracker.observe_retired_pc(0x1000, &mut restored),
        Some(PcCountTrackerUpdate::new(PcCountPair::new(0x1000, 2), true))
    );
    assert_eq!(
        tracker.observe_retired_pc(0x1000, &mut restored),
        Some(PcCountTrackerUpdate::new(
            PcCountPair::new(0x1000, 3),
            false
        ))
    );
    assert!(!restored.is_armed());
}

#[test]
fn pc_count_tracker_snapshot_rejects_ambiguous_counter_state() {
    assert_eq!(
        PcCountTrackerManager::from_snapshot(&PcCountTrackerSnapshot::new(
            vec![(0x1000, 1), (0x1000, 2)],
            Vec::new(),
            PcCountPair::new(0, 0),
            false,
        ))
        .unwrap_err(),
        StatsError::DuplicatePcCountCounter { pc: 0x1000 }
    );
    assert_eq!(
        PcCountTrackerManager::from_snapshot(&PcCountTrackerSnapshot::new(
            vec![(0x1000, 1)],
            vec![PcCountPair::new(0x2000, 1)],
            PcCountPair::new(0, 0),
            true,
        ))
        .unwrap_err(),
        StatsError::MissingPcCountCounter { pc: 0x2000 }
    );
    assert_eq!(
        PcCountTrackerManager::from_snapshot(&PcCountTrackerSnapshot::new(
            vec![(0x1000, 1)],
            vec![PcCountPair::new(0x1000, 2), PcCountPair::new(0x1000, 2)],
            PcCountPair::new(0, 0),
            true,
        ))
        .unwrap_err(),
        StatsError::DuplicatePcCountTarget {
            pair: PcCountPair::new(0x1000, 2),
        }
    );
    assert_eq!(
        PcCountTrackerManager::from_snapshot(&PcCountTrackerSnapshot::new(
            vec![(0x1000, 1)],
            Vec::new(),
            PcCountPair::new(0, 0),
            true,
        ))
        .unwrap_err(),
        StatsError::PcCountSnapshotTargetStateMismatch {
            armed: true,
            pending_targets: 0,
        }
    );
    assert_eq!(
        PcCountTrackerManager::from_snapshot(&PcCountTrackerSnapshot::new(
            vec![(0x1000, 1)],
            vec![PcCountPair::new(0x1000, 2)],
            PcCountPair::new(0, 0),
            false,
        ))
        .unwrap_err(),
        StatsError::PcCountSnapshotTargetStateMismatch {
            armed: false,
            pending_targets: 1,
        }
    );
    assert_eq!(
        PcCountTrackerManager::from_snapshot(&PcCountTrackerSnapshot::new(
            vec![(0x1000, 3)],
            vec![PcCountPair::new(0x1000, 2)],
            PcCountPair::new(0x1000, 3),
            true,
        ))
        .unwrap_err(),
        StatsError::UnreachablePcCountTarget {
            pair: PcCountPair::new(0x1000, 2),
            current_count: 3,
        }
    );
}

#[test]
fn pc_count_tracker_consumes_retired_pc_probe_events() {
    let mut probes = ProbeRegistry::new();
    let retired_pc = probes.register_point("cpu0", "RetiredInstsPC").unwrap();
    let fetch_pc = probes.register_point("cpu0", "FetchPC").unwrap();
    probes.add_listener(retired_pc, "pc_count_tracker").unwrap();
    probes.add_listener(fetch_pc, "pc_count_tracker").unwrap();

    let tracker = PcCountTracker::new(targets());
    let mut manager = PcCountTrackerManager::new(targets());

    let wrong_point = probes
        .emit(9, fetch_pc, ProbePayload::ProgramCounter { pc: 0x2000 })
        .unwrap();
    assert_eq!(
        tracker.observe_retired_pc_probe_event(wrong_point, retired_pc, &mut manager),
        None
    );
    assert_eq!(manager.pc_count(0x2000), Some(0));

    let non_target = probes
        .emit(10, retired_pc, ProbePayload::ProgramCounter { pc: 0x3000 })
        .unwrap();
    assert_eq!(
        tracker.observe_retired_pc_probe_event(non_target, retired_pc, &mut manager),
        None
    );

    let target = probes
        .emit(11, retired_pc, ProbePayload::ProgramCounter { pc: 0x2000 })
        .unwrap();
    assert_eq!(
        tracker.observe_retired_pc_probe_event(target, retired_pc, &mut manager),
        Some(PcCountTrackerUpdate::new(PcCountPair::new(0x2000, 1), true))
    );
    assert_eq!(target.listener_refs()[0].name(), "pc_count_tracker");
}

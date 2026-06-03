use rem6_cache::{
    Dueler, DuelingMonitor, DuelingMonitorConfig, DuelingMonitorError, DuelingRatio, DuelingTeam,
};

fn half() -> DuelingRatio {
    DuelingRatio::new(1, 2).unwrap()
}

#[test]
fn dueler_sets_sampling_bits_and_rejects_invalid_or_duplicate_ids() {
    let mut dueler = Dueler::new();

    for bit in 0..64 {
        assert_eq!(dueler.sample_team(1u64 << bit), None);
    }

    for bit in 0..3 {
        let team = if bit % 2 == 0 {
            DuelingTeam::True
        } else {
            DuelingTeam::False
        };
        dueler.set_sample(1u64 << bit, team).unwrap();
    }

    assert_eq!(dueler.sample_team(1), Some(DuelingTeam::True));
    assert_eq!(dueler.sample_team(2), Some(DuelingTeam::False));
    assert_eq!(dueler.sample_team(4), Some(DuelingTeam::True));
    assert_eq!(dueler.sample_team(8), None);
    assert_eq!(
        dueler.set_sample(0, DuelingTeam::True),
        Err(DuelingMonitorError::InvalidMonitorId { id: 0 })
    );
    assert_eq!(
        dueler.set_sample(3, DuelingTeam::True),
        Err(DuelingMonitorError::InvalidMonitorId { id: 3 })
    );
    assert_eq!(
        dueler.set_sample(1, DuelingTeam::False),
        Err(DuelingMonitorError::DuplicateSample { id: 1 })
    );
}

#[test]
fn dueling_monitor_assigns_balanced_samples_per_constituency() {
    let config = DuelingMonitorConfig::new(0, 8, 2, 4, half(), half()).unwrap();
    let mut monitor = DuelingMonitor::new(config);
    let mut entries = vec![Dueler::new(); 32];

    for entry in &mut entries {
        monitor.init_entry(entry).unwrap();
    }

    let mut true_samples = 0;
    let mut false_samples = 0;
    for entry in &entries {
        match monitor.sample_team(entry) {
            Some(DuelingTeam::True) => true_samples += 1,
            Some(DuelingTeam::False) => false_samples += 1,
            None => {}
        }
    }

    assert_eq!(true_samples, 8);
    assert_eq!(false_samples, 8);
    assert_eq!(monitor.region_counter(), 0);
}

#[test]
fn dueling_monitor_uses_hysteresis_thresholds_for_winner_selection() {
    let low = DuelingRatio::new(2, 7).unwrap();
    let high = DuelingRatio::new(4, 7).unwrap();
    let config = DuelingMonitorConfig::new(1, 4, 1, 3, low, high).unwrap();
    let mut monitor = DuelingMonitor::new(config);
    let mut entries = vec![Dueler::new(); 4];

    for entry in &mut entries {
        monitor.init_entry(entry).unwrap();
    }
    let false_sample = *entries
        .iter()
        .find(|entry| monitor.sample_team(entry) == Some(DuelingTeam::False))
        .unwrap();
    let true_sample = *entries
        .iter()
        .find(|entry| monitor.sample_team(entry) == Some(DuelingTeam::True))
        .unwrap();
    let follower = *entries
        .iter()
        .find(|entry| monitor.sample_team(entry).is_none())
        .unwrap();

    assert_eq!(monitor.winner(), DuelingTeam::True);
    monitor.sample(&false_sample);
    assert_eq!(monitor.winner(), DuelingTeam::True);
    monitor.sample(&false_sample);
    assert_eq!(monitor.winner(), DuelingTeam::False);

    for _ in 0..20 {
        monitor.sample(&follower);
    }
    assert_eq!(monitor.winner(), DuelingTeam::False);

    monitor.sample(&true_sample);
    assert_eq!(monitor.winner(), DuelingTeam::False);
    monitor.sample(&true_sample);
    assert_eq!(monitor.winner(), DuelingTeam::False);
    monitor.sample(&true_sample);
    assert_eq!(monitor.winner(), DuelingTeam::True);
}

#[test]
fn dueling_monitor_snapshot_restore_preserves_selector_and_entry_masks() {
    let config = DuelingMonitorConfig::new(2, 4, 1, 3, half(), half()).unwrap();
    let mut monitor = DuelingMonitor::new(config.clone());
    let mut entries = vec![Dueler::new(); 4];

    for entry in &mut entries {
        monitor.init_entry(entry).unwrap();
    }
    let false_sample = *entries
        .iter()
        .find(|entry| monitor.sample_team(entry) == Some(DuelingTeam::False))
        .unwrap();
    monitor.sample(&false_sample);
    monitor.sample(&false_sample);

    let monitor_snapshot = monitor.snapshot();
    let entry_snapshots = entries.iter().map(Dueler::snapshot).collect::<Vec<_>>();

    let mut restored = DuelingMonitor::new(config);
    restored.restore(&monitor_snapshot).unwrap();
    let restored_entries = entry_snapshots
        .iter()
        .map(Dueler::from_snapshot)
        .collect::<Vec<_>>();

    assert_eq!(restored.snapshot(), monitor_snapshot);
    for (entry, restored_entry) in entries.iter().zip(restored_entries.iter()) {
        assert_eq!(restored_entry.snapshot(), entry.snapshot());
        assert_eq!(
            restored.sample_team(restored_entry),
            monitor.sample_team(entry)
        );
    }
}

#[test]
fn dueling_monitor_config_rejects_gem5_fatal_shapes() {
    assert_eq!(
        DuelingMonitorConfig::new(64, 4, 1, 3, half(), half()),
        Err(DuelingMonitorError::MonitorIndexOutOfRange { index: 64 })
    );
    assert_eq!(
        DuelingMonitorConfig::new(0, 1, 1, 3, half(), half()),
        Err(DuelingMonitorError::ConstituencyTooSmall {
            constituency_size: 1,
            team_size: 1,
        })
    );
    assert_eq!(
        DuelingMonitorConfig::new(0, 4, 0, 3, half(), half()),
        Err(DuelingMonitorError::ZeroTeamSize)
    );
    assert_eq!(
        DuelingMonitorConfig::new(0, 4, 1, 0, half(), half()),
        Err(DuelingMonitorError::SelectorBitsOutOfRange { bits: 0 })
    );
    assert_eq!(
        DuelingRatio::new(0, 1),
        Err(DuelingMonitorError::ThresholdOutOfRange {
            numerator: 0,
            denominator: 1,
        })
    );
    assert_eq!(
        DuelingRatio::new(1, 1),
        Err(DuelingMonitorError::ThresholdOutOfRange {
            numerator: 1,
            denominator: 1,
        })
    );
    assert_eq!(
        DuelingMonitorConfig::new(
            0,
            4,
            1,
            3,
            DuelingRatio::new(3, 4).unwrap(),
            DuelingRatio::new(1, 4).unwrap(),
        ),
        Err(DuelingMonitorError::LowThresholdAboveHigh)
    );
}

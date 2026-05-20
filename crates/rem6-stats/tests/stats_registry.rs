use rem6_kernel::Tick;
use rem6_stats::{StatId, StatSample, StatSnapshot, StatsError, StatsRegistry, StatsResetRecord};

#[test]
fn stats_registry_snapshots_counters_and_resets_epochs() {
    let mut stats = StatsRegistry::new();
    let insts = stats
        .register_counter("cpu0.committed_insts", "count")
        .unwrap();
    let mem_reads = stats.register_counter("system.mem_reads", "count").unwrap();

    assert_eq!(insts, StatId::new(0));
    assert_eq!(mem_reads, StatId::new(1));

    stats.increment(insts, 12).unwrap();
    stats.increment(mem_reads, 4).unwrap();

    assert_eq!(
        stats.snapshot(10),
        StatSnapshot::new(
            10,
            0,
            0,
            vec![
                StatSample::new(insts, "cpu0.committed_insts", "count", 12),
                StatSample::new(mem_reads, "system.mem_reads", "count", 4),
            ],
        )
    );

    assert_eq!(
        stats.reset(15),
        StatsResetRecord::new(15, 1, vec![(insts, 12), (mem_reads, 4)])
    );
    assert_eq!(
        stats.snapshot(16),
        StatSnapshot::new(
            16,
            1,
            15,
            vec![
                StatSample::new(insts, "cpu0.committed_insts", "count", 0),
                StatSample::new(mem_reads, "system.mem_reads", "count", 0),
            ],
        )
    );

    stats.increment(insts, 3).unwrap();
    assert_eq!(
        stats.snapshot(20),
        StatSnapshot::new(
            20,
            1,
            15,
            vec![
                StatSample::new(insts, "cpu0.committed_insts", "count", 3),
                StatSample::new(mem_reads, "system.mem_reads", "count", 0),
            ],
        )
    );
}

#[test]
fn stats_registry_rejects_duplicate_empty_unknown_and_overflowing_counters() {
    let mut stats = StatsRegistry::new();
    let insts = stats
        .register_counter("cpu0.committed_insts", "count")
        .unwrap();

    assert_eq!(
        stats.register_counter("", "count").unwrap_err(),
        StatsError::EmptyPath
    );
    assert_eq!(
        stats
            .register_counter("cpu0.committed_insts", "count")
            .unwrap_err(),
        StatsError::DuplicatePath {
            path: "cpu0.committed_insts".to_string(),
        }
    );
    assert_eq!(
        stats.increment(StatId::new(99), 1).unwrap_err(),
        StatsError::UnknownStat {
            stat: StatId::new(99),
        }
    );

    stats.increment(insts, u64::MAX).unwrap();
    assert_eq!(
        stats.increment(insts, 1).unwrap_err(),
        StatsError::CounterOverflow { stat: insts }
    );
}

#[test]
fn stats_snapshot_rejects_time_before_last_reset() {
    let mut stats = StatsRegistry::new();
    stats.register_counter("cpu0.cycles", "cycles").unwrap();
    stats.reset(50);

    assert_eq!(
        stats.try_snapshot(49).unwrap_err(),
        StatsError::SnapshotBeforeReset {
            tick: 49 as Tick,
            reset_tick: 50,
        }
    );
}

use rem6_kernel::Tick;
use rem6_stats::{
    ProbeEvent, ProbeListenerId, ProbePayload, ProbePointId, ProbeRegistry, ProbeSnapshot, StatId,
    StatSample, StatSnapshot, StatsError, StatsRegistry, StatsResetRecord,
};

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

#[test]
fn stats_reset_rejects_time_before_last_reset_without_mutating_scope() {
    let mut stats = StatsRegistry::new();
    let cycles = stats.register_counter("cpu0.cycles", "cycles").unwrap();
    stats.increment(cycles, 7).unwrap();
    stats.reset(50);
    stats.increment(cycles, 3).unwrap();

    assert_eq!(
        stats.try_reset(49).unwrap_err(),
        StatsError::ResetBeforeLastReset {
            tick: 49 as Tick,
            reset_tick: 50,
        }
    );
    assert_eq!(stats.epoch(), 1);
    assert_eq!(stats.reset_tick(), 50);
    assert_eq!(
        stats.snapshot(55),
        StatSnapshot::new(
            55,
            1,
            50,
            vec![StatSample::new(cycles, "cpu0.cycles", "cycles", 3)],
        )
    );
}

#[test]
fn probe_registry_records_typed_events_and_listener_state() {
    let mut probes = ProbeRegistry::new();
    let committed = probes.register_point("cpu0", "commit").unwrap();
    let miss = probes.register_point("l1d", "miss").unwrap();

    assert_eq!(committed, ProbePointId::new(0));
    assert_eq!(miss, ProbePointId::new(1));
    assert_eq!(
        probes.add_listener(committed, "commit_counter").unwrap(),
        ProbeListenerId::new(0)
    );
    let trace_listener = probes.add_listener(committed, "commit_trace").unwrap();

    let event = probes
        .emit(10, committed, ProbePayload::Counter { amount: 4 })
        .unwrap()
        .clone();
    assert_eq!(
        event,
        ProbeEvent::new(10, 0, committed, 2, ProbePayload::Counter { amount: 4 })
    );

    probes.remove_listener(committed, trace_listener).unwrap();
    assert_eq!(
        probes
            .emit(11, committed, ProbePayload::Unit)
            .unwrap()
            .listener_count(),
        1
    );

    assert_eq!(
        probes.snapshot(),
        ProbeSnapshot::new(
            vec![
                ("cpu0".to_string(), "commit".to_string(), committed),
                ("l1d".to_string(), "miss".to_string(), miss),
            ],
            vec![(
                "commit_counter".to_string(),
                committed,
                ProbeListenerId::new(0)
            )],
            vec![
                ProbeEvent::new(10, 0, committed, 2, ProbePayload::Counter { amount: 4 }),
                ProbeEvent::new(11, 1, committed, 1, ProbePayload::Unit),
            ],
        )
    );

    assert_eq!(
        probes.events()[0].payload(),
        &ProbePayload::Counter { amount: 4 }
    );
}

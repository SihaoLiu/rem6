use rem6_kernel::Tick;
use rem6_stats::{
    ProbeEvent, ProbeListenerId, ProbePayload, ProbePointId, ProbeRegistry, ProbeSnapshot,
    StatDeltaSample, StatDumpId, StatDumpRecord, StatId, StatPathError, StatSample, StatSnapshot,
    StatSnapshotDelta, StatsError, StatsRegistry, StatsResetRecord,
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
fn stats_registry_rejects_ambiguous_counter_paths_without_consuming_ids() {
    let mut stats = StatsRegistry::new();

    assert_eq!(
        stats
            .register_counter(".cpu0.cycles", "cycles")
            .unwrap_err(),
        StatsError::InvalidPath {
            path: ".cpu0.cycles".to_string(),
            reason: StatPathError::EmptySegment { index: 0 },
        },
    );
    assert_eq!(
        stats
            .register_counter("cpu0..cycles", "cycles")
            .unwrap_err(),
        StatsError::InvalidPath {
            path: "cpu0..cycles".to_string(),
            reason: StatPathError::EmptySegment { index: 1 },
        },
    );
    assert_eq!(
        stats.register_counter("0cpu.cycles", "cycles").unwrap_err(),
        StatsError::InvalidPath {
            path: "0cpu.cycles".to_string(),
            reason: StatPathError::InvalidSegmentStart {
                segment: "0cpu".to_string(),
                character: '0',
            },
        },
    );
    assert_eq!(
        stats
            .register_counter("cpu-0.cycles", "cycles")
            .unwrap_err(),
        StatsError::InvalidPath {
            path: "cpu-0.cycles".to_string(),
            reason: StatPathError::InvalidSegmentCharacter {
                segment: "cpu-0".to_string(),
                character: '-',
            },
        },
    );
    assert_eq!(
        stats
            .register_counter("cpu0.cycles ", "cycles")
            .unwrap_err(),
        StatsError::InvalidPath {
            path: "cpu0.cycles ".to_string(),
            reason: StatPathError::InvalidSegmentCharacter {
                segment: "cycles ".to_string(),
                character: ' ',
            },
        },
    );

    assert_eq!(
        stats.register_counter("cpu0.cycles", "cycles").unwrap(),
        StatId::new(0)
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
fn stats_snapshot_derives_scope_checked_counter_deltas() {
    let mut stats = StatsRegistry::new();
    let insts = stats
        .register_counter("cpu0.committed_insts", "count")
        .unwrap();
    let mem_reads = stats.register_counter("system.mem_reads", "count").unwrap();
    stats.increment(insts, 20).unwrap();
    stats.increment(mem_reads, 5).unwrap();
    let previous = stats.snapshot(100);
    stats.increment(insts, 12).unwrap();
    stats.increment(mem_reads, 2).unwrap();
    let current = stats.snapshot(140);

    assert_eq!(
        current.delta_since(&previous).unwrap(),
        StatSnapshotDelta::new(
            100,
            140,
            0,
            0,
            vec![
                StatDeltaSample::new(insts, "cpu0.committed_insts", "count", 20, 32),
                StatDeltaSample::new(mem_reads, "system.mem_reads", "count", 5, 7),
            ],
        )
    );

    stats.reset(150);
    stats.increment(insts, 3).unwrap();
    let reset_scope = stats.snapshot(170);
    assert_eq!(
        reset_scope.delta_since(&current).unwrap_err(),
        StatsError::SnapshotDeltaScopeMismatch {
            previous_epoch: 0,
            current_epoch: 1,
            previous_reset_tick: 0,
            current_reset_tick: 150,
        },
    );
    assert_eq!(
        previous.delta_since(&current).unwrap_err(),
        StatsError::SnapshotDeltaTimeWentBack {
            previous_tick: 140,
            current_tick: 100,
        },
    );
}

#[test]
fn stats_snapshot_delta_rejects_counter_regression() {
    let stat = StatId::new(7);
    let previous = StatSnapshot::new(
        10,
        0,
        0,
        vec![StatSample::new(stat, "cpu0.committed_insts", "count", 12)],
    );
    let current = StatSnapshot::new(
        20,
        0,
        0,
        vec![StatSample::new(stat, "cpu0.committed_insts", "count", 9)],
    );

    assert_eq!(
        current.delta_since(&previous).unwrap_err(),
        StatsError::SnapshotDeltaValueWentBack {
            stat,
            previous: 12,
            current: 9,
        },
    );
}

#[test]
fn stats_snapshot_delta_rejects_schema_drift() {
    let committed = StatId::new(7);
    let misses = StatId::new(8);
    let previous = StatSnapshot::new(
        10,
        0,
        0,
        vec![StatSample::new(
            committed,
            "cpu0.committed_insts",
            "count",
            12,
        )],
    );

    let extra_current = StatSnapshot::new(
        20,
        0,
        0,
        vec![
            StatSample::new(committed, "cpu0.committed_insts", "count", 15),
            StatSample::new(misses, "system.l2.misses", "count", 1),
        ],
    );
    assert_eq!(
        extra_current.delta_since(&previous).unwrap_err(),
        StatsError::SnapshotDeltaUnexpectedStat { stat: misses },
    );

    let renamed_current = StatSnapshot::new(
        20,
        0,
        0,
        vec![StatSample::new(
            committed,
            "cpu0.retired_insts",
            "count",
            15,
        )],
    );
    assert_eq!(
        renamed_current.delta_since(&previous).unwrap_err(),
        StatsError::SnapshotDeltaDescriptorMismatch {
            stat: committed,
            previous_path: "cpu0.committed_insts".to_string(),
            current_path: "cpu0.retired_insts".to_string(),
            previous_unit: "count".to_string(),
            current_unit: "count".to_string(),
        },
    );

    let unit_changed_current = StatSnapshot::new(
        20,
        0,
        0,
        vec![StatSample::new(
            committed,
            "cpu0.committed_insts",
            "ops",
            15,
        )],
    );
    assert_eq!(
        unit_changed_current.delta_since(&previous).unwrap_err(),
        StatsError::SnapshotDeltaDescriptorMismatch {
            stat: committed,
            previous_path: "cpu0.committed_insts".to_string(),
            current_path: "cpu0.committed_insts".to_string(),
            previous_unit: "count".to_string(),
            current_unit: "ops".to_string(),
        },
    );
}

#[test]
fn stats_registry_records_typed_dump_history() {
    let mut stats = StatsRegistry::new();
    let insts = stats
        .register_counter("cpu0.committed_insts", "count")
        .unwrap();
    let mem_reads = stats.register_counter("system.mem_reads", "count").unwrap();

    stats.increment(insts, 10).unwrap();
    stats.increment(mem_reads, 4).unwrap();
    let first_dump = stats.try_dump(100).unwrap();
    assert_eq!(
        first_dump,
        StatDumpRecord::new(
            StatDumpId::new(0),
            StatSnapshot::new(
                100,
                0,
                0,
                vec![
                    StatSample::new(insts, "cpu0.committed_insts", "count", 10),
                    StatSample::new(mem_reads, "system.mem_reads", "count", 4),
                ],
            ),
        )
    );

    stats.increment(insts, 5).unwrap();
    let second_dump = stats.dump(120);
    assert_eq!(second_dump.id(), StatDumpId::new(1));
    assert_eq!(
        second_dump.snapshot(),
        &StatSnapshot::new(
            120,
            0,
            0,
            vec![
                StatSample::new(insts, "cpu0.committed_insts", "count", 15),
                StatSample::new(mem_reads, "system.mem_reads", "count", 4),
            ],
        )
    );

    stats.reset(130);
    stats.increment(mem_reads, 2).unwrap();
    let reset_scope_dump = stats.dump(140);
    assert_eq!(reset_scope_dump.id(), StatDumpId::new(2));
    assert_eq!(reset_scope_dump.snapshot().epoch(), 1);
    assert_eq!(reset_scope_dump.snapshot().reset_tick(), 130);
    assert_eq!(
        stats.dump_records(),
        &[first_dump, second_dump, reset_scope_dump]
    );
}

#[test]
fn stats_registry_rejects_dump_before_reset_without_recording_history() {
    let mut stats = StatsRegistry::new();
    let insts = stats.register_counter("cpu0.cycles", "cycles").unwrap();
    stats.increment(insts, 7).unwrap();
    stats.reset(50);

    assert_eq!(
        stats.try_dump(49).unwrap_err(),
        StatsError::SnapshotBeforeReset {
            tick: 49 as Tick,
            reset_tick: 50,
        },
    );
    assert!(stats.dump_records().is_empty());

    let valid_dump = stats.dump(50);
    assert_eq!(valid_dump.id(), StatDumpId::new(0));
    assert_eq!(stats.dump_records(), &[valid_dump]);
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

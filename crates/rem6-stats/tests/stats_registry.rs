use rem6_kernel::Tick;
use rem6_stats::{
    ProbeEvent, ProbeListenerId, ProbeListenerRef, ProbePayload, ProbePointId, ProbeRegistry,
    ProbeSnapshot, StatDeltaSample, StatDescription, StatDescriptionError, StatDumpId,
    StatDumpRecord, StatGroupDescriptor, StatGroupId, StatHistoryRecord, StatId, StatPath,
    StatPathError, StatResetId, StatResetPolicy, StatResetSample, StatSample, StatScope,
    StatSnapshot, StatSnapshotDelta, StatUnit, StatUnitError, StatsError, StatsRegistry,
    StatsResetRecord,
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
fn stats_registry_rejects_ambiguous_counter_units_without_consuming_ids() {
    let mut stats = StatsRegistry::new();

    assert_eq!(
        stats.register_counter("cpu0.cycles", "").unwrap_err(),
        StatsError::InvalidUnit {
            unit: String::new(),
            reason: StatUnitError::Empty,
        },
    );
    assert_eq!(
        stats
            .register_counter("cpu0.cycles", "cycle count")
            .unwrap_err(),
        StatsError::InvalidUnit {
            unit: "cycle count".to_string(),
            reason: StatUnitError::InvalidCharacter { character: ' ' },
        },
    );
    assert_eq!(
        stats
            .register_counter("cpu0.cycles", "cycle-count")
            .unwrap_err(),
        StatsError::InvalidUnit {
            unit: "cycle-count".to_string(),
            reason: StatUnitError::InvalidCharacter { character: '-' },
        },
    );

    let cycles = stats.register_counter("cpu0.cycles", "Cycle").unwrap();
    let ipc = stats.register_counter("cpu0.ipc", "(Count/Cycle)").unwrap();
    assert_eq!(cycles, StatId::new(0));
    assert_eq!(ipc, StatId::new(1));
    assert_eq!(
        stats.snapshot(10),
        StatSnapshot::new(
            10,
            0,
            0,
            vec![
                StatSample::new(cycles, "cpu0.cycles", "Cycle", 0),
                StatSample::new(ipc, "cpu0.ipc", "(Count/Cycle)", 0),
            ],
        ),
    );
}

#[test]
fn stats_registry_records_structured_counter_units_without_consuming_ids_on_bad_rates() {
    let mut stats = StatsRegistry::new();

    assert_eq!(
        stats
            .register_counter("cpu0.ipc", "Count/Cycle")
            .unwrap_err(),
        StatsError::InvalidUnit {
            unit: "Count/Cycle".to_string(),
            reason: StatUnitError::TrailingInput {
                index: 5,
                character: '/',
            },
        },
    );
    assert_eq!(
        stats.register_counter("cpu0.ipc", "(Count/)").unwrap_err(),
        StatsError::InvalidUnit {
            unit: "(Count/)".to_string(),
            reason: StatUnitError::ExpectedTerm { index: 7 },
        },
    );
    assert_eq!(
        stats
            .register_counter("cpu0.ipc", "(Count/Cycle")
            .unwrap_err(),
        StatsError::InvalidUnit {
            unit: "(Count/Cycle".to_string(),
            reason: StatUnitError::ExpectedRateTerminator { index: 12 },
        },
    );

    let nested_rate = StatUnit::rate(
        StatUnit::rate(StatUnit::bit(), StatUnit::second()),
        StatUnit::rate(StatUnit::count(), StatUnit::cycle()),
    );
    let bandwidth_per_ipc = stats
        .register_counter_with_unit("cpu0.bandwidth_per_ipc", nested_rate.clone())
        .unwrap();
    assert_eq!(bandwidth_per_ipc, StatId::new(0));

    let snapshot = stats.snapshot(10);
    let sample = &snapshot.samples()[0];
    assert_eq!(sample.id(), bandwidth_per_ipc);
    assert_eq!(sample.unit(), nested_rate.as_str());
    assert_eq!(sample.stat_unit(), &nested_rate);
}

#[test]
fn stats_registry_records_scoped_counter_identity_without_string_joining() {
    let mut stats = StatsRegistry::new();

    assert_eq!(
        stats
            .register_scoped_counter(["system", "cpu-0"], "cycles", "Cycle")
            .unwrap_err(),
        StatsError::InvalidPath {
            path: "system.cpu-0.cycles".to_string(),
            reason: StatPathError::InvalidSegmentCharacter {
                segment: "cpu-0".to_string(),
                character: '-',
            },
        },
    );
    assert_eq!(
        stats
            .register_scoped_counter(["system", "cpu0"], "", "Cycle")
            .unwrap_err(),
        StatsError::InvalidPath {
            path: "system.cpu0.".to_string(),
            reason: StatPathError::EmptySegment { index: 2 },
        },
    );

    let cycles = stats
        .register_scoped_counter_with_unit(["system", "cpu0"], "cycles", StatUnit::cycle())
        .unwrap();
    let scoped_path = StatPath::new(["system", "cpu0"], "cycles").unwrap();

    assert_eq!(cycles, StatId::new(0));
    assert_eq!(
        stats
            .register_counter("system.cpu0.cycles", "Cycle")
            .unwrap_err(),
        StatsError::DuplicatePath {
            path: "system.cpu0.cycles".to_string(),
        },
    );

    let snapshot = stats.snapshot(10);
    let sample = &snapshot.samples()[0];
    assert_eq!(sample.id(), cycles);
    assert_eq!(sample.path(), "system.cpu0.cycles");
    assert_eq!(sample.scope(), ["system".to_string(), "cpu0".to_string()]);
    assert_eq!(sample.name(), "cycles");
    assert_eq!(sample.stat_path(), &scoped_path);
}

#[test]
fn stats_registry_records_group_owned_counters_and_rejects_bad_groups() {
    let mut stats = StatsRegistry::new();

    assert_eq!(
        stats.register_group(["system", "cpu-0"]).unwrap_err(),
        StatsError::InvalidPath {
            path: "system.cpu-0".to_string(),
            reason: StatPathError::InvalidSegmentCharacter {
                segment: "cpu-0".to_string(),
                character: '-',
            },
        },
    );
    assert_eq!(
        stats.register_group(["system.cpu0"]).unwrap_err(),
        StatsError::InvalidPath {
            path: "system.cpu0".to_string(),
            reason: StatPathError::InvalidSegmentCharacter {
                segment: "system.cpu0".to_string(),
                character: '.',
            },
        },
    );

    let cpu0 = stats.register_group(["system", "cpu0"]).unwrap();
    let cpu1 = stats.register_group(["system", "cpu1"]).unwrap();
    assert_eq!(cpu0, StatGroupId::new(0));
    assert_eq!(cpu1, StatGroupId::new(1));
    assert_eq!(
        stats.register_group(["system", "cpu0"]).unwrap_err(),
        StatsError::DuplicateGroup {
            scope: "system.cpu0".to_string(),
        },
    );
    assert_eq!(
        stats
            .register_group_counter(StatGroupId::new(99), "cycles", "Cycle")
            .unwrap_err(),
        StatsError::UnknownStatGroup {
            group: StatGroupId::new(99),
        },
    );

    let cycles = stats
        .register_group_counter_with_unit(cpu0, "cycles", StatUnit::cycle())
        .unwrap();
    let insts = stats
        .register_group_counter(cpu0, "committed_insts", "Count")
        .unwrap();
    assert_eq!(cycles, StatId::new(0));
    assert_eq!(insts, StatId::new(1));
    assert_eq!(
        stats
            .register_scoped_counter(["system", "cpu0"], "cycles", "Cycle")
            .unwrap_err(),
        StatsError::DuplicatePath {
            path: "system.cpu0.cycles".to_string(),
        },
    );

    stats.increment(cycles, 4).unwrap();
    stats.increment(insts, 9).unwrap();
    let snapshot = stats.snapshot(20);
    let cycles_sample = &snapshot.samples()[0];
    assert_eq!(cycles_sample.group(), Some(cpu0));
    assert_eq!(
        cycles_sample.scope(),
        StatScope::new(["system", "cpu0"]).unwrap().segments()
    );
    assert_eq!(cycles_sample.path(), "system.cpu0.cycles");
    assert_eq!(cycles_sample.name(), "cycles");
    assert_eq!(cycles_sample.value(), 4);
    assert_eq!(snapshot.samples()[1].group(), Some(cpu0));

    let later = stats.snapshot(25);
    let delta = later.delta_since(&snapshot).unwrap();
    assert_eq!(delta.samples()[0].group(), Some(cpu0));
}

#[test]
fn stats_registry_records_counter_descriptions_without_consuming_ids_on_bad_metadata() {
    let mut stats = StatsRegistry::new();

    assert_eq!(
        stats
            .register_counter_with_description("cpu0.cycles", "Cycle", "   ")
            .unwrap_err(),
        StatsError::InvalidDescription {
            description: "   ".to_string(),
            reason: StatDescriptionError::Empty,
        },
    );

    let cycles = stats
        .register_counter_with_description("cpu0.cycles", "Cycle", "Architected cycles")
        .unwrap();
    let cpu0 = stats.register_group(["system", "cpu0"]).unwrap();
    let insts = stats
        .register_group_counter_with_description(
            cpu0,
            "committed_insts",
            "Count",
            "Committed instructions",
        )
        .unwrap();
    assert_eq!(cycles, StatId::new(0));
    assert_eq!(insts, StatId::new(1));

    stats.increment(cycles, 8).unwrap();
    stats.increment(insts, 3).unwrap();
    let snapshot = stats.snapshot(30);
    assert_eq!(
        snapshot.samples()[0].description(),
        Some("Architected cycles")
    );
    assert_eq!(
        snapshot.samples()[0].stat_description(),
        Some(&StatDescription::new("Architected cycles").unwrap())
    );
    assert_eq!(
        snapshot.samples()[1].description(),
        Some("Committed instructions")
    );
    assert_eq!(
        stats.dump(31).snapshot().samples()[1].description(),
        Some("Committed instructions")
    );

    stats.increment(cycles, 2).unwrap();
    stats.increment(insts, 4).unwrap();
    let later = stats.snapshot(35);
    let delta = later.delta_since(&snapshot).unwrap();
    assert_eq!(delta.samples()[0].description(), Some("Architected cycles"));
    assert_eq!(
        delta.samples()[1].description(),
        Some("Committed instructions")
    );
}

#[test]
fn stats_snapshots_dumps_and_deltas_carry_group_catalogs() {
    let mut stats = StatsRegistry::new();
    let cpu0 = stats.register_group(["system", "cpu0"]).unwrap();
    let cpu1 = stats.register_group(["system", "cpu1"]).unwrap();
    let cpu0_cycles = stats
        .register_group_counter_with_unit(cpu0, "cycles", StatUnit::cycle())
        .unwrap();
    let cpu1_cycles = stats
        .register_group_counter_with_unit(cpu1, "cycles", StatUnit::cycle())
        .unwrap();

    stats.increment(cpu0_cycles, 11).unwrap();
    stats.increment(cpu1_cycles, 13).unwrap();
    let snapshot = stats.snapshot(40);
    let expected_groups = vec![
        StatGroupDescriptor::new(cpu0, StatScope::new(["system", "cpu0"]).unwrap()),
        StatGroupDescriptor::new(cpu1, StatScope::new(["system", "cpu1"]).unwrap()),
    ];
    assert_eq!(snapshot.groups(), expected_groups.as_slice());
    assert_eq!(snapshot.group_scope(cpu0).unwrap().as_str(), "system.cpu0");
    assert_eq!(snapshot.group_scope(cpu1).unwrap().as_str(), "system.cpu1");
    assert_eq!(snapshot.group_scope(StatGroupId::new(99)), None);

    let dump = stats.dump(42);
    assert_eq!(dump.snapshot().groups(), expected_groups.as_slice());

    stats.increment(cpu0_cycles, 2).unwrap();
    stats.increment(cpu1_cycles, 3).unwrap();
    let later = stats.snapshot(45);
    let delta = later.delta_since(&snapshot).unwrap();
    assert_eq!(delta.groups(), expected_groups.as_slice());
    assert_eq!(delta.group_scope(cpu0).unwrap().as_str(), "system.cpu0");
    assert_eq!(delta.group_scope(cpu1).unwrap().as_str(), "system.cpu1");
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
fn stats_snapshot_delta_rejects_description_drift() {
    let stat = StatId::new(7);
    let path = StatPath::new(["cpu0"], "cycles").unwrap();
    let previous_description = StatDescription::new("Original cycle count").unwrap();
    let current_description = StatDescription::new("Renamed cycle count").unwrap();
    let previous = StatSnapshot::new(
        10,
        0,
        0,
        vec![StatSample::from_parts_with_description(
            stat,
            path.clone(),
            StatUnit::cycle(),
            Some(previous_description.clone()),
            12,
        )],
    );
    let current = StatSnapshot::new(
        20,
        0,
        0,
        vec![StatSample::from_parts_with_description(
            stat,
            path,
            StatUnit::cycle(),
            Some(current_description.clone()),
            15,
        )],
    );

    assert_eq!(
        current.delta_since(&previous).unwrap_err(),
        StatsError::SnapshotDeltaDescriptionMismatch {
            stat,
            previous_description: Some(previous_description),
            current_description: Some(current_description),
        },
    );
}

#[test]
fn stats_snapshot_delta_rejects_reset_policy_drift() {
    let stat = StatId::new(7);
    let path = StatPath::new(["cpu0"], "cycles").unwrap();
    let previous = StatSnapshot::new(
        10,
        0,
        0,
        vec![StatSample::from_parts_with_reset_policy(
            stat,
            path.clone(),
            StatUnit::cycle(),
            StatResetPolicy::Resettable,
            12,
        )],
    );
    let current = StatSnapshot::new(
        20,
        0,
        0,
        vec![StatSample::from_parts_with_reset_policy(
            stat,
            path,
            StatUnit::cycle(),
            StatResetPolicy::Monotonic,
            15,
        )],
    );

    assert_eq!(
        current.delta_since(&previous).unwrap_err(),
        StatsError::SnapshotDeltaResetPolicyMismatch {
            stat,
            previous_policy: StatResetPolicy::Resettable,
            current_policy: StatResetPolicy::Monotonic,
        },
    );
}

#[test]
fn stats_snapshot_delta_rejects_group_catalog_drift() {
    let stat = StatId::new(0);
    let group = StatGroupId::new(0);
    let previous_group =
        StatGroupDescriptor::new(group, StatScope::new(["system", "cpu0"]).unwrap());
    let current_group =
        StatGroupDescriptor::new(group, StatScope::new(["system", "cpu1"]).unwrap());
    let path = StatPath::new(["system", "cpu0"], "cycles").unwrap();
    let previous = StatSnapshot::with_groups(
        10,
        0,
        0,
        vec![previous_group.clone()],
        vec![StatSample::from_registered_parts(
            stat,
            Some(group),
            path.clone(),
            StatUnit::cycle(),
            12,
        )],
    );
    let current = StatSnapshot::with_groups(
        20,
        0,
        0,
        vec![current_group.clone()],
        vec![StatSample::from_registered_parts(
            stat,
            Some(group),
            path,
            StatUnit::cycle(),
            15,
        )],
    );

    assert_eq!(
        current.delta_since(&previous).unwrap_err(),
        StatsError::SnapshotDeltaGroupCatalogMismatch {
            previous_groups: vec![previous_group],
            current_groups: vec![current_group],
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
fn stats_registry_records_typed_reset_history() {
    let mut stats = StatsRegistry::new();
    let cycles = stats.register_counter("cpu0.cycles", "cycles").unwrap();
    let misses = stats.register_counter("l1d.misses", "count").unwrap();

    stats.increment(cycles, 50).unwrap();
    stats.increment(misses, 3).unwrap();
    let first_reset = stats.try_reset(20).unwrap();
    assert_eq!(first_reset.id(), StatResetId::new(0));
    assert_eq!(
        first_reset,
        StatsResetRecord::new(20, 1, vec![(cycles, 50), (misses, 3)])
    );
    assert_eq!(stats.reset_records(), std::slice::from_ref(&first_reset));

    stats.increment(cycles, 7).unwrap();
    assert_eq!(
        stats.try_reset(19).unwrap_err(),
        StatsError::ResetBeforeLastReset {
            tick: 19 as Tick,
            reset_tick: 20,
        },
    );
    assert_eq!(stats.reset_records(), std::slice::from_ref(&first_reset));
    assert_eq!(
        stats.history_records(),
        [StatHistoryRecord::Reset(first_reset.clone())].as_slice(),
    );

    let second_reset = stats.reset(25);
    assert_eq!(second_reset.id(), StatResetId::new(1));
    assert_eq!(
        second_reset,
        StatsResetRecord::with_id(StatResetId::new(1), 25, 2, vec![(cycles, 7), (misses, 0)])
    );
    assert_eq!(
        stats.reset_records(),
        [first_reset, second_reset].as_slice()
    );
}

#[test]
fn stats_reset_preserves_declared_constant_and_monotonic_counters_with_typed_audit() {
    let mut stats = StatsRegistry::new();
    let cpu0 = stats.register_group(["system", "cpu0"]).unwrap();
    let committed = stats
        .register_group_counter_with_reset_policy(
            cpu0,
            "committed_insts",
            "Count",
            StatResetPolicy::Resettable,
        )
        .unwrap();
    let thread_contexts = stats
        .register_group_counter_with_reset_policy(
            cpu0,
            "thread_contexts",
            "Count",
            StatResetPolicy::Constant,
        )
        .unwrap();
    let elapsed_ticks = stats
        .register_counter_with_reset_policy(
            "system.elapsed_ticks",
            "Tick",
            StatResetPolicy::Monotonic,
        )
        .unwrap();

    stats.increment(committed, 21).unwrap();
    stats.increment(thread_contexts, 4).unwrap();
    stats.increment(elapsed_ticks, 1000).unwrap();

    let reset = stats.reset(100);

    assert_eq!(
        reset.reset_samples(),
        [
            StatResetSample::new(committed, StatResetPolicy::Resettable, 21, 0),
            StatResetSample::new(thread_contexts, StatResetPolicy::Constant, 4, 4),
            StatResetSample::new(elapsed_ticks, StatResetPolicy::Monotonic, 1000, 1000),
        ]
        .as_slice(),
    );
    assert_eq!(
        reset.previous_values(),
        [(committed, 21), (thread_contexts, 4), (elapsed_ticks, 1000)].as_slice(),
    );

    let after_reset = stats.snapshot(101);
    assert_eq!(
        after_reset.samples()[0].reset_policy(),
        StatResetPolicy::Resettable
    );
    assert_eq!(after_reset.samples()[0].value(), 0);
    assert_eq!(
        after_reset.samples()[1].reset_policy(),
        StatResetPolicy::Constant
    );
    assert_eq!(after_reset.samples()[1].value(), 4);
    assert_eq!(
        after_reset.samples()[2].reset_policy(),
        StatResetPolicy::Monotonic
    );
    assert_eq!(after_reset.samples()[2].value(), 1000);

    stats.increment(committed, 3).unwrap();
    stats.increment(elapsed_ticks, 50).unwrap();
    let later = stats.snapshot(120);
    let delta = later.delta_since(&after_reset).unwrap();
    assert_eq!(
        delta.samples()[0].reset_policy(),
        StatResetPolicy::Resettable
    );
    assert_eq!(delta.samples()[0].delta_value(), 3);
    assert_eq!(delta.samples()[1].reset_policy(), StatResetPolicy::Constant);
    assert_eq!(delta.samples()[1].delta_value(), 0);
    assert_eq!(
        delta.samples()[2].reset_policy(),
        StatResetPolicy::Monotonic
    );
    assert_eq!(delta.samples()[2].delta_value(), 50);
}

#[test]
fn stats_registry_records_interleaved_typed_history() {
    let mut stats = StatsRegistry::new();
    let cycles = stats.register_counter("cpu0.cycles", "cycles").unwrap();

    stats.increment(cycles, 8).unwrap();
    let first_dump = stats.dump(10);
    stats.increment(cycles, 4).unwrap();
    let reset = stats.reset(12);
    stats.increment(cycles, 3).unwrap();
    let second_dump = stats.dump(14);

    assert_eq!(
        stats.history_records(),
        [
            StatHistoryRecord::Dump(first_dump.clone()),
            StatHistoryRecord::Reset(reset.clone()),
            StatHistoryRecord::Dump(second_dump.clone()),
        ]
        .as_slice(),
    );
    assert_eq!(stats.history_records()[0].tick(), 10);
    assert_eq!(stats.history_records()[1].tick(), 12);
    assert_eq!(stats.history_records()[1].epoch(), 1);
    assert_eq!(stats.history_records()[2].reset_tick(), 12);
    assert_eq!(stats.dump_records(), [first_dump, second_dump].as_slice());
    assert_eq!(stats.reset_records(), std::slice::from_ref(&reset));
}

#[test]
fn stats_registry_rejects_schema_registration_after_history_begins() {
    let mut stats = StatsRegistry::new();
    let cpu0 = stats.register_group(["system", "cpu0"]).unwrap();
    let cycles = stats
        .register_group_counter(cpu0, "cycles", "Cycle")
        .unwrap();

    let first_dump = stats.dump(10);

    assert_eq!(
        stats
            .register_counter("system.mem_reads", "Count")
            .unwrap_err(),
        StatsError::SchemaLocked { history_records: 1 },
    );
    assert_eq!(
        stats.register_group(["system", "cpu1"]).unwrap_err(),
        StatsError::SchemaLocked { history_records: 1 },
    );
    assert_eq!(
        stats
            .register_group_counter(cpu0, "committed_insts", "Count")
            .unwrap_err(),
        StatsError::SchemaLocked { history_records: 1 },
    );
    assert_eq!(
        stats
            .register_scoped_counter(["system", "cpu0"], "ipc", "Ratio")
            .unwrap_err(),
        StatsError::SchemaLocked { history_records: 1 },
    );
    assert_eq!(stats.dump_records(), std::slice::from_ref(&first_dump));
    assert_eq!(
        stats.history_records(),
        [StatHistoryRecord::Dump(first_dump.clone())].as_slice(),
    );

    stats.increment(cycles, 4).unwrap();
    let reset = stats.reset(12);
    assert_eq!(
        stats.register_counter("l1d.misses", "Count").unwrap_err(),
        StatsError::SchemaLocked { history_records: 2 },
    );
    assert_eq!(
        stats.history_records(),
        [
            StatHistoryRecord::Dump(first_dump),
            StatHistoryRecord::Reset(reset),
        ]
        .as_slice(),
    );
}

#[test]
fn stats_registry_rejects_dump_before_reset_without_recording_history() {
    let mut stats = StatsRegistry::new();
    let insts = stats.register_counter("cpu0.cycles", "cycles").unwrap();
    stats.increment(insts, 7).unwrap();
    let reset = stats.reset(50);

    assert_eq!(
        stats.try_dump(49).unwrap_err(),
        StatsError::SnapshotBeforeReset {
            tick: 49 as Tick,
            reset_tick: 50,
        },
    );
    assert!(stats.dump_records().is_empty());
    assert_eq!(
        stats.history_records(),
        [StatHistoryRecord::Reset(reset.clone())].as_slice(),
    );

    let valid_dump = stats.dump(50);
    assert_eq!(valid_dump.id(), StatDumpId::new(0));
    assert_eq!(stats.dump_records(), std::slice::from_ref(&valid_dump));
    assert_eq!(
        stats.history_records(),
        [
            StatHistoryRecord::Reset(reset),
            StatHistoryRecord::Dump(valid_dump),
        ]
        .as_slice(),
    );
}

#[test]
fn stats_registry_rejects_history_ticks_before_last_record_without_mutation() {
    let mut stats = StatsRegistry::new();
    let cycles = stats.register_counter("cpu0.cycles", "cycles").unwrap();

    stats.increment(cycles, 10).unwrap();
    let first_dump = stats.dump(100);
    stats.increment(cycles, 1).unwrap();

    assert_eq!(
        stats.try_dump(90).unwrap_err(),
        StatsError::HistoryTickBeforeLastRecord {
            tick: 90,
            last_history_tick: 100,
        },
    );
    assert_eq!(stats.dump_records(), std::slice::from_ref(&first_dump));
    assert_eq!(
        stats.history_records(),
        [StatHistoryRecord::Dump(first_dump.clone())].as_slice(),
    );

    assert_eq!(
        stats.try_reset(90).unwrap_err(),
        StatsError::HistoryTickBeforeLastRecord {
            tick: 90,
            last_history_tick: 100,
        },
    );
    assert_eq!(stats.epoch(), 0);
    assert_eq!(stats.reset_tick(), 0);
    assert_eq!(stats.reset_records(), &[]);

    let reset = stats.reset(100);
    assert_eq!(reset.id(), StatResetId::new(0));
    stats.increment(cycles, 3).unwrap();
    assert_eq!(stats.epoch(), 1);
    assert_eq!(stats.reset_tick(), 100);
    assert_eq!(
        stats.snapshot(101),
        StatSnapshot::new(
            101,
            1,
            100,
            vec![StatSample::new(cycles, "cpu0.cycles", "cycles", 3)]
        )
    );

    let second_dump = stats.dump(101);
    assert_eq!(second_dump.id(), StatDumpId::new(1));
    assert_eq!(
        stats.history_records(),
        [
            StatHistoryRecord::Dump(first_dump),
            StatHistoryRecord::Reset(reset),
            StatHistoryRecord::Dump(second_dump),
        ]
        .as_slice(),
    );
}

#[test]
fn probe_registry_records_typed_events_and_listener_state() {
    let mut probes = ProbeRegistry::new();
    let committed = probes.register_point("cpu0", "commit").unwrap();
    let miss = probes.register_point("l1d", "miss").unwrap();

    assert_eq!(committed, ProbePointId::new(0));
    assert_eq!(miss, ProbePointId::new(1));
    let counter_listener = probes.add_listener(committed, "commit_counter").unwrap();
    assert_eq!(counter_listener, ProbeListenerId::new(0));
    let trace_listener = probes.add_listener(committed, "commit_trace").unwrap();

    let event = probes
        .emit(10, committed, ProbePayload::Counter { amount: 4 })
        .unwrap()
        .clone();
    assert_eq!(
        event,
        ProbeEvent::new(
            10,
            0,
            committed,
            vec![
                ProbeListenerRef::new(counter_listener, "commit_counter"),
                ProbeListenerRef::new(trace_listener, "commit_trace"),
            ],
            ProbePayload::Counter { amount: 4 },
        )
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
        ProbeSnapshot::with_cursors(
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
                ProbeEvent::new(
                    10,
                    0,
                    committed,
                    vec![
                        ProbeListenerRef::new(counter_listener, "commit_counter"),
                        ProbeListenerRef::new(trace_listener, "commit_trace"),
                    ],
                    ProbePayload::Counter { amount: 4 },
                ),
                ProbeEvent::new(
                    11,
                    1,
                    committed,
                    vec![ProbeListenerRef::new(counter_listener, "commit_counter")],
                    ProbePayload::Unit,
                ),
            ],
            2,
            2,
            2,
        )
    );

    assert_eq!(
        probes.events()[0].payload(),
        &ProbePayload::Counter { amount: 4 }
    );
}

#[test]
fn probe_registry_rejects_ambiguous_identifiers_without_consuming_ids() {
    let mut probes = ProbeRegistry::new();

    assert_eq!(
        probes.register_point("cpu-0", "commit").unwrap_err(),
        StatsError::InvalidProbeComponent {
            component: "cpu-0".to_string(),
            reason: StatPathError::InvalidSegmentCharacter {
                segment: "cpu-0".to_string(),
                character: '-',
            },
        },
    );
    assert_eq!(
        probes.register_point("0cpu", "commit").unwrap_err(),
        StatsError::InvalidProbeComponent {
            component: "0cpu".to_string(),
            reason: StatPathError::InvalidSegmentStart {
                segment: "0cpu".to_string(),
                character: '0',
            },
        },
    );
    assert_eq!(
        probes.register_point("cpu0", "commit count").unwrap_err(),
        StatsError::InvalidProbeName {
            name: "commit count".to_string(),
            reason: StatPathError::InvalidSegmentCharacter {
                segment: "commit count".to_string(),
                character: ' ',
            },
        },
    );
    assert_eq!(
        probes.register_point("cpu0", "commit.event").unwrap_err(),
        StatsError::InvalidProbeName {
            name: "commit.event".to_string(),
            reason: StatPathError::InvalidSegmentCharacter {
                segment: "commit.event".to_string(),
                character: '.',
            },
        },
    );

    let committed = probes.register_point("cpu0", "commit").unwrap();
    assert_eq!(committed, ProbePointId::new(0));
    assert_eq!(
        probes
            .add_listener(committed, "commit listener")
            .unwrap_err(),
        StatsError::InvalidProbeListenerName {
            name: "commit listener".to_string(),
            reason: StatPathError::InvalidSegmentCharacter {
                segment: "commit listener".to_string(),
                character: ' ',
            },
        },
    );
    assert_eq!(
        probes.add_listener(committed, "commit_trace").unwrap(),
        ProbeListenerId::new(0),
    );
}

#[test]
fn probe_registry_rejects_time_regressing_events_without_consuming_sequence() {
    let mut probes = ProbeRegistry::new();
    let committed = probes.register_point("cpu0", "commit").unwrap();

    probes.emit(10, committed, ProbePayload::Unit).unwrap();
    assert_eq!(
        probes.emit(9, committed, ProbePayload::Unit).unwrap_err(),
        StatsError::ProbeEventTimeWentBack {
            previous_tick: 10,
            current_tick: 9,
        },
    );

    let event = probes
        .emit(10, committed, ProbePayload::Counter { amount: 1 })
        .unwrap()
        .clone();
    assert_eq!(event.sequence(), 1);
    assert_eq!(
        probes.events(),
        [
            ProbeEvent::new(10, 0, committed, Vec::new(), ProbePayload::Unit),
            ProbeEvent::new(
                10,
                1,
                committed,
                Vec::new(),
                ProbePayload::Counter { amount: 1 },
            ),
        ]
        .as_slice(),
    );
}

#[test]
fn probe_registry_records_listener_refs_for_historical_events() {
    let mut probes = ProbeRegistry::new();
    let committed = probes.register_point("cpu0", "commit").unwrap();
    let counter = probes.add_listener(committed, "commit_counter").unwrap();
    let trace = probes.add_listener(committed, "commit_trace").unwrap();

    let event = probes.emit(10, committed, ProbePayload::Unit).unwrap();
    assert_eq!(
        event.listener_refs(),
        [
            ProbeListenerRef::new(counter, "commit_counter"),
            ProbeListenerRef::new(trace, "commit_trace"),
        ]
        .as_slice(),
    );

    probes.remove_listener(committed, trace).unwrap();
    let snapshot = probes.snapshot();
    assert_eq!(
        snapshot.events()[0].listener_refs(),
        [
            ProbeListenerRef::new(counter, "commit_counter"),
            ProbeListenerRef::new(trace, "commit_trace"),
        ]
        .as_slice(),
    );

    let mut restored = ProbeRegistry::from_snapshot(&snapshot).unwrap();
    assert_eq!(
        restored.add_listener(committed, "commit_sample").unwrap(),
        ProbeListenerId::new(2),
    );
}

#[test]
fn probe_registry_restores_snapshot_cursors_without_reusing_removed_listener_ids() {
    let mut probes = ProbeRegistry::new();
    let committed = probes.register_point("cpu0", "commit").unwrap();
    let miss = probes.register_point("l1d", "miss").unwrap();
    probes.add_listener(committed, "commit_counter").unwrap();
    let removed = probes.add_listener(committed, "commit_trace").unwrap();
    probes
        .emit(10, committed, ProbePayload::Counter { amount: 4 })
        .unwrap();
    probes.remove_listener(committed, removed).unwrap();
    probes.emit(11, committed, ProbePayload::Unit).unwrap();

    let snapshot = probes.snapshot();
    assert_eq!(snapshot.next_point(), 2);
    assert_eq!(snapshot.next_listener(), 2);
    assert_eq!(snapshot.next_sequence(), 2);

    let mut restored = ProbeRegistry::from_snapshot(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(
        restored.register_point("l2", "evict").unwrap(),
        ProbePointId::new(2)
    );
    assert_eq!(
        restored.add_listener(miss, "miss_counter").unwrap(),
        ProbeListenerId::new(2)
    );
    assert_eq!(
        restored
            .emit(12, miss, ProbePayload::Counter { amount: 1 })
            .unwrap()
            .sequence(),
        2
    );
}

#[test]
fn probe_registry_rejects_malformed_snapshots_without_mutating_live_registry() {
    let mut probes = ProbeRegistry::new();
    let point = probes.register_point("cpu0", "commit").unwrap();
    probes.add_listener(point, "commit_counter").unwrap();
    probes.emit(10, point, ProbePayload::Unit).unwrap();
    let original = probes.snapshot();

    let duplicate_point_id = ProbeSnapshot::with_cursors(
        vec![
            ("cpu0".to_string(), "commit".to_string(), point),
            ("cpu1".to_string(), "commit".to_string(), point),
        ],
        Vec::new(),
        Vec::new(),
        1,
        0,
        0,
    );
    assert_eq!(
        probes.restore(&duplicate_point_id).unwrap_err(),
        StatsError::DuplicateProbePointId { point },
    );
    assert_eq!(probes.snapshot(), original);

    let unknown_listener_point = ProbeSnapshot::with_cursors(
        vec![("cpu0".to_string(), "commit".to_string(), point)],
        vec![(
            "commit_counter".to_string(),
            ProbePointId::new(9),
            ProbeListenerId::new(0),
        )],
        Vec::new(),
        1,
        1,
        0,
    );
    assert_eq!(
        probes.restore(&unknown_listener_point).unwrap_err(),
        StatsError::UnknownProbePoint {
            point: ProbePointId::new(9),
        },
    );
    assert_eq!(probes.snapshot(), original);

    let duplicate_listener_id = ProbeSnapshot::with_cursors(
        vec![("cpu0".to_string(), "commit".to_string(), point)],
        vec![
            ("commit_counter".to_string(), point, ProbeListenerId::new(0)),
            ("commit_trace".to_string(), point, ProbeListenerId::new(0)),
        ],
        Vec::new(),
        1,
        1,
        0,
    );
    assert_eq!(
        probes.restore(&duplicate_listener_id).unwrap_err(),
        StatsError::DuplicateProbeListenerId {
            listener: ProbeListenerId::new(0),
        },
    );
    assert_eq!(probes.snapshot(), original);

    let nonmonotonic_events = ProbeSnapshot::with_cursors(
        vec![("cpu0".to_string(), "commit".to_string(), point)],
        Vec::new(),
        vec![
            ProbeEvent::new(10, 1, point, Vec::new(), ProbePayload::Unit),
            ProbeEvent::new(11, 1, point, Vec::new(), ProbePayload::Unit),
        ],
        1,
        0,
        2,
    );
    assert_eq!(
        probes.restore(&nonmonotonic_events).unwrap_err(),
        StatsError::ProbeEventSequenceNotIncreasing {
            previous_sequence: 1,
            current_sequence: 1,
        },
    );
    assert_eq!(probes.snapshot(), original);

    let time_regressing_events = ProbeSnapshot::with_cursors(
        vec![("cpu0".to_string(), "commit".to_string(), point)],
        Vec::new(),
        vec![
            ProbeEvent::new(10, 0, point, Vec::new(), ProbePayload::Unit),
            ProbeEvent::new(9, 1, point, Vec::new(), ProbePayload::Unit),
        ],
        1,
        0,
        2,
    );
    assert_eq!(
        probes.restore(&time_regressing_events).unwrap_err(),
        StatsError::ProbeEventTimeWentBack {
            previous_tick: 10,
            current_tick: 9,
        },
    );
    assert_eq!(probes.snapshot(), original);

    let duplicate_event_listener_id = ProbeSnapshot::with_cursors(
        vec![("cpu0".to_string(), "commit".to_string(), point)],
        Vec::new(),
        vec![ProbeEvent::new(
            10,
            0,
            point,
            vec![
                ProbeListenerRef::new(ProbeListenerId::new(3), "commit_counter"),
                ProbeListenerRef::new(ProbeListenerId::new(3), "commit_trace"),
            ],
            ProbePayload::Unit,
        )],
        1,
        4,
        1,
    );
    assert_eq!(
        probes.restore(&duplicate_event_listener_id).unwrap_err(),
        StatsError::DuplicateProbeListenerId {
            listener: ProbeListenerId::new(3),
        },
    );
    assert_eq!(probes.snapshot(), original);

    let stale_event_listener_cursor = ProbeSnapshot::with_cursors(
        vec![("cpu0".to_string(), "commit".to_string(), point)],
        Vec::new(),
        vec![ProbeEvent::new(
            10,
            0,
            point,
            vec![ProbeListenerRef::new(
                ProbeListenerId::new(3),
                "commit_counter",
            )],
            ProbePayload::Unit,
        )],
        1,
        3,
        1,
    );
    assert_eq!(
        probes.restore(&stale_event_listener_cursor).unwrap_err(),
        StatsError::ProbeListenerCursorBehind {
            next_listener: 3,
            highest_listener: ProbeListenerId::new(3),
        },
    );
    assert_eq!(probes.snapshot(), original);

    let stale_point_cursor = ProbeSnapshot::with_cursors(
        vec![("cpu0".to_string(), "commit".to_string(), point)],
        Vec::new(),
        Vec::new(),
        0,
        0,
        0,
    );
    assert_eq!(
        probes.restore(&stale_point_cursor).unwrap_err(),
        StatsError::ProbePointCursorBehind {
            next_point: 0,
            highest_point: point,
        },
    );
    assert_eq!(probes.snapshot(), original);

    let invalid_point_name = ProbeSnapshot::with_cursors(
        vec![("cpu0".to_string(), "commit count".to_string(), point)],
        Vec::new(),
        Vec::new(),
        1,
        0,
        0,
    );
    assert_eq!(
        probes.restore(&invalid_point_name).unwrap_err(),
        StatsError::InvalidProbeName {
            name: "commit count".to_string(),
            reason: StatPathError::InvalidSegmentCharacter {
                segment: "commit count".to_string(),
                character: ' ',
            },
        },
    );
    assert_eq!(probes.snapshot(), original);

    let invalid_listener_name = ProbeSnapshot::with_cursors(
        vec![("cpu0".to_string(), "commit".to_string(), point)],
        vec![(
            "commit listener".to_string(),
            point,
            ProbeListenerId::new(0),
        )],
        Vec::new(),
        1,
        1,
        0,
    );
    assert_eq!(
        probes.restore(&invalid_listener_name).unwrap_err(),
        StatsError::InvalidProbeListenerName {
            name: "commit listener".to_string(),
            reason: StatPathError::InvalidSegmentCharacter {
                segment: "commit listener".to_string(),
                character: ' ',
            },
        },
    );
    assert_eq!(probes.snapshot(), original);
}

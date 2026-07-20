use super::*;
use crate::Rem6DramResourceSummary;

fn run_records_with_dram(
    final_tick: u64,
    dram: Rem6DramResourceSummary,
) -> Vec<PowerAnalysisRecord> {
    let resources = Rem6MemoryResourceSummary {
        dram,
        ..Rem6MemoryResourceSummary::default()
    };
    run_memory_power_records(final_tick, &resources)
}

fn record_for_target<'a>(
    records: &'a [PowerAnalysisRecord],
    target: &str,
) -> Option<&'a PowerAnalysisRecord> {
    records.iter().find(|record| record.target() == target)
}

fn assert_close(actual: f64, expected: f64, label: &str) {
    assert!(
        (actual - expected).abs() < 1e-12,
        "{label}: {actual} != {expected}"
    );
}

fn assert_record_values(
    record: &PowerAnalysisRecord,
    target: &str,
    dynamic_watts: f64,
    static_watts: f64,
    temperature_c: f64,
    residency_ticks: u64,
) {
    assert_eq!(record.target(), target);
    assert_close(record.dynamic_watts(), dynamic_watts, "dynamic watts");
    assert_close(record.static_watts(), static_watts, "static watts");
    assert_close(record.temperature_c(), temperature_c, "temperature");
    assert_eq!(record.residency_ticks(PowerStateKind::On), residency_ticks);
}

#[test]
fn run_power_emits_profile_only_dram_resource_with_legacy_baseline() {
    let records = run_records_with_dram(
        17,
        Rem6DramResourceSummary {
            profiled_targets: 2,
            ..Rem6DramResourceSummary::default()
        },
    );
    let record = record_for_target(&records, "memory.dram")
        .expect("configured DRAM profile retains a static power record");

    assert_record_values(record, "memory.dram", 0.001, 0.010_500, 38.001, 17);
}

#[test]
fn run_power_emits_active_only_dram_resource() {
    let records = run_records_with_dram(
        0,
        Rem6DramResourceSummary {
            active: 2,
            active_targets: 2,
            ..Rem6DramResourceSummary::default()
        },
    );
    let record = record_for_target(&records, "memory.dram")
        .expect("active DRAM topology retains a static power record");

    assert_record_values(record, "memory.dram", 0.001, 0.010_500, 38.001, 1);
}

#[test]
fn run_dram_power_uses_accesses_when_they_dominate_residency() {
    let records = run_records_with_dram(
        7,
        Rem6DramResourceSummary {
            activity: 5,
            active: 1,
            active_banks: 1,
            accesses: 29,
            read_bytes: 8,
            commands: 3,
            ..Rem6DramResourceSummary::default()
        },
    );
    let record = record_for_target(&records, "memory.dram").expect("DRAM accesses are activity");

    assert_record_values(record, "memory.dram", 0.001_033, 0.010_500, 38.001_033, 29);
}

#[test]
fn run_cache_power_emits_operation_only_resource() {
    for (name, scheduled_misses, coalesced_misses, expected_dynamic) in [
        ("scheduled", 2, 0, 0.001_008),
        ("coalesced", 0, 3, 0.001_012),
    ] {
        let mut resources = Rem6MemoryResourceSummary::default();
        resources.cache_instruction.l1 = Rem6CacheResourceSummary {
            bank_scheduled_misses: scheduled_misses,
            bank_coalesced_misses: coalesced_misses,
            ..Rem6CacheResourceSummary::default()
        };
        let records = run_memory_power_records(0, &resources);
        let record = record_for_target(&records, "cpu.instruction_cache")
            .unwrap_or_else(|| panic!("{name}-miss-only cache activity must emit a record"));

        assert_record_values(
            record,
            "cpu.instruction_cache",
            expected_dynamic,
            0.010,
            39.0 + expected_dynamic,
            1,
        );
    }
}

#[test]
fn run_cache_power_preserves_legacy_target_calibrations() {
    let mut resources = Rem6MemoryResourceSummary::default();
    resources.cache_instruction.l1 = Rem6CacheResourceSummary {
        activity: 5,
        cpu_responses: 1_000,
        directory_decisions: 2,
        dram_accesses: 1,
        bank_accepted: 3,
        bank_scheduled_misses: 4,
        bank_coalesced_misses: 5,
        prefetch_issued: 6,
        ..Rem6CacheResourceSummary::default()
    };
    resources.cache_data.l1 = Rem6CacheResourceSummary {
        activity: 7,
        cpu_responses: 2_000,
        directory_decisions: 1,
        dram_accesses: 2,
        bank_accepted: 2,
        bank_scheduled_misses: 3,
        bank_coalesced_misses: 4,
        prefetch_issued: 5,
        ..Rem6CacheResourceSummary::default()
    };
    resources.cache_l2 = Rem6CacheResourceSummary {
        activity: 4,
        cpu_responses: 7,
        directory_decisions: 2,
        dram_accesses: 2,
        bank_accepted: 3,
        bank_scheduled_misses: 5,
        bank_coalesced_misses: 11,
        prefetch_issued: 13,
        ..Rem6CacheResourceSummary::default()
    };
    resources.cache_l3 = Rem6CacheResourceSummary {
        activity: 6,
        cpu_responses: 17,
        directory_decisions: 1,
        dram_accesses: 3,
        bank_accepted: 2,
        bank_scheduled_misses: 3,
        bank_coalesced_misses: 5,
        prefetch_issued: 7,
        ..Rem6CacheResourceSummary::default()
    };

    let records = run_memory_power_records(11, &resources);
    let instruction_dynamic = 0.001_142;
    let data_dynamic = 0.001_166;
    let l2_dynamic = 0.001_207;
    let l3_dynamic = 0.001_231;

    assert_record_values(
        record_for_target(&records, "cpu.instruction_cache").unwrap(),
        "cpu.instruction_cache",
        instruction_dynamic,
        0.010,
        39.0 + instruction_dynamic,
        11,
    );
    assert_record_values(
        record_for_target(&records, "cpu.data_cache").unwrap(),
        "cpu.data_cache",
        data_dynamic,
        0.012,
        39.0 + data_dynamic,
        11,
    );
    assert_record_values(
        record_for_target(&records, "memory.cache.l2").unwrap(),
        "memory.cache.l2",
        l2_dynamic,
        0.016,
        38.5 + l2_dynamic,
        11,
    );
    assert_record_values(
        record_for_target(&records, "memory.cache.l3").unwrap(),
        "memory.cache.l3",
        l3_dynamic,
        0.016,
        38.5 + l3_dynamic,
        11,
    );
}

#[test]
fn run_power_emits_refresh_only_dram_resource() {
    let dram = Rem6DramResourceSummary {
        activity: 1,
        active: 1,
        refreshes: 1,
        refresh_ticks: 9,
        ..Rem6DramResourceSummary::default()
    };
    let records = run_records_with_dram(0, dram);
    let record = record_for_target(&records, "memory.dram").expect("refresh is DRAM activity");

    assert_eq!(record.residency_ticks(PowerStateKind::On), 9);
}

#[test]
fn run_power_emits_low_power_only_dram_resource() {
    #[derive(Clone, Copy)]
    struct LowPowerCase {
        name: &'static str,
        active_powerdown_entries: u64,
        active_powerdown_ticks: u64,
        precharge_powerdown_entries: u64,
        precharge_powerdown_ticks: u64,
        self_refresh_entries: u64,
        self_refresh_ticks: u64,
        exits: u64,
        exit_latency_ticks: u64,
        expected_residency: u64,
    }

    let cases = [
        LowPowerCase {
            name: "entries-dominate-self-refresh-residency",
            active_powerdown_entries: 2,
            active_powerdown_ticks: 7,
            precharge_powerdown_entries: 3,
            precharge_powerdown_ticks: 9,
            self_refresh_entries: 5,
            self_refresh_ticks: 11,
            exits: 4,
            exit_latency_ticks: 8,
            expected_residency: 11,
        },
        LowPowerCase {
            name: "exits-dominate-active-powerdown-residency",
            active_powerdown_entries: 1,
            active_powerdown_ticks: 17,
            precharge_powerdown_entries: 2,
            precharge_powerdown_ticks: 5,
            self_refresh_entries: 1,
            self_refresh_ticks: 7,
            exits: 9,
            exit_latency_ticks: 13,
            expected_residency: 17,
        },
        LowPowerCase {
            name: "precharge-powerdown-residency",
            active_powerdown_entries: 2,
            active_powerdown_ticks: 5,
            precharge_powerdown_entries: 4,
            precharge_powerdown_ticks: 19,
            self_refresh_entries: 1,
            self_refresh_ticks: 11,
            exits: 3,
            exit_latency_ticks: 13,
            expected_residency: 19,
        },
        LowPowerCase {
            name: "exit-latency-residency",
            active_powerdown_entries: 1,
            active_powerdown_ticks: 7,
            precharge_powerdown_entries: 2,
            precharge_powerdown_ticks: 9,
            self_refresh_entries: 1,
            self_refresh_ticks: 11,
            exits: 6,
            exit_latency_ticks: 23,
            expected_residency: 23,
        },
    ];
    let mut failures = Vec::new();

    for case in cases {
        let low_power_entries = case
            .active_powerdown_entries
            .saturating_add(case.precharge_powerdown_entries)
            .saturating_add(case.self_refresh_entries);
        let events = low_power_entries.max(case.exits);
        let dram = Rem6DramResourceSummary {
            activity: events,
            active: u64::from(events != 0),
            low_power_active_powerdown_entries: case.active_powerdown_entries,
            low_power_active_powerdown_ticks: case.active_powerdown_ticks,
            low_power_precharge_powerdown_entries: case.precharge_powerdown_entries,
            low_power_precharge_powerdown_ticks: case.precharge_powerdown_ticks,
            low_power_self_refresh_entries: case.self_refresh_entries,
            low_power_self_refresh_ticks: case.self_refresh_ticks,
            low_power_exits: case.exits,
            low_power_exit_latency_ticks: case.exit_latency_ticks,
            ..Rem6DramResourceSummary::default()
        };
        let records = run_records_with_dram(0, dram);
        let operations = low_power_entries.saturating_add(case.exits);
        let expected =
            watts_from_activity(events, operations, 0, 0.000_004, 0.000_003, 0.000_000_5);
        let Some(record) = record_for_target(&records, "memory.dram") else {
            failures.push(format!(
                "{}: missing memory.dram; expected dynamic watts {expected:.12}, residency {}",
                case.name, case.expected_residency
            ));
            continue;
        };
        let residency = record.residency_ticks(PowerStateKind::On);

        if residency != case.expected_residency {
            failures.push(format!(
                "{}: memory.dram residency {residency} != {}",
                case.name, case.expected_residency
            ));
        }
        if (record.dynamic_watts() - expected).abs() >= 1e-12 {
            failures.push(format!(
                "{}: memory.dram dynamic watts {} != canonical {expected:.12} from events={events}, operations={operations}, bytes=0",
                case.name,
                record.dynamic_watts()
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "low-power DRAM boundary failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn run_power_suppresses_zero_memory_resources() {
    let records = run_memory_power_records(20, &Rem6MemoryResourceSummary::default());

    assert!(records.is_empty(), "zero memory inputs emitted {records:?}");
}

#[test]
fn run_dram_power_uses_canonical_byte_total() {
    let dram = Rem6DramResourceSummary {
        activity: 2,
        active: 1,
        active_banks: 1,
        accesses: 2,
        reads: 1,
        writes: 1,
        read_bytes: 8,
        write_bytes: 4,
        commands: 3,
        ..Rem6DramResourceSummary::default()
    };
    let records = run_records_with_dram(20, dram);
    let record = record_for_target(&records, "memory.dram").unwrap();
    let expected = watts_from_activity(2, 3, 12, 0.000_004, 0.000_003, 0.000_000_5);

    assert!(
        (record.dynamic_watts() - expected).abs() < 1e-12,
        "memory.dram dynamic watts {} != canonical {expected} from 12 bytes",
        record.dynamic_watts()
    );
}

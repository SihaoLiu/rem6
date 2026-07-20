use super::*;

fn run_records_with_dram(final_tick: u64, dram: &Rem6DramSummary) -> Vec<PowerAnalysisRecord> {
    run_power_analysis_records_from_parts(
        final_tick,
        &[],
        &CliDataCacheSummary::default(),
        &CliDataCacheSummary::default(),
        &Rem6MemoryResourceSummary::default(),
        dram,
    )
}

fn record_for_target<'a>(
    records: &'a [PowerAnalysisRecord],
    target: &str,
) -> Option<&'a PowerAnalysisRecord> {
    records.iter().find(|record| record.target() == target)
}

#[test]
fn run_power_emits_refresh_only_dram_resource() {
    let dram = Rem6DramSummary {
        refreshes: 1,
        refresh_ticks: 9,
        ..Rem6DramSummary::default()
    };
    let records = run_records_with_dram(0, &dram);
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
        let dram = Rem6DramSummary {
            low_power_active_powerdown_entries: case.active_powerdown_entries,
            low_power_active_powerdown_ticks: case.active_powerdown_ticks,
            low_power_precharge_powerdown_entries: case.precharge_powerdown_entries,
            low_power_precharge_powerdown_ticks: case.precharge_powerdown_ticks,
            low_power_self_refresh_entries: case.self_refresh_entries,
            low_power_self_refresh_ticks: case.self_refresh_ticks,
            low_power_exits: case.exits,
            low_power_exit_latency_ticks: case.exit_latency_ticks,
            ..Rem6DramSummary::default()
        };
        let records = run_records_with_dram(0, &dram);
        let low_power_entries = dram
            .low_power_active_powerdown_entries
            .saturating_add(dram.low_power_precharge_powerdown_entries)
            .saturating_add(dram.low_power_self_refresh_entries);
        let events = low_power_entries.max(dram.low_power_exits);
        let operations = low_power_entries.saturating_add(dram.low_power_exits);
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
    let records = run_records_with_dram(20, &Rem6DramSummary::default());

    assert!(records.is_empty(), "zero memory inputs emitted {records:?}");
}

#[test]
fn run_dram_power_uses_canonical_byte_total() {
    let dram = Rem6DramSummary {
        active_banks: 1,
        accesses: 2,
        reads: 1,
        writes: 1,
        read_bytes: 8,
        write_bytes: 4,
        commands: 3,
        ..Rem6DramSummary::default()
    };
    let records = run_records_with_dram(20, &dram);
    let record = record_for_target(&records, "memory.dram").unwrap();
    let expected = watts_from_activity(2, 3, 12, 0.000_004, 0.000_003, 0.000_000_5);

    assert!(
        (record.dynamic_watts() - expected).abs() < 1e-12,
        "memory.dram dynamic watts {} != canonical {expected} from 12 bytes",
        record.dynamic_watts()
    );
}

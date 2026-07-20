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
    let dram = Rem6DramSummary {
        low_power_active_powerdown_entries: 2,
        low_power_active_powerdown_ticks: 7,
        low_power_precharge_powerdown_entries: 3,
        low_power_precharge_powerdown_ticks: 9,
        low_power_self_refresh_entries: 5,
        low_power_self_refresh_ticks: 11,
        low_power_exits: 4,
        ..Rem6DramSummary::default()
    };
    let records = run_records_with_dram(0, &dram);
    let low_power_entries = dram
        .low_power_active_powerdown_entries
        .saturating_add(dram.low_power_precharge_powerdown_entries)
        .saturating_add(dram.low_power_self_refresh_entries);
    let events = low_power_entries.max(dram.low_power_exits);
    let operations = low_power_entries.saturating_add(dram.low_power_exits);
    let expected = watts_from_activity(events, operations, 0, 0.000_004, 0.000_003, 0.000_000_5);
    let record = record_for_target(&records, "memory.dram").unwrap_or_else(|| {
        panic!("low-power activity must emit memory.dram with dynamic watts {expected:.12}")
    });

    assert_eq!(record.residency_ticks(PowerStateKind::On), 11);
    assert!(
        (record.dynamic_watts() - expected).abs() < 1e-12,
        "memory.dram dynamic watts {} != canonical {expected:.12} from events={events}, operations={operations}, bytes=0",
        record.dynamic_watts()
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

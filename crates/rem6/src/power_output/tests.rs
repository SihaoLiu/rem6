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
        low_power_self_refresh_entries: 1,
        low_power_self_refresh_ticks: 11,
        ..Rem6DramSummary::default()
    };
    let records = run_records_with_dram(0, &dram);
    let record = record_for_target(&records, "memory.dram").expect("self refresh is DRAM activity");

    assert_eq!(record.residency_ticks(PowerStateKind::On), 11);
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

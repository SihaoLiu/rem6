use std::collections::BTreeMap;

use rem6_stats::StatSnapshot;

pub(super) fn stats_snapshot_text(snapshot: &StatSnapshot) -> String {
    let mut output = "---------- Begin Simulation Statistics ----------\n".to_string();
    append_gem5_derived_text_stats(&mut output, snapshot);
    for sample in snapshot.samples() {
        output.push_str(&format!(
            "{:<64} {:>20} # kind={} unit={} reset_policy={}\n",
            sample.path(),
            sample.value(),
            sample.kind(),
            sample.unit(),
            sample.reset_policy()
        ));
        for bucket in sample.histogram_buckets() {
            output.push_str(&format!(
                "{:<64} {:>20} # histogram_bucket={} unit={} reset_policy={}\n",
                format!("{}.bucket", sample.path()),
                bucket.count(),
                bucket.bucket(),
                sample.unit(),
                sample.reset_policy()
            ));
        }
    }
    output.push_str("\n---------- End Simulation Statistics   ----------\n");
    output
}

fn append_gem5_derived_text_stats(output: &mut String, snapshot: &StatSnapshot) {
    if let (Some(final_tick), Some(sim_freq)) = (
        snapshot_value(snapshot, "finalTick"),
        snapshot_value(snapshot, "simFreq"),
    ) {
        if sim_freq != 0 {
            output.push_str(&format!(
                "{:<64} {:>20} # kind=derived unit=Second reset_policy=constant\n",
                "simSeconds",
                format_sim_seconds(final_tick, sim_freq)
            ));
        }
    }
    append_gem5_mem_ctrl_bandwidth_alias_stats(output, snapshot);
    append_gem5_dram_interface_ratio_stats(output, snapshot);
    append_gem5_dram_interface_latency_stats(output, snapshot);
    append_gem5_cpu_ratio_stats(output, snapshot);
    append_gem5_l1_cache_alias_stats(output, snapshot);
    append_gem5_shared_l2_cache_alias_stats(output, snapshot);
}

fn format_sim_seconds(final_tick: u64, sim_freq: u64) -> String {
    let whole = final_tick / sim_freq;
    let remainder = final_tick % sim_freq;
    let fractional = (u128::from(remainder) * 1_000_000_000_000_u128) / u128::from(sim_freq);
    format!("{whole}.{fractional:012}")
}

fn snapshot_value(snapshot: &StatSnapshot, path: &str) -> Option<u64> {
    snapshot
        .samples()
        .iter()
        .find(|sample| sample.path() == path)
        .map(|sample| sample.value())
}

fn append_gem5_l1_cache_alias_stats(output: &mut String, snapshot: &StatSnapshot) {
    if snapshot_value(snapshot, "sim.cores") != Some(1) {
        return;
    }
    append_gem5_l1_cache_alias_stats_for(
        output,
        snapshot,
        "system.cpu.icache",
        "sim.instruction_cache",
    );
    append_gem5_l1_cache_alias_stats_for(output, snapshot, "system.cpu.dcache", "sim.data_cache");
}

fn append_gem5_l1_cache_alias_stats_for(
    output: &mut String,
    snapshot: &StatSnapshot,
    alias_prefix: &str,
    source_prefix: &str,
) {
    let Some(inputs) = gem5_cache_hit_miss_inputs(snapshot, source_prefix) else {
        return;
    };
    if can_emit_gem5_l1_cache_demand_alias_stats(snapshot, source_prefix) {
        append_gem5_cache_hit_miss_alias_stats(
            output,
            alias_prefix,
            "demand",
            inputs.hits,
            inputs.misses,
        );
    }
    append_gem5_cache_hit_miss_alias_stats(
        output,
        alias_prefix,
        "overall",
        inputs.hits,
        inputs.misses,
    );
}

fn can_emit_gem5_l1_cache_demand_alias_stats(snapshot: &StatSnapshot, source_prefix: &str) -> bool {
    snapshot_value(snapshot, &format!("{source_prefix}.prefetch.issued")) == Some(0)
        && snapshot_value(snapshot, &format!("{source_prefix}.prefetch.queue.issued")) == Some(0)
}

#[derive(Clone, Copy, Debug)]
struct CacheHitMissInputs {
    hits: u64,
    misses: u64,
}

impl CacheHitMissInputs {
    fn accesses(self) -> u64 {
        self.hits.saturating_add(self.misses)
    }

    fn saturating_add(self, other: Self) -> Self {
        Self {
            hits: self.hits.saturating_add(other.hits),
            misses: self.misses.saturating_add(other.misses),
        }
    }
}

fn gem5_cache_hit_miss_inputs(
    snapshot: &StatSnapshot,
    source_prefix: &str,
) -> Option<CacheHitMissInputs> {
    let (Some(hits), Some(scheduled_misses), Some(coalesced_misses)) = (
        snapshot_value(snapshot, &format!("{source_prefix}.bank.immediate_hits")),
        snapshot_value(snapshot, &format!("{source_prefix}.bank.scheduled_misses")),
        snapshot_value(snapshot, &format!("{source_prefix}.bank.coalesced_misses")),
    ) else {
        return None;
    };
    Some(CacheHitMissInputs {
        hits,
        misses: scheduled_misses.saturating_add(coalesced_misses),
    })
}

fn append_gem5_shared_l2_cache_alias_stats(output: &mut String, snapshot: &StatSnapshot) {
    let instruction_l2 = gem5_cache_hit_miss_inputs(snapshot, "sim.instruction_cache.l2");
    let data_l2 = gem5_cache_hit_miss_inputs(snapshot, "sim.data_cache.l2");
    let inputs = match (instruction_l2, data_l2) {
        (Some(instruction_l2), Some(data_l2)) => Some(instruction_l2.saturating_add(data_l2)),
        (Some(inputs), None) | (None, Some(inputs)) => Some(inputs),
        (None, None) => None,
    };
    let Some(inputs) = inputs else {
        return;
    };
    if inputs.accesses() == 0 {
        return;
    }
    append_gem5_cache_hit_miss_alias_stats(
        output,
        "system.l2",
        "overall",
        inputs.hits,
        inputs.misses,
    );
}

fn append_gem5_cache_hit_miss_alias_stats(
    output: &mut String,
    alias_prefix: &str,
    alias_kind: &str,
    hits: u64,
    misses: u64,
) {
    append_derived_count_stat(output, &format!("{alias_prefix}.{alias_kind}Hits"), hits);
    append_derived_count_stat(
        output,
        &format!("{alias_prefix}.{alias_kind}Misses"),
        misses,
    );
    let accesses = hits.saturating_add(misses);
    append_derived_count_stat(
        output,
        &format!("{alias_prefix}.{alias_kind}Accesses"),
        accesses,
    );
    if accesses != 0 {
        append_derived_ratio_stat(
            output,
            &format!("{alias_prefix}.{alias_kind}MissRate"),
            misses,
            accesses,
        );
    }
}

fn append_derived_count_stat(output: &mut String, path: &str, value: u64) {
    output.push_str(&format!(
        "{path:<64} {value:>20} # kind=derived unit=Count reset_policy=monotonic\n"
    ));
}

fn append_derived_ratio_stat(output: &mut String, path: &str, numerator: u64, denominator: u64) {
    output.push_str(&format!(
        "{path:<64} {:>20} # kind=derived unit=Ratio reset_policy=monotonic\n",
        format_fixed_ratio(numerator, denominator)
    ));
}

fn append_gem5_mem_ctrl_bandwidth_alias_stats(output: &mut String, snapshot: &StatSnapshot) {
    let (Some(final_tick), Some(sim_freq)) = (
        snapshot_value(snapshot, "finalTick"),
        snapshot_value(snapshot, "simFreq"),
    ) else {
        return;
    };
    if final_tick == 0 || sim_freq == 0 {
        return;
    }
    append_gem5_mem_ctrl_bandwidth_alias_stat(
        output,
        snapshot,
        "system.mem_ctrl.avgRdBWSys",
        "system.mem_ctrl.bytesReadSys",
        sim_freq,
        final_tick,
        1,
        Some(8),
    );
    append_gem5_mem_ctrl_bandwidth_alias_stat(
        output,
        snapshot,
        "system.mem_ctrl.avgWrBWSys",
        "system.mem_ctrl.bytesWrittenSys",
        sim_freq,
        final_tick,
        1,
        Some(8),
    );
    append_gem5_mem_ctrl_bandwidth_alias_stat(
        output,
        snapshot,
        "system.mem_ctrl.dram.avgRdBW",
        "system.mem_ctrl.dram.dramBytesRead",
        sim_freq,
        final_tick,
        1_000_000,
        None,
    );
    append_gem5_mem_ctrl_bandwidth_alias_stat(
        output,
        snapshot,
        "system.mem_ctrl.dram.avgWrBW",
        "system.mem_ctrl.dram.dramBytesWritten",
        sim_freq,
        final_tick,
        1_000_000,
        None,
    );
}

fn append_gem5_mem_ctrl_bandwidth_alias_stat(
    output: &mut String,
    snapshot: &StatSnapshot,
    alias_path: &str,
    bytes_path: &str,
    sim_freq: u64,
    final_tick: u64,
    denominator_scale: u64,
    precision: Option<usize>,
) {
    let Some(bytes) = snapshot_value(snapshot, bytes_path) else {
        return;
    };
    output.push_str(&format!(
        "{alias_path:<64} {:>20} # kind=derived unit=(Byte/Second) reset_policy=monotonic\n",
        format_scaled_ratio(bytes, sim_freq, final_tick, denominator_scale, precision)
    ));
}

fn append_gem5_dram_interface_ratio_stats(output: &mut String, snapshot: &StatSnapshot) {
    append_gem5_dram_interface_row_hit_rate_stat(
        output,
        snapshot,
        "system.mem_ctrl.dram.readRowHitRate",
        "system.mem_ctrl.dram.readRowHits",
        "system.mem_ctrl.dram.readBursts",
    );
    append_gem5_dram_interface_row_hit_rate_stat(
        output,
        snapshot,
        "system.mem_ctrl.dram.writeRowHitRate",
        "system.mem_ctrl.dram.writeRowHits",
        "system.mem_ctrl.dram.writeBursts",
    );

    let (Some(row_hits), Some(read_bursts), Some(write_bursts)) = (
        snapshot_value(snapshot, "sim.memory.dram.row_hits"),
        snapshot_value(snapshot, "system.mem_ctrl.dram.readBursts"),
        snapshot_value(snapshot, "system.mem_ctrl.dram.writeBursts"),
    ) else {
        return;
    };
    let bursts = read_bursts.saturating_add(write_bursts);
    if bursts == 0 {
        return;
    }
    append_gem5_dram_interface_percent_ratio_stat(
        output,
        "system.mem_ctrl.dram.pageHitRate",
        row_hits,
        bursts,
    );
}

fn append_gem5_dram_interface_row_hit_rate_stat(
    output: &mut String,
    snapshot: &StatSnapshot,
    alias_path: &str,
    row_hits_path: &str,
    bursts_path: &str,
) {
    let (Some(row_hits), Some(bursts)) = (
        snapshot_value(snapshot, row_hits_path),
        snapshot_value(snapshot, bursts_path),
    ) else {
        return;
    };
    if bursts == 0 {
        return;
    }
    append_gem5_dram_interface_percent_ratio_stat(output, alias_path, row_hits, bursts);
}

fn append_gem5_dram_interface_percent_ratio_stat(
    output: &mut String,
    alias_path: &str,
    numerator: u64,
    denominator: u64,
) {
    output.push_str(&format!(
        "{alias_path:<64} {:>20} # kind=derived unit=Ratio reset_policy=monotonic\n",
        format_scaled_ratio(numerator, 100, denominator, 1, Some(2))
    ));
}

fn append_gem5_dram_interface_latency_stats(output: &mut String, snapshot: &StatSnapshot) {
    let (Some(total_latency), Some(read_bursts)) = (
        snapshot_value(snapshot, "system.mem_ctrl.dram.totMemAccLat"),
        snapshot_value(snapshot, "system.mem_ctrl.dram.readBursts"),
    ) else {
        return;
    };
    if read_bursts == 0 {
        return;
    }
    output.push_str(&format!(
        "{:<64} {:>20} # kind=derived unit=(Tick/Count) reset_policy=monotonic\n",
        "system.mem_ctrl.dram.avgMemAccLat",
        format_scaled_ratio(total_latency, 1, read_bursts, 1, Some(2))
    ));
}

#[derive(Clone, Copy, Debug, Default)]
struct CpuRatioInputs {
    instructions: Option<u64>,
    cycles: Option<u64>,
}

fn append_gem5_cpu_ratio_stats(output: &mut String, snapshot: &StatSnapshot) {
    let mut cpus = BTreeMap::<String, CpuRatioInputs>::new();
    let mut commit_stats0_instructions = BTreeMap::<String, u64>::new();
    for sample in snapshot.samples() {
        if let Some(prefix) = sample.path().strip_suffix(".numInsts") {
            if is_gem5_cpu_prefix(prefix) {
                cpus.entry(prefix.to_string()).or_default().instructions = Some(sample.value());
            }
        }
        if let Some(prefix) = sample.path().strip_suffix(".numCycles") {
            if is_gem5_cpu_prefix(prefix) {
                cpus.entry(prefix.to_string()).or_default().cycles = Some(sample.value());
            }
        }
        if let Some(prefix) = sample.path().strip_suffix(".commitStats0.numInsts") {
            if is_gem5_cpu_prefix(prefix) {
                commit_stats0_instructions.insert(prefix.to_string(), sample.value());
            }
        }
    }
    for (prefix, inputs) in &cpus {
        let (Some(instructions), Some(cycles)) = (inputs.instructions, inputs.cycles) else {
            continue;
        };
        append_gem5_cpu_ratio_stat_pair(output, prefix, instructions, cycles);
    }
    for (prefix, instructions) in commit_stats0_instructions {
        let Some(cycles) = cpus.get(&prefix).and_then(|inputs| inputs.cycles) else {
            continue;
        };
        append_gem5_cpu_ratio_stat_pair(
            output,
            &format!("{prefix}.commitStats0"),
            instructions,
            cycles,
        );
    }
}

fn append_gem5_cpu_ratio_stat_pair(
    output: &mut String,
    prefix: &str,
    instructions: u64,
    cycles: u64,
) {
    if instructions == 0 || cycles == 0 {
        return;
    }
    output.push_str(&format!(
        "{:<64} {:>20} # kind=derived unit=(Count/Cycle) reset_policy=monotonic\n",
        format!("{prefix}.ipc"),
        format_fixed_ratio(instructions, cycles)
    ));
    output.push_str(&format!(
        "{:<64} {:>20} # kind=derived unit=(Cycle/Count) reset_policy=monotonic\n",
        format!("{prefix}.cpi"),
        format_fixed_ratio(cycles, instructions)
    ));
}

fn is_gem5_cpu_prefix(prefix: &str) -> bool {
    prefix == "system.cpu"
        || prefix.strip_prefix("system.cpu").is_some_and(|suffix| {
            !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
        })
}

fn format_fixed_ratio(numerator: u64, denominator: u64) -> String {
    format!("{:.6}", numerator as f64 / denominator as f64)
}

fn format_scaled_ratio(
    value: u64,
    multiplier: u64,
    denominator: u64,
    denominator_scale: u64,
    precision: Option<usize>,
) -> String {
    let value = (value as f64 * multiplier as f64) / denominator as f64 / denominator_scale as f64;
    match precision {
        Some(precision) => format!("{value:.precision$}"),
        None if value == value.round() => format!("{value:.0}"),
        None => format!("{value:.6}"),
    }
}

#[cfg(test)]
mod tests {
    use rem6_stats::StatsRegistry;

    use super::stats_snapshot_text;

    #[test]
    fn stats_output_renders_gem5_sim_seconds_without_float_rounding() {
        let mut stats = StatsRegistry::new();
        let ticks = stats.register_counter("finalTick", "Tick").unwrap();
        let frequency = stats.register_counter("simFreq", "Hz").unwrap();
        stats.increment(ticks, 9_007_199_254_740_993).unwrap();
        stats.increment(frequency, 1_000_000_000_000).unwrap();

        let text = stats_snapshot_text(&stats.snapshot(0));

        assert!(text.contains("simSeconds"));
        assert!(text.contains("9007.199254740993"));
    }

    #[test]
    fn stats_output_renders_only_valid_gem5_cpu_ratio_prefixes() {
        let mut stats = StatsRegistry::new();
        let cpu0_insts = stats
            .register_counter("system.cpu0.numInsts", "Count")
            .unwrap();
        let cpu0_cycles = stats
            .register_counter("system.cpu0.numCycles", "Cycle")
            .unwrap();
        let cpu0_commit_insts = stats
            .register_counter("system.cpu0.commitStats0.numInsts", "Count")
            .unwrap();
        let cpu_named_insts = stats
            .register_counter("system.cpu.main.numInsts", "Count")
            .unwrap();
        let cpu_named_cycles = stats
            .register_counter("system.cpu.main.numCycles", "Cycle")
            .unwrap();
        let cpu_named_commit_insts = stats
            .register_counter("system.cpu.main.commitStats0.numInsts", "Count")
            .unwrap();
        let cpu1_insts = stats
            .register_counter("system.cpu1.numInsts", "Count")
            .unwrap();
        let cpu1_cycles = stats
            .register_counter("system.cpu1.numCycles", "Cycle")
            .unwrap();
        let cpu1_commit_insts = stats
            .register_counter("system.cpu1.commitStats0.numInsts", "Count")
            .unwrap();
        stats.increment(cpu0_insts, 3).unwrap();
        stats.increment(cpu0_cycles, 12).unwrap();
        stats.increment(cpu0_commit_insts, 3).unwrap();
        stats.increment(cpu_named_insts, 7).unwrap();
        stats.increment(cpu_named_cycles, 14).unwrap();
        stats.increment(cpu_named_commit_insts, 7).unwrap();
        stats.increment(cpu1_insts, 5).unwrap();
        stats.increment(cpu1_cycles, 0).unwrap();
        stats.increment(cpu1_commit_insts, 5).unwrap();

        let text = stats_snapshot_text(&stats.snapshot(0));

        assert!(text.contains("system.cpu0.ipc"));
        assert!(text.contains("0.250000"));
        assert!(text.contains("system.cpu0.cpi"));
        assert!(text.contains("4.000000"));
        assert!(text.contains("system.cpu0.commitStats0.ipc"));
        assert!(text.contains("system.cpu0.commitStats0.cpi"));
        assert!(!text.contains("system.cpu.main.ipc"));
        assert!(!text.contains("system.cpu.main.commitStats0.ipc"));
        assert!(!text.contains("system.cpu1.ipc"));
        assert!(!text.contains("system.cpu1.commitStats0.ipc"));
    }
}

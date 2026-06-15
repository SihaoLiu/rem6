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
    let Some(final_tick) = snapshot_value(snapshot, "finalTick") else {
        return;
    };
    let Some(sim_freq) = snapshot_value(snapshot, "simFreq") else {
        return;
    };
    if sim_freq == 0 {
        return;
    }
    output.push_str(&format!(
        "{:<64} {:>20} # kind=derived unit=Second reset_policy=constant\n",
        "simSeconds",
        format_sim_seconds(final_tick, sim_freq)
    ));
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
}

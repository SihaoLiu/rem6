use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::text::stats_snapshot_text;
use super::{
    emit_transport_stats, increment_stat, stats_snapshot_json, Rem6CliError,
    Rem6GupsExecutionSummary, Rem6GupsStatsInputs, Rem6StatsOutput,
};

pub(crate) fn gups_stats_output(
    inputs: Rem6GupsStatsInputs<'_>,
) -> Result<Rem6StatsOutput, Rem6CliError> {
    let mut stats = StatsRegistry::new();
    increment_stat(
        &mut stats,
        "sim.gups.memory_start",
        "Address",
        StatResetPolicy::Constant,
        inputs.config.memory_start(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.memory_size",
        "Byte",
        StatResetPolicy::Constant,
        inputs.config.memory_size(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.updates",
        "Count",
        StatResetPolicy::Constant,
        inputs.config.updates(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.max_tick",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.max_tick(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.rng_state",
        "Value",
        StatResetPolicy::Constant,
        inputs.config.rng_state(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.scheduler.min_remote_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.min_remote_delay(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.memory.route_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.memory_route_delay(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.memory.dumps",
        "Count",
        StatResetPolicy::Constant,
        inputs.memory_dumps.len() as u64,
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.final_tick",
        "Tick",
        StatResetPolicy::Monotonic,
        inputs.execution.final_tick,
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.scheduled_requests",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.scheduled_requests,
    )?;
    emit_gups_response_stats(&mut stats, inputs.execution)?;
    emit_gups_profile_stats(&mut stats, inputs.execution)?;
    emit_transport_stats(&mut stats, "sim.gups.transport", inputs.transport)?;

    let snapshot = stats.snapshot(0);
    Ok(Rem6StatsOutput {
        json: stats_snapshot_json(&snapshot),
        text: stats_snapshot_text(&snapshot),
    })
}

fn emit_gups_response_stats(
    stats: &mut StatsRegistry,
    execution: &Rem6GupsExecutionSummary,
) -> Result<(), Rem6CliError> {
    let response_stats = &execution.response_stats;
    increment_stat(
        stats,
        "sim.gups.responses",
        "Count",
        StatResetPolicy::Monotonic,
        response_stats.response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.gups.responses.completed",
        "Count",
        StatResetPolicy::Monotonic,
        response_stats.completed_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.gups.responses.retry",
        "Count",
        StatResetPolicy::Monotonic,
        response_stats.retry_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.gups.responses.store_conditional_failed",
        "Count",
        StatResetPolicy::Monotonic,
        response_stats.store_conditional_failed_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.gups.responses.reads",
        "Count",
        StatResetPolicy::Monotonic,
        response_stats.read_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.gups.responses.writes",
        "Count",
        StatResetPolicy::Monotonic,
        response_stats.write_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.gups.response_data_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        response_stats.response_data_byte_count(),
    )
}

fn emit_gups_profile_stats(
    stats: &mut StatsRegistry,
    execution: &Rem6GupsExecutionSummary,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        "sim.gups.traffic_profiles",
        "Count",
        StatResetPolicy::Constant,
        execution.profile_summaries.len() as u64,
    )?;

    for (index, summary) in execution.profile_summaries.iter().enumerate() {
        let profile = summary.profile();
        let generator_summary = profile.summary();
        let prefix = format!("sim.gups.traffic_profile{index}");
        increment_stat(
            stats,
            &format!("{prefix}.state"),
            "Value",
            StatResetPolicy::Constant,
            u64::from(summary.state().get()),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.generator_class"),
            "Value",
            StatResetPolicy::Constant,
            profile.generator_class().stat_code(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.memory_profile"),
            "Value",
            StatResetPolicy::Constant,
            profile.memory_profile().stat_code(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.packets"),
            "Count",
            StatResetPolicy::Monotonic,
            generator_summary.packet_count(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.reads"),
            "Count",
            StatResetPolicy::Monotonic,
            generator_summary.read_count(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.writes"),
            "Count",
            StatResetPolicy::Monotonic,
            generator_summary.write_count(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.bytes_read"),
            "Byte",
            StatResetPolicy::Monotonic,
            generator_summary.bytes_read(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.bytes_written"),
            "Byte",
            StatResetPolicy::Monotonic,
            generator_summary.bytes_written(),
        )?;
        if let Some(first_tick) = generator_summary.first_tick() {
            increment_stat(
                stats,
                &format!("{prefix}.first_tick"),
                "Tick",
                StatResetPolicy::Monotonic,
                first_tick,
            )?;
        }
        if let Some(last_tick) = generator_summary.last_tick() {
            increment_stat(
                stats,
                &format!("{prefix}.last_tick"),
                "Tick",
                StatResetPolicy::Monotonic,
                last_tick,
            )?;
        }
    }

    Ok(())
}

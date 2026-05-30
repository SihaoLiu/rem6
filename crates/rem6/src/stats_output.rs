use rem6_stats::{StatResetPolicy, StatSnapshot, StatsRegistry};

use super::formatting::json_escape;
use super::{
    parallel_stats, stats_error, Rem6CliError, Rem6ExecutionStop, Rem6ExecutionSummary,
    Rem6MemoryTransportCounters, Rem6MemoryTransportSummary, Rem6RunConfig,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Rem6StatsOutput {
    pub(super) json: String,
    pub(super) text: String,
}

pub(super) struct Rem6StatsInputs<'a> {
    pub(super) binary_bytes: u64,
    pub(super) load_segments: u64,
    pub(super) start_address: u64,
    pub(super) config: &'a Rem6RunConfig,
    pub(super) execution: Option<&'a Rem6ExecutionSummary>,
}

pub(super) fn run_stats_output(
    inputs: Rem6StatsInputs<'_>,
) -> Result<Rem6StatsOutput, Rem6CliError> {
    let mut stats = StatsRegistry::new();
    increment_stat(
        &mut stats,
        "sim.binary.bytes",
        "Byte",
        StatResetPolicy::Constant,
        inputs.binary_bytes,
    )?;
    increment_stat(
        &mut stats,
        "sim.elf.load_segments",
        "Count",
        StatResetPolicy::Constant,
        inputs.load_segments,
    )?;
    increment_stat(
        &mut stats,
        "sim.max_tick",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.max_tick(),
    )?;
    increment_stat(
        &mut stats,
        "sim.start_address",
        "Address",
        StatResetPolicy::Constant,
        inputs.start_address,
    )?;
    increment_stat(
        &mut stats,
        "sim.parallel.scheduler.min_remote_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.min_remote_delay(),
    )?;
    increment_stat(
        &mut stats,
        "sim.memory.route_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.memory_route_delay(),
    )?;
    increment_stat(
        &mut stats,
        "sim.host.event_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.host_event_delay(),
    )?;
    if let Some(max_instructions) = inputs.config.max_instructions() {
        increment_stat(
            &mut stats,
            "sim.instructions.limit",
            "Count",
            StatResetPolicy::Constant,
            max_instructions,
        )?;
    }
    increment_stat(
        &mut stats,
        "sim.cores",
        "Count",
        StatResetPolicy::Constant,
        inputs.config.cores() as u64,
    )?;
    increment_stat(
        &mut stats,
        "sim.parallel.scheduler.worker_limit",
        "Count",
        StatResetPolicy::Constant,
        inputs.config.parallel_workers() as u64,
    )?;

    if let Some(execution) = inputs.execution {
        increment_stat(
            &mut stats,
            "sim.instructions.committed",
            "Count",
            StatResetPolicy::Monotonic,
            execution.committed_instructions,
        )?;
        increment_stat(
            &mut stats,
            "sim.final_tick",
            "Tick",
            StatResetPolicy::Monotonic,
            execution.final_tick,
        )?;
        match execution.stop {
            Rem6ExecutionStop::HostTrap { stop_code, .. } => {
                increment_stat(
                    &mut stats,
                    "sim.stop.host_trap",
                    "Count",
                    StatResetPolicy::Constant,
                    1,
                )?;
                increment_stat(
                    &mut stats,
                    "sim.stop_code",
                    "Count",
                    StatResetPolicy::Constant,
                    stop_code as u64,
                )?;
            }
            Rem6ExecutionStop::TickLimit { .. } => {
                increment_stat(
                    &mut stats,
                    "sim.stop.tick_limit",
                    "Count",
                    StatResetPolicy::Constant,
                    1,
                )?;
            }
            Rem6ExecutionStop::InstructionLimit { .. } => {
                increment_stat(
                    &mut stats,
                    "sim.stop.instruction_limit",
                    "Count",
                    StatResetPolicy::Constant,
                    1,
                )?;
            }
        }
        increment_stat(
            &mut stats,
            "sim.memory.dumps",
            "Count",
            StatResetPolicy::Constant,
            execution.memory_dumps.len() as u64,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.loads",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_loads,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.stores",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_stores,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.atomics",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_atomics,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.load_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            execution.data_load_bytes,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.store_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            execution.data_store_bytes,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.atomic_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            execution.data_atomic_bytes,
        )?;
        parallel_stats::emit_scheduler_stats(&mut stats, execution)?;
        emit_transport_stats(&mut stats, "sim.memory.fetch", &execution.fetch_transport)?;
        emit_transport_stats(&mut stats, "sim.memory.data", &execution.data_transport)?;
        for core in &execution.cores {
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.instructions.committed", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.committed_instructions,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.loads", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.data_loads,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.stores", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.data_stores,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.atomics", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.data_atomics,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.load_bytes", core.cpu),
                "Byte",
                StatResetPolicy::Monotonic,
                core.data_load_bytes,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.store_bytes", core.cpu),
                "Byte",
                StatResetPolicy::Monotonic,
                core.data_store_bytes,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.atomic_bytes", core.cpu),
                "Byte",
                StatResetPolicy::Monotonic,
                core.data_atomic_bytes,
            )?;
        }
    }

    let snapshot = stats.snapshot(0);
    Ok(Rem6StatsOutput {
        json: stats_snapshot_json(&snapshot),
        text: stats_snapshot_text(&snapshot),
    })
}

fn stats_snapshot_json(snapshot: &StatSnapshot) -> String {
    let samples = snapshot
        .samples()
        .iter()
        .map(|sample| {
            format!(
                "{{\"path\":\"{}\",\"unit\":\"{}\",\"value\":{},\"reset_policy\":\"{}\"}}",
                json_escape(sample.path()),
                json_escape(sample.unit()),
                sample.value(),
                sample.reset_policy()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{samples}]")
}

fn stats_snapshot_text(snapshot: &StatSnapshot) -> String {
    let mut output = "\n---------- Begin Simulation Statistics ----------\n".to_string();
    for sample in snapshot.samples() {
        output.push_str(&format!(
            "{:<64} {:>20} # unit={} reset_policy={}\n",
            sample.path(),
            sample.value(),
            sample.unit(),
            sample.reset_policy()
        ));
    }
    output.push_str("\n---------- End Simulation Statistics   ----------\n");
    output
}

pub(super) fn increment_stat(
    stats: &mut StatsRegistry,
    path: &str,
    unit: &str,
    reset_policy: StatResetPolicy,
    value: u64,
) -> Result<(), Rem6CliError> {
    let stat = stats
        .register_counter_with_reset_policy(path, unit, reset_policy)
        .map_err(stats_error)?;
    stats.increment(stat, value).map_err(stats_error)
}

fn emit_transport_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    summary: &Rem6MemoryTransportSummary,
) -> Result<(), Rem6CliError> {
    emit_transport_counters(stats, prefix, &summary.counters)?;
    for route in &summary.routes {
        let route_prefix = format!(
            "{prefix}.route{}.source.{}",
            route.route.get(),
            endpoint_stat_path(&route.source)
        );
        emit_transport_counters(stats, &route_prefix, &route.counters)?;
    }
    Ok(())
}

fn emit_transport_counters(
    stats: &mut StatsRegistry,
    prefix: &str,
    counters: &Rem6MemoryTransportCounters,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!("{prefix}.requests"),
        "Count",
        StatResetPolicy::Monotonic,
        counters.requests,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.request_arrivals"),
        "Count",
        StatResetPolicy::Monotonic,
        counters.request_arrivals,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.responses"),
        "Count",
        StatResetPolicy::Monotonic,
        counters.responses,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.response_arrivals"),
        "Count",
        StatResetPolicy::Monotonic,
        counters.response_arrivals,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.round_trip_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        counters.round_trip_ticks,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.max_round_trip_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        counters.max_round_trip_ticks,
    )
}

fn endpoint_stat_path(endpoint: &str) -> String {
    endpoint
        .split('.')
        .map(stat_path_segment)
        .collect::<Vec<_>>()
        .join(".")
}

fn stat_path_segment(segment: &str) -> String {
    let mut output = String::new();
    for (index, character) in segment.chars().enumerate() {
        if index == 0 {
            if character.is_ascii_alphabetic() || character == '_' {
                output.push(character);
            } else {
                output.push('_');
                if character.is_ascii_alphanumeric() {
                    output.push(character);
                }
            }
        } else if character.is_ascii_alphanumeric() || character == '_' {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "_".to_string()
    } else {
        output
    }
}

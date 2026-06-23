use rem6_stats::{StatResetPolicy, StatsRegistry};

use crate::{
    Rem6CacheResourceSummary, Rem6DramResourceSummary, Rem6FabricResourceSummary,
    Rem6TransportResourceSummary,
};

use super::{dram::emit_dram_target_stats, increment_stat, Rem6CliError, Rem6ExecutionSummary};

pub(super) fn emit_memory_resource_stats(
    stats: &mut StatsRegistry,
    execution: &Rem6ExecutionSummary,
) -> Result<(), Rem6CliError> {
    for (path, value) in [
        (
            "sim.memory.resources.activity",
            execution.memory_resources.activity,
        ),
        (
            "sim.memory.resources.active",
            execution.memory_resources.active,
        ),
    ] {
        increment_stat(stats, path, "Count", StatResetPolicy::Monotonic, value)?;
    }
    emit_cache_resource_stats(
        stats,
        "sim.memory.resources.cache",
        &execution.memory_resources.cache,
    )?;
    emit_cache_resource_stats(
        stats,
        "sim.memory.resources.cache.l1",
        &execution.memory_resources.cache_l1,
    )?;
    emit_cache_resource_stats(
        stats,
        "sim.memory.resources.cache.l2",
        &execution.memory_resources.cache_l2,
    )?;
    emit_cache_resource_stats(
        stats,
        "sim.memory.resources.cache.l3",
        &execution.memory_resources.cache_l3,
    )?;
    emit_transport_resource_stats(
        stats,
        "sim.memory.resources.transport",
        &execution.memory_resources.transport,
    )?;
    emit_transport_resource_stats(
        stats,
        "sim.memory.resources.transport.fetch",
        &execution.memory_resources.transport_fetch,
    )?;
    emit_transport_resource_stats(
        stats,
        "sim.memory.resources.transport.data",
        &execution.memory_resources.transport_data,
    )?;
    emit_fabric_resource_stats(
        stats,
        "sim.memory.resources.fabric",
        &execution.memory_resources.fabric,
    )?;
    emit_dram_resource_stats(
        stats,
        "sim.memory.resources.dram",
        &execution.memory_resources.dram,
    )?;
    Ok(())
}

fn emit_dram_resource_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    summary: &Rem6DramResourceSummary,
) -> Result<(), Rem6CliError> {
    for (suffix, unit, value) in [
        ("activity", "Count", summary.activity),
        ("active", "Count", summary.active),
        ("active_targets", "Count", summary.active_targets),
        ("active_ports", "Count", summary.active_ports),
        ("active_banks", "Count", summary.active_banks),
        ("accesses", "Count", summary.accesses),
        ("reads", "Count", summary.reads),
        ("writes", "Count", summary.writes),
        ("row_hits", "Count", summary.row_hits),
        ("row_misses", "Count", summary.row_misses),
        ("commands", "Count", summary.commands),
        ("turnarounds", "Count", summary.turnarounds),
        (
            "total_ready_latency_ticks",
            "Tick",
            summary.total_ready_latency_ticks,
        ),
        (
            "max_ready_latency_ticks",
            "Tick",
            summary.max_ready_latency_ticks,
        ),
    ] {
        increment_stat(
            stats,
            &format!("{prefix}.{suffix}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    for target in &summary.targets {
        emit_dram_target_stats(stats, prefix, target)?;
    }
    Ok(())
}

fn emit_fabric_resource_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    summary: &Rem6FabricResourceSummary,
) -> Result<(), Rem6CliError> {
    for (suffix, unit, value) in [
        ("activity", "Count", summary.activity),
        ("active", "Count", summary.active),
        (
            "active_virtual_networks",
            "Count",
            summary.active_virtual_networks,
        ),
        ("bytes", "Byte", summary.bytes),
        ("flits", "Count", summary.flits),
        ("occupied_ticks", "Tick", summary.occupied_ticks),
        ("queue_delay_ticks", "Tick", summary.queue_delay_ticks),
        (
            "max_queue_delay_ticks",
            "Tick",
            summary.max_queue_delay_ticks,
        ),
        ("credit_delay_ticks", "Tick", summary.credit_delay_ticks),
        (
            "max_credit_delay_ticks",
            "Tick",
            summary.max_credit_delay_ticks,
        ),
        ("contended_lanes", "Count", summary.contended_lanes),
    ] {
        increment_stat(
            stats,
            &format!("{prefix}.{suffix}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    Ok(())
}

fn emit_transport_resource_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    summary: &Rem6TransportResourceSummary,
) -> Result<(), Rem6CliError> {
    for (suffix, unit, value) in [
        ("activity", "Count", summary.activity),
        ("active", "Count", summary.active),
        ("request_arrivals", "Count", summary.request_arrivals),
        ("responses", "Count", summary.responses),
        ("response_arrivals", "Count", summary.response_arrivals),
        ("round_trip_ticks", "Tick", summary.round_trip_ticks),
        ("max_round_trip_ticks", "Tick", summary.max_round_trip_ticks),
    ] {
        increment_stat(
            stats,
            &format!("{prefix}.{suffix}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    Ok(())
}

fn emit_cache_resource_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    summary: &Rem6CacheResourceSummary,
) -> Result<(), Rem6CliError> {
    for (suffix, value) in [
        ("activity", summary.activity),
        ("active", summary.active),
        ("cpu_responses", summary.cpu_responses),
        ("directory_decisions", summary.directory_decisions),
        ("dram_accesses", summary.dram_accesses),
        ("bank.accepted", summary.bank_accepted),
        ("bank.immediate_hits", summary.bank_immediate_hits),
        ("bank.scheduled_misses", summary.bank_scheduled_misses),
        ("bank.coalesced_misses", summary.bank_coalesced_misses),
    ] {
        increment_stat(
            stats,
            &format!("{prefix}.{suffix}"),
            "Count",
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    Ok(())
}

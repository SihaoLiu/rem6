use rem6_stats::{StatResetPolicy, StatsRegistry};

use crate::{
    Rem6CacheResourceHierarchySummary, Rem6CacheResourceSummary, Rem6DramResourceSummary,
    Rem6FabricResourceSummary, Rem6TransportResourceSummary,
};

use super::{
    dram::emit_dram_target_stats,
    fabric::{emit_fabric_hop_stats, emit_fabric_lane_stats, emit_fabric_virtual_network_stats},
    increment_stat, Rem6CliError, Rem6ExecutionSummary,
};

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
    emit_cache_resource_hierarchy_stats(
        stats,
        "sim.memory.resources.cache.instruction",
        &execution.memory_resources.cache_instruction,
    )?;
    emit_cache_resource_hierarchy_stats(
        stats,
        "sim.memory.resources.cache.data",
        &execution.memory_resources.cache_data,
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
        ("refreshes", "Count", summary.refreshes),
        ("refresh_ticks", "Tick", summary.refresh_ticks),
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
        (
            "low_power.active_powerdown.entries",
            "Count",
            summary.low_power_active_powerdown_entries,
        ),
        (
            "low_power.active_powerdown.ticks",
            "Tick",
            summary.low_power_active_powerdown_ticks,
        ),
        (
            "low_power.precharge_powerdown.entries",
            "Count",
            summary.low_power_precharge_powerdown_entries,
        ),
        (
            "low_power.precharge_powerdown.ticks",
            "Tick",
            summary.low_power_precharge_powerdown_ticks,
        ),
        (
            "low_power.self_refresh.entries",
            "Count",
            summary.low_power_self_refresh_entries,
        ),
        (
            "low_power.self_refresh.ticks",
            "Tick",
            summary.low_power_self_refresh_ticks,
        ),
        ("low_power.exits", "Count", summary.low_power_exits),
        (
            "low_power.exit_latency_ticks",
            "Tick",
            summary.low_power_exit_latency_ticks,
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
    emit_fabric_virtual_network_stats(stats, prefix, summary.virtual_network_activities())?;
    emit_fabric_lane_stats(stats, prefix, summary.lane_activities())?;
    emit_fabric_hop_stats(stats, prefix, summary.hop_activities())
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
        ("prefetch.identified", summary.prefetch_identified),
        ("prefetch.issued", summary.prefetch_issued),
        ("prefetch.useful", summary.prefetch_useful),
        ("prefetch.useful_but_miss", summary.prefetch_useful_but_miss),
        ("prefetch.unused", summary.prefetch_unused),
        (
            "prefetch.demand_mshr_misses",
            summary.prefetch_demand_mshr_misses,
        ),
        ("prefetch.hit_in_cache", summary.prefetch_hit_in_cache),
        ("prefetch.hit_in_mshr", summary.prefetch_hit_in_mshr),
        (
            "prefetch.hit_in_write_buffer",
            summary.prefetch_hit_in_write_buffer,
        ),
        ("prefetch.late", summary.prefetch_late),
        ("prefetch.span_page", summary.prefetch_span_page),
        (
            "prefetch.useful_span_page",
            summary.prefetch_useful_span_page,
        ),
        ("prefetch.in_cache", summary.prefetch_in_cache),
        ("prefetch.queue.enqueued", summary.prefetch_queue_enqueued),
        ("prefetch.queue.issued", summary.prefetch_queue_issued),
        ("prefetch.queue.dropped", summary.prefetch_queue_dropped),
        (
            "prefetch.translation_queue.enqueued",
            summary.prefetch_translation_queue_enqueued,
        ),
        (
            "prefetch.translation_queue.issued",
            summary.prefetch_translation_queue_issued,
        ),
        (
            "prefetch.translation_queue.translated",
            summary.prefetch_translation_queue_translated,
        ),
        (
            "prefetch.translation_queue.dropped",
            summary.prefetch_translation_queue_dropped,
        ),
    ] {
        increment_stat(
            stats,
            &format!("{prefix}.{suffix}"),
            "Count",
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    if let Some(accuracy_ppm) = summary.prefetch_accuracy_ppm {
        increment_stat(
            stats,
            &format!("{prefix}.prefetch.accuracy_ppm"),
            "Ppm",
            StatResetPolicy::Monotonic,
            accuracy_ppm,
        )?;
    }
    if let Some(coverage_ppm) = summary.prefetch_coverage_ppm {
        increment_stat(
            stats,
            &format!("{prefix}.prefetch.coverage_ppm"),
            "Ppm",
            StatResetPolicy::Monotonic,
            coverage_ppm,
        )?;
    }
    Ok(())
}

fn emit_cache_resource_hierarchy_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    summary: &Rem6CacheResourceHierarchySummary,
) -> Result<(), Rem6CliError> {
    emit_cache_resource_stats(stats, prefix, &summary.aggregate)?;
    emit_cache_resource_stats(stats, &format!("{prefix}.l1"), &summary.l1)?;
    emit_cache_resource_stats(stats, &format!("{prefix}.l2"), &summary.l2)?;
    emit_cache_resource_stats(stats, &format!("{prefix}.l3"), &summary.l3)
}

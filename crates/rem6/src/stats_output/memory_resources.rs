use rem6_stats::{StatResetPolicy, StatsRegistry};

use crate::Rem6CacheResourceSummary;

use super::{increment_stat, Rem6CliError, Rem6ExecutionSummary};

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
        (
            "sim.memory.resources.transport.activity",
            execution.memory_resources.transport_activity,
        ),
        (
            "sim.memory.resources.transport.active",
            execution.memory_resources.active_transports,
        ),
        (
            "sim.memory.resources.fabric.activity",
            execution.memory_resources.fabric_activity,
        ),
        (
            "sim.memory.resources.fabric.active",
            execution.memory_resources.active_fabric_resources,
        ),
        (
            "sim.memory.resources.dram.activity",
            execution.memory_resources.dram_activity,
        ),
        (
            "sim.memory.resources.dram.active",
            execution.memory_resources.active_dram_resources,
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

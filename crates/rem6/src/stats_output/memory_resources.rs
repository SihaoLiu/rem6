use rem6_stats::{StatResetPolicy, StatsRegistry};

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
            "sim.memory.resources.cache.activity",
            execution.memory_resources.cache_activity,
        ),
        (
            "sim.memory.resources.cache.active",
            execution.memory_resources.active_caches,
        ),
        (
            "sim.memory.resources.cache.bank.accepted",
            execution.memory_resources.cache_bank_accepted,
        ),
        (
            "sim.memory.resources.cache.bank.immediate_hits",
            execution.memory_resources.cache_bank_immediate_hits,
        ),
        (
            "sim.memory.resources.cache.bank.scheduled_misses",
            execution.memory_resources.cache_bank_scheduled_misses,
        ),
        (
            "sim.memory.resources.cache.bank.coalesced_misses",
            execution.memory_resources.cache_bank_coalesced_misses,
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
    Ok(())
}

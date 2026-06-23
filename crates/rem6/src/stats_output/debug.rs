use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::{increment_stat, Rem6CliError, Rem6ExecutionSummary};

pub(super) fn emit_debug_stats(
    stats: &mut StatsRegistry,
    execution: &Rem6ExecutionSummary,
) -> Result<(), Rem6CliError> {
    let debug = &execution.debug;
    increment_stat(
        stats,
        "sim.debug.flags",
        "Count",
        StatResetPolicy::Constant,
        debug.enabled_flag_count(),
    )?;
    for (path, value) in [
        ("sim.debug.exec_trace.records", debug.exec_trace_count()),
        ("sim.debug.fetch_trace.records", debug.fetch_trace_count()),
        ("sim.debug.data_trace.records", debug.data_trace_count()),
        ("sim.debug.fabric_trace.records", debug.fabric_trace_count()),
        (
            "sim.debug.fabric_trace.lanes",
            debug.fabric_lane_trace_count(),
        ),
        (
            "sim.debug.fabric_trace.hops",
            debug.fabric_hop_trace_count(),
        ),
        ("sim.debug.memory_trace.records", debug.memory_trace_count()),
        (
            "sim.debug.memory_trace.fetch.records",
            debug.memory_fetch_trace_count(),
        ),
        (
            "sim.debug.memory_trace.data.records",
            debug.memory_data_trace_count(),
        ),
        ("sim.debug.power_trace.records", debug.power_trace_count()),
        (
            "sim.debug.syscall_trace.records",
            debug.syscall_trace_count(),
        ),
    ] {
        increment_stat(stats, path, "Count", StatResetPolicy::Monotonic, value)?;
    }
    Ok(())
}

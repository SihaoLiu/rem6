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
        ("sim.debug.data_trace.loads", debug.data_load_trace_count()),
        (
            "sim.debug.data_trace.stores",
            debug.data_store_trace_count(),
        ),
        (
            "sim.debug.data_trace.atomics",
            debug.data_atomic_trace_count(),
        ),
        ("sim.debug.dram_trace.records", debug.dram_trace_count()),
        (
            "sim.debug.dram_trace.targets",
            debug.dram_target_trace_count(),
        ),
        ("sim.debug.dram_trace.ports", debug.dram_port_trace_count()),
        ("sim.debug.dram_trace.banks", debug.dram_bank_trace_count()),
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
        (
            "sim.debug.memory_trace.events.request_sent",
            debug.memory_request_sent_trace_count(),
        ),
        (
            "sim.debug.memory_trace.events.request_arrived",
            debug.memory_request_arrived_trace_count(),
        ),
        (
            "sim.debug.memory_trace.events.response_arrived",
            debug.memory_response_arrived_trace_count(),
        ),
        (
            "sim.debug.memory_trace.response_status.completed",
            debug.memory_completed_response_trace_count(),
        ),
        (
            "sim.debug.memory_trace.response_status.retry",
            debug.memory_retry_response_trace_count(),
        ),
        (
            "sim.debug.memory_trace.response_status.store_conditional_failed",
            debug.memory_store_conditional_failed_response_trace_count(),
        ),
        ("sim.debug.power_trace.records", debug.power_trace_count()),
        (
            "sim.debug.syscall_trace.records",
            debug.syscall_trace_count(),
        ),
        (
            "sim.debug.syscall_trace.returns",
            debug.syscall_return_trace_count(),
        ),
        (
            "sim.debug.syscall_trace.exits",
            debug.syscall_exit_trace_count(),
        ),
        (
            "sim.debug.syscall_trace.blocked",
            debug.syscall_blocked_trace_count(),
        ),
    ] {
        increment_stat(stats, path, "Count", StatResetPolicy::Monotonic, value)?;
    }
    Ok(())
}

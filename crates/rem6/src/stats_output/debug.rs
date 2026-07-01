use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::{increment_stat, stat_path_segment, Rem6CliError, Rem6ExecutionSummary};

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
        ("sim.debug.trace.records", debug.trace_record_count()),
        ("sim.debug.trace.categories", debug.trace_category_count()),
        ("sim.debug.trace.active_flags", debug.active_flag_count()),
        ("sim.debug.branch_trace.records", debug.branch_trace_count()),
        (
            "sim.debug.branch_trace.conditional",
            debug.branch_conditional_trace_count(),
        ),
        (
            "sim.debug.branch_trace.unconditional",
            debug.branch_unconditional_trace_count(),
        ),
        (
            "sim.debug.branch_trace.predicted_taken",
            debug.branch_predicted_taken_trace_count(),
        ),
        (
            "sim.debug.branch_trace.resolved_taken",
            debug.branch_resolved_taken_trace_count(),
        ),
        (
            "sim.debug.branch_trace.mispredictions",
            debug.branch_misprediction_trace_count(),
        ),
        (
            "sim.debug.branch_trace.repairs",
            debug.branch_repair_trace_count(),
        ),
        (
            "sim.debug.branch_trace.flushed",
            debug.branch_flushed_trace_count(),
        ),
        (
            "sim.debug.pipeline_trace.records",
            debug.pipeline_trace_count(),
        ),
        (
            "sim.debug.pipeline_trace.stall_cycles",
            debug.pipeline_stall_cycle_trace_count(),
        ),
        (
            "sim.debug.pipeline_trace.state_changed",
            debug.pipeline_state_changed_trace_count(),
        ),
        (
            "sim.debug.pipeline_trace.advanced",
            debug.pipeline_advanced_trace_count(),
        ),
        (
            "sim.debug.pipeline_trace.retired",
            debug.pipeline_retired_trace_count(),
        ),
        (
            "sim.debug.pipeline_trace.flushed",
            debug.pipeline_flushed_trace_count(),
        ),
        (
            "sim.debug.pipeline_trace.resource_blocked",
            debug.pipeline_resource_blocked_trace_count(),
        ),
        (
            "sim.debug.pipeline_trace.ordering_blocked",
            debug.pipeline_ordering_blocked_trace_count(),
        ),
        (
            "sim.debug.pipeline_trace.branch_predictions",
            debug.pipeline_branch_prediction_trace_count(),
        ),
        (
            "sim.debug.pipeline_trace.branch_mispredictions",
            debug.pipeline_branch_misprediction_trace_count(),
        ),
        (
            "sim.debug.pipeline_trace.branch_prediction_flushed",
            debug.pipeline_branch_prediction_flushed_trace_count(),
        ),
        (
            "sim.debug.pipeline_trace.redirects",
            debug.pipeline_redirect_trace_count(),
        ),
        (
            "sim.debug.pipeline_trace.before_in_flight",
            debug.pipeline_before_in_flight_trace_count(),
        ),
        (
            "sim.debug.pipeline_trace.after_in_flight",
            debug.pipeline_after_in_flight_trace_count(),
        ),
        ("sim.debug.exec_trace.records", debug.exec_trace_count()),
        (
            "sim.debug.exec_trace.retired",
            debug.exec_retired_trace_count(),
        ),
        ("sim.debug.fetch_trace.records", debug.fetch_trace_count()),
        (
            "sim.debug.host_action_trace.records",
            debug.host_action_trace_count(),
        ),
        (
            "sim.debug.host_action_trace.injected_commands",
            debug.host_action_injected_command_trace_count(),
        ),
        (
            "sim.debug.host_action_trace.guest_host_calls",
            debug.host_action_guest_host_call_trace_count(),
        ),
        (
            "sim.debug.host_action_trace.roi_begin",
            debug.host_action_roi_begin_trace_count(),
        ),
        (
            "sim.debug.host_action_trace.roi_end",
            debug.host_action_roi_end_trace_count(),
        ),
        (
            "sim.debug.host_action_trace.stats_resets",
            debug.host_action_stats_reset_trace_count(),
        ),
        (
            "sim.debug.host_action_trace.stats_dumps",
            debug.host_action_stats_dump_trace_count(),
        ),
        (
            "sim.debug.host_action_trace.checkpoints",
            debug.host_action_checkpoint_trace_count(),
        ),
        (
            "sim.debug.host_action_trace.execution_mode_switches",
            debug.host_action_execution_mode_switch_trace_count(),
        ),
        (
            "sim.debug.host_action_trace.stops",
            debug.host_action_stop_trace_count(),
        ),
        ("sim.debug.sbi_trace.records", debug.sbi_trace_count()),
        (
            "sim.debug.sbi_trace.console",
            debug.sbi_console_trace_count(),
        ),
        ("sim.debug.sbi_trace.timers", debug.sbi_timer_trace_count()),
        (
            "sim.debug.sbi_trace.hsm_events",
            debug.sbi_hsm_event_trace_count(),
        ),
        (
            "sim.debug.sbi_trace.hsm_wakes",
            debug.sbi_hsm_wake_trace_count(),
        ),
        (
            "sim.debug.sbi_trace.hsm_statuses",
            debug.sbi_hsm_status_trace_count(),
        ),
        ("sim.debug.sbi_trace.ipis", debug.sbi_ipi_trace_count()),
        (
            "sim.debug.sbi_trace.rfences",
            debug.sbi_rfence_trace_count(),
        ),
        (
            "sim.debug.sbi_trace.rfence_completions",
            debug.sbi_rfence_completion_trace_count(),
        ),
        ("sim.debug.sbi_trace.resets", debug.sbi_reset_trace_count()),
        (
            "sim.debug.sbi_trace.targets",
            debug.sbi_target_trace_count(),
        ),
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
        ("sim.debug.cache_trace.records", debug.cache_trace_count()),
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
            "sim.debug.memory_trace.requests",
            debug.memory_request_trace_count(),
        ),
        (
            "sim.debug.memory_trace.fetch.requests",
            debug.memory_fetch_request_trace_count(),
        ),
        (
            "sim.debug.memory_trace.data.requests",
            debug.memory_data_request_trace_count(),
        ),
        (
            "sim.debug.memory_trace.routes",
            debug.memory_route_trace_count(),
        ),
        (
            "sim.debug.memory_trace.fetch.routes",
            debug.memory_fetch_route_trace_count(),
        ),
        (
            "sim.debug.memory_trace.data.routes",
            debug.memory_data_route_trace_count(),
        ),
        (
            "sim.debug.memory_trace.request_agents",
            debug.memory_request_agent_trace_count(),
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
            "sim.debug.power_trace.targets",
            debug.power_target_trace_count(),
        ),
        (
            "sim.debug.power_trace.states",
            debug.power_state_trace_count(),
        ),
        (
            "sim.debug.power_trace.states.on",
            debug.power_on_state_trace_count(),
        ),
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
        (
            "sim.debug.syscall_trace.syscall_numbers",
            debug.syscall_number_trace_count(),
        ),
        (
            "sim.debug.syscall_trace.call_sites",
            debug.syscall_call_site_trace_count(),
        ),
        (
            "sim.debug.syscall_trace.cpus",
            debug.syscall_cpu_trace_count(),
        ),
        (
            "sim.debug.syscall_trace.argument_words",
            debug.syscall_argument_word_trace_count(),
        ),
        (
            "sim.debug.syscall_trace.nonzero_arguments",
            debug.syscall_nonzero_argument_trace_count(),
        ),
    ] {
        increment_stat(stats, path, "Count", StatResetPolicy::Monotonic, value)?;
    }
    for stat in debug.exec_trace_stats() {
        increment_stat(
            stats,
            &format!("sim.debug.exec_trace.{}", stat.path()),
            stat.unit(),
            StatResetPolicy::Monotonic,
            stat.value(),
        )?;
    }
    for stat in debug.branch_trace_stats(stat_path_segment) {
        increment_stat(
            stats,
            &format!("sim.debug.branch_trace.{}", stat.path()),
            stat.unit(),
            StatResetPolicy::Monotonic,
            stat.value(),
        )?;
    }
    for stat in debug.pipeline_trace_stats(stat_path_segment) {
        increment_stat(
            stats,
            &format!("sim.debug.pipeline_trace.{}", stat.path()),
            stat.unit(),
            StatResetPolicy::Monotonic,
            stat.value(),
        )?;
    }
    for stat in debug.fetch_trace_stats(stat_path_segment) {
        increment_stat(
            stats,
            &format!("sim.debug.fetch_trace.{}", stat.path()),
            stat.unit(),
            StatResetPolicy::Monotonic,
            stat.value(),
        )?;
    }
    for stat in debug.cache_trace_stats() {
        increment_stat(
            stats,
            &format!("sim.debug.cache_trace.{}", stat.suffix()),
            stat.unit(),
            StatResetPolicy::Monotonic,
            stat.value(),
        )?;
    }
    for stat in debug.data_trace_stats(stat_path_segment) {
        increment_stat(
            stats,
            &format!("sim.debug.data_trace.{}", stat.path()),
            stat.unit(),
            StatResetPolicy::Monotonic,
            stat.value(),
        )?;
    }
    for record in debug.cache_trace_records() {
        let hierarchy = stat_path_segment(record.hierarchy());
        let level = stat_path_segment(record.level());
        let prefix = format!("sim.debug.cache_trace.hierarchy.{hierarchy}.{level}");
        for stat in record.stats() {
            increment_stat(
                stats,
                &format!("{prefix}.{}", stat.suffix()),
                stat.unit(),
                StatResetPolicy::Monotonic,
                stat.value(),
            )?;
        }
    }
    for stat in debug.dram_trace_stats() {
        increment_stat(
            stats,
            &format!("sim.debug.dram_trace.{}", stat.path()),
            stat.unit(),
            StatResetPolicy::Monotonic,
            stat.value(),
        )?;
    }
    for stat in debug.dram_trace_kind_stats() {
        increment_stat(
            stats,
            &format!("sim.debug.dram_trace.{}", stat.path()),
            stat.unit(),
            StatResetPolicy::Monotonic,
            stat.value(),
        )?;
    }
    for stat in debug.dram_trace_low_power_kind_stats() {
        increment_stat(
            stats,
            &format!("sim.debug.dram_trace.{}", stat.path()),
            stat.unit(),
            StatResetPolicy::Monotonic,
            stat.value(),
        )?;
    }
    for stat in debug.fabric_trace_stats(stat_path_segment) {
        increment_stat(
            stats,
            &format!("sim.debug.fabric_trace.{}", stat.path()),
            stat.unit(),
            StatResetPolicy::Monotonic,
            stat.value(),
        )?;
    }
    for stat in debug.memory_trace_stats(stat_path_segment) {
        increment_stat(
            stats,
            &format!("sim.debug.memory_trace.{}", stat.path()),
            stat.unit(),
            StatResetPolicy::Monotonic,
            stat.value(),
        )?;
    }
    increment_stat(
        stats,
        "sim.debug.memory_trace.response_latency_ticks",
        "Tick",
        StatResetPolicy::Monotonic,
        debug.memory_response_latency_tick_count(),
    )?;
    increment_stat(
        stats,
        "sim.debug.memory_trace.max_response_latency_ticks",
        "Tick",
        StatResetPolicy::Monotonic,
        debug.memory_max_response_latency_tick_count(),
    )?;
    for stat in debug.syscall_trace_stats(stat_path_segment) {
        increment_stat(
            stats,
            &format!("sim.debug.syscall_trace.{}", stat.path()),
            stat.unit(),
            StatResetPolicy::Monotonic,
            stat.value(),
        )?;
    }
    increment_stat(
        stats,
        "sim.debug.trace.payload_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        debug.trace_payload_byte_count(),
    )?;
    increment_stat(
        stats,
        "sim.debug.exec_trace.bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        debug.exec_trace_byte_count(),
    )?;
    increment_stat(
        stats,
        "sim.debug.fetch_trace.bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        debug.fetch_trace_byte_count(),
    )?;
    increment_stat(
        stats,
        "sim.debug.data_trace.load_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        debug.data_load_trace_byte_count(),
    )?;
    increment_stat(
        stats,
        "sim.debug.data_trace.store_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        debug.data_store_trace_byte_count(),
    )?;
    increment_stat(
        stats,
        "sim.debug.data_trace.atomic_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        debug.data_atomic_trace_byte_count(),
    )?;
    increment_stat(
        stats,
        "sim.debug.sbi_trace.console.bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        debug.sbi_console_trace_byte_count(),
    )?;
    increment_stat(
        stats,
        "sim.debug.power_trace.residency_ticks",
        "Tick",
        StatResetPolicy::Monotonic,
        debug.power_residency_tick_count(),
    )?;
    increment_stat(
        stats,
        "sim.debug.power_trace.dynamic_microwatts",
        "MicroWatt",
        StatResetPolicy::Monotonic,
        debug.power_dynamic_microwatt_count(),
    )?;
    increment_stat(
        stats,
        "sim.debug.power_trace.static_microwatts",
        "MicroWatt",
        StatResetPolicy::Monotonic,
        debug.power_static_microwatt_count(),
    )?;
    increment_stat(
        stats,
        "sim.debug.power_trace.total_microwatts",
        "MicroWatt",
        StatResetPolicy::Monotonic,
        debug.power_total_microwatt_count(),
    )?;
    increment_stat(
        stats,
        "sim.debug.power_trace.dynamic_microwatt_ticks",
        "MicroWattTick",
        StatResetPolicy::Monotonic,
        debug.power_dynamic_microwatt_tick_count(),
    )?;
    increment_stat(
        stats,
        "sim.debug.power_trace.static_microwatt_ticks",
        "MicroWattTick",
        StatResetPolicy::Monotonic,
        debug.power_static_microwatt_tick_count(),
    )?;
    increment_stat(
        stats,
        "sim.debug.power_trace.total_microwatt_ticks",
        "MicroWattTick",
        StatResetPolicy::Monotonic,
        debug.power_total_microwatt_tick_count(),
    )?;
    increment_stat(
        stats,
        "sim.debug.power_trace.max_temperature_millicelsius",
        "MilliCelsius",
        StatResetPolicy::Monotonic,
        debug.power_max_temperature_millicelsius(),
    )?;
    for target in debug.power_target_trace_stats() {
        let target_path = target
            .target
            .split('.')
            .map(stat_path_segment)
            .collect::<Vec<_>>()
            .join(".");
        let prefix = format!("sim.debug.power_trace.target.{target_path}");
        for (suffix, unit, value) in [
            ("records", "Count", target.records),
            ("states.on", "Count", target.on_records),
            ("residency_ticks", "Tick", target.residency_ticks),
            ("dynamic_microwatts", "MicroWatt", target.dynamic_microwatts),
            ("static_microwatts", "MicroWatt", target.static_microwatts),
            ("total_microwatts", "MicroWatt", target.total_microwatts),
            (
                "dynamic_microwatt_ticks",
                "MicroWattTick",
                target.dynamic_microwatt_ticks,
            ),
            (
                "static_microwatt_ticks",
                "MicroWattTick",
                target.static_microwatt_ticks,
            ),
            (
                "total_microwatt_ticks",
                "MicroWattTick",
                target.total_microwatt_ticks,
            ),
            (
                "max_temperature_millicelsius",
                "MilliCelsius",
                target.max_temperature_millicelsius,
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
    }
    Ok(())
}

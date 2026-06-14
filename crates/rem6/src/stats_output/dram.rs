use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::increment_stat;
use crate::{
    Rem6CliError, Rem6DramBankSummary, Rem6DramPortSummary, Rem6DramSummary, Rem6DramTargetSummary,
};

pub(super) fn emit_dram_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    summary: &Rem6DramSummary,
) -> Result<(), Rem6CliError> {
    emit_dram_counter(
        stats,
        prefix,
        "active_targets",
        "Count",
        summary.active_targets,
    )?;
    emit_dram_counter(stats, prefix, "active_ports", "Count", summary.active_ports)?;
    emit_dram_counter(stats, prefix, "active_banks", "Count", summary.active_banks)?;
    emit_dram_counter(stats, prefix, "accesses", "Count", summary.accesses)?;
    emit_dram_counter(stats, prefix, "reads", "Count", summary.reads)?;
    emit_dram_counter(stats, prefix, "writes", "Count", summary.writes)?;
    emit_dram_counter(stats, prefix, "row_hits", "Count", summary.row_hits)?;
    emit_dram_counter(stats, prefix, "row_misses", "Count", summary.row_misses)?;
    emit_dram_counter(stats, prefix, "refreshes", "Count", summary.refreshes)?;
    emit_dram_counter(
        stats,
        prefix,
        "refresh_ticks",
        "Tick",
        summary.refresh_ticks,
    )?;
    emit_dram_counter(stats, prefix, "commands", "Count", summary.commands)?;
    emit_dram_counter(stats, prefix, "turnarounds", "Count", summary.turnarounds)?;
    emit_dram_counter(
        stats,
        prefix,
        "total_ready_latency_ticks",
        "Tick",
        summary.total_ready_latency_ticks,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "max_ready_latency_ticks",
        "Tick",
        summary.max_ready_latency_ticks,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.profiled_targets",
        "Count",
        summary.profiled_targets,
    )?;
    for technology in ["ddr", "hbm", "lpddr", "nvm"] {
        emit_dram_constant(
            stats,
            prefix,
            &format!("profile.technology.{technology}"),
            "Count",
            u64::from(summary.profile_technology == Some(technology)),
        )?;
    }
    emit_dram_constant(
        stats,
        prefix,
        "profile.parallel_ports",
        "Count",
        summary.profile_parallel_ports,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.topology_units",
        "Count",
        summary.profile_topology_units,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.scheduler_banks",
        "Count",
        summary.profile_scheduler_banks,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.topology_banks",
        "Count",
        summary.profile_topology_banks,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.scheduler_bank_groups",
        "Count",
        summary.profile_scheduler_bank_groups,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.geometry.bank_count",
        "Count",
        summary.profile_geometry_bank_count,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.geometry.row_size",
        "Byte",
        summary.profile_geometry_row_size,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.geometry.line_size",
        "Byte",
        summary.profile_geometry_line_size,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.geometry.lines_per_row",
        "Count",
        summary.profile_geometry_lines_per_row,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.geometry.bank_group_count",
        "Count",
        summary.profile_geometry_bank_group_count,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.activate_latency",
        "Tick",
        summary.profile_timing_activate_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.read_latency",
        "Tick",
        summary.profile_timing_read_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.write_latency",
        "Tick",
        summary.profile_timing_write_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.precharge_latency",
        "Tick",
        summary.profile_timing_precharge_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.bus_turnaround",
        "Tick",
        summary.profile_timing_bus_turnaround,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.burst_spacing",
        "Tick",
        summary.profile_timing_burst_spacing,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.same_bank_group_burst_spacing",
        "Tick",
        summary.profile_timing_same_bank_group_burst_spacing,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.command_window.window_cycles",
        "Tick",
        summary.profile_timing_command_window_cycles,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.command_window.max_commands",
        "Count",
        summary.profile_timing_command_window_max_commands,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.low_power_timing.precharge_powerdown_entry_delay",
        "Tick",
        summary.profile_low_power_precharge_powerdown_entry_delay,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.low_power_timing.self_refresh_entry_delay",
        "Tick",
        summary.profile_low_power_self_refresh_entry_delay,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.low_power_timing.exit_latency",
        "Tick",
        summary.profile_low_power_exit_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.low_power_timing.self_refresh_exit_latency",
        "Tick",
        summary.profile_low_power_self_refresh_exit_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.nvm_media.read_media_latency",
        "Tick",
        summary.profile_nvm_media_read_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.nvm_media.write_media_latency",
        "Tick",
        summary.profile_nvm_media_write_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.nvm_media.send_latency",
        "Tick",
        summary.profile_nvm_media_send_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.nvm_media.max_pending_reads",
        "Count",
        summary.profile_nvm_media_max_pending_reads,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.nvm_media.max_pending_writes",
        "Count",
        summary.profile_nvm_media_max_pending_writes,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "nvm.persistent_writes",
        "Count",
        summary.nvm_persistent_writes,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "nvm.persistent_write_bytes",
        "Byte",
        summary.nvm_persistent_write_bytes,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "nvm.max_pending_reads",
        "Count",
        summary.nvm_max_pending_reads,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "nvm.max_pending_persistent_writes",
        "Count",
        summary.nvm_max_pending_persistent_writes,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.active_powerdown.entries",
        "Count",
        summary.low_power_active_powerdown_entries,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.active_powerdown.ticks",
        "Tick",
        summary.low_power_active_powerdown_ticks,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.precharge_powerdown.entries",
        "Count",
        summary.low_power_precharge_powerdown_entries,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.precharge_powerdown.ticks",
        "Tick",
        summary.low_power_precharge_powerdown_ticks,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.self_refresh.entries",
        "Count",
        summary.low_power_self_refresh_entries,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.self_refresh.ticks",
        "Tick",
        summary.low_power_self_refresh_ticks,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.exits",
        "Count",
        summary.low_power_exits,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.exit_latency_ticks",
        "Tick",
        summary.low_power_exit_latency_ticks,
    )?;
    for target in &summary.targets {
        emit_dram_target_stats(stats, prefix, target)?;
    }
    Ok(())
}

fn emit_dram_target_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    target: &Rem6DramTargetSummary,
) -> Result<(), Rem6CliError> {
    let prefix = format!("{prefix}.target{}", target.target);
    emit_dram_counter(stats, &prefix, "active_ports", "Count", target.active_ports)?;
    emit_dram_counter(stats, &prefix, "active_banks", "Count", target.active_banks)?;
    emit_dram_counter(stats, &prefix, "accesses", "Count", target.accesses)?;
    emit_dram_counter(stats, &prefix, "reads", "Count", target.reads)?;
    emit_dram_counter(stats, &prefix, "writes", "Count", target.writes)?;
    emit_dram_counter(stats, &prefix, "row_hits", "Count", target.row_hits)?;
    emit_dram_counter(stats, &prefix, "row_misses", "Count", target.row_misses)?;
    emit_dram_counter(stats, &prefix, "refreshes", "Count", target.refreshes)?;
    emit_dram_counter(
        stats,
        &prefix,
        "refresh_ticks",
        "Tick",
        target.refresh_ticks,
    )?;
    emit_dram_counter(stats, &prefix, "commands", "Count", target.commands)?;
    emit_dram_counter(stats, &prefix, "turnarounds", "Count", target.turnarounds)?;
    emit_dram_counter(
        stats,
        &prefix,
        "total_ready_latency_ticks",
        "Tick",
        target.total_ready_latency_ticks,
    )?;
    emit_dram_counter(
        stats,
        &prefix,
        "max_ready_latency_ticks",
        "Tick",
        target.max_ready_latency_ticks,
    )?;
    for port in &target.ports {
        emit_dram_port_stats(stats, &prefix, port)?;
    }
    Ok(())
}

fn emit_dram_port_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    port: &Rem6DramPortSummary,
) -> Result<(), Rem6CliError> {
    let prefix = format!("{prefix}.port{}", port.port);
    emit_dram_counter(stats, &prefix, "accesses", "Count", port.accesses)?;
    emit_dram_counter(stats, &prefix, "reads", "Count", port.reads)?;
    emit_dram_counter(stats, &prefix, "writes", "Count", port.writes)?;
    emit_dram_counter(stats, &prefix, "turnarounds", "Count", port.turnarounds)?;
    emit_dram_counter(stats, &prefix, "commands", "Count", port.commands)?;
    for bank in &port.banks {
        emit_dram_bank_stats(stats, &prefix, bank)?;
    }
    Ok(())
}

fn emit_dram_bank_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    bank: &Rem6DramBankSummary,
) -> Result<(), Rem6CliError> {
    let prefix = format!("{prefix}.bank{}", bank.bank);
    emit_dram_counter(stats, &prefix, "accesses", "Count", bank.accesses)?;
    emit_dram_counter(stats, &prefix, "read_bytes", "Byte", bank.read_bytes)?;
    emit_dram_counter(stats, &prefix, "write_bytes", "Byte", bank.write_bytes)?;
    emit_dram_counter(stats, &prefix, "row_hits", "Count", bank.row_hits)?;
    emit_dram_counter(stats, &prefix, "row_misses", "Count", bank.row_misses)?;
    emit_dram_counter(stats, &prefix, "refreshes", "Count", bank.refreshes)?;
    emit_dram_counter(stats, &prefix, "refresh_ticks", "Tick", bank.refresh_ticks)?;
    emit_dram_counter(stats, &prefix, "commands", "Count", bank.commands)?;
    emit_dram_counter(
        stats,
        &prefix,
        "total_ready_latency_ticks",
        "Tick",
        bank.total_ready_latency_ticks,
    )?;
    emit_dram_counter(
        stats,
        &prefix,
        "max_ready_latency_ticks",
        "Tick",
        bank.max_ready_latency_ticks,
    )
}

fn emit_dram_counter(
    stats: &mut StatsRegistry,
    prefix: &str,
    name: &str,
    unit: &str,
    value: u64,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!("{prefix}.{name}"),
        unit,
        StatResetPolicy::Monotonic,
        value,
    )
}

fn emit_dram_constant(
    stats: &mut StatsRegistry,
    prefix: &str,
    name: &str,
    unit: &str,
    value: u64,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!("{prefix}.{name}"),
        unit,
        StatResetPolicy::Constant,
        value,
    )
}

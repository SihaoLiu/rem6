use crate::{Rem6DramBankSummary, Rem6DramPortSummary, Rem6DramSummary, Rem6DramTargetSummary};

use super::{optional_string_json, resources};

impl Rem6DramSummary {
    pub(crate) fn to_json(&self) -> String {
        let profile_technology = optional_string_json(self.profile_technology);
        let profile_parallel_port_label = optional_string_json(self.profile_parallel_port_label);
        let profile_topology_unit_label = optional_string_json(self.profile_topology_unit_label);
        let profile_timing_refresh_policy =
            optional_string_json(self.profile_timing_refresh_policy);
        let profile_timing_refresh_granularity =
            optional_string_json(self.profile_timing_refresh_granularity);
        format!(
            "{{\"active_targets\":{},\"active_ports\":{},\"active_banks\":{},\"accesses\":{},\"reads\":{},\"writes\":{},\"read_bytes\":{},\"write_bytes\":{},\"row_hits\":{},\"read_row_hits\":{},\"write_row_hits\":{},\"row_misses\":{},\"refreshes\":{},\"refresh_ticks\":{},\"commands\":{},\"turnarounds\":{},\"total_ready_latency_ticks\":{},\"read_ready_latency_ticks\":{},\"max_ready_latency_ticks\":{},\"profile\":{{\"technology\":{},\"parallel_port_label\":{},\"topology_unit_label\":{},\"geometry\":{{\"bank_count\":{},\"row_size\":{},\"line_size\":{},\"lines_per_row\":{},\"bank_group_count\":{}}},\"timing\":{{\"activate_latency\":{},\"read_latency\":{},\"write_latency\":{},\"precharge_latency\":{},\"bus_turnaround\":{},\"burst_spacing\":{},\"same_bank_group_burst_spacing\":{},\"refresh_interval\":{},\"refresh_recovery\":{},\"refresh_policy\":{},\"refresh_granularity\":{},\"command_window\":{{\"window_cycles\":{},\"max_commands\":{}}}}},\"low_power_timing\":{{\"precharge_powerdown_entry_delay\":{},\"self_refresh_entry_delay\":{},\"exit_latency\":{},\"self_refresh_exit_latency\":{}}},\"nvm_media\":{{\"read_media_latency\":{},\"write_media_latency\":{},\"send_latency\":{},\"max_pending_reads\":{},\"max_pending_writes\":{}}},\"profiled_targets\":{},\"parallel_ports\":{},\"topology_units\":{},\"scheduler_banks\":{},\"topology_banks\":{},\"scheduler_bank_groups\":{}}},\"nvm\":{{\"persistent_writes\":{},\"persistent_write_bytes\":{},\"max_pending_reads\":{},\"max_pending_persistent_writes\":{}}},\"low_power\":{{\"active_powerdown\":{{\"entries\":{},\"ticks\":{}}},\"precharge_powerdown\":{{\"entries\":{},\"ticks\":{}}},\"self_refresh\":{{\"entries\":{},\"ticks\":{}}},\"exits\":{},\"exit_latency_ticks\":{}}},\"targets\":[{}]}}",
            self.active_targets,
            self.active_ports,
            self.active_banks,
            self.accesses,
            self.reads,
            self.writes,
            self.read_bytes,
            self.write_bytes,
            self.row_hits,
            self.read_row_hits,
            self.write_row_hits,
            self.row_misses,
            self.refreshes,
            self.refresh_ticks,
            self.commands,
            self.turnarounds,
            self.total_ready_latency_ticks,
            self.read_ready_latency_ticks,
            self.max_ready_latency_ticks,
            profile_technology,
            profile_parallel_port_label,
            profile_topology_unit_label,
            self.profile_geometry_bank_count,
            self.profile_geometry_row_size,
            self.profile_geometry_line_size,
            self.profile_geometry_lines_per_row,
            self.profile_geometry_bank_group_count,
            self.profile_timing_activate_latency,
            self.profile_timing_read_latency,
            self.profile_timing_write_latency,
            self.profile_timing_precharge_latency,
            self.profile_timing_bus_turnaround,
            self.profile_timing_burst_spacing,
            self.profile_timing_same_bank_group_burst_spacing,
            self.profile_timing_refresh_interval,
            self.profile_timing_refresh_recovery,
            profile_timing_refresh_policy,
            profile_timing_refresh_granularity,
            self.profile_timing_command_window_cycles,
            self.profile_timing_command_window_max_commands,
            self.profile_low_power_precharge_powerdown_entry_delay,
            self.profile_low_power_self_refresh_entry_delay,
            self.profile_low_power_exit_latency,
            self.profile_low_power_self_refresh_exit_latency,
            self.profile_nvm_media_read_latency,
            self.profile_nvm_media_write_latency,
            self.profile_nvm_media_send_latency,
            self.profile_nvm_media_max_pending_reads,
            self.profile_nvm_media_max_pending_writes,
            self.profiled_targets,
            self.profile_parallel_ports,
            self.profile_topology_units,
            self.profile_scheduler_banks,
            self.profile_topology_banks,
            self.profile_scheduler_bank_groups,
            self.nvm_persistent_writes,
            self.nvm_persistent_write_bytes,
            self.nvm_max_pending_reads,
            self.nvm_max_pending_persistent_writes,
            self.low_power_active_powerdown_entries,
            self.low_power_active_powerdown_ticks,
            self.low_power_precharge_powerdown_entries,
            self.low_power_precharge_powerdown_ticks,
            self.low_power_self_refresh_entries,
            self.low_power_self_refresh_ticks,
            self.low_power_exits,
            self.low_power_exit_latency_ticks,
            dram_targets_json(&self.targets),
        )
    }
}

pub(crate) fn dram_targets_json(targets: &[Rem6DramTargetSummary]) -> String {
    targets
        .iter()
        .map(dram_target_json)
        .collect::<Vec<_>>()
        .join(",")
}

fn dram_target_json(summary: &Rem6DramTargetSummary) -> String {
    format!(
        "{{\"target\":{},\"active_ports\":{},\"active_banks\":{},\"accesses\":{},\"reads\":{},\"writes\":{},\"read_bytes\":{},\"write_bytes\":{},\"row_hits\":{},\"read_row_hits\":{},\"write_row_hits\":{},\"row_misses\":{},\"refreshes\":{},\"refresh_ticks\":{},\"commands\":{},\"turnarounds\":{},\"total_ready_latency_ticks\":{},\"max_ready_latency_ticks\":{},\"low_power\":{},\"ports\":[{}]}}",
        summary.target,
        summary.active_ports,
        summary.active_banks,
        summary.accesses,
        summary.reads,
        summary.writes,
        dram_target_read_bytes(summary),
        dram_target_write_bytes(summary),
        summary.row_hits,
        summary.read_row_hits,
        summary.write_row_hits,
        summary.row_misses,
        summary.refreshes,
        summary.refresh_ticks,
        summary.commands,
        summary.turnarounds,
        summary.total_ready_latency_ticks,
        summary.max_ready_latency_ticks,
        resources::dram_low_power_json(
            summary.low_power_active_powerdown_entries,
            summary.low_power_active_powerdown_ticks,
            summary.low_power_precharge_powerdown_entries,
            summary.low_power_precharge_powerdown_ticks,
            summary.low_power_self_refresh_entries,
            summary.low_power_self_refresh_ticks,
            summary.low_power_exits,
            summary.low_power_exit_latency_ticks,
        ),
        dram_ports_json(&summary.ports),
    )
}

fn dram_ports_json(ports: &[Rem6DramPortSummary]) -> String {
    ports
        .iter()
        .map(dram_port_json)
        .collect::<Vec<_>>()
        .join(",")
}

fn dram_port_json(summary: &Rem6DramPortSummary) -> String {
    format!(
        "{{\"port\":{},\"active_banks\":{},\"accesses\":{},\"reads\":{},\"writes\":{},\"read_bytes\":{},\"write_bytes\":{},\"row_hits\":{},\"read_row_hits\":{},\"write_row_hits\":{},\"row_misses\":{},\"refreshes\":{},\"refresh_ticks\":{},\"turnarounds\":{},\"commands\":{},\"total_ready_latency_ticks\":{},\"max_ready_latency_ticks\":{},\"low_power\":{},\"banks\":[{}]}}",
        summary.port,
        summary.banks.len(),
        summary.accesses,
        summary.reads,
        summary.writes,
        dram_port_read_bytes(summary),
        dram_port_write_bytes(summary),
        dram_port_row_hits(summary),
        dram_port_read_row_hits(summary),
        dram_port_write_row_hits(summary),
        dram_port_row_misses(summary),
        dram_port_refreshes(summary),
        dram_port_refresh_ticks(summary),
        summary.turnarounds,
        summary.commands,
        dram_port_total_ready_latency_ticks(summary),
        dram_port_max_ready_latency_ticks(summary),
        resources::dram_low_power_json(
            summary.low_power_active_powerdown_entries,
            summary.low_power_active_powerdown_ticks,
            summary.low_power_precharge_powerdown_entries,
            summary.low_power_precharge_powerdown_ticks,
            summary.low_power_self_refresh_entries,
            summary.low_power_self_refresh_ticks,
            summary.low_power_exits,
            summary.low_power_exit_latency_ticks,
        ),
        dram_banks_json(&summary.banks),
    )
}

fn dram_target_read_bytes(summary: &Rem6DramTargetSummary) -> u64 {
    summary.ports.iter().map(dram_port_read_bytes).sum()
}

fn dram_target_write_bytes(summary: &Rem6DramTargetSummary) -> u64 {
    summary.ports.iter().map(dram_port_write_bytes).sum()
}

fn dram_port_read_bytes(summary: &Rem6DramPortSummary) -> u64 {
    summary.banks.iter().map(|bank| bank.read_bytes).sum()
}

fn dram_port_write_bytes(summary: &Rem6DramPortSummary) -> u64 {
    summary.banks.iter().map(|bank| bank.write_bytes).sum()
}

fn dram_port_row_hits(summary: &Rem6DramPortSummary) -> u64 {
    summary.banks.iter().map(|bank| bank.row_hits).sum()
}

fn dram_port_read_row_hits(summary: &Rem6DramPortSummary) -> u64 {
    summary.banks.iter().map(|bank| bank.read_row_hits).sum()
}

fn dram_port_write_row_hits(summary: &Rem6DramPortSummary) -> u64 {
    summary.banks.iter().map(|bank| bank.write_row_hits).sum()
}

fn dram_port_row_misses(summary: &Rem6DramPortSummary) -> u64 {
    summary.banks.iter().map(|bank| bank.row_misses).sum()
}

fn dram_port_refreshes(summary: &Rem6DramPortSummary) -> u64 {
    summary.banks.iter().map(|bank| bank.refreshes).sum()
}

fn dram_port_refresh_ticks(summary: &Rem6DramPortSummary) -> u64 {
    summary.banks.iter().map(|bank| bank.refresh_ticks).sum()
}

fn dram_port_total_ready_latency_ticks(summary: &Rem6DramPortSummary) -> u64 {
    summary
        .banks
        .iter()
        .map(|bank| bank.total_ready_latency_ticks)
        .sum()
}

fn dram_port_max_ready_latency_ticks(summary: &Rem6DramPortSummary) -> u64 {
    summary
        .banks
        .iter()
        .map(|bank| bank.max_ready_latency_ticks)
        .max()
        .unwrap_or(0)
}

fn dram_banks_json(banks: &[Rem6DramBankSummary]) -> String {
    banks
        .iter()
        .map(dram_bank_json)
        .collect::<Vec<_>>()
        .join(",")
}

fn dram_bank_json(summary: &Rem6DramBankSummary) -> String {
    format!(
        "{{\"bank\":{},\"accesses\":{},\"reads\":{},\"writes\":{},\"read_bytes\":{},\"write_bytes\":{},\"row_hits\":{},\"read_row_hits\":{},\"write_row_hits\":{},\"row_misses\":{},\"refreshes\":{},\"refresh_ticks\":{},\"commands\":{},\"total_ready_latency_ticks\":{},\"max_ready_latency_ticks\":{},\"low_power\":{}}}",
        summary.bank,
        summary.accesses,
        summary.reads,
        summary.writes,
        summary.read_bytes,
        summary.write_bytes,
        summary.row_hits,
        summary.read_row_hits,
        summary.write_row_hits,
        summary.row_misses,
        summary.refreshes,
        summary.refresh_ticks,
        summary.commands,
        summary.total_ready_latency_ticks,
        summary.max_ready_latency_ticks,
        resources::dram_low_power_json(
            summary.low_power_active_powerdown_entries,
            summary.low_power_active_powerdown_ticks,
            summary.low_power_precharge_powerdown_entries,
            summary.low_power_precharge_powerdown_ticks,
            summary.low_power_self_refresh_entries,
            summary.low_power_self_refresh_ticks,
            summary.low_power_exits,
            summary.low_power_exit_latency_ticks,
        ),
    )
}

use std::collections::{BTreeMap, BTreeSet};

use rem6_cpu::{CpuFetchEventKind, RiscvCluster, RiscvCoreDriveAction, RiscvDataAccessEventKind};
use rem6_memory::MemoryOperation;
use rem6_power::{PowerAnalysisRecord, PowerStateKind};
use rem6_system::{RiscvSyscallTraceOutcome, RiscvSyscallTraceRecord, RiscvSystemRun};
use rem6_transport::MemoryTrace;

mod branch;
mod cache;
mod checkpoint_components_json;
mod dram;
mod fabric;
mod host_action;
mod memory;
mod o3;
mod pipeline;
mod sbi;
mod trace_stats;

use crate::formatting::{bytes_to_hex, json_escape};
use crate::{
    CliDebugFlag, Rem6DramSummary, Rem6HostActionSummary, Rem6HostCheckpointSummary,
    Rem6HostExecutionModeSwitchSummary, Rem6MemoryResourceSummary, Rem6RunConfig,
    Rem6RunFabricSummary,
};
use branch::{branch_trace_records, Rem6BranchTraceRecord};
use cache::{cache_trace_records, cache_trace_stats, Rem6CacheTraceRecord, Rem6CacheTraceStat};
use dram::{
    dram_trace_kind_stats, dram_trace_low_power_kind_stats, dram_trace_payload_byte_count,
    dram_trace_records, Rem6DramTraceRecord, Rem6DramTraceStat,
};
use fabric::{
    fabric_trace_payload_byte_count, fabric_trace_records, fabric_trace_stats,
    Rem6FabricTraceRecord, Rem6FabricTraceStat,
};
use host_action::{
    host_action_trace_checkpoint_restore_authority_stats,
    host_action_trace_execution_mode_switch_stats, host_action_trace_records,
    Rem6HostActionTraceRecord, Rem6HostActionTraceStat,
};
use memory::{
    memory_trace_channel_matches, memory_trace_records, memory_trace_stats, Rem6MemoryTraceRecord,
    Rem6MemoryTraceStat,
};
pub(crate) use o3::{
    o3_branch_direction_mismatch_to_json, o3_branch_target_mismatch_to_json,
    o3_event_summary_to_json,
};
use o3::{
    o3_trace_authority_stats, o3_trace_cpu_checkpoint_restore_authority_stats,
    o3_trace_cpu_checkpoint_restore_component_stats, o3_trace_cpu_execution_mode_authority_stats,
    o3_trace_cpu_stats, o3_trace_records, o3_trace_stats, Rem6O3ExecutionModeAuthorityStat,
    Rem6O3TraceRecord, Rem6O3TraceStat,
};
use pipeline::{pipeline_trace_records, pipeline_trace_summary_to_json, Rem6PipelineTraceRecord};
pub(crate) use sbi::Rem6SbiTraceInputs;
use sbi::{sbi_trace_records, Rem6SbiTraceRecord};
use trace_stats::{
    branch_trace_stats, data_trace_stats, exec_trace_stats, fetch_trace_stats, pipeline_trace_stats,
};
pub(crate) use trace_stats::{
    Rem6BranchTraceStat, Rem6DataTraceStat, Rem6ExecTraceStat, Rem6FetchTraceStat,
    Rem6PipelineTraceStat,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6DebugSummary {
    flags: Vec<CliDebugFlag>,
    branch_trace: Vec<Rem6BranchTraceRecord>,
    pipeline_trace: Vec<Rem6PipelineTraceRecord>,
    o3_trace: Vec<Rem6O3TraceRecord>,
    exec_trace: Vec<Rem6ExecTraceRecord>,
    fetch_trace: Vec<Rem6FetchTraceRecord>,
    host_action_trace: Vec<Rem6HostActionTraceRecord>,
    host_action_checkpoint_restores: Vec<Rem6HostCheckpointSummary>,
    host_action_execution_mode_switches: Vec<Rem6HostExecutionModeSwitchSummary>,
    data_trace: Vec<Rem6DataTraceRecord>,
    cache_trace: Vec<Rem6CacheTraceRecord>,
    dram_trace: Vec<Rem6DramTraceRecord>,
    fabric_trace: Vec<Rem6FabricTraceRecord>,
    memory_trace: Vec<Rem6MemoryTraceRecord>,
    power_trace: Vec<Rem6PowerTraceRecord>,
    sbi_trace: Vec<Rem6SbiTraceRecord>,
    syscall_trace: Vec<Rem6SyscallTraceRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rem6ExecTraceRecord {
    cpu: u32,
    tick: u64,
    pc: u64,
    bytes: Vec<u8>,
    retired: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rem6FetchTraceRecord {
    cpu: u32,
    tick: u64,
    pc: u64,
    sequence: u64,
    size: u64,
    route: u64,
    endpoint: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rem6DataTraceRecord {
    cpu: u32,
    tick: u64,
    kind: &'static str,
    address: u64,
    size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rem6PowerTraceRecord {
    target: String,
    state: &'static str,
    residency_ticks: u64,
    temperature_c: String,
    dynamic_watts: String,
    static_watts: String,
    total_watts: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rem6PowerTargetTraceStats<'a> {
    pub(crate) target: &'a str,
    pub(crate) records: u64,
    pub(crate) on_records: u64,
    pub(crate) residency_ticks: u64,
    pub(crate) dynamic_microwatts: u64,
    pub(crate) static_microwatts: u64,
    pub(crate) total_microwatts: u64,
    pub(crate) dynamic_microwatt_ticks: u64,
    pub(crate) static_microwatt_ticks: u64,
    pub(crate) total_microwatt_ticks: u64,
    pub(crate) max_temperature_millicelsius: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rem6SyscallTraceRecord {
    cpu: u32,
    tick: u64,
    pc: u64,
    number: u64,
    arguments: [u64; 6],
    outcome: RiscvSyscallTraceOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6SyscallTraceStat {
    path: String,
    unit: &'static str,
    value: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SyscallTraceStatSummary {
    records: u64,
    syscall_numbers: BTreeSet<u64>,
    call_sites: BTreeSet<u64>,
    cpus: BTreeSet<u32>,
    returns: u64,
    exits: u64,
    blocked: u64,
    argument_words: u64,
    nonzero_arguments: u64,
}

impl Rem6DebugSummary {
    pub(crate) fn from_run(
        config: &Rem6RunConfig,
        cluster: &RiscvCluster,
        run: &RiscvSystemRun,
        fetch_memory_trace: &MemoryTrace,
        data_memory_trace: &MemoryTrace,
        fabric: &Rem6RunFabricSummary,
        dram: &Rem6DramSummary,
        memory_resources: &Rem6MemoryResourceSummary,
        power_records: &[PowerAnalysisRecord],
        syscall_trace: &[RiscvSyscallTraceRecord],
        host_actions: &Rem6HostActionSummary,
        sbi: Rem6SbiTraceInputs<'_>,
    ) -> Self {
        let flags = config.debug_flags().to_vec();
        let branch_trace = if config.debug_branch_enabled() {
            branch_trace_records(cluster, config.cores() as u32)
        } else {
            Vec::new()
        };
        let pipeline_trace = if config.debug_pipeline_enabled() {
            pipeline_trace_records(cluster, config.cores() as u32)
        } else {
            Vec::new()
        };
        let o3_trace = if config.debug_o3_enabled() {
            o3_trace_records(
                cluster,
                config.cores() as u32,
                &host_actions.execution_modes,
                host_actions.stats_resets.last(),
                &host_actions.checkpoint_restores,
            )
        } else {
            Vec::new()
        };
        let exec_trace = if config.debug_exec_enabled() {
            exec_trace_records(run)
        } else {
            Vec::new()
        };
        let fetch_trace = if config.debug_fetch_enabled() {
            fetch_trace_records(cluster, config.cores() as u32)
        } else {
            Vec::new()
        };
        let host_action_trace = if config.debug_host_action_enabled() {
            host_action_trace_records(host_actions)
        } else {
            Vec::new()
        };
        let host_action_checkpoint_restores = if config.debug_host_action_enabled() {
            host_actions.checkpoint_restores.clone()
        } else {
            Vec::new()
        };
        let host_action_execution_mode_switches = if config.debug_host_action_enabled() {
            host_actions.execution_mode_switches.clone()
        } else {
            Vec::new()
        };
        let data_trace = if config.debug_data_enabled() {
            data_trace_records(cluster, config.cores() as u32)
        } else {
            Vec::new()
        };
        let cache_trace = if config.debug_cache_enabled() {
            cache_trace_records(memory_resources)
        } else {
            Vec::new()
        };
        let dram_trace = if config.debug_dram_enabled() {
            dram_trace_records(dram)
        } else {
            Vec::new()
        };
        let fabric_trace = if config.debug_fabric_enabled() {
            fabric_trace_records(fabric)
        } else {
            Vec::new()
        };
        let memory_trace = if config.debug_memory_enabled() {
            memory_trace_records(fetch_memory_trace, data_memory_trace)
        } else {
            Vec::new()
        };
        let power_trace = if config.debug_power_enabled() {
            power_trace_records(power_records)
        } else {
            Vec::new()
        };
        let sbi_trace = if config.debug_sbi_enabled() {
            sbi_trace_records(sbi)
        } else {
            Vec::new()
        };
        let syscall_trace = if config.debug_syscall_enabled() {
            syscall_trace_records(syscall_trace)
        } else {
            Vec::new()
        };
        Self {
            flags,
            branch_trace,
            pipeline_trace,
            o3_trace,
            exec_trace,
            fetch_trace,
            host_action_trace,
            host_action_checkpoint_restores,
            host_action_execution_mode_switches,
            data_trace,
            cache_trace,
            dram_trace,
            fabric_trace,
            memory_trace,
            power_trace,
            sbi_trace,
            syscall_trace,
        }
    }

    pub(crate) fn has_enabled_flags(&self) -> bool {
        !self.flags.is_empty()
    }

    pub(crate) fn enabled_flag_count(&self) -> u64 {
        self.flags.len() as u64
    }

    pub(crate) fn trace_record_count(&self) -> u64 {
        self.trace_counts()
            .into_iter()
            .fold(0u64, |acc, value| acc.saturating_add(value))
    }

    pub(crate) fn trace_category_count(&self) -> u64 {
        self.trace_counts()
            .into_iter()
            .filter(|count| *count > 0)
            .count() as u64
    }

    pub(crate) fn active_flag_count(&self) -> u64 {
        self.flags
            .iter()
            .filter(|flag| self.trace_count_for_flag(**flag) > 0)
            .count() as u64
    }

    pub(crate) fn trace_payload_byte_count(&self) -> u64 {
        [
            self.exec_trace_byte_count(),
            self.fetch_trace_byte_count(),
            self.data_load_trace_byte_count(),
            self.data_store_trace_byte_count(),
            self.data_atomic_trace_byte_count(),
            dram_trace_payload_byte_count(&self.dram_trace),
            fabric_trace_payload_byte_count(&self.fabric_trace),
            self.sbi_console_trace_byte_count(),
        ]
        .into_iter()
        .fold(0u64, |acc, value| acc.saturating_add(value))
    }

    pub(crate) fn exec_trace_count(&self) -> u64 {
        self.exec_trace.len() as u64
    }

    pub(crate) fn exec_retired_trace_count(&self) -> u64 {
        self.exec_trace
            .iter()
            .filter(|record| record.retired)
            .count() as u64
    }

    pub(crate) fn exec_trace_byte_count(&self) -> u64 {
        self.exec_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(record.bytes.len() as u64)
        })
    }

    pub(crate) fn exec_trace_stats(&self) -> Vec<Rem6ExecTraceStat> {
        exec_trace_stats(&self.exec_trace)
    }

    pub(crate) fn branch_trace_count(&self) -> u64 {
        self.branch_trace.len() as u64
    }

    pub(crate) fn branch_conditional_trace_count(&self) -> u64 {
        self.branch_trace
            .iter()
            .filter(|record| record.conditional)
            .count() as u64
    }

    pub(crate) fn branch_unconditional_trace_count(&self) -> u64 {
        self.branch_trace
            .iter()
            .filter(|record| !record.conditional)
            .count() as u64
    }

    pub(crate) fn branch_predicted_taken_trace_count(&self) -> u64 {
        self.branch_trace
            .iter()
            .filter(|record| record.predicted_taken)
            .count() as u64
    }

    pub(crate) fn branch_resolved_taken_trace_count(&self) -> u64 {
        self.branch_trace
            .iter()
            .filter(|record| record.resolved_taken)
            .count() as u64
    }

    pub(crate) fn branch_misprediction_trace_count(&self) -> u64 {
        self.branch_trace
            .iter()
            .filter(|record| record.mispredicted)
            .count() as u64
    }

    pub(crate) fn branch_repair_trace_count(&self) -> u64 {
        self.branch_trace
            .iter()
            .filter(|record| record.repair_target_pc.is_some())
            .count() as u64
    }

    pub(crate) fn branch_flushed_trace_count(&self) -> u64 {
        self.branch_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(record.flushed_sequences.len() as u64)
        })
    }

    pub(crate) fn branch_trace_stats(
        &self,
        stat_path_segment: impl Fn(&str) -> String,
    ) -> Vec<Rem6BranchTraceStat> {
        branch_trace_stats(&self.branch_trace, stat_path_segment)
    }

    pub(crate) fn pipeline_trace_count(&self) -> u64 {
        self.pipeline_trace.len() as u64
    }

    pub(crate) fn pipeline_stall_cycle_trace_count(&self) -> u64 {
        self.pipeline_trace
            .iter()
            .fold(0u64, |acc, record| acc.saturating_add(record.stall_cycles))
    }

    pub(crate) fn pipeline_state_changed_trace_count(&self) -> u64 {
        self.pipeline_trace
            .iter()
            .filter(|record| record.state_changed)
            .count() as u64
    }

    pub(crate) fn pipeline_advanced_trace_count(&self) -> u64 {
        self.pipeline_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(record.advanced.len() as u64)
        })
    }

    pub(crate) fn pipeline_retired_trace_count(&self) -> u64 {
        self.pipeline_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(
                record
                    .advanced
                    .iter()
                    .filter(|advance| advance.retires)
                    .count() as u64,
            )
        })
    }

    pub(crate) fn pipeline_flushed_trace_count(&self) -> u64 {
        self.pipeline_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(record.flushed.len() as u64)
        })
    }

    pub(crate) fn pipeline_resource_blocked_trace_count(&self) -> u64 {
        self.pipeline_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(record.resource_blocked.len() as u64)
        })
    }

    pub(crate) fn pipeline_ordering_blocked_trace_count(&self) -> u64 {
        self.pipeline_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(record.ordering_blocked.len() as u64)
        })
    }

    pub(crate) fn pipeline_branch_prediction_trace_count(&self) -> u64 {
        self.pipeline_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(record.branch_predictions)
        })
    }

    pub(crate) fn pipeline_branch_misprediction_trace_count(&self) -> u64 {
        self.pipeline_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(record.branch_mispredictions)
        })
    }

    pub(crate) fn pipeline_branch_prediction_flushed_trace_count(&self) -> u64 {
        self.pipeline_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(record.branch_prediction_flushed)
        })
    }

    pub(crate) fn pipeline_redirect_trace_count(&self) -> u64 {
        self.pipeline_trace
            .iter()
            .filter(|record| record.redirect_target_pc.is_some())
            .count() as u64
    }

    pub(crate) fn pipeline_before_in_flight_trace_count(&self) -> u64 {
        self.pipeline_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(record.before_in_flight.len() as u64)
        })
    }

    pub(crate) fn pipeline_after_in_flight_trace_count(&self) -> u64 {
        self.pipeline_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(record.after_in_flight.len() as u64)
        })
    }

    pub(crate) fn pipeline_trace_stats(
        &self,
        stat_path_segment: impl Fn(&str) -> String,
    ) -> Vec<Rem6PipelineTraceStat> {
        pipeline_trace_stats(&self.pipeline_trace, stat_path_segment)
    }

    pub(crate) fn o3_trace_count(&self) -> u64 {
        self.o3_trace.len() as u64
    }

    pub(crate) fn o3_trace_stats(&self) -> Vec<Rem6O3TraceStat> {
        o3_trace_stats(&self.o3_trace)
    }

    pub(crate) fn o3_trace_authority_stats(
        &self,
        stat_path_segment: impl Fn(&str) -> String,
    ) -> Vec<Rem6O3ExecutionModeAuthorityStat> {
        o3_trace_authority_stats(&self.o3_trace, stat_path_segment)
    }

    pub(crate) fn o3_trace_cpu_stats(&self) -> Vec<(u32, Rem6O3TraceStat)> {
        o3_trace_cpu_stats(&self.o3_trace)
    }

    pub(crate) fn o3_trace_cpu_authority_stats(
        &self,
        stat_path_segment: impl Fn(&str) -> String,
    ) -> Vec<(u32, Rem6O3ExecutionModeAuthorityStat)> {
        let mut stats =
            o3_trace_cpu_execution_mode_authority_stats(&self.o3_trace, &stat_path_segment);
        stats.extend(o3_trace_cpu_checkpoint_restore_authority_stats(
            &self.o3_trace,
            &stat_path_segment,
        ));
        stats.extend(o3_trace_cpu_checkpoint_restore_component_stats(
            &self.o3_trace,
            stat_path_segment,
        ));
        stats
    }

    pub(crate) fn fetch_trace_count(&self) -> u64 {
        self.fetch_trace.len() as u64
    }

    pub(crate) fn fetch_trace_byte_count(&self) -> u64 {
        self.fetch_trace
            .iter()
            .fold(0u64, |acc, record| acc.saturating_add(record.size))
    }

    pub(crate) fn fetch_trace_stats(
        &self,
        stat_path_segment: impl Fn(&str) -> String,
    ) -> Vec<Rem6FetchTraceStat> {
        fetch_trace_stats(&self.fetch_trace, stat_path_segment)
    }

    pub(crate) fn host_action_trace_count(&self) -> u64 {
        self.host_action_trace.len() as u64
    }

    pub(crate) fn host_action_injected_command_trace_count(&self) -> u64 {
        self.host_action_kind_trace_count("injected_command")
    }

    pub(crate) fn host_action_guest_host_call_trace_count(&self) -> u64 {
        self.host_action_kind_trace_count("guest_host_call")
    }

    pub(crate) fn host_action_roi_begin_trace_count(&self) -> u64 {
        self.host_action_kind_trace_count("roi_begin")
    }

    pub(crate) fn host_action_roi_end_trace_count(&self) -> u64 {
        self.host_action_kind_trace_count("roi_end")
    }

    pub(crate) fn host_action_stats_reset_trace_count(&self) -> u64 {
        self.host_action_kind_trace_count("stats_reset")
    }

    pub(crate) fn host_action_stats_dump_trace_count(&self) -> u64 {
        self.host_action_kind_trace_count("stats_dump")
    }

    pub(crate) fn host_action_checkpoint_trace_count(&self) -> u64 {
        self.host_action_kind_trace_count("checkpoint")
    }

    pub(crate) fn host_action_checkpoint_restore_trace_count(&self) -> u64 {
        self.host_action_kind_trace_count("checkpoint_restore")
    }

    pub(crate) fn host_action_execution_mode_switch_trace_count(&self) -> u64 {
        self.host_action_kind_trace_count("execution_mode_switch")
    }

    pub(crate) fn host_action_stop_trace_count(&self) -> u64 {
        self.host_action_kind_trace_count("stop")
    }

    pub(crate) fn host_action_trace_stats(
        &self,
        stat_path_segment: impl Fn(&str) -> String,
    ) -> Vec<Rem6HostActionTraceStat> {
        let mut stats = host_action_trace_checkpoint_restore_authority_stats(
            &self.host_action_checkpoint_restores,
            &stat_path_segment,
        );
        stats.extend(host_action_trace_execution_mode_switch_stats(
            &self.host_action_execution_mode_switches,
            stat_path_segment,
        ));
        stats
    }

    pub(crate) fn data_trace_count(&self) -> u64 {
        self.data_trace.len() as u64
    }

    pub(crate) fn data_load_trace_count(&self) -> u64 {
        self.data_kind_trace_count("load")
    }

    pub(crate) fn data_store_trace_count(&self) -> u64 {
        self.data_kind_trace_count("store")
    }

    pub(crate) fn data_atomic_trace_count(&self) -> u64 {
        self.data_kind_trace_count("atomic")
    }

    pub(crate) fn data_load_trace_byte_count(&self) -> u64 {
        self.data_kind_trace_byte_count("load")
    }

    pub(crate) fn data_store_trace_byte_count(&self) -> u64 {
        self.data_kind_trace_byte_count("store")
    }

    pub(crate) fn data_atomic_trace_byte_count(&self) -> u64 {
        self.data_kind_trace_byte_count("atomic")
    }

    pub(crate) fn data_trace_stats(
        &self,
        stat_path_segment: impl Fn(&str) -> String,
    ) -> Vec<Rem6DataTraceStat> {
        data_trace_stats(&self.data_trace, stat_path_segment)
    }

    pub(crate) fn cache_trace_count(&self) -> u64 {
        self.cache_trace.len() as u64
    }

    pub(crate) fn cache_trace_records(&self) -> &[Rem6CacheTraceRecord] {
        &self.cache_trace
    }

    pub(crate) fn cache_trace_stats(&self) -> Vec<Rem6CacheTraceStat> {
        cache_trace_stats(&self.cache_trace)
    }

    pub(crate) fn dram_trace_count(&self) -> u64 {
        self.dram_trace.len() as u64
    }

    pub(crate) fn dram_trace_stats(&self) -> Vec<Rem6DramTraceStat> {
        self.dram_trace
            .iter()
            .flat_map(Rem6DramTraceRecord::stats)
            .collect()
    }

    pub(crate) fn dram_trace_kind_stats(&self) -> Vec<Rem6DramTraceStat> {
        dram_trace_kind_stats(&self.dram_trace)
    }

    pub(crate) fn dram_trace_low_power_kind_stats(&self) -> Vec<Rem6DramTraceStat> {
        dram_trace_low_power_kind_stats(&self.dram_trace)
    }

    pub(crate) fn dram_target_trace_count(&self) -> u64 {
        self.dram_kind_trace_count("target")
    }

    pub(crate) fn dram_port_trace_count(&self) -> u64 {
        self.dram_kind_trace_count("port")
    }

    pub(crate) fn dram_bank_trace_count(&self) -> u64 {
        self.dram_kind_trace_count("bank")
    }

    pub(crate) fn fabric_trace_count(&self) -> u64 {
        self.fabric_trace.len() as u64
    }

    pub(crate) fn fabric_trace_stats(
        &self,
        stat_path_segment: impl Fn(&str) -> String,
    ) -> Vec<Rem6FabricTraceStat> {
        fabric_trace_stats(&self.fabric_trace, stat_path_segment)
    }

    pub(crate) fn fabric_lane_trace_count(&self) -> u64 {
        self.fabric_trace
            .iter()
            .filter(|record| matches!(record, Rem6FabricTraceRecord::Lane { .. }))
            .count() as u64
    }

    pub(crate) fn fabric_hop_trace_count(&self) -> u64 {
        self.fabric_trace
            .iter()
            .filter(|record| matches!(record, Rem6FabricTraceRecord::Hop { .. }))
            .count() as u64
    }

    pub(crate) fn memory_trace_count(&self) -> u64 {
        self.memory_trace.len() as u64
    }

    pub(crate) fn memory_fetch_trace_count(&self) -> u64 {
        self.memory_channel_trace_count("fetch")
    }

    pub(crate) fn memory_data_trace_count(&self) -> u64 {
        self.memory_channel_trace_count("data")
    }

    pub(crate) fn memory_request_trace_count(&self) -> u64 {
        self.memory_request_key_count(None)
    }

    pub(crate) fn memory_fetch_request_trace_count(&self) -> u64 {
        self.memory_request_key_count(Some("fetch"))
    }

    pub(crate) fn memory_data_request_trace_count(&self) -> u64 {
        self.memory_request_key_count(Some("data"))
    }

    pub(crate) fn memory_route_trace_count(&self) -> u64 {
        self.memory_route_key_count(None)
    }

    pub(crate) fn memory_fetch_route_trace_count(&self) -> u64 {
        self.memory_route_key_count(Some("fetch"))
    }

    pub(crate) fn memory_data_route_trace_count(&self) -> u64 {
        self.memory_route_key_count(Some("data"))
    }

    pub(crate) fn memory_request_agent_trace_count(&self) -> u64 {
        let mut request_agents = BTreeSet::new();
        for record in &self.memory_trace {
            request_agents.insert(record.request_agent);
        }
        request_agents.len() as u64
    }

    pub(crate) fn memory_request_sent_trace_count(&self) -> u64 {
        self.memory_kind_trace_count("request_sent")
    }

    pub(crate) fn memory_request_arrived_trace_count(&self) -> u64 {
        self.memory_kind_trace_count("request_arrived")
    }

    pub(crate) fn memory_response_arrived_trace_count(&self) -> u64 {
        self.memory_kind_trace_count("response_arrived")
    }

    pub(crate) fn memory_completed_response_trace_count(&self) -> u64 {
        self.memory_response_status_trace_count("completed")
    }

    pub(crate) fn memory_retry_response_trace_count(&self) -> u64 {
        self.memory_response_status_trace_count("retry")
    }

    pub(crate) fn memory_store_conditional_failed_response_trace_count(&self) -> u64 {
        self.memory_response_status_trace_count("store_conditional_failed")
    }

    pub(crate) fn memory_response_latency_tick_count(&self) -> u64 {
        self.memory_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(record.response_latency_ticks.unwrap_or(0))
        })
    }

    pub(crate) fn memory_max_response_latency_tick_count(&self) -> u64 {
        self.memory_trace
            .iter()
            .filter_map(|record| record.response_latency_ticks)
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn memory_trace_stats(
        &self,
        stat_path_segment: impl Fn(&str) -> String,
    ) -> Vec<Rem6MemoryTraceStat> {
        memory_trace_stats(&self.memory_trace, stat_path_segment)
    }

    pub(crate) fn power_trace_count(&self) -> u64 {
        self.power_trace.len() as u64
    }

    pub(crate) fn power_target_trace_count(&self) -> u64 {
        let mut targets = BTreeSet::new();
        for record in &self.power_trace {
            targets.insert(record.target.as_str());
        }
        targets.len() as u64
    }

    pub(crate) fn power_state_trace_count(&self) -> u64 {
        let mut states = BTreeSet::new();
        for record in &self.power_trace {
            states.insert(record.state);
        }
        states.len() as u64
    }

    pub(crate) fn power_on_state_trace_count(&self) -> u64 {
        self.power_trace
            .iter()
            .filter(|record| record.state == "on")
            .count() as u64
    }

    pub(crate) fn power_residency_tick_count(&self) -> u64 {
        self.power_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(record.residency_ticks)
        })
    }

    pub(crate) fn power_dynamic_microwatt_count(&self) -> u64 {
        self.power_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(watts_to_microwatts(&record.dynamic_watts))
        })
    }

    pub(crate) fn power_static_microwatt_count(&self) -> u64 {
        self.power_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(watts_to_microwatts(&record.static_watts))
        })
    }

    pub(crate) fn power_total_microwatt_count(&self) -> u64 {
        self.power_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(watts_to_microwatts(&record.total_watts))
        })
    }

    pub(crate) fn power_dynamic_microwatt_tick_count(&self) -> u64 {
        self.power_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(watts_to_microwatt_ticks(
                &record.dynamic_watts,
                record.residency_ticks,
            ))
        })
    }

    pub(crate) fn power_static_microwatt_tick_count(&self) -> u64 {
        self.power_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(watts_to_microwatt_ticks(
                &record.static_watts,
                record.residency_ticks,
            ))
        })
    }

    pub(crate) fn power_total_microwatt_tick_count(&self) -> u64 {
        self.power_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(watts_to_microwatt_ticks(
                &record.total_watts,
                record.residency_ticks,
            ))
        })
    }

    pub(crate) fn power_target_trace_stats(&self) -> Vec<Rem6PowerTargetTraceStats<'_>> {
        let mut targets = BTreeMap::<&str, Rem6PowerTargetTraceStats<'_>>::new();
        for record in &self.power_trace {
            targets
                .entry(record.target.as_str())
                .and_modify(|stats| stats.add_record(record))
                .or_insert_with(|| Rem6PowerTargetTraceStats::from_record(record));
        }
        targets.into_values().collect()
    }

    pub(crate) fn power_max_temperature_millicelsius(&self) -> u64 {
        self.power_trace
            .iter()
            .map(|record| celsius_to_millicelsius(&record.temperature_c))
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn sbi_trace_count(&self) -> u64 {
        self.sbi_trace.len() as u64
    }

    pub(crate) fn sbi_console_trace_count(&self) -> u64 {
        self.sbi_kind_trace_count("console")
    }

    pub(crate) fn sbi_timer_trace_count(&self) -> u64 {
        self.sbi_kind_trace_count("timer")
    }

    pub(crate) fn sbi_hsm_event_trace_count(&self) -> u64 {
        self.sbi_kind_trace_count("hsm_event")
    }

    pub(crate) fn sbi_hsm_wake_trace_count(&self) -> u64 {
        self.sbi_kind_trace_count("hsm_wake")
    }

    pub(crate) fn sbi_hsm_status_trace_count(&self) -> u64 {
        self.sbi_kind_trace_count("hsm_status")
    }

    pub(crate) fn sbi_ipi_trace_count(&self) -> u64 {
        self.sbi_kind_trace_count("ipi")
    }

    pub(crate) fn sbi_rfence_trace_count(&self) -> u64 {
        self.sbi_kind_trace_count("rfence")
    }

    pub(crate) fn sbi_rfence_completion_trace_count(&self) -> u64 {
        self.sbi_kind_trace_count("rfence_completion")
    }

    pub(crate) fn sbi_reset_trace_count(&self) -> u64 {
        self.sbi_kind_trace_count("reset")
    }

    pub(crate) fn sbi_console_trace_byte_count(&self) -> u64 {
        self.sbi_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(record.console_byte_count())
        })
    }

    pub(crate) fn sbi_target_trace_count(&self) -> u64 {
        self.sbi_trace.iter().fold(0u64, |acc, record| {
            acc.saturating_add(record.target_count())
        })
    }

    pub(crate) fn syscall_trace_count(&self) -> u64 {
        self.syscall_trace.len() as u64
    }

    pub(crate) fn syscall_return_trace_count(&self) -> u64 {
        self.syscall_outcome_trace_count(|outcome| {
            matches!(outcome, RiscvSyscallTraceOutcome::Return { .. })
        })
    }

    pub(crate) fn syscall_exit_trace_count(&self) -> u64 {
        self.syscall_outcome_trace_count(|outcome| {
            matches!(outcome, RiscvSyscallTraceOutcome::Exit { .. })
        })
    }

    pub(crate) fn syscall_blocked_trace_count(&self) -> u64 {
        self.syscall_outcome_trace_count(|outcome| {
            matches!(outcome, RiscvSyscallTraceOutcome::Blocked)
        })
    }

    pub(crate) fn syscall_number_trace_count(&self) -> u64 {
        let mut numbers = BTreeSet::new();
        for record in &self.syscall_trace {
            numbers.insert(record.number);
        }
        numbers.len() as u64
    }

    pub(crate) fn syscall_call_site_trace_count(&self) -> u64 {
        let mut call_sites = BTreeSet::new();
        for record in &self.syscall_trace {
            call_sites.insert(record.pc);
        }
        call_sites.len() as u64
    }

    pub(crate) fn syscall_cpu_trace_count(&self) -> u64 {
        let mut cpus = BTreeSet::new();
        for record in &self.syscall_trace {
            cpus.insert(record.cpu);
        }
        cpus.len() as u64
    }

    pub(crate) fn syscall_argument_word_trace_count(&self) -> u64 {
        self.syscall_trace
            .iter()
            .map(|record| record.arguments.len() as u64)
            .sum()
    }

    pub(crate) fn syscall_nonzero_argument_trace_count(&self) -> u64 {
        self.syscall_trace
            .iter()
            .flat_map(|record| record.arguments)
            .filter(|argument| *argument != 0)
            .count() as u64
    }

    pub(crate) fn syscall_trace_stats(
        &self,
        stat_path_segment: impl Fn(&str) -> String,
    ) -> Vec<Rem6SyscallTraceStat> {
        syscall_trace_stats(&self.syscall_trace, stat_path_segment)
    }

    pub(crate) fn to_json(&self) -> String {
        let flags = self
            .flags
            .iter()
            .map(|flag| format!("\"{}\"", flag.as_str()))
            .collect::<Vec<_>>()
            .join(",");
        let branch_trace = self
            .branch_trace
            .iter()
            .map(Rem6BranchTraceRecord::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let pipeline_trace = self
            .pipeline_trace
            .iter()
            .map(Rem6PipelineTraceRecord::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let pipeline_summary = pipeline_trace_summary_to_json(&self.pipeline_trace);
        let o3_trace = self
            .o3_trace
            .iter()
            .map(Rem6O3TraceRecord::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let exec_trace = self
            .exec_trace
            .iter()
            .map(Rem6ExecTraceRecord::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let fetch_trace = self
            .fetch_trace
            .iter()
            .map(Rem6FetchTraceRecord::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let host_action_trace = self
            .host_action_trace
            .iter()
            .map(Rem6HostActionTraceRecord::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let data_trace = self
            .data_trace
            .iter()
            .map(Rem6DataTraceRecord::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let cache_trace = self
            .cache_trace
            .iter()
            .copied()
            .map(Rem6CacheTraceRecord::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let dram_trace = self
            .dram_trace
            .iter()
            .map(Rem6DramTraceRecord::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let fabric_trace = self
            .fabric_trace
            .iter()
            .map(Rem6FabricTraceRecord::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let memory_trace = self
            .memory_trace
            .iter()
            .map(Rem6MemoryTraceRecord::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let power_trace = self
            .power_trace
            .iter()
            .map(Rem6PowerTraceRecord::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let sbi_trace = self
            .sbi_trace
            .iter()
            .map(Rem6SbiTraceRecord::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let syscall_trace = self
            .syscall_trace
            .iter()
            .map(Rem6SyscallTraceRecord::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"flags\":[{}],\"branch_trace\":[{}],\"pipeline_trace\":[{}],\"pipeline_summary\":{},\"o3_trace\":[{}],\"exec_trace\":[{}],\"fetch_trace\":[{}],\"host_action_trace\":[{}],\"data_trace\":[{}],\"cache_trace\":[{}],\"dram_trace\":[{}],\"fabric_trace\":[{}],\"memory_trace\":[{}],\"power_trace\":[{}],\"sbi_trace\":[{}],\"syscall_trace\":[{}]}}",
            flags,
            branch_trace,
            pipeline_trace,
            pipeline_summary,
            o3_trace,
            exec_trace,
            fetch_trace,
            host_action_trace,
            data_trace,
            cache_trace,
            dram_trace,
            fabric_trace,
            memory_trace,
            power_trace,
            sbi_trace,
            syscall_trace
        )
    }

    fn memory_channel_trace_count(&self, channel: &str) -> u64 {
        self.memory_trace
            .iter()
            .filter(|record| record.channel == channel)
            .count() as u64
    }

    fn memory_kind_trace_count(&self, kind: &str) -> u64 {
        self.memory_trace
            .iter()
            .filter(|record| record.kind == kind)
            .count() as u64
    }

    fn memory_response_status_trace_count(&self, status: &str) -> u64 {
        self.memory_trace
            .iter()
            .filter(|record| record.response_status == Some(status))
            .count() as u64
    }

    fn memory_request_key_count(&self, channel: Option<&str>) -> u64 {
        let mut requests = BTreeSet::new();
        for record in &self.memory_trace {
            if !memory_trace_channel_matches(record, channel) {
                continue;
            }
            requests.insert((record.channel, record.request_agent, record.request));
        }
        requests.len() as u64
    }

    fn memory_route_key_count(&self, channel: Option<&str>) -> u64 {
        let mut routes = BTreeSet::new();
        for record in &self.memory_trace {
            if !memory_trace_channel_matches(record, channel) {
                continue;
            }
            routes.insert((record.channel, record.route));
        }
        routes.len() as u64
    }

    fn data_kind_trace_count(&self, kind: &str) -> u64 {
        self.data_trace
            .iter()
            .filter(|record| record.kind == kind)
            .count() as u64
    }

    fn data_kind_trace_byte_count(&self, kind: &str) -> u64 {
        self.data_trace
            .iter()
            .filter(|record| record.kind == kind)
            .fold(0u64, |acc, record| acc.saturating_add(record.size))
    }

    fn host_action_kind_trace_count(&self, kind: &str) -> u64 {
        self.host_action_trace
            .iter()
            .filter(|record| record.kind() == kind)
            .count() as u64
    }

    fn sbi_kind_trace_count(&self, kind: &str) -> u64 {
        self.sbi_trace
            .iter()
            .filter(|record| record.kind() == kind)
            .count() as u64
    }

    fn dram_kind_trace_count(&self, kind: &str) -> u64 {
        self.dram_trace
            .iter()
            .filter(|record| record.kind() == kind)
            .count() as u64
    }

    fn syscall_outcome_trace_count(
        &self,
        matches_outcome: impl Fn(RiscvSyscallTraceOutcome) -> bool,
    ) -> u64 {
        self.syscall_trace
            .iter()
            .filter(|record| matches_outcome(record.outcome))
            .count() as u64
    }

    fn trace_counts(&self) -> [u64; 14] {
        [
            self.branch_trace_count(),
            self.pipeline_trace_count(),
            self.o3_trace_count(),
            self.exec_trace_count(),
            self.fetch_trace_count(),
            self.host_action_trace_count(),
            self.data_trace_count(),
            self.cache_trace_count(),
            self.dram_trace_count(),
            self.fabric_trace_count(),
            self.memory_trace_count(),
            self.power_trace_count(),
            self.sbi_trace_count(),
            self.syscall_trace_count(),
        ]
    }

    fn trace_count_for_flag(&self, flag: CliDebugFlag) -> u64 {
        match flag {
            CliDebugFlag::Branch => self.branch_trace_count(),
            CliDebugFlag::Cache => self.cache_trace_count(),
            CliDebugFlag::Data => self.data_trace_count(),
            CliDebugFlag::Dram => self.dram_trace_count(),
            CliDebugFlag::Exec => self.exec_trace_count(),
            CliDebugFlag::Fabric => self.fabric_trace_count(),
            CliDebugFlag::Fetch => self.fetch_trace_count(),
            CliDebugFlag::HostAction => self.host_action_trace_count(),
            CliDebugFlag::Memory => self.memory_trace_count(),
            CliDebugFlag::O3 => self.o3_trace_count(),
            CliDebugFlag::Pipeline => self.pipeline_trace_count(),
            CliDebugFlag::Power => self.power_trace_count(),
            CliDebugFlag::Sbi => self.sbi_trace_count(),
            CliDebugFlag::Syscall => self.syscall_trace_count(),
        }
    }
}

impl Rem6ExecTraceRecord {
    fn to_json(&self) -> String {
        format!(
            "{{\"cpu\":{},\"tick\":{},\"pc\":\"0x{:x}\",\"bytes\":\"{}\",\"retired\":{}}}",
            self.cpu,
            self.tick,
            self.pc,
            bytes_to_hex(&self.bytes),
            self.retired,
        )
    }
}

impl Rem6FetchTraceRecord {
    fn to_json(&self) -> String {
        format!(
            "{{\"cpu\":{},\"tick\":{},\"pc\":\"0x{:x}\",\"sequence\":{},\"size\":{},\"route\":{},\"endpoint\":\"{}\"}}",
            self.cpu,
            self.tick,
            self.pc,
            self.sequence,
            self.size,
            self.route,
            json_escape(&self.endpoint),
        )
    }
}

impl Rem6DataTraceRecord {
    fn to_json(&self) -> String {
        format!(
            "{{\"cpu\":{},\"tick\":{},\"kind\":\"{}\",\"address\":\"0x{:x}\",\"size\":{}}}",
            self.cpu, self.tick, self.kind, self.address, self.size,
        )
    }
}

impl Rem6PowerTraceRecord {
    fn to_json(&self) -> String {
        format!(
            "{{\"target\":\"{}\",\"state\":\"{}\",\"residency_ticks\":{},\"temperature_c\":{},\"dynamic_watts\":{},\"static_watts\":{},\"total_watts\":{}}}",
            json_escape(&self.target),
            self.state,
            self.residency_ticks,
            self.temperature_c,
            self.dynamic_watts,
            self.static_watts,
            self.total_watts,
        )
    }
}

impl<'a> Rem6PowerTargetTraceStats<'a> {
    fn from_record(record: &'a Rem6PowerTraceRecord) -> Self {
        let mut stats = Self {
            target: record.target.as_str(),
            records: 0,
            on_records: 0,
            residency_ticks: 0,
            dynamic_microwatts: 0,
            static_microwatts: 0,
            total_microwatts: 0,
            dynamic_microwatt_ticks: 0,
            static_microwatt_ticks: 0,
            total_microwatt_ticks: 0,
            max_temperature_millicelsius: 0,
        };
        stats.add_record(record);
        stats
    }

    fn add_record(&mut self, record: &Rem6PowerTraceRecord) {
        self.records = self.records.saturating_add(1);
        if record.state == "on" {
            self.on_records = self.on_records.saturating_add(1);
        }
        self.residency_ticks = self.residency_ticks.saturating_add(record.residency_ticks);
        self.dynamic_microwatts = self
            .dynamic_microwatts
            .saturating_add(watts_to_microwatts(&record.dynamic_watts));
        self.static_microwatts = self
            .static_microwatts
            .saturating_add(watts_to_microwatts(&record.static_watts));
        self.total_microwatts = self
            .total_microwatts
            .saturating_add(watts_to_microwatts(&record.total_watts));
        self.dynamic_microwatt_ticks =
            self.dynamic_microwatt_ticks
                .saturating_add(watts_to_microwatt_ticks(
                    &record.dynamic_watts,
                    record.residency_ticks,
                ));
        self.static_microwatt_ticks =
            self.static_microwatt_ticks
                .saturating_add(watts_to_microwatt_ticks(
                    &record.static_watts,
                    record.residency_ticks,
                ));
        self.total_microwatt_ticks =
            self.total_microwatt_ticks
                .saturating_add(watts_to_microwatt_ticks(
                    &record.total_watts,
                    record.residency_ticks,
                ));
        self.max_temperature_millicelsius = self
            .max_temperature_millicelsius
            .max(celsius_to_millicelsius(&record.temperature_c));
    }
}

impl Rem6SyscallTraceRecord {
    fn to_json(&self) -> String {
        let arguments = self
            .arguments
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"cpu\":{},\"tick\":{},\"pc\":\"0x{:x}\",\"number\":{},\"arguments\":[{}],\"outcome\":{}}}",
            self.cpu,
            self.tick,
            self.pc,
            self.number,
            arguments,
            syscall_outcome_json(self.outcome),
        )
    }
}

impl Rem6SyscallTraceStat {
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) const fn unit(&self) -> &'static str {
        self.unit
    }

    pub(crate) const fn value(&self) -> u64 {
        self.value
    }
}

impl SyscallTraceStatSummary {
    fn add_record(&mut self, record: &Rem6SyscallTraceRecord) {
        self.records = self.records.saturating_add(1);
        self.syscall_numbers.insert(record.number);
        self.call_sites.insert(record.pc);
        self.cpus.insert(record.cpu);
        self.argument_words = self
            .argument_words
            .saturating_add(record.arguments.len() as u64);
        self.nonzero_arguments = self.nonzero_arguments.saturating_add(
            record
                .arguments
                .iter()
                .filter(|argument| **argument != 0)
                .count() as u64,
        );
        match record.outcome {
            RiscvSyscallTraceOutcome::Return { .. } => {
                self.returns = self.returns.saturating_add(1);
            }
            RiscvSyscallTraceOutcome::Exit { .. } => {
                self.exits = self.exits.saturating_add(1);
            }
            RiscvSyscallTraceOutcome::Blocked => {
                self.blocked = self.blocked.saturating_add(1);
            }
        }
    }

    fn push_stats(&self, stats: &mut Vec<Rem6SyscallTraceStat>, prefix: &str) {
        for (suffix, value) in [
            ("records", self.records),
            ("returns", self.returns),
            ("exits", self.exits),
            ("blocked", self.blocked),
            ("syscall_numbers", self.syscall_numbers.len() as u64),
            ("call_sites", self.call_sites.len() as u64),
            ("cpus", self.cpus.len() as u64),
            ("argument_words", self.argument_words),
            ("nonzero_arguments", self.nonzero_arguments),
        ] {
            stats.push(Rem6SyscallTraceStat {
                path: format!("{prefix}.{suffix}"),
                unit: "Count",
                value,
            });
        }
    }
}

fn syscall_trace_stats(
    records: &[Rem6SyscallTraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6SyscallTraceStat> {
    let mut cpus = BTreeMap::<u32, SyscallTraceStatSummary>::new();
    let mut numbers = BTreeMap::<u64, SyscallTraceStatSummary>::new();
    let mut call_sites = BTreeMap::<u64, SyscallTraceStatSummary>::new();
    let mut outcomes = BTreeMap::<&'static str, SyscallTraceStatSummary>::new();
    for record in records {
        cpus.entry(record.cpu).or_default().add_record(record);
        numbers.entry(record.number).or_default().add_record(record);
        call_sites.entry(record.pc).or_default().add_record(record);
        outcomes
            .entry(syscall_outcome_kind(record.outcome))
            .or_default()
            .add_record(record);
    }

    let mut stats = Vec::new();
    for (cpu, summary) in cpus {
        summary.push_stats(&mut stats, &format!("cpu.cpu{cpu}"));
    }
    for (number, summary) in numbers {
        summary.push_stats(&mut stats, &format!("number.syscall{number}"));
    }
    for (pc, summary) in call_sites {
        let call_site = format!("0x{pc:x}");
        summary.push_stats(
            &mut stats,
            &format!("call_site.{}", stat_path_segment(&call_site)),
        );
    }
    for (outcome, summary) in outcomes {
        summary.push_stats(
            &mut stats,
            &format!("outcome.{}", stat_path_segment(outcome)),
        );
    }
    stats
}

fn power_trace_records(records: &[PowerAnalysisRecord]) -> Vec<Rem6PowerTraceRecord> {
    records
        .iter()
        .map(|record| {
            let dynamic_watts = format!("{:.6}", record.dynamic_watts());
            let static_watts = format!("{:.6}", record.static_watts());
            let total_watts = format!("{:.6}", record.total_watts());
            Rem6PowerTraceRecord {
                target: record.target().to_string(),
                state: power_state_name(record.current_state()),
                residency_ticks: record.residency_ticks(record.current_state()),
                temperature_c: format!("{:.6}", record.temperature_c()),
                dynamic_watts,
                static_watts,
                total_watts,
            }
        })
        .collect()
}

fn watts_to_microwatts(watts: &str) -> u64 {
    let Ok(watts) = watts.parse::<f64>() else {
        return 0;
    };
    if !watts.is_finite() || watts <= 0.0 {
        0
    } else {
        (watts * 1_000_000.0).round() as u64
    }
}

fn watts_to_microwatt_ticks(watts: &str, residency_ticks: u64) -> u64 {
    watts_to_microwatts(watts).saturating_mul(residency_ticks)
}

fn celsius_to_millicelsius(celsius: &str) -> u64 {
    let Ok(celsius) = celsius.parse::<f64>() else {
        return 0;
    };
    if !celsius.is_finite() || celsius <= 0.0 {
        0
    } else {
        (celsius * 1_000.0).round() as u64
    }
}

const fn power_state_name(state: PowerStateKind) -> &'static str {
    match state {
        PowerStateKind::Undefined => "undefined",
        PowerStateKind::On => "on",
        PowerStateKind::ClockGated => "clock_gated",
        PowerStateKind::SramRetention => "sram_retention",
        PowerStateKind::Off => "off",
    }
}

fn exec_trace_records(run: &RiscvSystemRun) -> Vec<Rem6ExecTraceRecord> {
    let mut records = Vec::new();
    for event in run.turns().iter().flat_map(|turn| turn.core_events()) {
        let RiscvCoreDriveAction::InstructionExecuted(instruction) = event.action() else {
            continue;
        };
        records.push(Rem6ExecTraceRecord {
            cpu: event.cpu().get(),
            tick: instruction.fetch().tick(),
            pc: instruction.fetch_pc().get(),
            bytes: instruction.fetch().data().unwrap_or_default().to_vec(),
            retired: instruction.counts_as_retired_instruction(),
        });
    }
    records
}

fn syscall_trace_records(records: &[RiscvSyscallTraceRecord]) -> Vec<Rem6SyscallTraceRecord> {
    records
        .iter()
        .map(|record| Rem6SyscallTraceRecord {
            cpu: record.cpu().get(),
            tick: record.tick(),
            pc: record.pc(),
            number: record.number(),
            arguments: record.arguments(),
            outcome: record.outcome(),
        })
        .collect()
}

fn syscall_outcome_json(outcome: RiscvSyscallTraceOutcome) -> String {
    match outcome {
        RiscvSyscallTraceOutcome::Blocked => "{\"kind\":\"blocked\"}".to_string(),
        RiscvSyscallTraceOutcome::Exit { code } => {
            format!("{{\"kind\":\"exit\",\"code\":{code}}}")
        }
        RiscvSyscallTraceOutcome::Return { value } => {
            format!("{{\"kind\":\"return\",\"value\":{value}}}")
        }
    }
}

const fn syscall_outcome_kind(outcome: RiscvSyscallTraceOutcome) -> &'static str {
    match outcome {
        RiscvSyscallTraceOutcome::Blocked => "blocked",
        RiscvSyscallTraceOutcome::Exit { .. } => "exit",
        RiscvSyscallTraceOutcome::Return { .. } => "return",
    }
}

fn data_trace_records(cluster: &RiscvCluster, core_count: u32) -> Vec<Rem6DataTraceRecord> {
    let mut records = Vec::new();
    for cpu_index in 0..core_count {
        let cpu = rem6_cpu::CpuId::new(cpu_index);
        let Ok(core) = cluster.core(cpu) else {
            continue;
        };
        records.extend(core.data_access_events().into_iter().filter_map(|event| {
            if event.kind() != RiscvDataAccessEventKind::Completed {
                return None;
            }
            Some(Rem6DataTraceRecord {
                cpu: cpu.get(),
                tick: event.tick(),
                kind: data_trace_kind(event.operation())?,
                address: event.physical_address().get(),
                size: event.size().bytes(),
            })
        }));
    }
    records.sort_by_key(|record| (record.tick, record.cpu, record.address, record.size));
    records
}

fn data_trace_kind(operation: MemoryOperation) -> Option<&'static str> {
    match operation {
        MemoryOperation::ReadShared
        | MemoryOperation::ReadUnique
        | MemoryOperation::LoadLocked
        | MemoryOperation::LockedRmwRead => Some("load"),
        MemoryOperation::Write
        | MemoryOperation::StoreConditional
        | MemoryOperation::StoreConditionalFail
        | MemoryOperation::StoreConditionalUpgrade
        | MemoryOperation::StoreConditionalUpgradeFail
        | MemoryOperation::LockedRmwWrite => Some("store"),
        MemoryOperation::Atomic | MemoryOperation::AtomicNoReturn => Some("atomic"),
        MemoryOperation::NoAccess
        | MemoryOperation::InstructionFetch
        | MemoryOperation::CacheBlockZero
        | MemoryOperation::Upgrade
        | MemoryOperation::PrefetchRead
        | MemoryOperation::PrefetchWrite
        | MemoryOperation::WriteClean
        | MemoryOperation::WritebackClean
        | MemoryOperation::WritebackDirty
        | MemoryOperation::CleanShared
        | MemoryOperation::CleanEvict
        | MemoryOperation::Invalidate
        | MemoryOperation::InvalidateWritable => None,
    }
}

fn fetch_trace_records(cluster: &RiscvCluster, core_count: u32) -> Vec<Rem6FetchTraceRecord> {
    let mut records = Vec::new();
    for cpu_index in 0..core_count {
        let cpu = rem6_cpu::CpuId::new(cpu_index);
        let Ok(core) = cluster.core(cpu) else {
            continue;
        };
        records.extend(
            core.inner()
                .fetch_history()
                .into_iter()
                .filter_map(|event| {
                    (event.kind() == CpuFetchEventKind::Issued).then(|| Rem6FetchTraceRecord {
                        cpu: cpu.get(),
                        tick: event.tick(),
                        pc: event.pc().get(),
                        sequence: event.request_id().sequence(),
                        size: event.size().bytes(),
                        route: event.route().get(),
                        endpoint: event.endpoint().as_str().to_string(),
                    })
                }),
        );
    }
    records.sort_by_key(|record| (record.tick, record.cpu, record.sequence));
    records
}

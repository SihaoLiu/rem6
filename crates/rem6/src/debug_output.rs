use std::collections::{BTreeMap, BTreeSet};

use rem6_cpu::{CpuFetchEventKind, RiscvCluster, RiscvCoreDriveAction, RiscvDataAccessEventKind};
use rem6_memory::{MemoryOperation, ResponseStatus};
use rem6_power::{PowerAnalysisRecord, PowerStateKind};
use rem6_system::{RiscvSyscallTraceOutcome, RiscvSyscallTraceRecord, RiscvSystemRun};
use rem6_transport::{MemoryTrace, MemoryTraceEvent, MemoryTraceKind};

mod cache;
mod dram;
mod fabric;

use crate::formatting::{bytes_to_hex, json_escape};
use crate::{
    CliDebugFlag, Rem6DramSummary, Rem6MemoryResourceSummary, Rem6RunConfig, Rem6RunFabricSummary,
};
use cache::{cache_trace_records, cache_trace_stats, Rem6CacheTraceRecord, Rem6CacheTraceStat};
use dram::{dram_trace_records, Rem6DramTraceRecord, Rem6DramTraceStat};
use fabric::{
    fabric_trace_records, fabric_trace_stats, Rem6FabricTraceRecord, Rem6FabricTraceStat,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6DebugSummary {
    flags: Vec<CliDebugFlag>,
    exec_trace: Vec<Rem6ExecTraceRecord>,
    fetch_trace: Vec<Rem6FetchTraceRecord>,
    data_trace: Vec<Rem6DataTraceRecord>,
    cache_trace: Vec<Rem6CacheTraceRecord>,
    dram_trace: Vec<Rem6DramTraceRecord>,
    fabric_trace: Vec<Rem6FabricTraceRecord>,
    memory_trace: Vec<Rem6MemoryTraceRecord>,
    power_trace: Vec<Rem6PowerTraceRecord>,
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
pub(crate) struct Rem6ExecTraceStat {
    path: String,
    unit: &'static str,
    value: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ExecTraceStatSummary {
    records: u64,
    retired: u64,
    bytes: u64,
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
pub(crate) struct Rem6DataTraceStat {
    path: String,
    unit: &'static str,
    value: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DataTraceStatSummary {
    records: u64,
    loads: u64,
    stores: u64,
    atomics: u64,
    bytes: u64,
    load_bytes: u64,
    store_bytes: u64,
    atomic_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rem6MemoryTraceRecord {
    channel: &'static str,
    tick: u64,
    kind: &'static str,
    route: u64,
    endpoint: String,
    request_agent: u32,
    request: u64,
    response_status: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6MemoryTraceStat {
    path: String,
    unit: &'static str,
    value: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct MemoryTraceStatSummary {
    records: u64,
    requests: BTreeSet<(u32, u64)>,
    routes: BTreeSet<u64>,
    request_agents: BTreeSet<u32>,
    events: BTreeMap<&'static str, u64>,
    response_status: BTreeMap<&'static str, u64>,
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
    ) -> Self {
        let flags = config.debug_flags().to_vec();
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
        let syscall_trace = if config.debug_syscall_enabled() {
            syscall_trace_records(syscall_trace)
        } else {
            Vec::new()
        };
        Self {
            flags,
            exec_trace,
            fetch_trace,
            data_trace,
            cache_trace,
            dram_trace,
            fabric_trace,
            memory_trace,
            power_trace,
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
            self.dram_bank_read_byte_count(),
            self.dram_bank_write_byte_count(),
            self.fabric_lane_byte_count(),
            self.fabric_hop_byte_count(),
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

    pub(crate) fn fetch_trace_count(&self) -> u64 {
        self.fetch_trace.len() as u64
    }

    pub(crate) fn fetch_trace_byte_count(&self) -> u64 {
        self.fetch_trace
            .iter()
            .fold(0u64, |acc, record| acc.saturating_add(record.size))
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

    pub(crate) fn dram_target_trace_count(&self) -> u64 {
        self.dram_kind_trace_count("target")
    }

    pub(crate) fn dram_port_trace_count(&self) -> u64 {
        self.dram_kind_trace_count("port")
    }

    pub(crate) fn dram_bank_trace_count(&self) -> u64 {
        self.dram_kind_trace_count("bank")
    }

    pub(crate) fn dram_target_access_count(&self) -> u64 {
        self.dram_trace_sum(|record| match record {
            Rem6DramTraceRecord::Target { accesses, .. } => Some(*accesses),
            _ => None,
        })
    }

    pub(crate) fn dram_target_read_count(&self) -> u64 {
        self.dram_trace_sum(|record| match record {
            Rem6DramTraceRecord::Target { reads, .. } => Some(*reads),
            _ => None,
        })
    }

    pub(crate) fn dram_target_write_count(&self) -> u64 {
        self.dram_trace_sum(|record| match record {
            Rem6DramTraceRecord::Target { writes, .. } => Some(*writes),
            _ => None,
        })
    }

    pub(crate) fn dram_port_command_count(&self) -> u64 {
        self.dram_trace_sum(|record| match record {
            Rem6DramTraceRecord::Port { commands, .. } => Some(*commands),
            _ => None,
        })
    }

    pub(crate) fn dram_port_row_hit_count(&self) -> u64 {
        self.dram_trace_sum(|record| match record {
            Rem6DramTraceRecord::Port { row_hits, .. } => Some(*row_hits),
            _ => None,
        })
    }

    pub(crate) fn dram_port_row_miss_count(&self) -> u64 {
        self.dram_trace_sum(|record| match record {
            Rem6DramTraceRecord::Port { row_misses, .. } => Some(*row_misses),
            _ => None,
        })
    }

    pub(crate) fn dram_port_refresh_count(&self) -> u64 {
        self.dram_trace_sum(|record| match record {
            Rem6DramTraceRecord::Port { refreshes, .. } => Some(*refreshes),
            _ => None,
        })
    }

    pub(crate) fn dram_port_refresh_tick_count(&self) -> u64 {
        self.dram_trace_sum(|record| match record {
            Rem6DramTraceRecord::Port { refresh_ticks, .. } => Some(*refresh_ticks),
            _ => None,
        })
    }

    pub(crate) fn dram_port_total_ready_latency_tick_count(&self) -> u64 {
        self.dram_trace_sum(|record| match record {
            Rem6DramTraceRecord::Port {
                total_ready_latency_ticks,
                ..
            } => Some(*total_ready_latency_ticks),
            _ => None,
        })
    }

    pub(crate) fn dram_port_max_ready_latency_tick_count(&self) -> u64 {
        self.dram_trace_max(|record| match record {
            Rem6DramTraceRecord::Port {
                max_ready_latency_ticks,
                ..
            } => Some(*max_ready_latency_ticks),
            _ => None,
        })
    }

    pub(crate) fn dram_bank_read_byte_count(&self) -> u64 {
        self.dram_trace_sum(|record| match record {
            Rem6DramTraceRecord::Bank { read_bytes, .. } => Some(*read_bytes),
            _ => None,
        })
    }

    pub(crate) fn dram_bank_write_byte_count(&self) -> u64 {
        self.dram_trace_sum(|record| match record {
            Rem6DramTraceRecord::Bank { write_bytes, .. } => Some(*write_bytes),
            _ => None,
        })
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

    pub(crate) fn fabric_lane_transfer_count(&self) -> u64 {
        self.fabric_trace.iter().fold(0u64, |acc, record| {
            if let Rem6FabricTraceRecord::Lane { transfer_count, .. } = record {
                acc.saturating_add(*transfer_count)
            } else {
                acc
            }
        })
    }

    pub(crate) fn fabric_lane_byte_count(&self) -> u64 {
        self.fabric_trace.iter().fold(0u64, |acc, record| {
            if let Rem6FabricTraceRecord::Lane { byte_count, .. } = record {
                acc.saturating_add(*byte_count)
            } else {
                acc
            }
        })
    }

    pub(crate) fn fabric_lane_flit_count(&self) -> u64 {
        self.fabric_trace.iter().fold(0u64, |acc, record| {
            if let Rem6FabricTraceRecord::Lane { flit_count, .. } = record {
                acc.saturating_add(*flit_count)
            } else {
                acc
            }
        })
    }

    pub(crate) fn fabric_hop_byte_count(&self) -> u64 {
        self.fabric_trace.iter().fold(0u64, |acc, record| {
            if let Rem6FabricTraceRecord::Hop { bytes, .. } = record {
                acc.saturating_add(*bytes)
            } else {
                acc
            }
        })
    }

    pub(crate) fn fabric_hop_flit_count(&self) -> u64 {
        self.fabric_trace.iter().fold(0u64, |acc, record| {
            if let Rem6FabricTraceRecord::Hop { flits, .. } = record {
                acc.saturating_add(*flits)
            } else {
                acc
            }
        })
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
        let syscall_trace = self
            .syscall_trace
            .iter()
            .map(Rem6SyscallTraceRecord::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"flags\":[{}],\"exec_trace\":[{}],\"fetch_trace\":[{}],\"data_trace\":[{}],\"cache_trace\":[{}],\"dram_trace\":[{}],\"fabric_trace\":[{}],\"memory_trace\":[{}],\"power_trace\":[{}],\"syscall_trace\":[{}]}}",
            flags, exec_trace, fetch_trace, data_trace, cache_trace, dram_trace, fabric_trace, memory_trace, power_trace, syscall_trace
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

    fn dram_kind_trace_count(&self, kind: &str) -> u64 {
        self.dram_trace
            .iter()
            .filter(|record| record.kind() == kind)
            .count() as u64
    }

    fn dram_trace_sum<F>(&self, value: F) -> u64
    where
        F: Fn(&Rem6DramTraceRecord) -> Option<u64>,
    {
        self.dram_trace
            .iter()
            .filter_map(value)
            .fold(0u64, |acc, value| acc.saturating_add(value))
    }

    fn dram_trace_max<F>(&self, value: F) -> u64
    where
        F: Fn(&Rem6DramTraceRecord) -> Option<u64>,
    {
        self.dram_trace.iter().filter_map(value).max().unwrap_or(0)
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

    fn trace_counts(&self) -> [u64; 9] {
        [
            self.exec_trace_count(),
            self.fetch_trace_count(),
            self.data_trace_count(),
            self.cache_trace_count(),
            self.dram_trace_count(),
            self.fabric_trace_count(),
            self.memory_trace_count(),
            self.power_trace_count(),
            self.syscall_trace_count(),
        ]
    }

    fn trace_count_for_flag(&self, flag: CliDebugFlag) -> u64 {
        match flag {
            CliDebugFlag::Cache => self.cache_trace_count(),
            CliDebugFlag::Data => self.data_trace_count(),
            CliDebugFlag::Dram => self.dram_trace_count(),
            CliDebugFlag::Exec => self.exec_trace_count(),
            CliDebugFlag::Fabric => self.fabric_trace_count(),
            CliDebugFlag::Fetch => self.fetch_trace_count(),
            CliDebugFlag::Memory => self.memory_trace_count(),
            CliDebugFlag::Power => self.power_trace_count(),
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

impl Rem6ExecTraceStat {
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

impl ExecTraceStatSummary {
    fn add_record(&mut self, record: &Rem6ExecTraceRecord) {
        self.records = self.records.saturating_add(1);
        if record.retired {
            self.retired = self.retired.saturating_add(1);
        }
        self.bytes = self.bytes.saturating_add(record.bytes.len() as u64);
    }

    fn push_stats(&self, stats: &mut Vec<Rem6ExecTraceStat>, prefix: &str) {
        for (suffix, unit, value) in [
            ("records", "Count", self.records),
            ("retired", "Count", self.retired),
            ("bytes", "Byte", self.bytes),
        ] {
            stats.push(Rem6ExecTraceStat {
                path: format!("{prefix}.{suffix}"),
                unit,
                value,
            });
        }
    }
}

fn exec_trace_stats(records: &[Rem6ExecTraceRecord]) -> Vec<Rem6ExecTraceStat> {
    let mut cpus = BTreeMap::<u32, ExecTraceStatSummary>::new();
    let mut retirement = BTreeMap::<&str, ExecTraceStatSummary>::new();
    for record in records {
        cpus.entry(record.cpu).or_default().add_record(record);
        retirement
            .entry(exec_retirement_path(record.retired))
            .or_default()
            .add_record(record);
    }

    let mut stats = Vec::new();
    for (cpu, summary) in cpus {
        summary.push_stats(&mut stats, &format!("cpu.cpu{cpu}"));
    }
    for (retirement, summary) in retirement {
        summary.push_stats(&mut stats, &format!("retirement.{retirement}"));
    }
    stats
}

const fn exec_retirement_path(retired: bool) -> &'static str {
    match retired {
        true => "retired",
        false => "not_retired",
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

impl Rem6DataTraceStat {
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

impl DataTraceStatSummary {
    fn add_record(&mut self, record: &Rem6DataTraceRecord) {
        self.records = self.records.saturating_add(1);
        self.bytes = self.bytes.saturating_add(record.size);
        match record.kind {
            "load" => {
                self.loads = self.loads.saturating_add(1);
                self.load_bytes = self.load_bytes.saturating_add(record.size);
            }
            "store" => {
                self.stores = self.stores.saturating_add(1);
                self.store_bytes = self.store_bytes.saturating_add(record.size);
            }
            "atomic" => {
                self.atomics = self.atomics.saturating_add(1);
                self.atomic_bytes = self.atomic_bytes.saturating_add(record.size);
            }
            other => unreachable!("unexpected data trace kind {other}"),
        }
    }

    fn push_stats(&self, stats: &mut Vec<Rem6DataTraceStat>, prefix: &str) {
        for (suffix, unit, value) in [
            ("records", "Count", self.records),
            ("loads", "Count", self.loads),
            ("stores", "Count", self.stores),
            ("atomics", "Count", self.atomics),
            ("bytes", "Byte", self.bytes),
            ("load_bytes", "Byte", self.load_bytes),
            ("store_bytes", "Byte", self.store_bytes),
            ("atomic_bytes", "Byte", self.atomic_bytes),
        ] {
            stats.push(Rem6DataTraceStat {
                path: format!("{prefix}.{suffix}"),
                unit,
                value,
            });
        }
    }
}

fn data_trace_stats(
    records: &[Rem6DataTraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6DataTraceStat> {
    let mut cpus = BTreeMap::<u32, DataTraceStatSummary>::new();
    let mut kinds = BTreeMap::<&str, DataTraceStatSummary>::new();
    for record in records {
        cpus.entry(record.cpu).or_default().add_record(record);
        kinds.entry(record.kind).or_default().add_record(record);
    }

    let mut stats = Vec::new();
    for (cpu, summary) in cpus {
        summary.push_stats(&mut stats, &format!("cpu.cpu{cpu}"));
    }
    for (kind, summary) in kinds {
        summary.push_stats(&mut stats, &format!("kind.{}", stat_path_segment(kind)));
    }
    stats
}

fn memory_trace_channel_matches(record: &Rem6MemoryTraceRecord, channel: Option<&str>) -> bool {
    channel.map_or(true, |expected| record.channel == expected)
}

impl Rem6MemoryTraceRecord {
    fn to_json(&self) -> String {
        let response_status = self
            .response_status
            .map(|status| format!("\"{status}\""))
            .unwrap_or_else(|| "null".to_string());
        format!(
            "{{\"channel\":\"{}\",\"tick\":{},\"kind\":\"{}\",\"route\":{},\"endpoint\":\"{}\",\"request_agent\":{},\"request\":{},\"response_status\":{}}}",
            self.channel,
            self.tick,
            self.kind,
            self.route,
            json_escape(&self.endpoint),
            self.request_agent,
            self.request,
            response_status,
        )
    }
}

impl Rem6MemoryTraceStat {
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

impl MemoryTraceStatSummary {
    fn add_record(&mut self, record: &Rem6MemoryTraceRecord) {
        self.records = self.records.saturating_add(1);
        self.requests.insert((record.request_agent, record.request));
        self.routes.insert(record.route);
        self.request_agents.insert(record.request_agent);
        self.events
            .entry(record.kind)
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
        if let Some(status) = record.response_status {
            self.response_status
                .entry(status)
                .and_modify(|count| *count = count.saturating_add(1))
                .or_insert(1);
        }
    }

    fn push_stats(&self, stats: &mut Vec<Rem6MemoryTraceStat>, prefix: &str) {
        push_memory_trace_stats(
            stats,
            prefix,
            &[
                ("records", self.records),
                ("requests", self.requests.len() as u64),
                ("routes", self.routes.len() as u64),
                ("request_agents", self.request_agents.len() as u64),
            ],
        );
        for (kind, value) in &self.events {
            stats.push(Rem6MemoryTraceStat {
                path: format!("{prefix}.events.{kind}"),
                unit: "Count",
                value: *value,
            });
        }
        for (status, value) in &self.response_status {
            stats.push(Rem6MemoryTraceStat {
                path: format!("{prefix}.response_status.{status}"),
                unit: "Count",
                value: *value,
            });
        }
    }
}

fn memory_trace_stats(
    records: &[Rem6MemoryTraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6MemoryTraceStat> {
    let mut channels = BTreeMap::<String, MemoryTraceStatSummary>::new();
    let mut routes = BTreeMap::<(String, u64, String), MemoryTraceStatSummary>::new();
    let mut request_agents = BTreeMap::<(String, u32), MemoryTraceStatSummary>::new();
    for record in records {
        let channel = record.channel.to_string();
        channels
            .entry(channel.clone())
            .or_default()
            .add_record(record);
        routes
            .entry((channel.clone(), record.route, record.endpoint.clone()))
            .or_default()
            .add_record(record);
        request_agents
            .entry((channel, record.request_agent))
            .or_default()
            .add_record(record);
    }

    let mut stats = Vec::new();
    for (channel, summary) in channels {
        let prefix = format!("channel.{}", stat_path_segment(&channel));
        summary.push_stats(&mut stats, &prefix);
    }
    for ((channel, route, endpoint), summary) in routes {
        let prefix = format!(
            "channel.{}.route{route}.endpoint.{}",
            stat_path_segment(&channel),
            stat_path_segment(&endpoint)
        );
        summary.push_stats(&mut stats, &prefix);
    }
    for ((channel, request_agent), summary) in request_agents {
        let prefix = format!(
            "channel.{}.request_agent.agent{request_agent}",
            stat_path_segment(&channel)
        );
        summary.push_stats(&mut stats, &prefix);
    }
    stats
}

fn push_memory_trace_stats(
    stats: &mut Vec<Rem6MemoryTraceStat>,
    prefix: &str,
    entries: &[(&'static str, u64)],
) {
    for (suffix, value) in entries {
        stats.push(Rem6MemoryTraceStat {
            path: format!("{prefix}.{suffix}"),
            unit: "Count",
            value: *value,
        });
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

fn memory_trace_records(
    fetch_memory_trace: &MemoryTrace,
    data_memory_trace: &MemoryTrace,
) -> Vec<Rem6MemoryTraceRecord> {
    let mut records = Vec::new();
    records.extend(memory_trace_channel_records("fetch", fetch_memory_trace));
    records.extend(memory_trace_channel_records("data", data_memory_trace));
    records.sort_by_key(|record| {
        (
            record.tick,
            record.channel,
            record.route,
            record.request_agent,
            record.request,
            record.kind,
        )
    });
    records
}

fn memory_trace_channel_records(
    channel: &'static str,
    trace: &MemoryTrace,
) -> Vec<Rem6MemoryTraceRecord> {
    trace
        .snapshot()
        .into_iter()
        .map(|event| memory_trace_record(channel, event))
        .collect()
}

fn memory_trace_record(channel: &'static str, event: MemoryTraceEvent) -> Rem6MemoryTraceRecord {
    let request = event.request_id();
    Rem6MemoryTraceRecord {
        channel,
        tick: event.tick(),
        kind: memory_trace_kind(event.kind()),
        route: event.route().get(),
        endpoint: event.endpoint().as_str().to_string(),
        request_agent: request.agent().get(),
        request: request.sequence(),
        response_status: event.response_status().map(response_status_name),
    }
}

const fn memory_trace_kind(kind: MemoryTraceKind) -> &'static str {
    match kind {
        MemoryTraceKind::RequestSent => "request_sent",
        MemoryTraceKind::RequestArrived => "request_arrived",
        MemoryTraceKind::ResponseArrived => "response_arrived",
    }
}

const fn response_status_name(status: ResponseStatus) -> &'static str {
    match status {
        ResponseStatus::Completed => "completed",
        ResponseStatus::Retry => "retry",
        ResponseStatus::StoreConditionalFailed => "store_conditional_failed",
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

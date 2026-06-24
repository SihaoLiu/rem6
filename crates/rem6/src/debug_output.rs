use rem6_cpu::{CpuFetchEventKind, RiscvCluster, RiscvCoreDriveAction, RiscvDataAccessEventKind};
use rem6_memory::{MemoryOperation, ResponseStatus};
use rem6_power::{PowerAnalysisRecord, PowerStateKind};
use rem6_system::{RiscvSyscallTraceOutcome, RiscvSyscallTraceRecord, RiscvSystemRun};
use rem6_transport::{MemoryTrace, MemoryTraceEvent, MemoryTraceKind};

use crate::formatting::{bytes_to_hex, json_escape};
use crate::{CliDebugFlag, Rem6DramSummary, Rem6RunConfig, Rem6RunFabricSummary};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6DebugSummary {
    flags: Vec<CliDebugFlag>,
    exec_trace: Vec<Rem6ExecTraceRecord>,
    fetch_trace: Vec<Rem6FetchTraceRecord>,
    data_trace: Vec<Rem6DataTraceRecord>,
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
enum Rem6DramTraceRecord {
    Target {
        target: u32,
        accesses: u64,
        reads: u64,
        writes: u64,
        row_hits: u64,
        row_misses: u64,
        refreshes: u64,
        refresh_ticks: u64,
        commands: u64,
        turnarounds: u64,
        total_ready_latency_ticks: u64,
        max_ready_latency_ticks: u64,
    },
    Port {
        target: u32,
        port: u32,
        accesses: u64,
        reads: u64,
        writes: u64,
        commands: u64,
        turnarounds: u64,
    },
    Bank {
        target: u32,
        port: u32,
        bank: u32,
        accesses: u64,
        read_bytes: u64,
        write_bytes: u64,
        row_hits: u64,
        row_misses: u64,
        refreshes: u64,
        refresh_ticks: u64,
        commands: u64,
        total_ready_latency_ticks: u64,
        max_ready_latency_ticks: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Rem6FabricTraceRecord {
    Lane {
        link: String,
        virtual_network: u64,
        transfer_count: u64,
        byte_count: u64,
        flit_count: u64,
        occupied_ticks: u64,
        queue_delay_ticks: u64,
        max_queue_delay_ticks: u64,
        credit_delay_ticks: u64,
        max_credit_delay_ticks: u64,
        first_tick: u64,
        last_tick: u64,
    },
    Hop {
        packet: u64,
        hop_index: u64,
        link: String,
        virtual_network: u64,
        bytes: u64,
        flits: u64,
        ready_tick: u64,
        start_tick: u64,
        occupied_ticks: u64,
        queue_delay_ticks: u64,
        credit_delay_ticks: u64,
        depart_tick: u64,
        arrival_tick: u64,
    },
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
struct Rem6PowerTraceRecord {
    target: String,
    state: &'static str,
    residency_ticks: u64,
    temperature_c: String,
    dynamic_watts: String,
    static_watts: String,
    total_watts: String,
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

impl Rem6DebugSummary {
    pub(crate) fn from_run(
        config: &Rem6RunConfig,
        cluster: &RiscvCluster,
        run: &RiscvSystemRun,
        fetch_memory_trace: &MemoryTrace,
        data_memory_trace: &MemoryTrace,
        fabric: &Rem6RunFabricSummary,
        dram: &Rem6DramSummary,
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

    pub(crate) fn dram_trace_count(&self) -> u64 {
        self.dram_trace.len() as u64
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

    pub(crate) fn power_trace_count(&self) -> u64 {
        self.power_trace.len() as u64
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
            "{{\"flags\":[{}],\"exec_trace\":[{}],\"fetch_trace\":[{}],\"data_trace\":[{}],\"dram_trace\":[{}],\"fabric_trace\":[{}],\"memory_trace\":[{}],\"power_trace\":[{}],\"syscall_trace\":[{}]}}",
            flags, exec_trace, fetch_trace, data_trace, dram_trace, fabric_trace, memory_trace, power_trace, syscall_trace
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

    fn syscall_outcome_trace_count(
        &self,
        matches_outcome: impl Fn(RiscvSyscallTraceOutcome) -> bool,
    ) -> u64 {
        self.syscall_trace
            .iter()
            .filter(|record| matches_outcome(record.outcome))
            .count() as u64
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

impl Rem6DramTraceRecord {
    fn to_json(&self) -> String {
        match self {
            Self::Target {
                target,
                accesses,
                reads,
                writes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                turnarounds,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
            } => format!(
                "{{\"kind\":\"target\",\"target\":{},\"accesses\":{},\"reads\":{},\"writes\":{},\"row_hits\":{},\"row_misses\":{},\"refreshes\":{},\"refresh_ticks\":{},\"commands\":{},\"turnarounds\":{},\"total_ready_latency_ticks\":{},\"max_ready_latency_ticks\":{}}}",
                target,
                accesses,
                reads,
                writes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                turnarounds,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
            ),
            Self::Port {
                target,
                port,
                accesses,
                reads,
                writes,
                commands,
                turnarounds,
            } => format!(
                "{{\"kind\":\"port\",\"target\":{},\"port\":{},\"accesses\":{},\"reads\":{},\"writes\":{},\"commands\":{},\"turnarounds\":{}}}",
                target, port, accesses, reads, writes, commands, turnarounds,
            ),
            Self::Bank {
                target,
                port,
                bank,
                accesses,
                read_bytes,
                write_bytes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
            } => format!(
                "{{\"kind\":\"bank\",\"target\":{},\"port\":{},\"bank\":{},\"accesses\":{},\"read_bytes\":{},\"write_bytes\":{},\"row_hits\":{},\"row_misses\":{},\"refreshes\":{},\"refresh_ticks\":{},\"commands\":{},\"total_ready_latency_ticks\":{},\"max_ready_latency_ticks\":{}}}",
                target,
                port,
                bank,
                accesses,
                read_bytes,
                write_bytes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
            ),
        }
    }

    const fn kind(&self) -> &'static str {
        match self {
            Self::Target { .. } => "target",
            Self::Port { .. } => "port",
            Self::Bank { .. } => "bank",
        }
    }

    const fn sort_key(&self) -> (u32, u8, u32, u32) {
        match self {
            Self::Target { target, .. } => (*target, 0, u32::MAX, u32::MAX),
            Self::Port { target, port, .. } => (*target, 1, *port, u32::MAX),
            Self::Bank {
                target, port, bank, ..
            } => (*target, 2, *port, *bank),
        }
    }
}

impl Rem6FabricTraceRecord {
    fn to_json(&self) -> String {
        match self {
            Self::Lane {
                link,
                virtual_network,
                transfer_count,
                byte_count,
                flit_count,
                occupied_ticks,
                queue_delay_ticks,
                max_queue_delay_ticks,
                credit_delay_ticks,
                max_credit_delay_ticks,
                first_tick,
                last_tick,
            } => format!(
                "{{\"kind\":\"lane\",\"link\":\"{}\",\"virtual_network\":{},\"transfer_count\":{},\"byte_count\":{},\"flit_count\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"max_credit_delay_ticks\":{},\"first_tick\":{},\"last_tick\":{}}}",
                json_escape(link),
                virtual_network,
                transfer_count,
                byte_count,
                flit_count,
                occupied_ticks,
                queue_delay_ticks,
                max_queue_delay_ticks,
                credit_delay_ticks,
                max_credit_delay_ticks,
                first_tick,
                last_tick,
            ),
            Self::Hop {
                packet,
                hop_index,
                link,
                virtual_network,
                bytes,
                flits,
                ready_tick,
                start_tick,
                occupied_ticks,
                queue_delay_ticks,
                credit_delay_ticks,
                depart_tick,
                arrival_tick,
            } => format!(
                "{{\"kind\":\"hop\",\"packet\":{},\"hop_index\":{},\"link\":\"{}\",\"virtual_network\":{},\"bytes\":{},\"flits\":{},\"ready_tick\":{},\"start_tick\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"depart_tick\":{},\"arrival_tick\":{}}}",
                packet,
                hop_index,
                json_escape(link),
                virtual_network,
                bytes,
                flits,
                ready_tick,
                start_tick,
                occupied_ticks,
                queue_delay_ticks,
                credit_delay_ticks,
                depart_tick,
                arrival_tick,
            ),
        }
    }

    fn sort_key(&self) -> (u64, u8, String, u64, u64, u64) {
        match self {
            Self::Lane {
                first_tick,
                link,
                virtual_network,
                ..
            } => (*first_tick, 0, link.clone(), *virtual_network, 0, 0),
            Self::Hop {
                start_tick,
                link,
                virtual_network,
                packet,
                hop_index,
                ..
            } => (
                *start_tick,
                1,
                link.clone(),
                *virtual_network,
                *packet,
                *hop_index,
            ),
        }
    }
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

fn power_trace_records(records: &[PowerAnalysisRecord]) -> Vec<Rem6PowerTraceRecord> {
    records
        .iter()
        .map(|record| Rem6PowerTraceRecord {
            target: record.target().to_string(),
            state: power_state_name(record.current_state()),
            residency_ticks: record.residency_ticks(record.current_state()),
            temperature_c: format!("{:.6}", record.temperature_c()),
            dynamic_watts: format!("{:.6}", record.dynamic_watts()),
            static_watts: format!("{:.6}", record.static_watts()),
            total_watts: format!("{:.6}", record.total_watts()),
        })
        .collect()
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

fn dram_trace_records(dram: &Rem6DramSummary) -> Vec<Rem6DramTraceRecord> {
    let mut records = Vec::new();
    for target in &dram.targets {
        records.push(Rem6DramTraceRecord::Target {
            target: target.target,
            accesses: target.accesses,
            reads: target.reads,
            writes: target.writes,
            row_hits: target.row_hits,
            row_misses: target.row_misses,
            refreshes: target.refreshes,
            refresh_ticks: target.refresh_ticks,
            commands: target.commands,
            turnarounds: target.turnarounds,
            total_ready_latency_ticks: target.total_ready_latency_ticks,
            max_ready_latency_ticks: target.max_ready_latency_ticks,
        });
        for port in &target.ports {
            records.push(Rem6DramTraceRecord::Port {
                target: target.target,
                port: port.port,
                accesses: port.accesses,
                reads: port.reads,
                writes: port.writes,
                commands: port.commands,
                turnarounds: port.turnarounds,
            });
            for bank in &port.banks {
                records.push(Rem6DramTraceRecord::Bank {
                    target: target.target,
                    port: port.port,
                    bank: bank.bank,
                    accesses: bank.accesses,
                    read_bytes: bank.read_bytes,
                    write_bytes: bank.write_bytes,
                    row_hits: bank.row_hits,
                    row_misses: bank.row_misses,
                    refreshes: bank.refreshes,
                    refresh_ticks: bank.refresh_ticks,
                    commands: bank.commands,
                    total_ready_latency_ticks: bank.total_ready_latency_ticks,
                    max_ready_latency_ticks: bank.max_ready_latency_ticks,
                });
            }
        }
    }
    records.sort_by_key(Rem6DramTraceRecord::sort_key);
    records
}

fn fabric_trace_records(fabric: &Rem6RunFabricSummary) -> Vec<Rem6FabricTraceRecord> {
    let mut records = Vec::new();
    records.extend(
        fabric
            .lane_activities()
            .iter()
            .map(|activity| Rem6FabricTraceRecord::Lane {
                link: activity.link().as_str().to_string(),
                virtual_network: u64::from(activity.virtual_network().get()),
                transfer_count: activity.transfer_count() as u64,
                byte_count: activity.byte_count(),
                flit_count: activity.flit_count(),
                occupied_ticks: activity.occupied_ticks(),
                queue_delay_ticks: activity.queue_delay_ticks(),
                max_queue_delay_ticks: activity.max_queue_delay_ticks(),
                credit_delay_ticks: activity.credit_delay_ticks(),
                max_credit_delay_ticks: activity.max_credit_delay_ticks(),
                first_tick: activity.first_tick(),
                last_tick: activity.last_tick(),
            }),
    );
    records.extend(
        fabric
            .hop_activities()
            .iter()
            .map(|activity| Rem6FabricTraceRecord::Hop {
                packet: activity.packet().get(),
                hop_index: activity.hop_index() as u64,
                link: activity.link().as_str().to_string(),
                virtual_network: u64::from(activity.virtual_network().get()),
                bytes: activity.bytes(),
                flits: activity.flits(),
                ready_tick: activity.ready_tick(),
                start_tick: activity.start_tick(),
                occupied_ticks: activity.occupied_ticks(),
                queue_delay_ticks: activity.queue_delay_ticks(),
                credit_delay_ticks: activity.credit_delay_ticks(),
                depart_tick: activity.depart_tick(),
                arrival_tick: activity.arrival_tick(),
            }),
    );
    records.sort_by_key(Rem6FabricTraceRecord::sort_key);
    records
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

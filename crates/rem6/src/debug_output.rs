use rem6_cpu::{CpuFetchEventKind, RiscvCluster, RiscvCoreDriveAction, RiscvDataAccessEventKind};
use rem6_memory::{MemoryOperation, ResponseStatus};
use rem6_power::{PowerAnalysisRecord, PowerStateKind};
use rem6_system::{RiscvSyscallTraceOutcome, RiscvSyscallTraceRecord, RiscvSystemRun};
use rem6_transport::{MemoryTrace, MemoryTraceEvent, MemoryTraceKind};

use crate::formatting::{bytes_to_hex, json_escape};
use crate::{CliDebugFlag, Rem6RunConfig};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6DebugSummary {
    flags: Vec<CliDebugFlag>,
    exec_trace: Vec<Rem6ExecTraceRecord>,
    fetch_trace: Vec<Rem6FetchTraceRecord>,
    data_trace: Vec<Rem6DataTraceRecord>,
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
            memory_trace,
            power_trace,
            syscall_trace,
        }
    }

    pub(crate) fn has_enabled_flags(&self) -> bool {
        !self.flags.is_empty()
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
            "{{\"flags\":[{}],\"exec_trace\":[{}],\"fetch_trace\":[{}],\"data_trace\":[{}],\"memory_trace\":[{}],\"power_trace\":[{}],\"syscall_trace\":[{}]}}",
            flags, exec_trace, fetch_trace, data_trace, memory_trace, power_trace, syscall_trace
        )
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

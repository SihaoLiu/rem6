use rem6_cpu::{CpuFetchEventKind, RiscvCluster, RiscvCoreDriveAction};
use rem6_system::RiscvSystemRun;

use crate::formatting::{bytes_to_hex, json_escape};
use crate::{CliDebugFlag, Rem6RunConfig};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6DebugSummary {
    flags: Vec<CliDebugFlag>,
    exec_trace: Vec<Rem6ExecTraceRecord>,
    fetch_trace: Vec<Rem6FetchTraceRecord>,
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

impl Rem6DebugSummary {
    pub(crate) fn from_run(
        config: &Rem6RunConfig,
        cluster: &RiscvCluster,
        run: &RiscvSystemRun,
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
        Self {
            flags,
            exec_trace,
            fetch_trace,
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
        format!(
            "{{\"flags\":[{}],\"exec_trace\":[{}],\"fetch_trace\":[{}]}}",
            flags, exec_trace, fetch_trace
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

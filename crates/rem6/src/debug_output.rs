use rem6_cpu::RiscvCoreDriveAction;
use rem6_system::RiscvSystemRun;

use crate::formatting::bytes_to_hex;
use crate::{CliDebugFlag, Rem6RunConfig};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6DebugSummary {
    flags: Vec<CliDebugFlag>,
    exec_trace: Vec<Rem6ExecTraceRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rem6ExecTraceRecord {
    cpu: u32,
    tick: u64,
    pc: u64,
    bytes: Vec<u8>,
    retired: bool,
}

impl Rem6DebugSummary {
    pub(crate) fn from_run(config: &Rem6RunConfig, run: &RiscvSystemRun) -> Self {
        let flags = config.debug_flags().to_vec();
        let exec_trace = if config.debug_exec_enabled() {
            exec_trace_records(run)
        } else {
            Vec::new()
        };
        Self { flags, exec_trace }
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
        format!("{{\"flags\":[{}],\"exec_trace\":[{}]}}", flags, exec_trace)
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

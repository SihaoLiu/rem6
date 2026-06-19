use rem6_stats::{PcCountPair, ProbePayload};
use rem6_system::{RiscvRetiredInstructionProbeSnapshot, RiscvSystemRun};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6InstructionProbeSummary {
    pub(crate) event_count: u64,
    pub(crate) retired_instruction_events: u64,
    pub(crate) tracked_instructions: u64,
    pub(crate) pc_sample_events: u64,
    pub(crate) pc_target_counters: u64,
    pub(crate) pc_count: Option<Rem6PcCountTrackerSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6PcCountTrackerSummary {
    pub(crate) armed: bool,
    pub(crate) current_pair: Rem6PcCountPairSummary,
    pub(crate) counters: Vec<Rem6PcCountPairSummary>,
    pub(crate) pending_targets: Vec<Rem6PcCountPairSummary>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6PcCountPairSummary {
    pub(crate) pc: u64,
    pub(crate) count: u64,
}

pub(crate) fn instruction_probe_summary(run: &RiscvSystemRun) -> Rem6InstructionProbeSummary {
    run.retired_instruction_probes()
        .map(instruction_probe_snapshot_summary)
        .unwrap_or_default()
}

fn instruction_probe_snapshot_summary(
    probes: &RiscvRetiredInstructionProbeSnapshot,
) -> Rem6InstructionProbeSummary {
    let mut retired_instruction_events = 0_u64;
    let mut pc_sample_events = 0_u64;
    for event in probes.probes().events() {
        match event.payload() {
            ProbePayload::Counter { .. } => {
                retired_instruction_events = retired_instruction_events.saturating_add(1);
            }
            ProbePayload::ProgramCounter { .. } => {
                pc_sample_events = pc_sample_events.saturating_add(1);
            }
            ProbePayload::Unit | ProbePayload::MemoryPacket(_) => {}
        }
    }

    let pc_count = probes.pc_count().map(|pc_count| Rem6PcCountTrackerSummary {
        armed: pc_count.is_armed(),
        current_pair: pc_count_pair_summary(pc_count.current_pair()),
        counters: pc_count
            .counters()
            .iter()
            .map(|(pc, count)| Rem6PcCountPairSummary {
                pc: *pc,
                count: *count,
            })
            .collect(),
        pending_targets: pc_count
            .pending_targets()
            .iter()
            .copied()
            .map(pc_count_pair_summary)
            .collect(),
    });

    Rem6InstructionProbeSummary {
        event_count: probes.probes().events().len() as u64,
        retired_instruction_events,
        tracked_instructions: probes.tracker().counter(),
        pc_sample_events,
        pc_target_counters: pc_count
            .as_ref()
            .map(|pc_count| pc_count.counters.len() as u64)
            .unwrap_or(0),
        pc_count,
    }
}

const fn pc_count_pair_summary(pair: PcCountPair) -> Rem6PcCountPairSummary {
    Rem6PcCountPairSummary {
        pc: pair.pc(),
        count: pair.count(),
    }
}

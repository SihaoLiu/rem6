use std::collections::BTreeMap;

use rem6_cpu::{
    CpuFetchEventKind, InOrderPipelineRunSummary, InOrderPipelineSnapshot, InOrderPipelineStage,
    RiscvCore,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6InOrderPipelineStageInFlightSummary {
    pub(crate) fetch1: u64,
    pub(crate) fetch2: u64,
    pub(crate) decode: u64,
    pub(crate) execute: u64,
    pub(crate) commit: u64,
}

impl Rem6InOrderPipelineStageInFlightSummary {
    fn max_with(self, other: Self) -> Self {
        Self {
            fetch1: self.fetch1.max(other.fetch1),
            fetch2: self.fetch2.max(other.fetch2),
            decode: self.decode.max(other.decode),
            execute: self.execute.max(other.execute),
            commit: self.commit.max(other.commit),
        }
    }
}

pub(super) fn in_order_pipeline_run_summary(core: &RiscvCore) -> InOrderPipelineRunSummary {
    InOrderPipelineRunSummary::from_cycle_records(core.in_order_pipeline_cycle_records())
}

pub(super) fn in_order_pipeline_stage_in_flight(
    snapshot: &InOrderPipelineSnapshot,
) -> Rem6InOrderPipelineStageInFlightSummary {
    stage_in_flight_from_snapshot(snapshot)
}

pub(super) fn in_order_pipeline_stage_max_in_flight(
    core: &RiscvCore,
    final_snapshot: &InOrderPipelineSnapshot,
) -> Rem6InOrderPipelineStageInFlightSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        stage_in_flight_from_snapshot(final_snapshot),
        |summary, record| {
            summary
                .max_with(stage_in_flight_from_snapshot(record.before()))
                .max_with(stage_in_flight_from_snapshot(record.after()))
        },
    )
}

fn stage_in_flight_from_snapshot(
    snapshot: &InOrderPipelineSnapshot,
) -> Rem6InOrderPipelineStageInFlightSummary {
    let mut summary = Rem6InOrderPipelineStageInFlightSummary::default();
    for instruction in snapshot.in_flight() {
        match instruction.stage() {
            InOrderPipelineStage::Fetch1 => summary.fetch1 += 1,
            InOrderPipelineStage::Fetch2 => summary.fetch2 += 1,
            InOrderPipelineStage::Decode => summary.decode += 1,
            InOrderPipelineStage::Execute => summary.execute += 1,
            InOrderPipelineStage::Commit => summary.commit += 1,
        }
    }
    summary
}

pub(super) fn in_order_pipeline_fetch_wait_cycles(core: &RiscvCore) -> u64 {
    let mut issued_ticks = BTreeMap::new();
    let mut wait_cycles = 0u64;
    for event in core.inner().fetch_events() {
        match event.kind() {
            CpuFetchEventKind::Issued => {
                issued_ticks.insert(event.request_id(), event.tick());
            }
            CpuFetchEventKind::Completed => {
                if let Some(issued) = issued_ticks.remove(&event.request_id()) {
                    wait_cycles = wait_cycles.saturating_add(event.tick().saturating_sub(issued));
                }
            }
            CpuFetchEventKind::Retry | CpuFetchEventKind::Failed => {
                issued_ticks.remove(&event.request_id());
            }
        }
    }
    wait_cycles
}

pub(super) fn in_order_pipeline_data_wait_cycles(core: &RiscvCore) -> u64 {
    core.execution_events()
        .iter()
        .map(|event| event.in_order_pipeline_data_wait_cycles())
        .sum()
}

use std::collections::BTreeMap;

use rem6_cpu::{
    CpuFetchEventKind, InOrderPipelineInstruction, InOrderPipelineRunSummary,
    InOrderPipelineSnapshot, InOrderPipelineStage, RiscvCore,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6InOrderPipelineStageSummary {
    pub(crate) fetch1: u64,
    pub(crate) fetch2: u64,
    pub(crate) decode: u64,
    pub(crate) execute: u64,
    pub(crate) commit: u64,
}

impl Rem6InOrderPipelineStageSummary {
    fn max_with(self, other: Self) -> Self {
        Self {
            fetch1: self.fetch1.max(other.fetch1),
            fetch2: self.fetch2.max(other.fetch2),
            decode: self.decode.max(other.decode),
            execute: self.execute.max(other.execute),
            commit: self.commit.max(other.commit),
        }
    }

    fn saturating_add(self, other: Self) -> Self {
        Self {
            fetch1: self.fetch1.saturating_add(other.fetch1),
            fetch2: self.fetch2.saturating_add(other.fetch2),
            decode: self.decode.saturating_add(other.decode),
            execute: self.execute.saturating_add(other.execute),
            commit: self.commit.saturating_add(other.commit),
        }
    }

    pub(crate) fn values(self) -> [u64; 5] {
        [
            self.fetch1,
            self.fetch2,
            self.decode,
            self.execute,
            self.commit,
        ]
    }
}

pub(super) fn in_order_pipeline_run_summary(core: &RiscvCore) -> InOrderPipelineRunSummary {
    InOrderPipelineRunSummary::from_cycle_records(core.in_order_pipeline_cycle_records())
}

pub(super) fn in_order_pipeline_stage_in_flight(
    snapshot: &InOrderPipelineSnapshot,
) -> Rem6InOrderPipelineStageSummary {
    stage_in_flight_from_snapshot(snapshot)
}

pub(super) fn in_order_pipeline_stage_widths(
    snapshot: &InOrderPipelineSnapshot,
) -> Rem6InOrderPipelineStageSummary {
    Rem6InOrderPipelineStageSummary {
        fetch1: snapshot.config().width(InOrderPipelineStage::Fetch1) as u64,
        fetch2: snapshot.config().width(InOrderPipelineStage::Fetch2) as u64,
        decode: snapshot.config().width(InOrderPipelineStage::Decode) as u64,
        execute: snapshot.config().width(InOrderPipelineStage::Execute) as u64,
        commit: snapshot.config().width(InOrderPipelineStage::Commit) as u64,
    }
}

pub(super) fn in_order_pipeline_stage_max_in_flight(
    core: &RiscvCore,
    final_snapshot: &InOrderPipelineSnapshot,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        stage_in_flight_from_snapshot(final_snapshot),
        |summary, record| {
            summary
                .max_with(stage_in_flight_from_snapshot(record.before()))
                .max_with(stage_in_flight_from_snapshot(record.after()))
        },
    )
}

pub(super) fn in_order_pipeline_stage_occupied_cycles(
    core: &RiscvCore,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| summary.saturating_add(stage_in_flight_from_snapshot(record.before())),
    )
}

pub(super) fn in_order_pipeline_stage_resource_blocked(
    core: &RiscvCore,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| {
            summary.saturating_add(stage_summary_from_instructions(
                record.plan().resource_blocked(),
            ))
        },
    )
}

pub(super) fn in_order_pipeline_stage_ordering_blocked(
    core: &RiscvCore,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| {
            summary.saturating_add(stage_summary_from_instructions(
                record.plan().ordering_blocked(),
            ))
        },
    )
}

pub(super) fn in_order_pipeline_stage_flushed(core: &RiscvCore) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| {
            summary.saturating_add(stage_summary_from_instructions(record.plan().flushed()))
        },
    )
}

pub(super) fn in_order_pipeline_stage_branch_prediction_flushed(
    core: &RiscvCore,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| {
            record
                .branch_predictions()
                .iter()
                .fold(summary, |summary, prediction| {
                    summary.saturating_add(stage_summary_from_instructions(prediction.flushed()))
                })
        },
    )
}

fn stage_in_flight_from_snapshot(
    snapshot: &InOrderPipelineSnapshot,
) -> Rem6InOrderPipelineStageSummary {
    let mut summary = Rem6InOrderPipelineStageSummary::default();
    for instruction in snapshot.in_flight() {
        summary.record_stage(instruction.stage());
    }
    summary
}

fn stage_summary_from_instructions(
    instructions: &[InOrderPipelineInstruction],
) -> Rem6InOrderPipelineStageSummary {
    let mut summary = Rem6InOrderPipelineStageSummary::default();
    for instruction in instructions {
        summary.record_stage(instruction.stage());
    }
    summary
}

impl Rem6InOrderPipelineStageSummary {
    fn record_stage(&mut self, stage: InOrderPipelineStage) {
        match stage {
            InOrderPipelineStage::Fetch1 => self.fetch1 += 1,
            InOrderPipelineStage::Fetch2 => self.fetch2 += 1,
            InOrderPipelineStage::Decode => self.decode += 1,
            InOrderPipelineStage::Execute => self.execute += 1,
            InOrderPipelineStage::Commit => self.commit += 1,
        }
    }
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

use std::collections::BTreeMap;

use rem6_cpu::{
    CpuFetchEventKind, InOrderPipelineAdvance, InOrderPipelineInstruction,
    InOrderPipelineRedirectCause, InOrderPipelineRunSummary, InOrderPipelineSnapshot,
    InOrderPipelineStage, InOrderPipelineStallCause, RiscvCore,
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

    fn presence(self) -> Self {
        Self {
            fetch1: u64::from(self.fetch1 > 0),
            fetch2: u64::from(self.fetch2 > 0),
            decode: u64::from(self.decode > 0),
            execute: u64::from(self.execute > 0),
            commit: u64::from(self.commit > 0),
        }
    }

    fn saturating_mul(self, scalar: u64) -> Self {
        Self {
            fetch1: self.fetch1.saturating_mul(scalar),
            fetch2: self.fetch2.saturating_mul(scalar),
            decode: self.decode.saturating_mul(scalar),
            execute: self.execute.saturating_mul(scalar),
            commit: self.commit.saturating_mul(scalar),
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

pub(super) fn in_order_pipeline_stage_advanced(
    core: &RiscvCore,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| {
            summary.saturating_add(stage_summary_from_advances(record.plan().advanced()))
        },
    )
}

pub(super) fn in_order_pipeline_stage_advanced_cycles(
    core: &RiscvCore,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| {
            summary.saturating_add(stage_presence_summary_from_advances(
                record.plan().advanced(),
            ))
        },
    )
}

pub(super) fn in_order_pipeline_stage_retired(core: &RiscvCore) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| {
            summary.saturating_add(stage_summary_from_retiring_advances(
                record.plan().advanced(),
            ))
        },
    )
}

pub(super) fn in_order_pipeline_stage_retired_cycles(
    core: &RiscvCore,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| {
            summary.saturating_add(stage_presence_summary_from_retiring_advances(
                record.plan().advanced(),
            ))
        },
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

pub(super) fn in_order_pipeline_stage_resource_blocked_cycles(
    core: &RiscvCore,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| {
            summary.saturating_add(stage_presence_summary_from_instructions(
                record.plan().resource_blocked(),
            ))
        },
    )
}

pub(super) fn in_order_pipeline_stage_resource_blocked_for_stall_cause(
    core: &RiscvCore,
    cause: InOrderPipelineStallCause,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| {
            if record.stall_cause() == Some(cause) {
                summary.saturating_add(stage_summary_from_instructions(
                    record.plan().resource_blocked(),
                ))
            } else {
                summary
            }
        },
    )
}

pub(super) fn in_order_pipeline_stage_resource_blocked_cycles_for_stall_cause(
    core: &RiscvCore,
    cause: InOrderPipelineStallCause,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| {
            if record.stall_cause() == Some(cause) {
                summary.saturating_add(
                    stage_presence_summary_from_instructions(record.plan().resource_blocked())
                        .saturating_mul(record.stall_cycle_count()),
                )
            } else {
                summary
            }
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

pub(super) fn in_order_pipeline_stage_ordering_blocked_cycles(
    core: &RiscvCore,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| {
            summary.saturating_add(stage_presence_summary_from_instructions(
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

pub(super) fn in_order_pipeline_stage_flushed_cycles(
    core: &RiscvCore,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| {
            summary.saturating_add(stage_presence_summary_from_instructions(
                record.plan().flushed(),
            ))
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

pub(super) fn in_order_pipeline_stage_branch_prediction_flushed_cycles(
    core: &RiscvCore,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| {
            let cycle_summary = record.branch_predictions().iter().fold(
                Rem6InOrderPipelineStageSummary::default(),
                |summary, prediction| {
                    summary.saturating_add(stage_summary_from_instructions(prediction.flushed()))
                },
            );
            summary.saturating_add(cycle_summary.presence())
        },
    )
}

pub(super) fn in_order_pipeline_stage_trap_redirect_flushed(
    core: &RiscvCore,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| match record.plan().redirect().map(|redirect| redirect.cause()) {
            Some(InOrderPipelineRedirectCause::Trap) => {
                summary.saturating_add(stage_summary_from_instructions(record.plan().flushed()))
            }
            Some(InOrderPipelineRedirectCause::BranchPrediction) | None => summary,
        },
    )
}

pub(super) fn in_order_pipeline_stage_trap_redirect_flushed_cycles(
    core: &RiscvCore,
) -> Rem6InOrderPipelineStageSummary {
    core.in_order_pipeline_cycle_records().into_iter().fold(
        Rem6InOrderPipelineStageSummary::default(),
        |summary, record| match record.plan().redirect().map(|redirect| redirect.cause()) {
            Some(InOrderPipelineRedirectCause::Trap) => summary.saturating_add(
                stage_presence_summary_from_instructions(record.plan().flushed()),
            ),
            Some(InOrderPipelineRedirectCause::BranchPrediction) | None => summary,
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

fn stage_summary_from_advances(
    advances: &[InOrderPipelineAdvance],
) -> Rem6InOrderPipelineStageSummary {
    let mut summary = Rem6InOrderPipelineStageSummary::default();
    for advance in advances {
        summary.record_stage(advance.source_stage());
    }
    summary
}

fn stage_presence_summary_from_advances(
    advances: &[InOrderPipelineAdvance],
) -> Rem6InOrderPipelineStageSummary {
    let mut summary = Rem6InOrderPipelineStageSummary::default();
    for stage in InOrderPipelineStage::ALL {
        if advances
            .iter()
            .any(|advance| advance.source_stage() == stage)
        {
            summary.record_stage(stage);
        }
    }
    summary
}

fn stage_summary_from_retiring_advances(
    advances: &[InOrderPipelineAdvance],
) -> Rem6InOrderPipelineStageSummary {
    let mut summary = Rem6InOrderPipelineStageSummary::default();
    for advance in advances {
        if advance.retires() {
            summary.record_stage(advance.source_stage());
        }
    }
    summary
}

fn stage_presence_summary_from_retiring_advances(
    advances: &[InOrderPipelineAdvance],
) -> Rem6InOrderPipelineStageSummary {
    let mut summary = Rem6InOrderPipelineStageSummary::default();
    for stage in InOrderPipelineStage::ALL {
        if advances
            .iter()
            .any(|advance| advance.retires() && advance.source_stage() == stage)
        {
            summary.record_stage(stage);
        }
    }
    summary
}

fn stage_presence_summary_from_instructions(
    instructions: &[InOrderPipelineInstruction],
) -> Rem6InOrderPipelineStageSummary {
    let mut summary = Rem6InOrderPipelineStageSummary::default();
    for stage in InOrderPipelineStage::ALL {
        if instructions
            .iter()
            .any(|instruction| instruction.stage() == stage)
        {
            summary.record_stage(stage);
        }
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

pub(super) fn in_order_pipeline_execute_wait_cycles(core: &RiscvCore) -> u64 {
    core.in_order_pipeline_cycle_records()
        .into_iter()
        .filter(|record| record.stall_cause() == Some(InOrderPipelineStallCause::ExecuteWait))
        .map(|record| record.stall_cycle_count())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_presence_summary_counts_each_stage_once_per_cycle() {
        let instructions = [
            InOrderPipelineInstruction::new(1, InOrderPipelineStage::Execute),
            InOrderPipelineInstruction::new(2, InOrderPipelineStage::Execute),
            InOrderPipelineInstruction::new(3, InOrderPipelineStage::Decode),
        ];

        assert_eq!(
            stage_presence_summary_from_instructions(&instructions),
            Rem6InOrderPipelineStageSummary {
                decode: 1,
                execute: 1,
                ..Rem6InOrderPipelineStageSummary::default()
            }
        );
    }
}

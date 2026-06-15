use rem6_isa_riscv::RiscvInstruction;
use rem6_memory::{AccessSize, Address, MemoryRequestId};

use crate::{
    riscv_execution_event::RiscvRetiredBranchUpdates, CpuFetchEvent, CpuFetchEventKind,
    CpuFetchRecord, InOrderBranchPrediction, InOrderPipelineCycleRecord,
    InOrderPipelineInstruction, InOrderPipelineStage, RiscvCore, RiscvCoreState, RiscvCpuError,
    RiscvCpuExecutionEvent, RiscvGShareBranchUpdate, RiscvTournamentBranchUpdate,
    RISCV_LOCAL_GSHARE_THREAD, RISCV_LOCAL_TOURNAMENT_THREAD,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiscvPendingFetchPrefix {
    fetch: CpuFetchEvent,
    bytes: [u8; 2],
}

impl RiscvPendingFetchPrefix {
    pub(crate) const fn new(fetch: CpuFetchEvent, bytes: [u8; 2]) -> Self {
        Self { fetch, bytes }
    }
}

impl RiscvCore {
    pub fn execute_next_completed_fetch(
        &self,
    ) -> Result<Option<RiscvCpuExecutionEvent>, RiscvCpuError> {
        let fetch_events = self.core.fetch_events();
        let mut state = self.state.lock().expect("riscv core lock");
        if state.pending_trap.is_some() {
            return Ok(None);
        }
        sync_completed_fetches_to_in_order_pipeline(&mut state, &fetch_events)?;

        if let Some(prefix) = state.pending_fetch_prefix.clone() {
            let architectural = Address::new(state.hart.pc());
            if prefix.fetch.pc() != architectural {
                state.pending_fetch_prefix = None;
                return Err(RiscvCpuError::PcMismatch {
                    fetch: prefix.fetch.pc(),
                    architectural,
                });
            }
            let suffix_pc = Address::new(prefix.fetch.pc().get() + 2);
            let Some(suffix) = fetch_events.iter().find(|event| {
                event.kind() == CpuFetchEventKind::Completed
                    && event.pc() == suffix_pc
                    && !state.executed_fetches.contains(&event.request_id())
            }) else {
                return Ok(None);
            };
            let suffix_data = suffix.data().ok_or(RiscvCpuError::MissingFetchData {
                request: suffix.request_id(),
            })?;
            let [suffix_low, suffix_high] = suffix_data else {
                return Err(RiscvCpuError::InvalidFetchWidth {
                    request: suffix.request_id(),
                    bytes: suffix_data.len() as u64,
                });
            };
            let raw =
                u32::from_le_bytes([prefix.bytes[0], prefix.bytes[1], *suffix_low, *suffix_high]);
            let fetch = CpuFetchEvent::completed(
                CpuFetchRecord::new(
                    prefix.fetch.tick(),
                    prefix.fetch.partition(),
                    prefix.fetch.route(),
                    prefix.fetch.endpoint().clone(),
                    prefix.fetch.request_id(),
                    prefix.fetch.pc(),
                    AccessSize::new(4).expect("RISC-V word fetch width is nonzero"),
                ),
                raw.to_le_bytes().to_vec(),
            );
            let consumed = [prefix.fetch.request_id(), suffix.request_id()];
            state.pending_fetch_prefix = None;
            return self
                .retire_completed_fetch(&mut state, fetch, raw, &consumed)
                .map(Some);
        }

        let Some(fetch) = fetch_events.into_iter().find(|event| {
            event.kind() == CpuFetchEventKind::Completed
                && !state.executed_fetches.contains(&event.request_id())
        }) else {
            return Ok(None);
        };

        let architectural = Address::new(state.hart.pc());
        if fetch.pc() != architectural {
            return Err(RiscvCpuError::PcMismatch {
                fetch: fetch.pc(),
                architectural,
            });
        }

        let data = fetch.data().ok_or(RiscvCpuError::MissingFetchData {
            request: fetch.request_id(),
        })?;
        let raw = match data {
            [low, high] if low & 0x3 != 0x3 => u32::from(u16::from_le_bytes([*low, *high])),
            [_, _] => {
                state.pending_fetch_prefix = Some(RiscvPendingFetchPrefix::new(
                    fetch.clone(),
                    [data[0], data[1]],
                ));
                state.executed_fetches.insert(fetch.request_id());
                return Ok(None);
            }
            [a, b, c, d] => u32::from_le_bytes([*a, *b, *c, *d]),
            _ => {
                return Err(RiscvCpuError::InvalidFetchWidth {
                    request: fetch.request_id(),
                    bytes: data.len() as u64,
                });
            }
        };
        self.retire_completed_fetch(&mut state, fetch.clone(), raw, &[fetch.request_id()])
            .map(Some)
    }

    fn retire_completed_fetch(
        &self,
        state: &mut RiscvCoreState,
        fetch: CpuFetchEvent,
        raw: u32,
        consumed_requests: &[MemoryRequestId],
    ) -> Result<RiscvCpuExecutionEvent, RiscvCpuError> {
        let decoded = RiscvInstruction::decode_with_length(raw).map_err(RiscvCpuError::Isa)?;
        let instruction = decoded.instruction();
        let execution = state
            .hart
            .execute_decoded(decoded)
            .map_err(RiscvCpuError::Isa)?;
        let next_pc = Address::new(execution.next_pc());
        let sequential_next_pc = fetch
            .pc()
            .get()
            .wrapping_add(u64::from(execution.instruction_bytes()));
        let redirects_fetch = execution.trap().is_some() || next_pc.get() != sequential_next_pc;
        let has_completed_successor_fetch = self.core.fetch_events().iter().any(|event| {
            event.kind() == CpuFetchEventKind::Completed
                && event.pc() == next_pc
                && !state.executed_fetches.contains(&event.request_id())
                && !consumed_requests.contains(&event.request_id())
        });
        if redirects_fetch || !has_completed_successor_fetch || self.core.pc().get() < next_pc.get()
        {
            self.core.set_pc(next_pc);
        }
        if let Some(trap) = execution.trap().copied() {
            state.pending_trap = Some(trap);
        }
        state.apply_riscv_system_event(execution.system_event());
        let retired_branch = retire_branch_predictions(
            state,
            fetch.request_id().sequence(),
            fetch.pc(),
            instruction,
            &execution,
        )?;
        let pipeline_branch_prediction = in_order_pipeline_branch_prediction(
            fetch.request_id().sequence(),
            fetch.pc(),
            next_pc,
            retired_branch.branch_update(),
        );
        let pipeline_cycle = if execution.memory_access().is_none() {
            Some(record_retired_in_order_pipeline_cycle(
                state,
                fetch.request_id().sequence(),
                pipeline_branch_prediction,
            )?)
        } else {
            None
        };

        let event = RiscvCpuExecutionEvent::with_all_branch_updates_pipeline_cycle_and_retired_instruction_counting(
            fetch.clone(),
            instruction,
            execution,
            retired_branch,
            pipeline_cycle,
            0,
            true,
        );
        let squashed_requests = event
            .in_order_pipeline_cycle()
            .map(|cycle| {
                squashed_completed_fetch_requests(
                    state,
                    &self.core.fetch_events(),
                    cycle,
                    consumed_requests,
                )
            })
            .unwrap_or_default();
        state
            .executed_fetches
            .extend(consumed_requests.iter().copied());
        state.executed_fetches.extend(squashed_requests);
        state.events.push(event.clone());
        Ok(event)
    }
}

pub(crate) fn record_retired_in_order_pipeline_cycle(
    state: &mut RiscvCoreState,
    sequence: u64,
    branch_prediction: Option<InOrderBranchPrediction>,
) -> Result<InOrderPipelineCycleRecord, RiscvCpuError> {
    record_retired_in_order_pipeline_cycle_after_wait(state, sequence, branch_prediction, 0)
}

pub(crate) fn record_retired_in_order_pipeline_cycle_after_wait(
    state: &mut RiscvCoreState,
    sequence: u64,
    branch_prediction: Option<InOrderBranchPrediction>,
    wait_cycles: u64,
) -> Result<InOrderPipelineCycleRecord, RiscvCpuError> {
    if !state.in_order_pipeline.contains_sequence(sequence) {
        state
            .in_order_pipeline
            .replace_in_flight([InOrderPipelineInstruction::new(
                sequence,
                InOrderPipelineStage::Fetch1,
            )])
            .map_err(RiscvCpuError::InOrderPipeline)?;
    }
    let max_retire_cycles =
        InOrderPipelineStage::ALL.len() + state.in_order_pipeline.in_flight().len();
    for _ in 0..max_retire_cycles {
        let resolves_branch = branch_prediction.is_some()
            && state
                .in_order_pipeline
                .snapshot()
                .in_flight()
                .iter()
                .any(|instruction| {
                    instruction.sequence() == sequence
                        && instruction.stage() == InOrderPipelineStage::Commit
                });
        let record = state
            .in_order_pipeline
            .try_advance_cycle_recorded_with_prediction(
                resolves_branch.then_some(branch_prediction).flatten(),
            )
            .map_err(RiscvCpuError::InOrderPipeline)?;
        if record.after().in_flight().iter().any(|instruction| {
            instruction.sequence() == sequence
                && instruction.stage() == InOrderPipelineStage::Execute
        }) {
            state
                .in_order_pipeline
                .try_stall_cycles(wait_cycles)
                .map_err(RiscvCpuError::InOrderPipeline)?;
        }
        if record_retires_sequence(&record, sequence) {
            return Ok(record);
        }
    }

    unreachable!("default in-order pipeline retires an instruction within its stage count")
}

fn record_retires_sequence(record: &InOrderPipelineCycleRecord, sequence: u64) -> bool {
    record
        .plan()
        .advanced()
        .iter()
        .any(|advance| advance.sequence() == sequence && advance.retires())
}

fn sync_completed_fetches_to_in_order_pipeline(
    state: &mut RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> Result<(), RiscvCpuError> {
    let mut completed = fetch_events
        .iter()
        .filter(|event| {
            event.kind() == CpuFetchEventKind::Completed
                && !state.executed_fetches.contains(&event.request_id())
                && event.data().is_some_and(|data| data.len() == 4)
        })
        .collect::<Vec<_>>();
    completed.sort_by_key(|event| event.request_id().sequence());

    for fetch in completed {
        state
            .in_order_pipeline
            .enqueue_fetch(fetch.request_id().sequence())
            .map_err(RiscvCpuError::InOrderPipeline)?;
    }
    Ok(())
}

fn squashed_completed_fetch_requests(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    cycle: &InOrderPipelineCycleRecord,
    consumed_requests: &[MemoryRequestId],
) -> Vec<MemoryRequestId> {
    if cycle.plan().flushed().is_empty() {
        return Vec::new();
    }

    fetch_events
        .iter()
        .filter(|event| {
            event.kind() == CpuFetchEventKind::Completed
                && cycle
                    .plan()
                    .flushed_sequences()
                    .any(|sequence| sequence == event.request_id().sequence())
                && !state.executed_fetches.contains(&event.request_id())
                && !consumed_requests.contains(&event.request_id())
        })
        .map(|event| event.request_id())
        .collect()
}

fn in_order_pipeline_branch_prediction(
    sequence: u64,
    fetch_pc: Address,
    actual_next_pc: Address,
    branch_update: Option<&crate::BranchUpdate>,
) -> Option<InOrderBranchPrediction> {
    let update = branch_update?;
    let resolved_target_pc =
        (update.actual_taken() || update.predicted_taken()).then_some(actual_next_pc.get());
    Some(InOrderBranchPrediction::new(
        sequence,
        InOrderPipelineStage::Commit,
        fetch_pc.get(),
        update.predicted_taken(),
        update.predicted_target().map(Address::get),
        update.actual_taken(),
        resolved_target_pc,
    ))
}

fn retire_branch_predictions(
    state: &mut RiscvCoreState,
    sequence: u64,
    pc: Address,
    instruction: RiscvInstruction,
    execution: &rem6_isa_riscv::RiscvExecutionRecord,
) -> Result<RiscvRetiredBranchUpdates, RiscvCpuError> {
    if execution.trap().is_some() {
        discard_branch_speculation(state, sequence)?;
        return Ok(RiscvRetiredBranchUpdates::default());
    }

    let sequential_pc = pc
        .get()
        .wrapping_add(u64::from(execution.instruction_bytes()));
    let next_pc = execution.next_pc();
    let (conditional, actual_taken, actual_target) = match instruction {
        RiscvInstruction::Beq { .. }
        | RiscvInstruction::Bne { .. }
        | RiscvInstruction::Blt { .. }
        | RiscvInstruction::Bge { .. }
        | RiscvInstruction::Bltu { .. }
        | RiscvInstruction::Bgeu { .. } => {
            let taken = next_pc != sequential_pc;
            (true, taken, taken.then_some(Address::new(next_pc)))
        }
        RiscvInstruction::Jal { .. } | RiscvInstruction::Jalr { .. } => {
            (false, true, Some(Address::new(next_pc)))
        }
        _ => {
            return Ok(RiscvRetiredBranchUpdates::default());
        }
    };

    let branch_update = state
        .branch_predictor
        .update(pc, actual_taken, actual_target);
    resolve_branch_speculation(state, sequence, &branch_update)?;
    let prediction = if conditional {
        state
            .gshare_branch_predictor
            .predict(RISCV_LOCAL_GSHARE_THREAD, pc)
    } else {
        state
            .gshare_branch_predictor
            .predict_unconditional(RISCV_LOCAL_GSHARE_THREAD, pc)
    }
    .map_err(RiscvCpuError::GShareBranchPredictor)?;
    let history_update = state
        .gshare_branch_predictor
        .update_history(prediction.history(), actual_taken)
        .map_err(RiscvCpuError::GShareBranchPredictor)?;
    let training_update = state
        .gshare_branch_predictor
        .train(prediction.history(), actual_taken, false)
        .map_err(RiscvCpuError::GShareBranchPredictor)?;
    let tournament_prediction = if conditional {
        state
            .tournament_branch_predictor
            .predict(RISCV_LOCAL_TOURNAMENT_THREAD, pc)
    } else {
        state
            .tournament_branch_predictor
            .predict_unconditional(RISCV_LOCAL_TOURNAMENT_THREAD, pc)
    }
    .map_err(RiscvCpuError::TournamentBranchPredictor)?;
    let tournament_history_update = state
        .tournament_branch_predictor
        .update_history(tournament_prediction.history(), actual_taken)
        .map_err(RiscvCpuError::TournamentBranchPredictor)?;
    let tournament_training_update = state
        .tournament_branch_predictor
        .train(tournament_prediction.history(), actual_taken, false)
        .map_err(RiscvCpuError::TournamentBranchPredictor)?;

    Ok(RiscvRetiredBranchUpdates::new(
        branch_update,
        RiscvGShareBranchUpdate::new(prediction, history_update, training_update),
        RiscvTournamentBranchUpdate::new(
            tournament_prediction,
            tournament_history_update,
            tournament_training_update,
        ),
    ))
}

fn resolve_branch_speculation(
    state: &mut RiscvCoreState,
    sequence: u64,
    update: &crate::BranchUpdate,
) -> Result<(), RiscvCpuError> {
    let Some(speculation) = state.branch_speculations.remove(&sequence) else {
        return Ok(());
    };

    let predicted_correctly = update.predicted_taken() == update.actual_taken()
        && (!update.predicted_taken() || update.predicted_target() == update.actual_target());
    if !predicted_correctly {
        let repair = state
            .branch_predictor
            .repair_speculation(speculation, update.actual_taken())
            .map_err(RiscvCpuError::BranchPredictor)?;
        remove_branch_speculation_mappings(state, repair.removed_youngers());
    }
    state
        .branch_predictor
        .commit_speculation(speculation)
        .map_err(RiscvCpuError::BranchPredictor)?;
    Ok(())
}

fn discard_branch_speculation(
    state: &mut RiscvCoreState,
    sequence: u64,
) -> Result<(), RiscvCpuError> {
    let Some(speculation) = state.branch_speculations.remove(&sequence) else {
        return Ok(());
    };

    let discard = state
        .branch_predictor
        .discard_speculation(speculation)
        .map_err(RiscvCpuError::BranchPredictor)?;
    remove_branch_speculation_mappings(state, discard.removed_youngers());
    Ok(())
}

fn remove_branch_speculation_mappings(
    state: &mut RiscvCoreState,
    removed: &[crate::BranchSpeculation],
) {
    state.branch_speculations.retain(|_, pending| {
        !removed
            .iter()
            .any(|removed_speculation| removed_speculation.id() == *pending)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retire_cycle_waits_for_requested_sequence_when_older_work_is_stale() {
        let mut state = RiscvCoreState::new(0x8000, 0);
        state
            .in_order_pipeline
            .replace_in_flight([
                InOrderPipelineInstruction::new(0, InOrderPipelineStage::Commit),
                InOrderPipelineInstruction::new(1, InOrderPipelineStage::Fetch1),
            ])
            .unwrap();

        let record =
            record_retired_in_order_pipeline_cycle_after_wait(&mut state, 1, None, 0).unwrap();

        assert!(record
            .plan()
            .advanced()
            .iter()
            .any(|advance| advance.sequence() == 1 && advance.retires()));
        assert!(!record
            .plan()
            .advanced()
            .iter()
            .any(|advance| advance.sequence() == 0 && advance.retires()));
    }
}

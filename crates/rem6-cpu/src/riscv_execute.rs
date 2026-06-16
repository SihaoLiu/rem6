use std::collections::BTreeSet;

use rem6_isa_riscv::RiscvInstruction;
use rem6_memory::{AccessSize, Address, MemoryRequestId};

use crate::{
    riscv_execution_event::RiscvRetiredBranchUpdates, CpuFetchEvent, CpuFetchEventKind,
    CpuFetchRecord, InOrderBranchPrediction, InOrderBranchRedirect, InOrderPipelineCycleRecord,
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
        if self
            .state
            .lock()
            .expect("riscv core lock")
            .pending_trap
            .is_some()
        {
            return Ok(None);
        }
        self.sync_in_order_fetch_state()?;
        let fetch_events = self.core.fetch_events();
        let mut state = self.state.lock().expect("riscv core lock");

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

        let architectural = Address::new(state.hart.pc());
        let Some(fetch) =
            next_completed_fetch_for_architectural_pc(&mut state, &fetch_events, architectural)
        else {
            return Ok(None);
        };

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
        let pipeline_redirect = execution.trap().is_some().then(|| {
            InOrderBranchRedirect::new(
                fetch.request_id().sequence(),
                InOrderPipelineStage::Commit,
                next_pc.get(),
            )
        });
        let pipeline_cycle = if execution.memory_access().is_none() {
            Some(record_retired_in_order_pipeline_cycle_with_redirect(
                state,
                fetch.request_id().sequence(),
                pipeline_branch_prediction,
                pipeline_redirect,
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
        let fetch_events = self.core.fetch_events();
        let squashed_requests = event
            .in_order_pipeline_cycle()
            .map(|cycle| squashed_fetch_requests(state, &fetch_events, cycle, consumed_requests))
            .unwrap_or_default();
        let redirected_target = redirects_fetch.then_some(next_pc);
        let stale_requests = stale_fetch_requests_after_retire(
            state,
            &fetch_events,
            fetch.pc(),
            consumed_requests,
            redirected_target,
        );
        let discarded_requests = squashed_requests
            .into_iter()
            .chain(stale_requests)
            .collect::<BTreeSet<_>>();
        self.core
            .discard_outstanding_fetches(discarded_requests.iter().copied());
        state
            .executed_fetches
            .extend(consumed_requests.iter().copied());
        state.executed_fetches.extend(discarded_requests);
        state.events.push(event.clone());
        Ok(event)
    }
}

fn next_completed_fetch_for_architectural_pc(
    state: &mut RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    architectural: Address,
) -> Option<CpuFetchEvent> {
    let mut completed = fetch_events
        .iter()
        .filter(|event| {
            event.kind() == CpuFetchEventKind::Completed
                && !state.executed_fetches.contains(&event.request_id())
        })
        .collect::<Vec<_>>();
    completed.sort_by_key(|event| event.request_id().sequence());

    for event in completed {
        if event.pc() == architectural {
            return Some(event.clone());
        }
    }
    None
}

fn stale_fetch_requests_after_retire(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    retired_pc: Address,
    consumed_requests: &[MemoryRequestId],
    redirected_target: Option<Address>,
) -> Vec<MemoryRequestId> {
    let consumed = consumed_requests.iter().copied().collect::<BTreeSet<_>>();
    let Some(max_consumed_sequence) = consumed.iter().map(|request| request.sequence()).max()
    else {
        return Vec::new();
    };

    fetch_events
        .iter()
        .filter(|event| {
            matches!(
                event.kind(),
                CpuFetchEventKind::Issued | CpuFetchEventKind::Completed
            )
        })
        .filter_map(|event| {
            let request = event.request_id();
            if state.executed_fetches.contains(&request) || consumed.contains(&request) {
                return None;
            }
            if redirected_target.is_some_and(|target| event.pc() == target) {
                return None;
            }
            if request.sequence() <= max_consumed_sequence || event.pc() == retired_pc {
                return Some(request);
            }
            redirected_target
                .is_some_and(|target| event.pc() != target)
                .then_some(request)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn record_retired_in_order_pipeline_cycle_after_wait(
    state: &mut RiscvCoreState,
    sequence: u64,
    branch_prediction: Option<InOrderBranchPrediction>,
    wait_cycles: u64,
) -> Result<InOrderPipelineCycleRecord, RiscvCpuError> {
    record_retired_in_order_pipeline_cycle_with_redirect_after_wait(
        state,
        sequence,
        branch_prediction,
        None,
        wait_cycles,
    )
}

fn record_retired_in_order_pipeline_cycle_with_redirect(
    state: &mut RiscvCoreState,
    sequence: u64,
    branch_prediction: Option<InOrderBranchPrediction>,
    redirect: Option<InOrderBranchRedirect>,
) -> Result<InOrderPipelineCycleRecord, RiscvCpuError> {
    record_retired_in_order_pipeline_cycle_with_redirect_after_wait(
        state,
        sequence,
        branch_prediction,
        redirect,
        0,
    )
}

fn record_retired_in_order_pipeline_cycle_with_redirect_after_wait(
    state: &mut RiscvCoreState,
    sequence: u64,
    branch_prediction: Option<InOrderBranchPrediction>,
    redirect: Option<InOrderBranchRedirect>,
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
        let snapshot = state.in_order_pipeline.snapshot();
        let active_prediction = branch_prediction.filter(|prediction| {
            snapshot.in_flight().iter().any(|instruction| {
                instruction.sequence() == prediction.sequence()
                    && instruction.stage() == prediction.resolved_stage()
            })
        });
        let active_redirect = redirect.filter(|redirect| {
            snapshot.in_flight().iter().any(|instruction| {
                instruction.sequence() == redirect.sequence()
                    && instruction.stage() == redirect.resolved_stage()
            })
        });
        let record = if active_prediction.is_some() {
            state
                .in_order_pipeline
                .try_advance_cycle_recorded_with_prediction(active_prediction)
        } else {
            state
                .in_order_pipeline
                .try_advance_cycle_recorded_with_redirect(active_redirect)
        }
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

fn squashed_fetch_requests(
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
            matches!(
                event.kind(),
                CpuFetchEventKind::Issued | CpuFetchEventKind::Completed
            ) && cycle
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
    use crate::CpuFetchRecord;
    use rem6_kernel::PartitionId;
    use rem6_memory::{AccessSize, AgentId, MemoryRequestId};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    fn request(sequence: u64) -> MemoryRequestId {
        MemoryRequestId::new(AgentId::new(7), sequence)
    }

    fn completed(sequence: u64, pc: u64) -> CpuFetchEvent {
        CpuFetchEvent::completed(
            CpuFetchRecord::new(
                0,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                request(sequence),
                Address::new(pc),
                AccessSize::new(4).unwrap(),
            ),
            vec![0; 4],
        )
    }

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

    #[test]
    fn stale_fetches_after_retire_discard_duplicate_and_redirect_wrong_path_requests() {
        let state = RiscvCoreState::new(0x8000, 0);
        let events = vec![
            completed(0, 0x8008),
            completed(1, 0x8008),
            completed(2, 0x800e),
            completed(3, 0x8000),
        ];

        let stale = stale_fetch_requests_after_retire(
            &state,
            &events,
            Address::new(0x8008),
            &[request(0)],
            Some(Address::new(0x8000)),
        );

        assert_eq!(stale, vec![request(1), request(2)]);
    }

    #[test]
    fn stale_fetches_after_retire_keep_same_pc_redirect_target_request() {
        let state = RiscvCoreState::new(0x8000, 0);
        let events = vec![completed(0, 0x8000), completed(1, 0x8000)];

        let stale = stale_fetch_requests_after_retire(
            &state,
            &events,
            Address::new(0x8000),
            &[request(0)],
            Some(Address::new(0x8000)),
        );

        assert!(stale.is_empty());
    }

    #[test]
    fn stale_fetches_after_retire_discard_backedge_wrong_path_request() {
        let state = RiscvCoreState::new(0x8000, 0);
        let events = vec![
            completed(0, 0x8010),
            completed(1, 0x8014),
            completed(2, 0x8008),
        ];

        let stale = stale_fetch_requests_after_retire(
            &state,
            &events,
            Address::new(0x8010),
            &[request(0)],
            Some(Address::new(0x8008)),
        );

        assert_eq!(stale, vec![request(1)]);
    }
}

use std::collections::BTreeSet;

use rem6_isa_riscv::{RiscvInstruction, RiscvTrapKind};
use rem6_kernel::PartitionedScheduler;
use rem6_memory::{AccessSize, Address, MemoryRequestId};

use crate::{
    riscv_branch_kind::riscv_branch_target_kind, riscv_execution_event::RiscvRetiredBranchUpdates,
    riscv_fu_latency::riscv_execute_wait_cycles,
    riscv_live_retire_window::RiscvLiveRetireWindowRequest, BranchTargetKind, CpuFetchEvent,
    CpuFetchEventKind, CpuFetchRecord, InOrderBranchPrediction, InOrderBranchRedirect,
    InOrderPipelineCycleRecord, InOrderPipelineInstruction, InOrderPipelineStage,
    InOrderPipelineStallCause, RiscvBiModeBranchUpdate, RiscvCore, RiscvCoreState, RiscvCpuError,
    RiscvCpuExecutionEvent, RiscvGShareBranchUpdate, RiscvMultiperspectivePerceptronBranchUpdate,
    RiscvSelectedBranchSpeculation, RiscvTageScLBranchUpdate, RiscvTournamentBranchUpdate,
    StatisticalCorrectorBranchKind, RISCV_LOCAL_BIMODE_THREAD, RISCV_LOCAL_GSHARE_THREAD,
    RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD, RISCV_LOCAL_TAGE_SC_L_THREAD,
    RISCV_LOCAL_TOURNAMENT_THREAD,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiscvPendingFetchPrefix {
    pub(crate) fetch: CpuFetchEvent,
    pub(crate) bytes: [u8; 2],
}

impl RiscvPendingFetchPrefix {
    pub(crate) const fn new(fetch: CpuFetchEvent, bytes: [u8; 2]) -> Self {
        Self { fetch, bytes }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RiscvLiveRetireGateWakeKind {
    Serial,
    Parallel,
}

impl RiscvCore {
    /// Executes without creating cycle-visible waits. A gate already started by a
    /// scheduler-aware drive remains blocking until that scheduler reaches its wake.
    pub fn execute_next_completed_fetch(
        &self,
    ) -> Result<Option<RiscvCpuExecutionEvent>, RiscvCpuError> {
        self.execute_next_completed_fetch_inner(None)
    }

    pub(crate) fn execute_next_completed_fetch_serial(
        &self,
        scheduler: &mut PartitionedScheduler,
    ) -> Result<Option<RiscvCpuExecutionEvent>, RiscvCpuError> {
        self.execute_next_completed_fetch_inner(Some((
            scheduler,
            RiscvLiveRetireGateWakeKind::Serial,
        )))
    }

    pub(crate) fn execute_next_completed_fetch_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
    ) -> Result<Option<RiscvCpuExecutionEvent>, RiscvCpuError> {
        self.execute_next_completed_fetch_inner(Some((
            scheduler,
            RiscvLiveRetireGateWakeKind::Parallel,
        )))
    }

    fn execute_next_completed_fetch_inner(
        &self,
        mut gate_scheduler: Option<(&mut PartitionedScheduler, RiscvLiveRetireGateWakeKind)>,
    ) -> Result<Option<RiscvCpuExecutionEvent>, RiscvCpuError> {
        let state = self.state.lock().expect("riscv core lock");
        if state.pending_trap.is_some() || state.o3_runtime.has_ready_live_scalar_memory_event() {
            return Ok(None);
        }
        drop(state);
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
            let Some(suffix) = next_completed_fetch_suffix(&state, &fetch_events, &prefix) else {
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
            let Some(retire_tick) = self.live_retire_gate_retire_tick(
                &mut state,
                &mut gate_scheduler,
                RiscvLiveRetireWindowRequest::new(
                    fetch.request_id(),
                    fetch.pc(),
                    raw,
                    fetch.tick(),
                    &fetch_events,
                ),
            )?
            else {
                return Ok(None);
            };
            state.pending_fetch_prefix = None;
            return self
                .retire_completed_fetch(&mut state, fetch, raw, &consumed, retire_tick)
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
        let Some(retire_tick) = self.live_retire_gate_retire_tick(
            &mut state,
            &mut gate_scheduler,
            RiscvLiveRetireWindowRequest::new(
                fetch.request_id(),
                fetch.pc(),
                raw,
                fetch.tick(),
                &fetch_events,
            ),
        )?
        else {
            return Ok(None);
        };
        self.retire_completed_fetch(
            &mut state,
            fetch.clone(),
            raw,
            &[fetch.request_id()],
            retire_tick,
        )
        .map(Some)
    }

    fn retire_completed_fetch(
        &self,
        state: &mut RiscvCoreState,
        fetch: CpuFetchEvent,
        raw: u32,
        consumed_requests: &[MemoryRequestId],
        retire_tick: u64,
    ) -> Result<RiscvCpuExecutionEvent, RiscvCpuError> {
        let decoded = RiscvInstruction::decode_with_length(raw).map_err(RiscvCpuError::Isa)?;
        let instruction = decoded.instruction();
        let execution = state
            .hart
            .execute_decoded(decoded)
            .map_err(RiscvCpuError::Isa)?;
        let trap_kind = execution.trap().map(|trap| trap.kind());
        let retires_instruction = !matches!(trap_kind, Some(RiscvTrapKind::Interrupt { .. }));
        let primary_hart = state.hart.clone();
        if let Some(checker) = &mut state.checker {
            checker
                .check_execution(
                    retires_instruction,
                    fetch.request_id().sequence(),
                    fetch.pc(),
                    decoded,
                    &execution,
                    &primary_hart,
                )
                .map_err(RiscvCpuError::Isa)?;
        }
        let next_pc = Address::new(execution.next_pc());
        let sequential_next_pc = fetch
            .pc()
            .get()
            .wrapping_add(u64::from(execution.instruction_bytes()));
        let live_control_sequence = instruction_is_conditional_branch(instruction)
            .then(|| {
                state
                    .o3_runtime
                    .snapshot()
                    .reorder_buffer()
                    .iter()
                    .find(|entry| entry.is_live_staged() && entry.pc() == fetch.pc())
                    .map(|entry| entry.sequence())
            })
            .flatten();
        let retired_branch = retire_branch_predictions(
            state,
            fetch.request_id().sequence(),
            fetch.pc(),
            instruction,
            &execution,
        )?;
        let branch_prediction_redirects = retired_branch
            .fetch_prediction()
            .is_some_and(branch_prediction_redirects_fetch);
        let redirects_fetch = execution.trap().is_some()
            || next_pc.get() != sequential_next_pc
            || branch_prediction_redirects;
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
        state.quiesce_for_immediate_terminal_event(execution.system_event());
        let fetch_events = self.core.fetch_events();
        let direct_jump_fetch_ahead_target = direct_jump_fetch_ahead_prediction_target(
            state,
            &fetch_events,
            fetch.request_id(),
            instruction,
            next_pc,
        );
        let pipeline_branch_prediction = in_order_pipeline_branch_prediction(
            fetch.request_id().sequence(),
            fetch.pc(),
            next_pc,
            instruction_is_conditional_branch(instruction),
            retired_branch.fetch_prediction(),
            direct_jump_fetch_ahead_target,
        );
        let pipeline_redirect = execution.trap().map(|trap| {
            let sequence = fetch.request_id().sequence();
            match trap.kind() {
                RiscvTrapKind::Interrupt { .. } => InOrderBranchRedirect::interrupt(
                    sequence,
                    InOrderPipelineStage::Commit,
                    next_pc.get(),
                ),
                _ => InOrderBranchRedirect::trap(
                    sequence,
                    InOrderPipelineStage::Commit,
                    next_pc.get(),
                ),
            }
        });
        let execute_wait_cycles = riscv_execute_wait_cycles(instruction);
        let pipeline_cycle = if execution.memory_access().is_none() {
            Some(
                record_retired_in_order_pipeline_cycle_with_redirect_after_wait(
                    state,
                    fetch.request_id().sequence(),
                    pipeline_branch_prediction,
                    pipeline_redirect,
                    execute_wait_cycles,
                    InOrderPipelineStallCause::ExecuteWait,
                )?,
            )
        } else {
            None
        };
        let event = RiscvCpuExecutionEvent::with_all_branch_updates_pipeline_cycle_and_retired_instruction_counting(
            fetch.clone(),
            instruction,
            execution,
            retired_branch.into_updates(),
            pipeline_cycle,
            0,
            retires_instruction,
        );
        if retires_instruction {
            state
                .o3_runtime
                .retire_live_staged_instruction(&event, consumed_requests, retire_tick);
            let detailed = state.live_retire_gate.detailed_policy_enabled();
            assert!(state
                .o3_runtime
                .defer_scalar_memory_if_detailed(detailed, &event));
        }
        if redirects_fetch {
            if instruction_is_conditional_branch(instruction)
                && event.execution().trap().is_none()
            {
                if branch_prediction_redirects {
                    if let Some(sequence) = live_control_sequence {
                        state
                            .o3_runtime
                            .discard_live_control_descendants_from(sequence);
                    } else {
                        state.o3_runtime.discard_live_staged_instructions();
                    }
                }
            } else {
                state.o3_runtime.discard_live_staged_instructions();
            }
        }
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
        let discarded_sequences = discarded_requests
            .iter()
            .map(|request| request.sequence())
            .collect::<BTreeSet<_>>();
        state.executed_fetches.extend(discarded_requests);
        remove_fetch_sequences_from_pipeline(state, &discarded_sequences)?;
        state.events.push(event.clone());
        Ok(event)
    }
}

fn next_completed_fetch_suffix<'a>(
    state: &RiscvCoreState,
    fetch_events: &'a [CpuFetchEvent],
    prefix: &RiscvPendingFetchPrefix,
) -> Option<&'a CpuFetchEvent> {
    let suffix_pc = Address::new(prefix.fetch.pc().get() + 2);
    oldest_completed_fetch_at(
        &state.executed_fetches,
        fetch_events,
        prefix.fetch.request_id(),
        suffix_pc,
    )
}

pub(super) fn oldest_completed_fetch_at<'a>(
    executed_fetches: &BTreeSet<MemoryRequestId>,
    fetch_events: &'a [CpuFetchEvent],
    current_request: MemoryRequestId,
    pc: Address,
) -> Option<&'a CpuFetchEvent> {
    fetch_events
        .iter()
        .filter(|event| {
            event.kind() == CpuFetchEventKind::Completed
                && event.pc() == pc
                && event.request_id().agent() == current_request.agent()
                && event.request_id().sequence() > current_request.sequence()
                && !executed_fetches.contains(&event.request_id())
        })
        .min_by_key(|event| event.request_id().sequence())
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

#[cfg(test)]
pub(crate) fn record_retired_in_order_pipeline_cycle_after_wait_with_cause(
    state: &mut RiscvCoreState,
    sequence: u64,
    branch_prediction: Option<InOrderBranchPrediction>,
    wait_cycles: u64,
    stall_cause: InOrderPipelineStallCause,
) -> Result<InOrderPipelineCycleRecord, RiscvCpuError> {
    record_retired_in_order_pipeline_cycle_after_waits_with_causes(
        state,
        sequence,
        branch_prediction,
        &[(wait_cycles, stall_cause)],
    )
}

pub(crate) fn record_retired_in_order_pipeline_cycle_after_waits_with_causes(
    state: &mut RiscvCoreState,
    sequence: u64,
    branch_prediction: Option<InOrderBranchPrediction>,
    waits: &[(u64, InOrderPipelineStallCause)],
) -> Result<InOrderPipelineCycleRecord, RiscvCpuError> {
    record_retired_in_order_pipeline_cycle_with_redirect_after_waits(
        state,
        sequence,
        branch_prediction,
        None,
        waits,
    )
}

fn record_retired_in_order_pipeline_cycle_with_redirect_after_wait(
    state: &mut RiscvCoreState,
    sequence: u64,
    branch_prediction: Option<InOrderBranchPrediction>,
    redirect: Option<InOrderBranchRedirect>,
    wait_cycles: u64,
    stall_cause: InOrderPipelineStallCause,
) -> Result<InOrderPipelineCycleRecord, RiscvCpuError> {
    record_retired_in_order_pipeline_cycle_with_redirect_after_waits(
        state,
        sequence,
        branch_prediction,
        redirect,
        &[(wait_cycles, stall_cause)],
    )
}

fn record_retired_in_order_pipeline_cycle_with_redirect_after_waits(
    state: &mut RiscvCoreState,
    sequence: u64,
    branch_prediction: Option<InOrderBranchPrediction>,
    redirect: Option<InOrderBranchRedirect>,
    waits: &[(u64, InOrderPipelineStallCause)],
) -> Result<InOrderPipelineCycleRecord, RiscvCpuError> {
    if !state.in_order_pipeline.contains_sequence(sequence) {
        state
            .in_order_pipeline
            .replace_in_flight([InOrderPipelineInstruction::new(
                sequence,
                InOrderPipelineStage::Fetch1,
            )])
            .map_err(RiscvCpuError::InOrderPipeline)?;
    } else {
        discard_stale_in_order_pipeline_before_retire(state, sequence)?;
    }
    let execute_wait_completed = state.in_order_pipeline.execute_wait_completed(sequence);
    let waits = waits
        .iter()
        .copied()
        .filter(|(cycles, cause)| {
            *cycles > 0
                && !(execute_wait_completed && *cause == InOrderPipelineStallCause::ExecuteWait)
        })
        .collect::<Vec<_>>();
    let mut wait_recorded = waits.is_empty();
    let max_retire_cycles =
        InOrderPipelineStage::ALL.len() + state.in_order_pipeline.in_flight().len();
    for _ in 0..max_retire_cycles {
        if !wait_recorded && in_order_pipeline_sequence_can_record_wait(state, sequence) {
            record_in_order_resource_waits(state, &waits)?;
            wait_recorded = true;
        }
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
        let record = state
            .in_order_pipeline
            .try_advance_cycle_recorded_retiring_sequence(
                sequence,
                active_prediction,
                active_redirect,
            )
            .map_err(RiscvCpuError::InOrderPipeline)?;
        let completes_sequence = record.completes_sequence(sequence);
        state.in_order_pipeline_cycle_records.push(record.clone());
        if !wait_recorded
            && record.after().in_flight().iter().any(|instruction| {
                instruction.sequence() == sequence
                    && instruction.stage() == InOrderPipelineStage::Execute
            })
        {
            record_in_order_resource_waits(state, &waits)?;
            wait_recorded = true;
        }
        wait_recorded |= completes_sequence;
        if completes_sequence {
            return Ok(record);
        }
    }

    unreachable!(
        "default in-order pipeline completes a sequence within its stage count: sequence {sequence}, in_flight {:?}",
        state.in_order_pipeline.in_flight()
    )
}

fn discard_stale_in_order_pipeline_before_retire(
    state: &mut RiscvCoreState,
    sequence: u64,
) -> Result<(), RiscvCpuError> {
    let stale_sequences = state
        .in_order_pipeline
        .in_flight()
        .iter()
        .filter(|instruction| instruction.sequence() < sequence)
        .map(|instruction| instruction.sequence())
        .collect::<BTreeSet<_>>();
    remove_fetch_sequences_from_pipeline(state, &stale_sequences)
}

fn in_order_pipeline_sequence_can_record_wait(state: &RiscvCoreState, sequence: u64) -> bool {
    matches!(
        in_order_pipeline_sequence_stage(state, sequence),
        Some(InOrderPipelineStage::Execute | InOrderPipelineStage::Commit)
    )
}

fn in_order_pipeline_sequence_stage(
    state: &RiscvCoreState,
    sequence: u64,
) -> Option<InOrderPipelineStage> {
    state
        .in_order_pipeline
        .in_flight()
        .iter()
        .find_map(|instruction| (instruction.sequence() == sequence).then_some(instruction.stage()))
}

fn record_in_order_resource_wait_cycles(
    state: &mut RiscvCoreState,
    wait_cycles: u64,
    stall_cause: InOrderPipelineStallCause,
) -> Result<(), RiscvCpuError> {
    for _ in 0..wait_cycles {
        let stall_record = state
            .in_order_pipeline
            .try_record_resource_stall_cycle_with_cause(stall_cause)
            .map_err(RiscvCpuError::InOrderPipeline)?;
        state.in_order_pipeline_cycle_records.push(stall_record);
    }
    Ok(())
}

fn record_in_order_resource_waits(
    state: &mut RiscvCoreState,
    waits: &[(u64, InOrderPipelineStallCause)],
) -> Result<(), RiscvCpuError> {
    for (wait_cycles, stall_cause) in waits {
        record_in_order_resource_wait_cycles(state, *wait_cycles, *stall_cause)?;
    }
    Ok(())
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

fn direct_jump_fetch_ahead_prediction_target(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    retired_request: MemoryRequestId,
    instruction: RiscvInstruction,
    actual_next_pc: Address,
) -> Option<Address> {
    if !matches!(
        instruction,
        RiscvInstruction::Jal { .. } | RiscvInstruction::Jalr { .. }
    ) {
        return None;
    }

    fetch_events
        .iter()
        .any(|event| {
            matches!(
                event.kind(),
                CpuFetchEventKind::Issued | CpuFetchEventKind::Completed
            ) && event.request_id().sequence() > retired_request.sequence()
                && event.pc() == actual_next_pc
                && !state.executed_fetches.contains(&event.request_id())
        })
        .then_some(actual_next_pc)
}

fn in_order_pipeline_branch_prediction(
    sequence: u64,
    fetch_pc: Address,
    actual_next_pc: Address,
    conditional_branch: bool,
    branch_prediction: Option<RiscvResolvedBranchPrediction>,
    direct_jump_fetch_ahead_target: Option<Address>,
) -> Option<InOrderBranchPrediction> {
    if let Some(predicted_target) = direct_jump_fetch_ahead_target {
        return Some(InOrderBranchPrediction::new(
            sequence,
            InOrderPipelineStage::Commit,
            fetch_pc.get(),
            conditional_branch,
            true,
            Some(predicted_target.get()),
            true,
            Some(actual_next_pc.get()),
        ));
    }

    let prediction = branch_prediction?;
    let resolved_target_pc =
        (prediction.actual_taken() || prediction.predicted_taken()).then_some(actual_next_pc.get());
    Some(InOrderBranchPrediction::new(
        sequence,
        InOrderPipelineStage::Commit,
        fetch_pc.get(),
        conditional_branch,
        prediction.predicted_taken(),
        prediction.predicted_target().map(Address::get),
        prediction.actual_taken(),
        resolved_target_pc,
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvResolvedBranchPrediction {
    predicted_taken: bool,
    predicted_target: Option<Address>,
    actual_taken: bool,
    actual_target: Option<Address>,
}

impl RiscvResolvedBranchPrediction {
    fn from_branch_update(update: &crate::BranchUpdate) -> Self {
        Self {
            predicted_taken: update.predicted_taken(),
            predicted_target: update.predicted_target(),
            actual_taken: update.actual_taken(),
            actual_target: update.actual_target(),
        }
    }

    const fn predicted_taken(self) -> bool {
        self.predicted_taken
    }

    const fn predicted_target(self) -> Option<Address> {
        self.predicted_target
    }

    const fn actual_taken(self) -> bool {
        self.actual_taken
    }
}

fn branch_prediction_redirects_fetch(prediction: RiscvResolvedBranchPrediction) -> bool {
    prediction.predicted_taken != prediction.actual_taken
        || prediction.predicted_target != prediction.actual_target
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RiscvRetiredBranchResolution {
    updates: RiscvRetiredBranchUpdates,
    fetch_prediction: Option<RiscvResolvedBranchPrediction>,
}

impl RiscvRetiredBranchResolution {
    fn new(
        updates: RiscvRetiredBranchUpdates,
        fetch_prediction: Option<RiscvResolvedBranchPrediction>,
    ) -> Self {
        Self {
            updates,
            fetch_prediction,
        }
    }

    fn fetch_prediction(&self) -> Option<RiscvResolvedBranchPrediction> {
        self.fetch_prediction.or_else(|| {
            self.updates
                .branch_update()
                .map(RiscvResolvedBranchPrediction::from_branch_update)
        })
    }

    fn into_updates(self) -> RiscvRetiredBranchUpdates {
        self.updates
    }
}

fn retire_branch_predictions(
    state: &mut RiscvCoreState,
    sequence: u64,
    pc: Address,
    instruction: RiscvInstruction,
    execution: &rem6_isa_riscv::RiscvExecutionRecord,
) -> Result<RiscvRetiredBranchResolution, RiscvCpuError> {
    if execution.trap().is_some() {
        discard_branch_speculation(state, sequence)?;
        return Ok(RiscvRetiredBranchResolution::default());
    }

    let sequential_pc = pc
        .get()
        .wrapping_add(u64::from(execution.instruction_bytes()));
    let next_pc = execution.next_pc();
    let conditional = instruction_is_conditional_branch(instruction);
    let (actual_taken, actual_target) = if conditional {
        let taken = next_pc != sequential_pc;
        (taken, taken.then_some(Address::new(next_pc)))
    } else if matches!(
        instruction,
        RiscvInstruction::Jal { .. } | RiscvInstruction::Jalr { .. }
    ) {
        (true, Some(Address::new(next_pc)))
    } else {
        return Ok(RiscvRetiredBranchResolution::default());
    };

    let branch_kind = riscv_branch_target_kind(instruction);
    let branch_update = state
        .branch_predictor
        .update(pc, actual_taken, actual_target);
    if let Some(target) = actual_target {
        state.branch_target_buffer.update(pc, target, branch_kind);
    }
    let selected_branch_speculation = state.selected_branch_speculations.remove(&sequence);
    let selected_prediction =
        resolve_branch_speculation(state, sequence, branch_kind, &branch_update)?;
    let selected_prediction_correct = selected_prediction
        .map(|prediction| !branch_prediction_redirects_fetch(prediction))
        .unwrap_or(false);
    let tage_sc_l_target = if conditional {
        static_conditional_branch_target(pc, instruction).unwrap_or(Address::new(next_pc))
    } else {
        Address::new(next_pc)
    };
    let gshare_branch_update = retire_gshare_branch_update(
        state,
        pc,
        conditional,
        actual_taken,
        selected_prediction_correct,
        selected_branch_speculation.as_ref(),
    )?;
    let bimode_branch_update = retire_bimode_branch_update(
        state,
        pc,
        conditional,
        actual_taken,
        selected_prediction_correct,
        selected_branch_speculation.as_ref(),
    )?;
    let tournament_branch_update = retire_tournament_branch_update(
        state,
        pc,
        conditional,
        actual_taken,
        selected_prediction_correct,
        selected_branch_speculation.as_ref(),
    )?;
    let tage_sc_l_branch_update = retire_tage_sc_l_branch_update(
        state,
        pc,
        conditional,
        actual_taken,
        statistical_corrector_branch_kind(instruction),
        tage_sc_l_target,
        selected_branch_speculation.as_ref(),
    )?;
    let multiperspective_perceptron_target = if conditional {
        static_conditional_branch_target(pc, instruction).unwrap_or(Address::new(next_pc))
    } else {
        Address::new(next_pc)
    };
    let multiperspective_perceptron_branch_update =
        retire_multiperspective_perceptron_branch_update(
            state,
            pc,
            conditional,
            actual_taken,
            multiperspective_perceptron_target,
            selected_branch_speculation.as_ref(),
        )?;

    Ok(RiscvRetiredBranchResolution::new(
        RiscvRetiredBranchUpdates::new(
            branch_update,
            gshare_branch_update,
            bimode_branch_update,
            tournament_branch_update,
            tage_sc_l_branch_update,
            multiperspective_perceptron_branch_update,
        ),
        selected_prediction,
    ))
}

fn retire_gshare_branch_update(
    state: &mut RiscvCoreState,
    pc: Address,
    conditional: bool,
    actual_taken: bool,
    selected_prediction_correct: bool,
    selected: Option<&RiscvSelectedBranchSpeculation>,
) -> Result<RiscvGShareBranchUpdate, RiscvCpuError> {
    if let Some(RiscvSelectedBranchSpeculation::GShare {
        prediction,
        history_update,
    }) = selected
    {
        let history_update = if selected_prediction_correct {
            history_update
                .clone()
                .expect("recorded gshare speculation includes a history update")
        } else {
            state
                .gshare_branch_predictor
                .squash(prediction.history())
                .map_err(RiscvCpuError::GShareBranchPredictor)?;
            state
                .gshare_branch_predictor
                .update_history(prediction.history(), actual_taken)
                .map_err(RiscvCpuError::GShareBranchPredictor)?
        };
        let training_update = state
            .gshare_branch_predictor
            .train(prediction.history(), actual_taken, false)
            .map_err(RiscvCpuError::GShareBranchPredictor)?;
        return Ok(RiscvGShareBranchUpdate::new(
            prediction.clone(),
            history_update,
            training_update,
        ));
    }

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
    Ok(RiscvGShareBranchUpdate::new(
        prediction,
        history_update,
        training_update,
    ))
}

fn retire_bimode_branch_update(
    state: &mut RiscvCoreState,
    pc: Address,
    conditional: bool,
    actual_taken: bool,
    selected_prediction_correct: bool,
    selected: Option<&RiscvSelectedBranchSpeculation>,
) -> Result<RiscvBiModeBranchUpdate, RiscvCpuError> {
    if let Some(RiscvSelectedBranchSpeculation::BiMode {
        prediction,
        history_update,
    }) = selected
    {
        let history_update = if selected_prediction_correct {
            history_update
                .clone()
                .expect("recorded bimode speculation includes a history update")
        } else {
            state
                .bimode_branch_predictor
                .squash(prediction.history())
                .map_err(RiscvCpuError::BiModeBranchPredictor)?;
            state
                .bimode_branch_predictor
                .update_history(prediction.history(), actual_taken)
                .map_err(RiscvCpuError::BiModeBranchPredictor)?
        };
        let training_update = state
            .bimode_branch_predictor
            .train(prediction.history(), actual_taken, false)
            .map_err(RiscvCpuError::BiModeBranchPredictor)?;
        return Ok(RiscvBiModeBranchUpdate::new(
            prediction.clone(),
            history_update,
            training_update,
        ));
    }

    let prediction = if conditional {
        state
            .bimode_branch_predictor
            .predict(RISCV_LOCAL_BIMODE_THREAD, pc)
    } else {
        state
            .bimode_branch_predictor
            .predict_unconditional(RISCV_LOCAL_BIMODE_THREAD, pc)
    }
    .map_err(RiscvCpuError::BiModeBranchPredictor)?;
    let history_update = state
        .bimode_branch_predictor
        .update_history(prediction.history(), actual_taken)
        .map_err(RiscvCpuError::BiModeBranchPredictor)?;
    let training_update = state
        .bimode_branch_predictor
        .train(prediction.history(), actual_taken, false)
        .map_err(RiscvCpuError::BiModeBranchPredictor)?;
    Ok(RiscvBiModeBranchUpdate::new(
        prediction,
        history_update,
        training_update,
    ))
}

fn retire_tournament_branch_update(
    state: &mut RiscvCoreState,
    pc: Address,
    conditional: bool,
    actual_taken: bool,
    selected_prediction_correct: bool,
    selected: Option<&RiscvSelectedBranchSpeculation>,
) -> Result<RiscvTournamentBranchUpdate, RiscvCpuError> {
    if let Some(RiscvSelectedBranchSpeculation::Tournament {
        prediction,
        history_update,
    }) = selected
    {
        let history_update = if selected_prediction_correct {
            history_update
                .clone()
                .expect("recorded tournament speculation includes a history update")
        } else {
            state
                .tournament_branch_predictor
                .squash(prediction.history())
                .map_err(RiscvCpuError::TournamentBranchPredictor)?;
            state
                .tournament_branch_predictor
                .update_history(prediction.history(), actual_taken)
                .map_err(RiscvCpuError::TournamentBranchPredictor)?
        };
        let training_update = state
            .tournament_branch_predictor
            .train(prediction.history(), actual_taken, false)
            .map_err(RiscvCpuError::TournamentBranchPredictor)?;
        return Ok(RiscvTournamentBranchUpdate::new(
            prediction.clone(),
            history_update,
            training_update,
        ));
    }

    let prediction = if conditional {
        state
            .tournament_branch_predictor
            .predict(RISCV_LOCAL_TOURNAMENT_THREAD, pc)
    } else {
        state
            .tournament_branch_predictor
            .predict_unconditional(RISCV_LOCAL_TOURNAMENT_THREAD, pc)
    }
    .map_err(RiscvCpuError::TournamentBranchPredictor)?;
    let history_update = state
        .tournament_branch_predictor
        .update_history(prediction.history(), actual_taken)
        .map_err(RiscvCpuError::TournamentBranchPredictor)?;
    let training_update = state
        .tournament_branch_predictor
        .train(prediction.history(), actual_taken, false)
        .map_err(RiscvCpuError::TournamentBranchPredictor)?;
    Ok(RiscvTournamentBranchUpdate::new(
        prediction,
        history_update,
        training_update,
    ))
}

fn retire_tage_sc_l_branch_update(
    state: &mut RiscvCoreState,
    pc: Address,
    conditional: bool,
    actual_taken: bool,
    kind: StatisticalCorrectorBranchKind,
    target: Address,
    selected: Option<&RiscvSelectedBranchSpeculation>,
) -> Result<RiscvTageScLBranchUpdate, RiscvCpuError> {
    if let Some(RiscvSelectedBranchSpeculation::TageScL {
        prediction,
        snapshot_before_update,
        ..
    }) = selected
    {
        if let Some(snapshot) = snapshot_before_update {
            state
                .tage_sc_l_branch_predictor
                .restore(snapshot)
                .map_err(RiscvCpuError::TageScLBranchPredictor)?;
        }
        let training_update = state
            .tage_sc_l_branch_predictor
            .train(prediction.history(), actual_taken, kind, target)
            .map_err(RiscvCpuError::TageScLBranchPredictor)?;
        state.reapply_tage_sc_l_selected_branch_speculations()?;
        return Ok(RiscvTageScLBranchUpdate::new(
            prediction.clone(),
            training_update,
        ));
    }

    let prediction = state
        .tage_sc_l_branch_predictor
        .predict(RISCV_LOCAL_TAGE_SC_L_THREAD, pc, conditional)
        .map_err(RiscvCpuError::TageScLBranchPredictor)?;
    let training_update = state
        .tage_sc_l_branch_predictor
        .train(prediction.history(), actual_taken, kind, target)
        .map_err(RiscvCpuError::TageScLBranchPredictor)?;
    Ok(RiscvTageScLBranchUpdate::new(prediction, training_update))
}

fn retire_multiperspective_perceptron_branch_update(
    state: &mut RiscvCoreState,
    pc: Address,
    conditional: bool,
    actual_taken: bool,
    target: Address,
    selected: Option<&RiscvSelectedBranchSpeculation>,
) -> Result<RiscvMultiperspectivePerceptronBranchUpdate, RiscvCpuError> {
    if let Some(RiscvSelectedBranchSpeculation::MultiperspectivePerceptron {
        prediction,
        snapshot_before_update,
        ..
    }) = selected
    {
        if let Some(snapshot) = snapshot_before_update {
            state
                .multiperspective_perceptron
                .restore(snapshot)
                .map_err(RiscvCpuError::MultiperspectivePerceptron)?;
        }
        let training_update = state
            .multiperspective_perceptron
            .train(prediction.history(), actual_taken, target)
            .map_err(RiscvCpuError::MultiperspectivePerceptron)?;
        state.reapply_multiperspective_selected_branch_speculations()?;
        return Ok(RiscvMultiperspectivePerceptronBranchUpdate::new(
            prediction.clone(),
            training_update,
        ));
    }

    let prediction = state
        .multiperspective_perceptron
        .predict(
            RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD,
            pc,
            conditional,
        )
        .map_err(RiscvCpuError::MultiperspectivePerceptron)?;
    let training_update = state
        .multiperspective_perceptron
        .train(prediction.history(), actual_taken, target)
        .map_err(RiscvCpuError::MultiperspectivePerceptron)?;
    Ok(RiscvMultiperspectivePerceptronBranchUpdate::new(
        prediction,
        training_update,
    ))
}

const fn statistical_corrector_branch_kind(
    instruction: RiscvInstruction,
) -> StatisticalCorrectorBranchKind {
    match instruction {
        RiscvInstruction::Jal { .. } => StatisticalCorrectorBranchKind::DirectUnconditional,
        RiscvInstruction::Jalr { .. } => StatisticalCorrectorBranchKind::IndirectUnconditional,
        _ => StatisticalCorrectorBranchKind::DirectConditional,
    }
}

fn static_conditional_branch_target(pc: Address, instruction: RiscvInstruction) -> Option<Address> {
    let offset = match instruction {
        RiscvInstruction::Beq { offset, .. }
        | RiscvInstruction::Bne { offset, .. }
        | RiscvInstruction::Blt { offset, .. }
        | RiscvInstruction::Bge { offset, .. }
        | RiscvInstruction::Bltu { offset, .. }
        | RiscvInstruction::Bgeu { offset, .. } => offset.value(),
        _ => return None,
    };
    checked_add_signed(pc.get(), offset).map(Address::new)
}

fn instruction_is_conditional_branch(instruction: RiscvInstruction) -> bool {
    matches!(
        instruction,
        RiscvInstruction::Beq { .. }
            | RiscvInstruction::Bne { .. }
            | RiscvInstruction::Blt { .. }
            | RiscvInstruction::Bge { .. }
            | RiscvInstruction::Bltu { .. }
            | RiscvInstruction::Bgeu { .. }
    )
}

fn checked_add_signed(value: u64, offset: i64) -> Option<u64> {
    if offset >= 0 {
        value.checked_add(offset as u64)
    } else {
        value.checked_sub(offset.unsigned_abs())
    }
}

fn resolve_branch_speculation(
    state: &mut RiscvCoreState,
    sequence: u64,
    branch_kind: BranchTargetKind,
    update: &crate::BranchUpdate,
) -> Result<Option<RiscvResolvedBranchPrediction>, RiscvCpuError> {
    let Some(speculation) = state.branch_speculations.remove(&sequence) else {
        return Ok(None);
    };
    state.branch_speculation_kinds.remove(&sequence);

    let pending = state
        .branch_predictor
        .pending_speculation(speculation)
        .ok_or(crate::BranchPredictorError::UnknownSpeculation { id: speculation })
        .map_err(RiscvCpuError::BranchPredictor)?;
    let predicted_taken = pending.predicted_taken();
    let predicted_target = pending.target();
    let branch_target_prediction = state.branch_target_predictions.remove(&sequence);
    let selected_prediction = RiscvResolvedBranchPrediction {
        predicted_taken,
        predicted_target,
        actual_taken: update.actual_taken(),
        actual_target: update.actual_target(),
    };
    state.branch_speculation_summary.record_btb_resolution(
        branch_kind,
        predicted_taken,
        predicted_target,
        update.actual_taken(),
        update.actual_target(),
        branch_target_prediction,
    );
    let predicted_correctly = predicted_taken == update.actual_taken()
        && (!predicted_taken || predicted_target == update.actual_target());
    if !predicted_correctly {
        state.commit_return_address_stack_speculation(sequence, predicted_correctly)?;
        let repair = state
            .branch_predictor
            .repair_speculation(speculation, update.actual_taken())
            .map_err(RiscvCpuError::BranchPredictor)?;
        state
            .branch_speculation_summary
            .record_repair(repair.removed_youngers().len() as u64);
        remove_branch_speculation_mappings(state, repair.removed_youngers(), true)?;
    } else {
        state.commit_return_address_stack_speculation(sequence, predicted_correctly)?;
    }
    state
        .branch_predictor
        .commit_speculation(speculation)
        .map_err(RiscvCpuError::BranchPredictor)?;
    Ok(Some(selected_prediction))
}

fn discard_branch_speculation(
    state: &mut RiscvCoreState,
    sequence: u64,
) -> Result<(), RiscvCpuError> {
    let Some(speculation) = state.branch_speculations.remove(&sequence) else {
        return Ok(());
    };
    state.branch_speculation_kinds.remove(&sequence);
    state.branch_target_predictions.remove(&sequence);
    state.squash_return_address_stack_speculation(sequence)?;

    let discard = state
        .branch_predictor
        .discard_speculation(speculation)
        .map_err(RiscvCpuError::BranchPredictor)?;
    remove_branch_speculation_mappings(state, discard.removed_youngers(), false)?;
    Ok(())
}

fn remove_branch_speculation_mappings(
    state: &mut RiscvCoreState,
    removed: &[crate::BranchSpeculation],
    record_squashes: bool,
) -> Result<(), RiscvCpuError> {
    let removed_ids = removed
        .iter()
        .map(|removed_speculation| removed_speculation.id())
        .collect::<BTreeSet<_>>();
    if record_squashes {
        let squashed_sequences = state
            .branch_speculations
            .iter()
            .filter_map(|(sequence, pending)| removed_ids.contains(pending).then_some(*sequence))
            .collect::<Vec<_>>();
        for sequence in squashed_sequences {
            if let Some(branch_kind) = state.branch_speculation_kinds.remove(&sequence) {
                state
                    .branch_speculation_summary
                    .record_squashed_branch_kind(branch_kind);
            }
        }
    }
    state
        .branch_speculations
        .retain(|_, pending| !removed_ids.contains(pending));
    let active_sequences = state
        .branch_speculations
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    state
        .branch_target_predictions
        .retain(|sequence, _| active_sequences.contains(sequence));
    state
        .branch_speculation_kinds
        .retain(|sequence, _| active_sequences.contains(sequence));
    state.squash_inactive_return_address_stack_speculations(&active_sequences)?;
    state.rollback_inactive_selected_branch_speculations(&active_sequences)
}

fn enqueue_in_order_fetch_if_available(
    state: &mut RiscvCoreState,
    sequence: u64,
) -> Result<bool, RiscvCpuError> {
    if state.in_order_pipeline.contains_sequence(sequence) {
        return Ok(true);
    }
    if !state.in_order_pipeline.fetch1_has_slot() {
        return Ok(false);
    }
    state
        .in_order_pipeline
        .enqueue_fetch(sequence)
        .map_err(RiscvCpuError::InOrderPipeline)?;
    Ok(true)
}

pub(crate) fn sync_in_order_fetch_state(
    state: &mut RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> Result<(), RiscvCpuError> {
    let mut current_sequences = fetch_events
        .iter()
        .map(|event| event.request_id().sequence())
        .collect::<BTreeSet<_>>();
    if let Some(prefix) = state.pending_fetch_prefix.as_ref() {
        current_sequences.insert(prefix.fetch.request_id().sequence());
    }
    if !fetch_events.is_empty() {
        rebind_orphaned_execute_wait_fetch(state, fetch_events, &current_sequences);
    }
    let orphaned_sequences = state
        .in_order_pipeline
        .in_flight()
        .iter()
        .filter(|instruction| {
            !current_sequences.contains(&instruction.sequence())
                && (!fetch_events.is_empty() || instruction.execute_wait_total_cycles().is_none())
        })
        .map(|instruction| instruction.sequence())
        .collect::<BTreeSet<_>>();
    remove_fetch_sequences_from_pipeline(state, &orphaned_sequences)?;
    let failed_or_retried = fetch_events
        .iter()
        .filter(|event| {
            matches!(
                event.kind(),
                CpuFetchEventKind::Retry | CpuFetchEventKind::Failed
            )
        })
        .map(CpuFetchEvent::request_id)
        .collect::<BTreeSet<_>>();
    let failed_or_retried_sequences = failed_or_retried
        .iter()
        .map(|request| request.sequence())
        .collect::<BTreeSet<_>>();
    remove_fetch_sequences_from_pipeline(state, &failed_or_retried_sequences)?;
    if let Some(sequence) = state
        .pending_fetch_prefix
        .as_ref()
        .map(|prefix| prefix.fetch.request_id().sequence())
        .filter(|sequence| !state.in_order_pipeline.contains_sequence(*sequence))
    {
        if !enqueue_in_order_fetch_if_available(state, sequence)? {
            return Ok(());
        }
    }
    let mut fetches = fetch_events
        .iter()
        .filter(|event| {
            !failed_or_retried.contains(&event.request_id())
                && !state.executed_fetches.contains(&event.request_id())
                && match event.kind() {
                    CpuFetchEventKind::Issued => event.size().bytes() == 4,
                    CpuFetchEventKind::Completed => {
                        event.data().is_some_and(|data| data.len() == 4)
                    }
                    CpuFetchEventKind::Retry | CpuFetchEventKind::Failed => false,
                }
        })
        .collect::<Vec<_>>();
    fetches.sort_by_key(|event| event.request_id().sequence());

    for fetch in fetches {
        if !enqueue_in_order_fetch_if_available(state, fetch.request_id().sequence())? {
            break;
        }
    }
    Ok(())
}

fn rebind_orphaned_execute_wait_fetch(
    state: &mut RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    current_sequences: &BTreeSet<u64>,
) {
    let Some(instruction) = state.in_order_pipeline.in_flight().first().copied() else {
        return;
    };
    if current_sequences.contains(&instruction.sequence())
        || instruction.execute_wait_remaining_cycles().is_none()
    {
        return;
    }
    let architectural = Address::new(state.hart.pc());
    let mut replacements = fetch_events.iter().filter(|event| {
        matches!(
            event.kind(),
            CpuFetchEventKind::Issued | CpuFetchEventKind::Completed
        ) && event.pc() == architectural
            && !state.executed_fetches.contains(&event.request_id())
    });
    let Some(replacement_sequence) = replacements
        .next()
        .map(|event| event.request_id().sequence())
    else {
        return;
    };
    if replacements.next().is_some() {
        return;
    }
    state
        .in_order_pipeline
        .rebind_execute_wait_sequence(instruction.sequence(), replacement_sequence);
    state
        .rebound_in_order_execute_waits
        .insert(replacement_sequence);
}

pub(crate) fn remove_fetch_sequences_from_pipeline(
    state: &mut RiscvCoreState,
    sequences: &BTreeSet<u64>,
) -> Result<(), RiscvCpuError> {
    if sequences.is_empty() {
        return Ok(());
    }

    let retained = state
        .in_order_pipeline
        .in_flight()
        .iter()
        .copied()
        .filter(|instruction| !sequences.contains(&instruction.sequence()))
        .collect::<Vec<_>>();
    state
        .rebound_in_order_execute_waits
        .retain(|sequence| !sequences.contains(sequence));
    state
        .in_order_pipeline
        .replace_in_flight(retained)
        .map_err(RiscvCpuError::InOrderPipeline)
}

#[cfg(test)]
#[path = "riscv_execute_tests.rs"]
mod tests;

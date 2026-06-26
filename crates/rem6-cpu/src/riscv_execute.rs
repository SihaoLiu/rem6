use std::collections::BTreeSet;

use rem6_isa_riscv::{
    RiscvInstruction, RiscvVectorFloatInstruction, RiscvVectorSaturatingInstruction,
    RiscvVectorWideningIntegerInstruction,
};
use rem6_memory::{AccessSize, Address, MemoryRequestId};

use crate::{
    riscv_execution_event::RiscvRetiredBranchUpdates, BranchTargetKind, CpuFetchEvent,
    CpuFetchEventKind, CpuFetchRecord, InOrderBranchPrediction, InOrderBranchRedirect,
    InOrderPipelineCycleRecord, InOrderPipelineInstruction, InOrderPipelineStage,
    RiscvBiModeBranchUpdate, RiscvCore, RiscvCoreState, RiscvCpuError, RiscvCpuExecutionEvent,
    RiscvGShareBranchUpdate, RiscvMultiperspectivePerceptronBranchUpdate, RiscvTageScLBranchUpdate,
    RiscvTournamentBranchUpdate, StatisticalCorrectorBranchKind, RISCV_LOCAL_BIMODE_THREAD,
    RISCV_LOCAL_GSHARE_THREAD, RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD,
    RISCV_LOCAL_TAGE_SC_L_THREAD, RISCV_LOCAL_TOURNAMENT_THREAD,
};

const RISCV_SCALAR_INTEGER_MUL_EXTRA_EXECUTE_CYCLES: u64 = 2;
const RISCV_SCALAR_INTEGER_DIV_EXTRA_EXECUTE_CYCLES: u64 = 19;
const RISCV_VECTOR_INTEGER_MUL_EXTRA_EXECUTE_CYCLES: u64 =
    RISCV_SCALAR_INTEGER_MUL_EXTRA_EXECUTE_CYCLES;
const RISCV_VECTOR_INTEGER_DIV_EXTRA_EXECUTE_CYCLES: u64 =
    RISCV_SCALAR_INTEGER_DIV_EXTRA_EXECUTE_CYCLES;
const RISCV_SCALAR_FLOAT_ADD_EXTRA_EXECUTE_CYCLES: u64 = 1;
const RISCV_SCALAR_FLOAT_CMP_EXTRA_EXECUTE_CYCLES: u64 = 1;
const RISCV_SCALAR_FLOAT_CVT_EXTRA_EXECUTE_CYCLES: u64 = 1;
const RISCV_SCALAR_FLOAT_MISC_EXTRA_EXECUTE_CYCLES: u64 = 2;
const RISCV_SCALAR_FLOAT_MUL_EXTRA_EXECUTE_CYCLES: u64 = 3;
const RISCV_SCALAR_FLOAT_MUL_ADD_EXTRA_EXECUTE_CYCLES: u64 = 4;
const RISCV_SCALAR_FLOAT_DIV_EXTRA_EXECUTE_CYCLES: u64 = 11;
const RISCV_SCALAR_FLOAT_SQRT_EXTRA_EXECUTE_CYCLES: u64 = 23;
const RISCV_VECTOR_FLOAT_ADD_EXTRA_EXECUTE_CYCLES: u64 =
    RISCV_SCALAR_FLOAT_ADD_EXTRA_EXECUTE_CYCLES;
const RISCV_VECTOR_FLOAT_CMP_EXTRA_EXECUTE_CYCLES: u64 =
    RISCV_SCALAR_FLOAT_CMP_EXTRA_EXECUTE_CYCLES;
const RISCV_VECTOR_FLOAT_CVT_EXTRA_EXECUTE_CYCLES: u64 =
    RISCV_SCALAR_FLOAT_CVT_EXTRA_EXECUTE_CYCLES;
const RISCV_VECTOR_FLOAT_MISC_EXTRA_EXECUTE_CYCLES: u64 =
    RISCV_SCALAR_FLOAT_MISC_EXTRA_EXECUTE_CYCLES;
const RISCV_VECTOR_FLOAT_MUL_EXTRA_EXECUTE_CYCLES: u64 =
    RISCV_SCALAR_FLOAT_MUL_EXTRA_EXECUTE_CYCLES;
const RISCV_VECTOR_FLOAT_MUL_ADD_EXTRA_EXECUTE_CYCLES: u64 =
    RISCV_SCALAR_FLOAT_MUL_ADD_EXTRA_EXECUTE_CYCLES;
const RISCV_VECTOR_FLOAT_DIV_EXTRA_EXECUTE_CYCLES: u64 =
    RISCV_SCALAR_FLOAT_DIV_EXTRA_EXECUTE_CYCLES;
const RISCV_VECTOR_FLOAT_SQRT_EXTRA_EXECUTE_CYCLES: u64 =
    RISCV_SCALAR_FLOAT_SQRT_EXTRA_EXECUTE_CYCLES;

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
        let primary_hart = state.hart.clone();
        if let Some(checker) = &mut state.checker {
            checker
                .check_retired(
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
        let retired_branch = retire_branch_predictions(
            state,
            fetch.request_id().sequence(),
            fetch.pc(),
            instruction,
            &execution,
        )?;
        let redirects_fetch = execution.trap().is_some()
            || next_pc.get() != sequential_next_pc
            || retired_branch
                .fetch_prediction()
                .is_some_and(branch_prediction_redirects_fetch);
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
        let pipeline_redirect = execution.trap().is_some().then(|| {
            InOrderBranchRedirect::new(
                fetch.request_id().sequence(),
                InOrderPipelineStage::Commit,
                next_pc.get(),
            )
        });
        let execute_wait_cycles = in_order_execute_wait_cycles(instruction);
        let pipeline_cycle = if execution.memory_access().is_none() {
            Some(
                record_retired_in_order_pipeline_cycle_with_redirect_after_wait(
                    state,
                    fetch.request_id().sequence(),
                    pipeline_branch_prediction,
                    pipeline_redirect,
                    execute_wait_cycles,
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
            true,
        );
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
    } else {
        discard_stale_in_order_pipeline_before_retire(state, sequence)?;
    }
    let mut wait_recorded = wait_cycles == 0;
    let max_retire_cycles =
        InOrderPipelineStage::ALL.len() + state.in_order_pipeline.in_flight().len();
    for _ in 0..max_retire_cycles {
        if !wait_recorded && in_order_pipeline_sequence_is_in_execute(state, sequence) {
            record_in_order_resource_wait_cycles(state, wait_cycles)?;
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
        let retires_sequence = record_retires_sequence(&record, sequence);
        state.in_order_pipeline_cycle_records.push(record.clone());
        if !wait_recorded
            && record.after().in_flight().iter().any(|instruction| {
                instruction.sequence() == sequence
                    && instruction.stage() == InOrderPipelineStage::Execute
            })
        {
            record_in_order_resource_wait_cycles(state, wait_cycles)?;
            wait_recorded = true;
        }
        if retires_sequence {
            return Ok(record);
        }
    }

    unreachable!(
        "default in-order pipeline retires an instruction within its stage count: retiring sequence {sequence}, in_flight {:?}",
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

fn in_order_pipeline_sequence_is_in_execute(state: &RiscvCoreState, sequence: u64) -> bool {
    state
        .in_order_pipeline
        .in_flight()
        .iter()
        .any(|instruction| {
            instruction.sequence() == sequence
                && instruction.stage() == InOrderPipelineStage::Execute
        })
}

fn record_in_order_resource_wait_cycles(
    state: &mut RiscvCoreState,
    wait_cycles: u64,
) -> Result<(), RiscvCpuError> {
    for _ in 0..wait_cycles {
        let stall_record = state
            .in_order_pipeline
            .try_record_resource_stall_cycle()
            .map_err(RiscvCpuError::InOrderPipeline)?;
        state.in_order_pipeline_cycle_records.push(stall_record);
    }
    Ok(())
}

fn in_order_execute_wait_cycles(instruction: RiscvInstruction) -> u64 {
    match instruction {
        RiscvInstruction::Mul { .. }
        | RiscvInstruction::Mulh { .. }
        | RiscvInstruction::Mulhsu { .. }
        | RiscvInstruction::Mulhu { .. }
        | RiscvInstruction::Mulw { .. } => RISCV_SCALAR_INTEGER_MUL_EXTRA_EXECUTE_CYCLES,
        RiscvInstruction::Div { .. }
        | RiscvInstruction::Divu { .. }
        | RiscvInstruction::Rem { .. }
        | RiscvInstruction::Remu { .. }
        | RiscvInstruction::Divw { .. }
        | RiscvInstruction::Divuw { .. }
        | RiscvInstruction::Remw { .. }
        | RiscvInstruction::Remuw { .. } => RISCV_SCALAR_INTEGER_DIV_EXTRA_EXECUTE_CYCLES,
        RiscvInstruction::VectorMultiplyLowVv { .. }
        | RiscvInstruction::VectorMultiplyLowVx { .. }
        | RiscvInstruction::VectorMultiplyHighUnsignedVv { .. }
        | RiscvInstruction::VectorMultiplyHighUnsignedVx { .. }
        | RiscvInstruction::VectorMultiplyHighSignedUnsignedVv { .. }
        | RiscvInstruction::VectorMultiplyHighSignedUnsignedVx { .. }
        | RiscvInstruction::VectorMultiplyHighSignedVv { .. }
        | RiscvInstruction::VectorMultiplyHighSignedVx { .. } => {
            RISCV_VECTOR_INTEGER_MUL_EXTRA_EXECUTE_CYCLES
        }
        RiscvInstruction::VectorIntegerMultiplyAdd(_) => {
            RISCV_VECTOR_INTEGER_MUL_EXTRA_EXECUTE_CYCLES
        }
        RiscvInstruction::VectorSaturating(
            RiscvVectorSaturatingInstruction::MulSignedFractionalVv { .. }
            | RiscvVectorSaturatingInstruction::MulSignedFractionalVx { .. },
        ) => RISCV_VECTOR_INTEGER_MUL_EXTRA_EXECUTE_CYCLES,
        RiscvInstruction::VectorWideningInteger(
            RiscvVectorWideningIntegerInstruction::MultiplyUnsignedVv { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyUnsignedVx { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplySignedUnsignedVv { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplySignedUnsignedVx { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplySignedVv { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplySignedVx { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyAddUnsignedVv { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyAddUnsignedVx { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyAddSignedVv { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyAddSignedVx { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyAddUnsignedSignedVx { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyAddSignedUnsignedVv { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyAddSignedUnsignedVx { .. },
        ) => RISCV_VECTOR_INTEGER_MUL_EXTRA_EXECUTE_CYCLES,
        RiscvInstruction::VectorDivideUnsignedVv { .. }
        | RiscvInstruction::VectorDivideUnsignedVx { .. }
        | RiscvInstruction::VectorDivideSignedVv { .. }
        | RiscvInstruction::VectorDivideSignedVx { .. }
        | RiscvInstruction::VectorRemainderUnsignedVv { .. }
        | RiscvInstruction::VectorRemainderUnsignedVx { .. }
        | RiscvInstruction::VectorRemainderSignedVv { .. }
        | RiscvInstruction::VectorRemainderSignedVx { .. } => {
            RISCV_VECTOR_INTEGER_DIV_EXTRA_EXECUTE_CYCLES
        }
        RiscvInstruction::FloatAddS { .. }
        | RiscvInstruction::FloatAddD { .. }
        | RiscvInstruction::FloatSubS { .. }
        | RiscvInstruction::FloatSubD { .. } => RISCV_SCALAR_FLOAT_ADD_EXTRA_EXECUTE_CYCLES,
        RiscvInstruction::FloatMinS { .. }
        | RiscvInstruction::FloatMinD { .. }
        | RiscvInstruction::FloatMaxS { .. }
        | RiscvInstruction::FloatMaxD { .. }
        | RiscvInstruction::FloatLessOrEqualS { .. }
        | RiscvInstruction::FloatLessOrEqualD { .. }
        | RiscvInstruction::FloatLessThanS { .. }
        | RiscvInstruction::FloatLessThanD { .. }
        | RiscvInstruction::FloatEqualS { .. }
        | RiscvInstruction::FloatEqualD { .. } => RISCV_SCALAR_FLOAT_CMP_EXTRA_EXECUTE_CYCLES,
        RiscvInstruction::FloatMoveXFromS { .. }
        | RiscvInstruction::FloatMoveXFromD { .. }
        | RiscvInstruction::FloatMoveSFromX { .. }
        | RiscvInstruction::FloatMoveDFromX { .. }
        | RiscvInstruction::FloatConvertSFromW { .. }
        | RiscvInstruction::FloatConvertSFromWu { .. }
        | RiscvInstruction::FloatConvertSFromL { .. }
        | RiscvInstruction::FloatConvertSFromLu { .. }
        | RiscvInstruction::FloatConvertWFromS { .. }
        | RiscvInstruction::FloatConvertWuFromS { .. }
        | RiscvInstruction::FloatConvertLFromS { .. }
        | RiscvInstruction::FloatConvertLuFromS { .. }
        | RiscvInstruction::FloatConvertSFromD { .. }
        | RiscvInstruction::FloatConvertDFromS { .. }
        | RiscvInstruction::FloatConvertDFromW { .. }
        | RiscvInstruction::FloatConvertDFromWu { .. }
        | RiscvInstruction::FloatConvertDFromL { .. }
        | RiscvInstruction::FloatConvertDFromLu { .. }
        | RiscvInstruction::FloatConvertWFromD { .. }
        | RiscvInstruction::FloatConvertWuFromD { .. }
        | RiscvInstruction::FloatConvertLFromD { .. }
        | RiscvInstruction::FloatConvertLuFromD { .. } => {
            RISCV_SCALAR_FLOAT_CVT_EXTRA_EXECUTE_CYCLES
        }
        RiscvInstruction::FloatSignInjectS { .. }
        | RiscvInstruction::FloatSignInjectD { .. }
        | RiscvInstruction::FloatSignInjectNegS { .. }
        | RiscvInstruction::FloatSignInjectNegD { .. }
        | RiscvInstruction::FloatSignInjectXorS { .. }
        | RiscvInstruction::FloatSignInjectXorD { .. }
        | RiscvInstruction::FloatClassS { .. }
        | RiscvInstruction::FloatClassD { .. } => RISCV_SCALAR_FLOAT_MISC_EXTRA_EXECUTE_CYCLES,
        RiscvInstruction::FloatMulS { .. } | RiscvInstruction::FloatMulD { .. } => {
            RISCV_SCALAR_FLOAT_MUL_EXTRA_EXECUTE_CYCLES
        }
        RiscvInstruction::FloatMultiplyAddS { .. }
        | RiscvInstruction::FloatMultiplyAddD { .. }
        | RiscvInstruction::FloatMultiplySubtractS { .. }
        | RiscvInstruction::FloatMultiplySubtractD { .. }
        | RiscvInstruction::FloatNegativeMultiplySubtractS { .. }
        | RiscvInstruction::FloatNegativeMultiplySubtractD { .. }
        | RiscvInstruction::FloatNegativeMultiplyAddS { .. }
        | RiscvInstruction::FloatNegativeMultiplyAddD { .. } => {
            RISCV_SCALAR_FLOAT_MUL_ADD_EXTRA_EXECUTE_CYCLES
        }
        RiscvInstruction::FloatDivS { .. } | RiscvInstruction::FloatDivD { .. } => {
            RISCV_SCALAR_FLOAT_DIV_EXTRA_EXECUTE_CYCLES
        }
        RiscvInstruction::FloatSqrtS { .. } | RiscvInstruction::FloatSqrtD { .. } => {
            RISCV_SCALAR_FLOAT_SQRT_EXTRA_EXECUTE_CYCLES
        }
        RiscvInstruction::VectorFloat(vector_instruction) => {
            vector_float_execute_wait_cycles(vector_instruction)
        }
        _ => 0,
    }
}

fn vector_float_execute_wait_cycles(instruction: RiscvVectorFloatInstruction) -> u64 {
    match instruction {
        RiscvVectorFloatInstruction::AddVv { .. }
        | RiscvVectorFloatInstruction::AddVf { .. }
        | RiscvVectorFloatInstruction::SubVv { .. }
        | RiscvVectorFloatInstruction::SubVf { .. }
        | RiscvVectorFloatInstruction::ReverseSubVf { .. } => {
            RISCV_VECTOR_FLOAT_ADD_EXTRA_EXECUTE_CYCLES
        }
        RiscvVectorFloatInstruction::MinVv { .. }
        | RiscvVectorFloatInstruction::MinVf { .. }
        | RiscvVectorFloatInstruction::MaxVv { .. }
        | RiscvVectorFloatInstruction::MaxVf { .. }
        | RiscvVectorFloatInstruction::MaskEqualVv { .. }
        | RiscvVectorFloatInstruction::MaskEqualVf { .. }
        | RiscvVectorFloatInstruction::MaskNotEqualVv { .. }
        | RiscvVectorFloatInstruction::MaskNotEqualVf { .. }
        | RiscvVectorFloatInstruction::MaskLessThanVv { .. }
        | RiscvVectorFloatInstruction::MaskLessThanVf { .. }
        | RiscvVectorFloatInstruction::MaskLessEqualVv { .. }
        | RiscvVectorFloatInstruction::MaskLessEqualVf { .. } => {
            RISCV_VECTOR_FLOAT_CMP_EXTRA_EXECUTE_CYCLES
        }
        RiscvVectorFloatInstruction::ConvertFloatFromUnsignedIntV { .. }
        | RiscvVectorFloatInstruction::ConvertFloatFromSignedIntV { .. }
        | RiscvVectorFloatInstruction::ConvertUnsignedIntFromFloatV { .. }
        | RiscvVectorFloatInstruction::ConvertSignedIntFromFloatV { .. }
        | RiscvVectorFloatInstruction::ConvertUnsignedIntFromFloatTowardZeroV { .. }
        | RiscvVectorFloatInstruction::ConvertSignedIntFromFloatTowardZeroV { .. }
        | RiscvVectorFloatInstruction::MergeVf { .. }
        | RiscvVectorFloatInstruction::MoveVf { .. }
        | RiscvVectorFloatInstruction::MoveFv { .. }
        | RiscvVectorFloatInstruction::MoveSv { .. } => RISCV_VECTOR_FLOAT_CVT_EXTRA_EXECUTE_CYCLES,
        RiscvVectorFloatInstruction::SignInjectVv { .. }
        | RiscvVectorFloatInstruction::SignInjectVf { .. }
        | RiscvVectorFloatInstruction::SignInjectNegVv { .. }
        | RiscvVectorFloatInstruction::SignInjectNegVf { .. }
        | RiscvVectorFloatInstruction::SignInjectXorVv { .. }
        | RiscvVectorFloatInstruction::SignInjectXorVf { .. }
        | RiscvVectorFloatInstruction::ClassV { .. } => {
            RISCV_VECTOR_FLOAT_MISC_EXTRA_EXECUTE_CYCLES
        }
        RiscvVectorFloatInstruction::MulVv { .. } | RiscvVectorFloatInstruction::MulVf { .. } => {
            RISCV_VECTOR_FLOAT_MUL_EXTRA_EXECUTE_CYCLES
        }
        RiscvVectorFloatInstruction::MulAddVv { .. }
        | RiscvVectorFloatInstruction::MulAddVf { .. } => {
            RISCV_VECTOR_FLOAT_MUL_ADD_EXTRA_EXECUTE_CYCLES
        }
        RiscvVectorFloatInstruction::DivVv { .. }
        | RiscvVectorFloatInstruction::DivVf { .. }
        | RiscvVectorFloatInstruction::ReverseDivVf { .. } => {
            RISCV_VECTOR_FLOAT_DIV_EXTRA_EXECUTE_CYCLES
        }
        RiscvVectorFloatInstruction::SqrtV { .. } => RISCV_VECTOR_FLOAT_SQRT_EXTRA_EXECUTE_CYCLES,
    }
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

    let branch_update = state
        .branch_predictor
        .update(pc, actual_taken, actual_target);
    if let Some(target) = actual_target {
        state
            .branch_target_buffer
            .update(pc, target, branch_target_kind(instruction));
    }
    let selected_prediction = resolve_branch_speculation(state, sequence, &branch_update)?;
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
    let bimode_prediction = if conditional {
        state
            .bimode_branch_predictor
            .predict(RISCV_LOCAL_BIMODE_THREAD, pc)
    } else {
        state
            .bimode_branch_predictor
            .predict_unconditional(RISCV_LOCAL_BIMODE_THREAD, pc)
    }
    .map_err(RiscvCpuError::BiModeBranchPredictor)?;
    let bimode_history_update = state
        .bimode_branch_predictor
        .update_history(bimode_prediction.history(), actual_taken)
        .map_err(RiscvCpuError::BiModeBranchPredictor)?;
    let bimode_training_update = state
        .bimode_branch_predictor
        .train(bimode_prediction.history(), actual_taken, false)
        .map_err(RiscvCpuError::BiModeBranchPredictor)?;
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
    let tage_sc_l_prediction = state
        .tage_sc_l_branch_predictor
        .predict(RISCV_LOCAL_TAGE_SC_L_THREAD, pc, conditional)
        .map_err(RiscvCpuError::TageScLBranchPredictor)?;
    let tage_sc_l_target = if conditional {
        static_conditional_branch_target(pc, instruction).unwrap_or(Address::new(next_pc))
    } else {
        Address::new(next_pc)
    };
    let tage_sc_l_training_update = state
        .tage_sc_l_branch_predictor
        .train(
            tage_sc_l_prediction.history(),
            actual_taken,
            statistical_corrector_branch_kind(instruction),
            tage_sc_l_target,
        )
        .map_err(RiscvCpuError::TageScLBranchPredictor)?;
    let multiperspective_perceptron_prediction = state
        .multiperspective_perceptron
        .predict(
            RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD,
            pc,
            conditional,
        )
        .map_err(RiscvCpuError::MultiperspectivePerceptron)?;
    let multiperspective_perceptron_target = if conditional {
        static_conditional_branch_target(pc, instruction).unwrap_or(Address::new(next_pc))
    } else {
        Address::new(next_pc)
    };
    let multiperspective_perceptron_training_update = state
        .multiperspective_perceptron
        .train(
            multiperspective_perceptron_prediction.history(),
            actual_taken,
            multiperspective_perceptron_target,
        )
        .map_err(RiscvCpuError::MultiperspectivePerceptron)?;

    Ok(RiscvRetiredBranchResolution::new(
        RiscvRetiredBranchUpdates::new(
            branch_update,
            RiscvGShareBranchUpdate::new(prediction, history_update, training_update),
            RiscvBiModeBranchUpdate::new(
                bimode_prediction,
                bimode_history_update,
                bimode_training_update,
            ),
            RiscvTournamentBranchUpdate::new(
                tournament_prediction,
                tournament_history_update,
                tournament_training_update,
            ),
            RiscvTageScLBranchUpdate::new(tage_sc_l_prediction, tage_sc_l_training_update),
            RiscvMultiperspectivePerceptronBranchUpdate::new(
                multiperspective_perceptron_prediction,
                multiperspective_perceptron_training_update,
            ),
        ),
        selected_prediction,
    ))
}

const fn branch_target_kind(instruction: RiscvInstruction) -> BranchTargetKind {
    match instruction {
        RiscvInstruction::Beq { .. }
        | RiscvInstruction::Bne { .. }
        | RiscvInstruction::Blt { .. }
        | RiscvInstruction::Bge { .. }
        | RiscvInstruction::Bltu { .. }
        | RiscvInstruction::Bgeu { .. } => BranchTargetKind::DirectConditional,
        RiscvInstruction::Jal { .. } => BranchTargetKind::DirectUnconditional,
        RiscvInstruction::Jalr { .. } => BranchTargetKind::IndirectUnconditional,
        _ => BranchTargetKind::NoBranch,
    }
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
    update: &crate::BranchUpdate,
) -> Result<Option<RiscvResolvedBranchPrediction>, RiscvCpuError> {
    let Some(speculation) = state.branch_speculations.remove(&sequence) else {
        return Ok(None);
    };

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
        predicted_taken,
        update.actual_target(),
        branch_target_prediction,
    );
    let predicted_correctly = predicted_taken == update.actual_taken()
        && (!predicted_taken || predicted_target == update.actual_target());
    if !predicted_correctly {
        let repair = state
            .branch_predictor
            .repair_speculation(speculation, update.actual_taken())
            .map_err(RiscvCpuError::BranchPredictor)?;
        state
            .branch_speculation_summary
            .record_repair(repair.removed_youngers().len() as u64);
        remove_branch_speculation_mappings(state, repair.removed_youngers());
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
    state.branch_target_predictions.remove(&sequence);

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
    let active_sequences = state
        .branch_speculations
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    state
        .branch_target_predictions
        .retain(|sequence, _| active_sequences.contains(sequence));
}

pub(crate) fn sync_in_order_fetch_state(
    state: &mut RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> Result<(), RiscvCpuError> {
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
        if let Some(record) = state
            .in_order_pipeline
            .enqueue_fetch_recorded(fetch.request_id().sequence())
            .map_err(RiscvCpuError::InOrderPipeline)?
        {
            state.in_order_pipeline_cycle_records.push(record);
        }
    }
    Ok(())
}

fn remove_fetch_sequences_from_pipeline(
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
        .in_order_pipeline
        .replace_in_flight(retained)
        .map_err(RiscvCpuError::InOrderPipeline)
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
        assert!(!state.in_order_pipeline.contains_sequence(0));
        assert!(state.in_order_pipeline.in_flight().is_empty());
    }

    #[test]
    fn discarded_fetch_sequences_leave_in_order_pipeline_state() {
        let mut state = RiscvCoreState::new(0x8000, 0);
        state
            .in_order_pipeline
            .replace_in_flight([
                InOrderPipelineInstruction::new(1, InOrderPipelineStage::Commit),
                InOrderPipelineInstruction::new(2, InOrderPipelineStage::Fetch2),
                InOrderPipelineInstruction::new(3, InOrderPipelineStage::Fetch1),
            ])
            .unwrap();
        let discarded = [2, 3].into_iter().collect::<BTreeSet<_>>();

        remove_fetch_sequences_from_pipeline(&mut state, &discarded).unwrap();

        assert_eq!(
            state
                .in_order_pipeline
                .in_flight()
                .iter()
                .map(|instruction| (instruction.sequence(), instruction.stage()))
                .collect::<Vec<_>>(),
            vec![(1, InOrderPipelineStage::Commit)]
        );
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

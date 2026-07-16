use std::collections::BTreeSet;

use rem6_isa_riscv::{RiscvDecodedInstruction, RiscvInstruction};
use rem6_kernel::PartitionedScheduler;
use rem6_memory::{Address, MemoryRequestId};

use crate::{
    o3_runtime::{O3LiveIssueHeadReservation, O3LiveIssueRequest, O3RuntimeError},
    riscv_execute::{oldest_completed_fetch_at, RiscvLiveRetireGateWakeKind},
    riscv_live_retire_gate::{RiscvLiveRetireGateDecision, RiscvLiveRetireGateWake},
    riscv_o3_window_policy::{
        RiscvScalarIntegerLiveWindow, RiscvScalarIntegerYoungerDecision,
        O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS,
    },
    CpuFetchEvent, CpuFetchEventKind, RiscvCore, RiscvCoreState, RiscvCpuError,
    RiscvCpuExecutionEvent,
};

pub(super) struct RiscvLiveRetireWindowRequest<'a> {
    request: MemoryRequestId,
    pc: Address,
    raw: u32,
    fetch_tick: u64,
    fetch_events: &'a [CpuFetchEvent],
}

pub(crate) struct RiscvCompletedFetchInstruction {
    pc: Address,
    consumed_requests: Vec<MemoryRequestId>,
    decoded: RiscvDecodedInstruction,
}

impl RiscvCompletedFetchInstruction {
    pub(crate) const fn pc(&self) -> Address {
        self.pc
    }

    pub(crate) fn first_consumed_request(&self) -> MemoryRequestId {
        *self
            .consumed_requests
            .first()
            .expect("completed instruction consumes at least one fetch request")
    }

    pub(crate) fn last_consumed_request(&self) -> MemoryRequestId {
        *self
            .consumed_requests
            .last()
            .expect("completed instruction consumes at least one fetch request")
    }

    pub(crate) const fn decoded(&self) -> RiscvDecodedInstruction {
        self.decoded
    }
}

impl<'a> RiscvLiveRetireWindowRequest<'a> {
    pub(super) const fn new(
        request: MemoryRequestId,
        pc: Address,
        raw: u32,
        fetch_tick: u64,
        fetch_events: &'a [CpuFetchEvent],
    ) -> Self {
        Self {
            request,
            pc,
            raw,
            fetch_tick,
            fetch_events,
        }
    }
}

impl RiscvCore {
    pub(super) fn live_retire_gate_retire_tick(
        &self,
        state: &mut RiscvCoreState,
        gate_scheduler: &mut Option<(&mut PartitionedScheduler, RiscvLiveRetireGateWakeKind)>,
        window: RiscvLiveRetireWindowRequest<'_>,
    ) -> Result<Option<u64>, RiscvCpuError> {
        if detailed_scalar_memory_blocks_execution(state, window.raw)? {
            return Ok(None);
        }
        let live_speculative_ready_tick = if state.live_retire_gate.pending_ready_tick().is_none() {
            live_speculative_fu_ready_tick(state, &window)?
        } else {
            None
        };
        let completed_normal_execute_wait = state
            .in_order_pipeline
            .execute_wait_completed(window.request.sequence())
            && state.live_retire_gate.pending_ready_tick().is_none();
        let Some((scheduler, kind)) = gate_scheduler.as_mut() else {
            return Ok((completed_normal_execute_wait
                || live_speculative_ready_tick
                    .is_some_and(|ready_tick| ready_tick <= window.fetch_tick)
                || !state.live_retire_gate.blocks_without_scheduler())
            .then_some(window.fetch_tick));
        };
        let now = scheduler
            .partition_now(self.partition())
            .map_err(RiscvCpuError::Scheduler)?;
        if let Some(ready_tick) = state.live_retire_gate.pending_ready_tick() {
            if ready_tick <= now && state.live_retire_gate.detailed_policy_enabled() {
                stage_o3_live_retire_window(
                    state,
                    window.request,
                    window.pc,
                    window.raw,
                    now,
                    ready_tick,
                    window.fetch_events,
                )?;
            }
        }
        if completed_normal_execute_wait {
            return Ok(Some(now));
        }
        if live_speculative_ready_tick.is_some_and(|ready_tick| ready_tick <= now) {
            return Ok(Some(now));
        }
        let mut known_ready_tick = live_speculative_ready_tick;
        if known_ready_tick.is_none()
            && state.live_retire_gate.pending_ready_tick().is_none()
            && state.live_retire_gate.detailed_policy_enabled()
        {
            known_ready_tick = stage_live_speculative_fu_ready_tick(state, &window, now)?;
            if known_ready_tick.is_some_and(|ready_tick| ready_tick <= now) {
                return Ok(Some(now));
            }
        }
        let decision = if let Some(ready_tick) = known_ready_tick {
            state.live_retire_gate.before_retire_at_known_ready_tick(
                window.request,
                now,
                ready_tick,
            )
        } else {
            let ready_base_tick = now.max(window.fetch_tick);
            state.live_retire_gate.before_retire(
                window.request,
                window.raw,
                now,
                ready_base_tick,
            )?
        };
        match decision {
            RiscvLiveRetireGateDecision::Ready => Ok(Some(now)),
            RiscvLiveRetireGateDecision::Blocked => {
                let ready_tick = state
                    .live_retire_gate
                    .pending_ready_tick()
                    .expect("blocked live retire gate has a pending ready tick");
                if known_ready_tick.is_none() {
                    stage_o3_live_retire_window(
                        state,
                        window.request,
                        window.pc,
                        window.raw,
                        now,
                        ready_tick,
                        window.fetch_events,
                    )?;
                }
                Ok(None)
            }
            RiscvLiveRetireGateDecision::Schedule {
                ready_tick,
                created_wait_ticks,
            } => {
                let event_id = match *kind {
                    RiscvLiveRetireGateWakeKind::Serial => scheduler
                        .schedule_at(self.partition(), ready_tick, |_| {})
                        .map_err(RiscvCpuError::Scheduler)?,
                    RiscvLiveRetireGateWakeKind::Parallel => scheduler
                        .schedule_parallel_at(self.partition(), ready_tick, |_| {})
                        .map_err(RiscvCpuError::Scheduler)?,
                };
                let event = scheduler
                    .pending_event_snapshot(event_id)
                    .expect("newly scheduled live-retire-gate wake is pending");
                state.live_retire_gate.mark_scheduled(
                    window.request,
                    RiscvLiveRetireGateWake::new(scheduler.instance_id(), event),
                );
                if let Some(wait_ticks) = created_wait_ticks {
                    state.o3_runtime.record_live_retire_gate_wait(wait_ticks);
                }
                if known_ready_tick.is_none() {
                    stage_o3_live_retire_window(
                        state,
                        window.request,
                        window.pc,
                        window.raw,
                        now,
                        ready_tick,
                        window.fetch_events,
                    )?;
                }
                Ok(None)
            }
        }
    }
}

fn stage_live_speculative_fu_ready_tick(
    state: &mut RiscvCoreState,
    window: &RiscvLiveRetireWindowRequest<'_>,
    now: u64,
) -> Result<Option<u64>, RiscvCpuError> {
    let decoded = RiscvInstruction::decode_with_length(window.raw).map_err(RiscvCpuError::Isa)?;
    let latency = crate::riscv_fu_latency::riscv_execute_wait_cycles(decoded.instruction());
    if latency == 0 {
        return Ok(None);
    }
    let ready_base_tick = now.max(window.fetch_tick);
    let raw_ready_tick = ready_base_tick
        .checked_add(latency)
        .ok_or(RiscvCpuError::O3Runtime(
            O3RuntimeError::WritebackTickOverflow {
                tick: ready_base_tick,
            },
        ))?;
    stage_o3_live_retire_window(
        state,
        window.request,
        window.pc,
        window.raw,
        now,
        raw_ready_tick,
        window.fetch_events,
    )
}

fn live_speculative_fu_ready_tick(
    state: &RiscvCoreState,
    window: &RiscvLiveRetireWindowRequest<'_>,
) -> Result<Option<u64>, RiscvCpuError> {
    let decoded = RiscvInstruction::decode_with_length(window.raw).map_err(RiscvCpuError::Isa)?;
    if crate::riscv_fu_latency::riscv_execute_wait_cycles(decoded.instruction()) == 0 {
        return Ok(None);
    }
    let mut hart = state.hart.clone();
    hart.set_pc(window.pc.get());
    let execution = hart.execute_decoded(decoded).map_err(RiscvCpuError::Isa)?;
    let Some(fetch) = window.fetch_events.iter().find(|event| {
        event.kind() == CpuFetchEventKind::Completed
            && event.request_id() == window.request
            && event.pc() == window.pc
    }) else {
        return Ok(None);
    };
    let Some(instruction) = completed_fetch_instruction_starting_with(
        &state.executed_fetches,
        window.fetch_events,
        fetch,
    ) else {
        return Ok(None);
    };
    Ok(state
        .o3_runtime
        .live_speculative_execution_ready_tick(&instruction.consumed_requests, &execution))
}

fn detailed_scalar_memory_blocks_execution(
    state: &RiscvCoreState,
    raw: u32,
) -> Result<bool, RiscvCpuError> {
    if !state.live_retire_gate.detailed_policy_enabled()
        || !state.o3_runtime.has_pending_scalar_memory_retirement()
    {
        return Ok(false);
    }
    let instruction = RiscvInstruction::decode_with_length(raw)
        .map_err(RiscvCpuError::Isa)?
        .instruction();
    Ok(!state.can_overlap_detailed_scalar_memory_instruction(instruction))
}

fn stage_o3_live_retire_window(
    state: &mut RiscvCoreState,
    current_request: MemoryRequestId,
    pc: Address,
    raw: u32,
    earliest_tick: u64,
    ready_tick: u64,
    fetch_events: &[CpuFetchEvent],
) -> Result<Option<u64>, RiscvCpuError> {
    let decoded = RiscvInstruction::decode_with_length(raw).map_err(RiscvCpuError::Isa)?;
    let next_pc = Address::new(pc.get().wrapping_add(u64::from(decoded.bytes())));
    let younger = completed_fetch_instruction_window(
        state,
        fetch_events,
        current_request,
        next_pc,
        O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS.saturating_sub(1),
    );
    let younger = RiscvScalarIntegerLiveWindow::from_fu_head(decoded.instruction())
        .map(|window| accepted_scalar_integer_younger_window(window, younger))
        .unwrap_or_default();
    let Some(head_sequence) = state.o3_runtime.stage_live_retire_window(
        pc,
        decoded.instruction(),
        ready_tick,
        younger
            .iter()
            .map(|younger| (younger.pc, younger.decoded.instruction())),
    ) else {
        return Ok(None);
    };
    let head_issue_tick = ready_tick.saturating_sub(
        crate::riscv_fu_latency::riscv_execute_wait_cycles(decoded.instruction()),
    );
    let head = O3LiveIssueHeadReservation::for_instruction(
        head_sequence,
        head_issue_tick,
        decoded.instruction(),
    );
    let Some(head_fetch) = fetch_events.iter().find(|event| {
        event.kind() == CpuFetchEventKind::Completed
            && event.request_id() == current_request
            && event.pc() == pc
    }) else {
        return Ok(None);
    };
    let Some(head_instruction) = completed_fetch_instruction_starting_with(
        &state.executed_fetches,
        fetch_events,
        head_fetch,
    ) else {
        return Ok(None);
    };
    if head_instruction.decoded != decoded {
        return Ok(None);
    }
    if !state.o3_runtime.bind_live_staged_fetch_identity(
        pc,
        decoded.instruction(),
        &head_instruction.consumed_requests,
    ) {
        return Ok(None);
    }
    let mut head_hart = state.hart.clone();
    head_hart.set_pc(pc.get());
    let head_execution = head_hart
        .execute_decoded(decoded)
        .map_err(RiscvCpuError::Isa)?;
    let head_execution_key = head_execution.clone();
    if !state
        .o3_runtime
        .record_live_issue_head_execution(head, &head_instruction.consumed_requests, head_execution)
        .map_err(RiscvCpuError::O3Runtime)?
    {
        return Ok(None);
    }
    let admitted_tick = state
        .o3_runtime
        .live_speculative_execution_ready_tick(
            &head_instruction.consumed_requests,
            &head_execution_key,
        )
        .unwrap_or(ready_tick);
    if younger.is_empty() || !state.live_retire_gate.detailed_policy_enabled() {
        return Ok(Some(admitted_tick));
    }
    schedule_o3_live_speculative_younger_executions(state, head, &younger, earliest_tick)?;
    Ok(Some(admitted_tick))
}

pub(crate) fn stage_o3_scalar_memory_younger_window(
    state: &mut RiscvCoreState,
    execution: &RiscvCpuExecutionEvent,
    issue_tick: u64,
    fetch_events: &[CpuFetchEvent],
) {
    if !state.live_retire_gate.detailed_policy_enabled()
        && !state
            .o3_runtime
            .owns_pending_scalar_memory_retirement(execution.fetch().request_id())
    {
        return;
    }
    let row_limit = state
        .o3_runtime
        .scalar_memory_window_limit()
        .min(O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS);
    let Some(window) = state
        .o3_runtime
        .scalar_memory_integer_window(execution.fetch().request_id())
    else {
        return;
    };
    let younger = completed_scalar_integer_younger_window(
        state,
        fetch_events,
        execution.fetch().request_id(),
        Address::new(execution.execution().next_pc()),
        window,
        row_limit.saturating_sub(1),
    );
    let staged_rows = state.o3_runtime.stage_live_scalar_memory_younger_window(
        execution.fetch().request_id(),
        younger
            .iter()
            .map(|younger| (younger.pc, younger.decoded.instruction())),
    );
    let Some(head) = state
        .o3_runtime
        .live_scalar_memory_head_reservation(execution.fetch().request_id())
    else {
        return;
    };
    schedule_o3_live_speculative_younger_executions(
        state,
        head,
        &younger[..staged_rows.min(younger.len())],
        issue_tick,
    )
    .expect("live scalar memory younger writeback reservation");
}

pub(crate) fn wake_o3_scalar_memory_younger_window(
    state: &mut RiscvCoreState,
    issue_tick: u64,
    fetch_events: &[CpuFetchEvent],
) {
    let Some((tail_request, younger_pcs)) =
        state.o3_runtime.live_scalar_memory_younger_wakeup_seed()
    else {
        return;
    };
    let mut current_request = tail_request;
    let mut younger = Vec::with_capacity(younger_pcs.len());
    for pc in younger_pcs {
        let Some(instruction) =
            completed_fetch_instruction_at(state, fetch_events, current_request, pc)
        else {
            return;
        };
        current_request = instruction.last_consumed_request();
        younger.push(instruction);
    }
    let Some(head) = state
        .o3_runtime
        .live_scalar_memory_head_reservation(tail_request)
    else {
        return;
    };
    schedule_o3_live_speculative_younger_executions(state, head, &younger, issue_tick)
        .expect("live scalar memory wake writeback reservation");
}

fn accepted_scalar_integer_younger_window(
    mut window: RiscvScalarIntegerLiveWindow,
    younger: impl IntoIterator<Item = RiscvCompletedFetchInstruction>,
) -> Vec<RiscvCompletedFetchInstruction> {
    let mut accepted = Vec::new();
    for younger in younger {
        match window.classify_younger(younger.decoded.instruction()) {
            RiscvScalarIntegerYoungerDecision::AdmitContinue => accepted.push(younger),
            RiscvScalarIntegerYoungerDecision::AdmitStop
            | RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
            | RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
            | RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl => {
                accepted.push(younger);
                break;
            }
            RiscvScalarIntegerYoungerDecision::Reject => break,
        }
    }
    accepted
}

fn schedule_o3_live_speculative_younger_executions(
    state: &mut RiscvCoreState,
    head: O3LiveIssueHeadReservation,
    younger: &[RiscvCompletedFetchInstruction],
    issue_tick: u64,
) -> Result<(), RiscvCpuError> {
    for younger in younger {
        if !state.o3_runtime.bind_live_staged_fetch_identity(
            younger.pc,
            younger.decoded.instruction(),
            &younger.consumed_requests,
        ) {
            return Ok(());
        }
    }
    let requests = younger
        .iter()
        .map(|younger| {
            O3LiveIssueRequest::new(
                younger.pc,
                younger.consumed_requests.clone(),
                younger.decoded,
            )
        })
        .collect::<Vec<_>>();
    let hart = state.hart.clone();
    state
        .o3_runtime
        .schedule_live_speculative_issues(&hart, head, issue_tick, &requests)
        .map_err(RiscvCpuError::O3Runtime)
}

fn completed_fetch_instruction_window(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    mut current_request: MemoryRequestId,
    mut pc: Address,
    limit: usize,
) -> Vec<RiscvCompletedFetchInstruction> {
    let mut instructions = Vec::new();
    for _ in 0..limit {
        let Some(instruction) =
            completed_fetch_instruction_at(state, fetch_events, current_request, pc)
        else {
            break;
        };
        current_request = *instruction
            .consumed_requests
            .last()
            .expect("completed instruction consumes at least one fetch request");
        pc = Address::new(
            pc.get()
                .wrapping_add(u64::from(instruction.decoded.bytes())),
        );
        instructions.push(instruction);
    }
    instructions
}

fn completed_scalar_integer_younger_window(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    mut current_request: MemoryRequestId,
    mut pc: Address,
    mut window: RiscvScalarIntegerLiveWindow,
    limit: usize,
) -> Vec<RiscvCompletedFetchInstruction> {
    let mut instructions = Vec::new();
    let mut sequenced_return_addresses = Vec::new();
    for _ in 0..limit {
        let Some(instruction) =
            completed_fetch_instruction_at(state, fetch_events, current_request, pc)
        else {
            break;
        };
        let prediction_request = instruction.first_consumed_request();
        let sequential_pc = Address::new(
            instruction
                .pc
                .get()
                .wrapping_add(u64::from(instruction.decoded.bytes())),
        );
        sequenced_return_addresses.push((prediction_request.sequence(), sequential_pc));
        let classification = window.classify_sequenced_younger(
            instruction.decoded.instruction(),
            prediction_request.sequence(),
        );
        let decision = classification.decision();
        if decision == RiscvScalarIntegerYoungerDecision::Reject {
            break;
        }
        current_request = instruction.last_consumed_request();
        let next_pc = if matches!(
            decision,
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
                | RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
        ) {
            let target_authority =
                if decision == RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl {
                    let Some(push_sequence) = classification.ras_push_sequence() else {
                        break;
                    };
                    let Some(pushed_address) =
                        sequenced_return_addresses
                            .iter()
                            .rev()
                            .find_map(|(sequence, address)| {
                                (*sequence == push_sequence).then_some(*address)
                            })
                    else {
                        break;
                    };
                    crate::riscv_fetch_ahead::PredictedControlTargetAuthority::RasRequired {
                        push_sequence,
                        pushed_address,
                    }
                } else {
                    crate::riscv_fetch_ahead::PredictedControlTargetAuthority::Normal
                };
            let crate::riscv_fetch_ahead::RecordedPredictedPc::Ready(next_pc) =
                crate::riscv_fetch_ahead::recorded_predicted_pc(
                    state,
                    prediction_request,
                    sequential_pc,
                    target_authority,
                )
            else {
                break;
            };
            next_pc
        } else {
            sequential_pc
        };
        instructions.push(instruction);
        if matches!(
            decision,
            RiscvScalarIntegerYoungerDecision::AdmitStop
                | RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        ) {
            break;
        }
        pc = next_pc;
    }
    instructions
}

fn completed_fetch_instruction_at(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    current_request: MemoryRequestId,
    pc: Address,
) -> Option<RiscvCompletedFetchInstruction> {
    completed_fetch_instruction_from_events(
        &state.executed_fetches,
        fetch_events,
        current_request,
        pc,
    )
}

pub(crate) fn completed_fetch_instruction_from_events(
    executed_fetches: &BTreeSet<MemoryRequestId>,
    fetch_events: &[CpuFetchEvent],
    current_request: MemoryRequestId,
    pc: Address,
) -> Option<RiscvCompletedFetchInstruction> {
    let event = oldest_completed_fetch_at(executed_fetches, fetch_events, current_request, pc)?;
    completed_fetch_instruction_starting_with(executed_fetches, fetch_events, event)
}

pub(crate) fn completed_fetch_instruction_starting_with(
    executed_fetches: &BTreeSet<MemoryRequestId>,
    fetch_events: &[CpuFetchEvent],
    event: &CpuFetchEvent,
) -> Option<RiscvCompletedFetchInstruction> {
    let pc = event.pc();
    let data = event.data()?;
    let mut consumed_requests = vec![event.request_id()];
    let raw = match data {
        [low, high] if low & 0x3 != 0x3 => u32::from(u16::from_le_bytes([*low, *high])),
        [low, high] => {
            let suffix_pc = Address::new(pc.get().checked_add(2)?);
            let suffix = oldest_completed_fetch_at(
                executed_fetches,
                fetch_events,
                event.request_id(),
                suffix_pc,
            )?;
            let [suffix_low, suffix_high] = suffix.data()? else {
                return None;
            };
            consumed_requests.push(suffix.request_id());
            u32::from_le_bytes([*low, *high, *suffix_low, *suffix_high])
        }
        [a, b, c, d] => u32::from_le_bytes([*a, *b, *c, *d]),
        _ => return None,
    };
    let decoded = RiscvInstruction::decode_with_length(raw).ok()?;
    Some(RiscvCompletedFetchInstruction {
        pc,
        consumed_requests,
        decoded,
    })
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::{
        Immediate, MemoryAccessKind, MemoryWidth, Register, RiscvExecutionRecord,
    };
    use rem6_kernel::{PartitionId, PartitionedScheduler};
    use rem6_memory::{AccessSize, AgentId};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    use super::*;
    use crate::{
        o3_runtime::O3LiveWritebackReady, riscv_live_retire_gate::RiscvLiveRetireGatePolicy,
        CacheLineLayout, CpuCore, CpuFetchConfig, CpuFetchRecord, CpuId, CpuResetState,
    };

    #[test]
    fn live_younger_selection_matches_oldest_retirement_request_order() {
        let current = MemoryRequestId::new(AgentId::new(7), 10);
        let pc = Address::new(0x8004);
        let events = vec![
            completed_fetch(7, 12, pc),
            completed_fetch(8, 11, pc),
            completed_fetch(7, 9, pc),
            completed_fetch(7, 11, pc),
        ];
        let mut executed = BTreeSet::new();

        let selected = oldest_completed_fetch_at(&executed, &events, current, pc).unwrap();
        assert_eq!(selected.request_id().sequence(), 11);
        assert_eq!(selected.request_id().agent(), current.agent());

        executed.insert(selected.request_id());
        let next = oldest_completed_fetch_at(&executed, &events, current, pc).unwrap();
        assert_eq!(next.request_id().sequence(), 12);
    }

    #[test]
    fn live_younger_selection_assembles_split_word_fetch() {
        let current = MemoryRequestId::new(AgentId::new(7), 10);
        let pc = Address::new(0x800e);
        let raw = 0x0090_0213_u32;
        let bytes = raw.to_le_bytes();
        let events = vec![
            completed_fetch_with_data(7, 11, pc, bytes[..2].to_vec()),
            completed_fetch_with_data(7, 12, Address::new(pc.get() + 2), bytes[2..].to_vec()),
        ];

        let selected =
            completed_fetch_instruction_from_events(&BTreeSet::new(), &events, current, pc)
                .unwrap();

        assert_eq!(selected.consumed_requests[0].sequence(), 11);
        assert_eq!(
            selected.consumed_requests,
            vec![
                MemoryRequestId::new(AgentId::new(7), 11),
                MemoryRequestId::new(AgentId::new(7), 12),
            ]
        );
        assert_eq!(
            selected.decoded.instruction(),
            RiscvInstruction::decode(raw).unwrap()
        );
        assert_eq!(selected.decoded.bytes(), 4);
    }

    #[test]
    fn live_younger_window_collects_two_contiguous_instructions() {
        let current = MemoryRequestId::new(AgentId::new(7), 10);
        let first_pc = Address::new(0x8004);
        let events = vec![
            completed_fetch_with_data(7, 11, first_pc, 0x0050_0213_u32.to_le_bytes().to_vec()),
            completed_fetch_with_data(
                7,
                12,
                Address::new(0x8008),
                0x00b2_0293_u32.to_le_bytes().to_vec(),
            ),
        ];
        let state = RiscvCoreState::new(0x8000, 0);

        let window = completed_fetch_instruction_window(&state, &events, current, first_pc, 2);

        assert_eq!(window.len(), 2);
        assert_eq!(window[0].pc, Address::new(0x8004));
        assert_eq!(window[1].pc, Address::new(0x8008));
        assert_eq!(window[0].consumed_requests, vec![request(7, 11)]);
        assert_eq!(window[1].consumed_requests, vec![request(7, 12)]);
    }

    #[test]
    fn live_retire_replay_stops_before_return_without_recorded_ras_lineage() {
        let load = i_type(0, 2, 0x2, 6, 0x03);
        let call = j_type(8, 1);
        let return_jump = i_type(0, 1, 0x0, 0, 0x67);
        let descendant = i_type(1, 0, 0x0, 7, 0x13);
        let core = test_core();
        {
            let mut core_state = core.core.state.lock().expect("cpu core lock");
            for (sequence, pc, raw) in [
                (0, 0x8000, load),
                (1, 0x8004, call),
                (2, 0x800c, return_jump),
                (3, 0x8008, descendant),
            ] {
                core_state.events.push(completed_fetch_with_data(
                    7,
                    sequence,
                    Address::new(pc),
                    raw.to_le_bytes().to_vec(),
                ));
            }
        }
        core.set_detailed_live_retire_gate_enabled(true);
        core.set_o3_scalar_memory_depth(4);
        core.set_branch_lookahead(2);

        let call_decision = core.next_fetch_ahead_before_retire().unwrap();
        core.record_prepared_fetch_ahead_speculation(
            core.prepare_fetch_ahead_speculation(&call_decision)
                .unwrap(),
        );
        let return_decision = core.next_fetch_ahead_before_retire().unwrap();
        let return_sequence = 2;
        core.record_prepared_fetch_ahead_speculation(
            core.prepare_fetch_ahead_speculation(&return_decision)
                .unwrap(),
        );
        {
            let mut state = core.state.lock().expect("riscv core lock");
            state
                .squash_return_address_stack_speculation(return_sequence)
                .unwrap();
        }

        let fetch_events = core.core.fetch_events();
        let state = core.state.lock().expect("riscv core lock");
        let window = RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
            [Register::new(6).unwrap()],
            1,
            4,
        )
        .unwrap();
        let replayed = completed_scalar_integer_younger_window(
            &state,
            &fetch_events,
            request(7, 0),
            Address::new(0x8004),
            window,
            3,
        );

        assert_eq!(
            replayed
                .iter()
                .map(RiscvCompletedFetchInstruction::pc)
                .collect::<Vec<_>>(),
            [Address::new(0x8004)]
        );
    }

    #[test]
    fn live_retire_staging_collects_three_younger_scalar_alus() {
        let current = request(7, 10);
        let events = vec![
            completed_fetch_with_data(
                7,
                11,
                Address::new(0x8004),
                0x0050_0213_u32.to_le_bytes().to_vec(),
            ),
            completed_fetch_with_data(
                7,
                12,
                Address::new(0x8008),
                0x00b2_0293_u32.to_le_bytes().to_vec(),
            ),
            completed_fetch_with_data(
                7,
                13,
                Address::new(0x800c),
                0x0052_0333_u32.to_le_bytes().to_vec(),
            ),
        ];
        let mut state = RiscvCoreState::new(0x8000, 0);

        stage_o3_live_retire_window(
            &mut state,
            current,
            Address::new(0x8000),
            0x0220_c1b3,
            10,
            29,
            &events,
        )
        .unwrap();

        assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 4);
        assert_eq!(
            state
                .o3_runtime
                .snapshot()
                .reorder_buffer()
                .iter()
                .map(|entry| entry.pc())
                .collect::<Vec<_>>(),
            [0x8000, 0x8004, 0x8008, 0x800c]
                .into_iter()
                .map(Address::new)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn live_retire_staging_respects_scalar_fu_younger_boundaries() {
        for (boundary, label, stages_boundary) in [
            (0x00b1_8293_u32, "unshadowed head dependency", true),
            (0x0000_0073_u32, "unsupported system instruction", false),
            (0x0000_0013_u32, "zero destination", false),
        ] {
            let current = request(7, 10);
            let events = vec![
                completed_fetch_with_data(
                    7,
                    11,
                    Address::new(0x8004),
                    0x0050_0213_u32.to_le_bytes().to_vec(),
                ),
                completed_fetch_with_data(
                    7,
                    12,
                    Address::new(0x8008),
                    boundary.to_le_bytes().to_vec(),
                ),
                completed_fetch_with_data(
                    7,
                    13,
                    Address::new(0x800c),
                    0x0010_0313_u32.to_le_bytes().to_vec(),
                ),
            ];
            let mut state = RiscvCoreState::new(0x8000, 0);

            stage_o3_live_retire_window(
                &mut state,
                current,
                Address::new(0x8000),
                0x0220_c1b3,
                10,
                29,
                &events,
            )
            .unwrap();

            let mut expected = vec![Address::new(0x8000), Address::new(0x8004)];
            if stages_boundary {
                expected.push(Address::new(0x8008));
            }
            assert_eq!(
                state
                    .o3_runtime
                    .snapshot()
                    .reorder_buffer()
                    .iter()
                    .map(|entry| entry.pc())
                    .collect::<Vec<_>>(),
                expected,
                "{label} should terminate the live staging window"
            );
        }
    }

    #[test]
    fn live_retire_gate_arms_fixed_fu_admitted_tick() {
        for (kind, label) in [
            (RiscvLiveRetireGateWakeKind::Serial, "serial"),
            (RiscvLiveRetireGateWakeKind::Parallel, "parallel"),
        ] {
            let core = test_core();
            let mut state = RiscvCoreState::new(0x8000, 0);
            state
                .live_retire_gate
                .set_policy(RiscvLiveRetireGatePolicy::detailed());
            assert!(state.o3_runtime.set_writeback_width(1));
            state.hart.write(Register::new(1).unwrap(), 6);
            state.hart.write(Register::new(2).unwrap(), 7);
            state
                .o3_runtime
                .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(99, 12)])
                .unwrap();
            let raw = r_type(1, 2, 1, 0x0, 3, 0x33);
            let events = vec![completed_fetch_with_data(
                7,
                10,
                Address::new(0x8000),
                raw.to_le_bytes().to_vec(),
            )];
            let window = RiscvLiveRetireWindowRequest::new(
                request(7, 10),
                Address::new(0x8000),
                raw,
                10,
                &events,
            );
            let mut scheduler = PartitionedScheduler::new(1).unwrap();
            let mut gate_scheduler = Some((&mut scheduler, kind));

            let retire_tick = core
                .live_retire_gate_retire_tick(&mut state, &mut gate_scheduler, window)
                .unwrap();

            assert_eq!(
                retire_tick, None,
                "{label} path should schedule a gate wake"
            );
            assert_eq!(
                state.live_retire_gate.pending_ready_tick(),
                Some(13),
                "{label} path must arm the admitted writeback tick, not the raw FU tick"
            );
            let wakes = state.live_retire_gate.owned_scheduler_wakes();
            assert_eq!(wakes.len(), 1, "{label} path should own one gate wake");
            assert_eq!(
                wakes[0].tick(),
                13,
                "{label} path must not leave behind a transient raw-tick wake"
            );
            let head = state.o3_runtime.writeback_reservation(0).unwrap();
            assert_eq!(head.raw_ready_tick(), 12);
            assert_eq!(head.admitted_tick(), 13);
            assert_eq!(head.slot(), 0);
            assert_eq!(
                state.o3_runtime.snapshot().reorder_buffer()[0].ready_tick(),
                13,
                "{label} path should stage the head ROB row at the admitted tick"
            );
        }
    }

    #[test]
    fn scalar_load_head_staging_collects_three_younger_scalar_alus() {
        let execution = scalar_load_execution(7, 10, 12, 2, 0x9000);
        let events = vec![
            completed_fetch_with_data(
                7,
                11,
                Address::new(0x8004),
                0x0050_0693_u32.to_le_bytes().to_vec(),
            ),
            completed_fetch_with_data(
                7,
                12,
                Address::new(0x8008),
                0x00b6_8713_u32.to_le_bytes().to_vec(),
            ),
            completed_fetch_with_data(
                7,
                13,
                Address::new(0x800c),
                0x00e6_87b3_u32.to_le_bytes().to_vec(),
            ),
        ];
        let mut state = RiscvCoreState::new(0x8000, 0);
        state
            .live_retire_gate
            .set_policy(RiscvLiveRetireGatePolicy::detailed());
        state.o3_runtime.set_scalar_memory_window_limit(4);
        assert!(state
            .o3_runtime
            .stage_live_scalar_memory_issue(&execution, request(7, 20), 31));

        stage_o3_scalar_memory_younger_window(&mut state, &execution, 10, &events);

        assert_eq!(
            state.o3_runtime.snapshot().reorder_buffer().len(),
            4,
            "staged snapshot: {:?}",
            state.o3_runtime.snapshot()
        );
        assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 1);
        assert_eq!(
            state
                .o3_runtime
                .snapshot()
                .reorder_buffer()
                .iter()
                .map(|entry| entry.pc())
                .collect::<Vec<_>>(),
            [0x8000, 0x8004, 0x8008, 0x800c]
                .into_iter()
                .map(Address::new)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn scalar_load_head_staging_stops_before_unpredicted_nested_control() {
        let execution = scalar_load_execution(7, 10, 12, 2, 0x9000);
        let events = vec![
            completed_fetch_with_data(
                7,
                11,
                Address::new(0x8004),
                0x0062_8c63_u32.to_le_bytes().to_vec(),
            ),
            completed_fetch_with_data(
                7,
                12,
                Address::new(0x8008),
                0x0083_8863_u32.to_le_bytes().to_vec(),
            ),
            completed_fetch_with_data(
                7,
                13,
                Address::new(0x800c),
                0x0010_0313_u32.to_le_bytes().to_vec(),
            ),
        ];
        let mut state = RiscvCoreState::new(0x8000, 0);
        state
            .live_retire_gate
            .set_policy(RiscvLiveRetireGatePolicy::detailed());
        state.o3_runtime.set_scalar_memory_window_limit(4);
        let prediction = state.branch_predictor.predict_speculative_with_prediction(
            Address::new(0x8004),
            false,
            None,
        );
        state.branch_speculations.insert(11, prediction.id());
        assert!(state
            .o3_runtime
            .stage_live_scalar_memory_issue(&execution, request(7, 20), 31));

        stage_o3_scalar_memory_younger_window(&mut state, &execution, 10, &events);

        assert_eq!(
            state
                .o3_runtime
                .snapshot()
                .reorder_buffer()
                .iter()
                .map(|entry| entry.pc())
                .collect::<Vec<_>>(),
            [Address::new(0x8000), Address::new(0x8004)]
        );
    }

    #[test]
    fn split_suffix_replacement_does_not_reuse_speculative_fu_readiness() {
        let mut state = RiscvCoreState::new(0x8004, 0);
        let head = RiscvInstruction::Addi {
            rd: Register::new(3).unwrap(),
            rs1: Register::new(0).unwrap(),
            imm: Immediate::new(1),
        };
        let multiply = RiscvInstruction::Mul {
            rd: Register::new(7).unwrap(),
            rs1: Register::new(1).unwrap(),
            rs2: Register::new(2).unwrap(),
        };
        state.o3_runtime.stage_live_retire_window(
            Address::new(0x8000),
            head,
            0,
            Some((Address::new(0x8004), multiply)),
        );
        let candidate = state
            .o3_runtime
            .live_speculative_issue_candidate(Address::new(0x8004), multiply)
            .unwrap();
        state
            .o3_runtime
            .record_live_speculative_execution(
                candidate,
                &[request(7, 11), request(7, 12)],
                10,
                RiscvExecutionRecord::new(
                    multiply,
                    0x8004,
                    0x8008,
                    vec![rem6_isa_riscv::RegisterWrite::new(
                        Register::new(7).unwrap(),
                        42,
                    )],
                    None,
                ),
            )
            .unwrap();
        state.hart.write(Register::new(1).unwrap(), 6);
        state.hart.write(Register::new(2).unwrap(), 7);
        let raw = 0x0220_83b3_u32;
        let bytes = raw.to_le_bytes();
        let events = vec![
            completed_fetch_with_data(7, 11, Address::new(0x8004), bytes[..2].to_vec()),
            completed_fetch_with_data(7, 13, Address::new(0x8006), bytes[2..].to_vec()),
        ];
        let window = RiscvLiveRetireWindowRequest::new(
            request(7, 11),
            Address::new(0x8004),
            raw,
            11,
            &events,
        );

        assert_eq!(
            live_speculative_fu_ready_tick(&state, &window).unwrap(),
            None
        );
    }

    fn request(agent: u32, sequence: u64) -> MemoryRequestId {
        MemoryRequestId::new(AgentId::new(agent), sequence)
    }

    fn completed_fetch(agent: u32, sequence: u64, pc: Address) -> CpuFetchEvent {
        completed_fetch_with_data(agent, sequence, pc, 0x0000_0013_u32.to_le_bytes().to_vec())
    }

    fn completed_fetch_with_data(
        agent: u32,
        sequence: u64,
        pc: Address,
        data: Vec<u8>,
    ) -> CpuFetchEvent {
        let size = AccessSize::new(data.len() as u64).unwrap();
        CpuFetchEvent::completed(
            CpuFetchRecord::new(
                0,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                MemoryRequestId::new(AgentId::new(agent), sequence),
                pc,
                size,
            ),
            data,
        )
    }

    fn test_core() -> RiscvCore {
        RiscvCore::new(
            CpuCore::new(
                CpuResetState::new(
                    CpuId::new(0),
                    PartitionId::new(0),
                    AgentId::new(7),
                    Address::new(0x8000),
                ),
                CpuFetchConfig::new(
                    TransportEndpointId::new("cpu0.ifetch").unwrap(),
                    MemoryRouteId::new(0),
                    CacheLineLayout::new(16).unwrap(),
                    AccessSize::new(4).unwrap(),
                ),
            )
            .unwrap(),
        )
    }

    fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
        (funct7 << 25)
            | (u32::from(rs2) << 20)
            | (u32::from(rs1) << 15)
            | (funct3 << 12)
            | (u32::from(rd) << 7)
            | opcode
    }

    fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
        (((imm as u32) & 0x0fff) << 20)
            | (u32::from(rs1) << 15)
            | (funct3 << 12)
            | (u32::from(rd) << 7)
            | opcode
    }

    fn j_type(offset: i32, rd: u8) -> u32 {
        let imm = offset as u32;
        ((imm & 0x0010_0000) << 11)
            | (imm & 0x000f_f000)
            | ((imm & 0x0000_0800) << 9)
            | ((imm & 0x0000_07fe) << 20)
            | (u32::from(rd) << 7)
            | 0x6f
    }

    fn scalar_load_execution(
        agent: u32,
        sequence: u64,
        rd: u8,
        rs1: u8,
        address: u64,
    ) -> RiscvCpuExecutionEvent {
        let instruction = RiscvInstruction::Load {
            rd: Register::new(rd).unwrap(),
            rs1: Register::new(rs1).unwrap(),
            offset: Immediate::new(0),
            width: MemoryWidth::Word,
            signed: false,
        };
        RiscvCpuExecutionEvent::new(
            completed_fetch_with_data(
                agent,
                sequence,
                Address::new(0x8000),
                0x0001_2603_u32.to_le_bytes().to_vec(),
            ),
            instruction,
            RiscvExecutionRecord::new(
                instruction,
                0x8000,
                0x8004,
                Vec::new(),
                Some(MemoryAccessKind::Load {
                    rd: Register::new(rd).unwrap(),
                    address,
                    width: MemoryWidth::Word,
                    signed: false,
                }),
            ),
        )
    }
}

use std::collections::BTreeSet;

use rem6_isa_riscv::{RiscvDecodedInstruction, RiscvInstruction};
use rem6_kernel::PartitionedScheduler;
use rem6_memory::{Address, MemoryRequestId};

use crate::{
    o3_fu_latency::o3_fu_latency_class,
    o3_runtime_trace::O3RuntimeFuLatencyClass,
    riscv_execute::{oldest_completed_fetch_at, RiscvLiveRetireGateWakeKind},
    riscv_live_retire_gate::RiscvLiveRetireGateDecision,
    CpuFetchEvent, RiscvCore, RiscvCoreState, RiscvCpuError, RiscvCpuExecutionEvent,
};

const MAX_LIVE_RETIRE_YOUNGER_INSTRUCTIONS: usize = 2;

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
        let Some((scheduler, kind)) = gate_scheduler.as_mut() else {
            return Ok(
                (!state.live_retire_gate.blocks_without_scheduler()).then_some(window.fetch_tick)
            );
        };
        let now = scheduler
            .partition_now(self.partition())
            .map_err(RiscvCpuError::Scheduler)?;
        let ready_base_tick = now.max(window.fetch_tick);
        match state.live_retire_gate.before_retire(
            window.request,
            window.raw,
            now,
            ready_base_tick,
        )? {
            RiscvLiveRetireGateDecision::Ready => Ok(Some(now)),
            RiscvLiveRetireGateDecision::Blocked => {
                let ready_tick = state
                    .live_retire_gate
                    .pending_ready_tick()
                    .expect("blocked live retire gate has a pending ready tick");
                stage_o3_live_retire_window(
                    state,
                    window.request,
                    window.pc,
                    window.raw,
                    now,
                    ready_tick,
                    window.fetch_events,
                )?;
                Ok(None)
            }
            RiscvLiveRetireGateDecision::Schedule {
                ready_tick,
                created_wait_ticks,
            } => {
                match *kind {
                    RiscvLiveRetireGateWakeKind::Serial => scheduler
                        .schedule_at(self.partition(), ready_tick, |_| {})
                        .map_err(RiscvCpuError::Scheduler)?,
                    RiscvLiveRetireGateWakeKind::Parallel => scheduler
                        .schedule_parallel_at(self.partition(), ready_tick, |_| {})
                        .map_err(RiscvCpuError::Scheduler)?,
                };
                state.live_retire_gate.mark_scheduled(window.request);
                if let Some(wait_ticks) = created_wait_ticks {
                    state.o3_runtime.record_live_retire_gate_wait(wait_ticks);
                }
                stage_o3_live_retire_window(
                    state,
                    window.request,
                    window.pc,
                    window.raw,
                    now,
                    ready_tick,
                    window.fetch_events,
                )?;
                Ok(None)
            }
        }
    }
}

fn stage_o3_live_retire_window(
    state: &mut RiscvCoreState,
    current_request: MemoryRequestId,
    pc: Address,
    raw: u32,
    issue_tick: u64,
    ready_tick: u64,
    fetch_events: &[CpuFetchEvent],
) -> Result<(), RiscvCpuError> {
    let decoded = RiscvInstruction::decode_with_length(raw).map_err(RiscvCpuError::Isa)?;
    let next_pc = Address::new(pc.get().wrapping_add(u64::from(decoded.bytes())));
    let younger = completed_fetch_instruction_window(
        state,
        fetch_events,
        current_request,
        next_pc,
        MAX_LIVE_RETIRE_YOUNGER_INSTRUCTIONS,
    );
    state.o3_runtime.stage_live_retire_window(
        pc,
        decoded.instruction(),
        ready_tick,
        younger
            .iter()
            .map(|younger| (younger.pc, younger.decoded.instruction())),
    );
    if younger.is_empty() {
        return Ok(());
    }
    if !matches!(
        o3_fu_latency_class(decoded.instruction()),
        Some(O3RuntimeFuLatencyClass::ScalarIntegerMul | O3RuntimeFuLatencyClass::ScalarIntegerDiv)
    ) || !state.live_retire_gate.detailed_policy_enabled()
    {
        return Ok(());
    }
    record_o3_live_speculative_younger_executions(state, &younger, issue_tick);
    Ok(())
}

pub(crate) fn stage_o3_scalar_memory_younger_window(
    state: &mut RiscvCoreState,
    execution: &RiscvCpuExecutionEvent,
    issue_tick: u64,
    fetch_events: &[CpuFetchEvent],
) {
    if !state.live_retire_gate.detailed_policy_enabled() {
        return;
    }
    let younger = completed_fetch_instruction_window(
        state,
        fetch_events,
        execution.fetch().request_id(),
        Address::new(execution.execution().next_pc()),
        1,
    );
    state.o3_runtime.stage_live_scalar_memory_younger_window(
        execution.fetch().request_id(),
        younger
            .iter()
            .map(|younger| (younger.pc, younger.decoded.instruction())),
    );
    record_o3_live_speculative_younger_executions(state, &younger, issue_tick);
}

fn record_o3_live_speculative_younger_executions(
    state: &mut RiscvCoreState,
    younger: &[RiscvCompletedFetchInstruction],
    issue_tick: u64,
) {
    for younger in younger {
        let Some(candidate) = state
            .o3_runtime
            .live_speculative_issue_candidate(younger.pc, younger.decoded.instruction())
        else {
            continue;
        };

        let mut speculative_hart = state.hart.clone();
        for write in candidate.forwarded_register_writes() {
            speculative_hart.write(write.register(), write.value());
        }
        speculative_hart.set_pc(younger.pc.get());
        let Ok(speculative_execution) = speculative_hart.execute_decoded(younger.decoded) else {
            continue;
        };
        state.o3_runtime.record_live_speculative_execution(
            candidate,
            &younger.consumed_requests,
            issue_tick,
            speculative_execution,
        );
    }
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
    use rem6_kernel::PartitionId;
    use rem6_memory::{AccessSize, AgentId};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    use super::*;
    use crate::CpuFetchRecord;

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
}

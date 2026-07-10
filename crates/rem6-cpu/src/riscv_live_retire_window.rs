use rem6_isa_riscv::RiscvInstruction;
use rem6_kernel::PartitionedScheduler;
use rem6_memory::{Address, MemoryRequestId};

use crate::{
    riscv_execute::RiscvLiveRetireGateWakeKind,
    riscv_live_retire_gate::RiscvLiveRetireGateDecision, CpuFetchEvent, CpuFetchEventKind,
    RiscvCore, RiscvCoreState, RiscvCpuError,
};

pub(super) struct RiscvLiveRetireWindowRequest<'a> {
    request: MemoryRequestId,
    pc: Address,
    raw: u32,
    fetch_tick: u64,
    fetch_events: &'a [CpuFetchEvent],
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
                    window.pc,
                    window.raw,
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
                    window.pc,
                    window.raw,
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
    pc: Address,
    raw: u32,
    ready_tick: u64,
    fetch_events: &[CpuFetchEvent],
) -> Result<(), RiscvCpuError> {
    let decoded = RiscvInstruction::decode_with_length(raw).map_err(RiscvCpuError::Isa)?;
    let next_pc = Address::new(pc.get().wrapping_add(u64::from(decoded.bytes())));
    let younger = completed_fetch_instruction_at(state, fetch_events, next_pc);
    state.o3_runtime.stage_live_retire_window(
        pc,
        decoded.instruction(),
        ready_tick,
        younger.map(|instruction| (next_pc, instruction)),
    );
    Ok(())
}

fn completed_fetch_instruction_at(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    pc: Address,
) -> Option<RiscvInstruction> {
    let event = fetch_events.iter().find(|event| {
        event.kind() == CpuFetchEventKind::Completed
            && event.pc() == pc
            && !state.executed_fetches.contains(&event.request_id())
    })?;
    let data = event.data()?;
    let raw = match data {
        [low, high] if low & 0x3 != 0x3 => u32::from(u16::from_le_bytes([*low, *high])),
        [a, b, c, d] => u32::from_le_bytes([*a, *b, *c, *d]),
        _ => return None,
    };
    RiscvInstruction::decode_with_length(raw)
        .ok()
        .map(|decoded| decoded.instruction())
}

use std::collections::BTreeSet;

use rem6_isa_riscv::{
    Register, RiscvDecodedInstruction, RiscvExecutionRecord, RiscvInstruction,
    RiscvVectorMemoryInstruction,
};
use rem6_kernel::PartitionedScheduler;
use rem6_memory::{AccessSize, Address, MemoryRequestId};

use crate::{
    o3_runtime::{
        o3_memory_result_destination, o3_scalar_integer_source_registers,
        O3LiveIssueHeadReservation, O3RuntimeError,
    },
    riscv_execute::{oldest_completed_fetch_at, RiscvLiveRetireGateWakeKind},
    riscv_live_retire_gate::{RiscvLiveRetireGateDecision, RiscvLiveRetireGateWake},
    riscv_o3_window_policy::{
        RiscvScalarIntegerLiveWindow, RiscvScalarIntegerYoungerDecision,
        O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS,
    },
    CpuFetchEvent, CpuFetchEventKind, CpuFetchRecord, RiscvCore, RiscvCoreState, RiscvCpuError,
    RiscvCpuExecutionEvent,
};

mod dependent_result_address;
mod producer_forwarded_descendant;

pub(crate) use producer_forwarded_descendant::{
    stage_o3_producer_forwarded_control_descendant,
    stage_o3_producer_forwarded_control_descendant_for_response,
};

pub(super) struct RiscvLiveRetireWindowRequest<'a> {
    request: MemoryRequestId,
    pc: Address,
    raw: u32,
    fetch_tick: u64,
    fetch_events: &'a [CpuFetchEvent],
}

pub(crate) struct RiscvCompletedFetchInstruction {
    fetch: CpuFetchEvent,
    pc: Address,
    consumed_requests: Vec<MemoryRequestId>,
    decoded: RiscvDecodedInstruction,
}

impl RiscvCompletedFetchInstruction {
    pub(crate) const fn fetch(&self) -> &CpuFetchEvent {
        &self.fetch
    }

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

    pub(crate) fn consumed_requests(&self) -> &[MemoryRequestId] {
        &self.consumed_requests
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiscvPendingTerminalMemoryResult {
    predecessor_pc: Address,
    predecessor_instruction: RiscvInstruction,
    predecessor_consumed_requests: Vec<MemoryRequestId>,
    execution: RiscvCpuExecutionEvent,
    consumed_requests: Vec<MemoryRequestId>,
    decoded: RiscvDecodedInstruction,
    issue_wake_generation: u64,
    issue_ready: bool,
}

impl RiscvPendingTerminalMemoryResult {
    fn new(
        predecessor: &RiscvCompletedFetchInstruction,
        execution: RiscvCpuExecutionEvent,
        consumed_requests: Vec<MemoryRequestId>,
        decoded: RiscvDecodedInstruction,
        issue_wake_generation: u64,
    ) -> Self {
        Self {
            predecessor_pc: predecessor.pc(),
            predecessor_instruction: predecessor.decoded().instruction(),
            predecessor_consumed_requests: predecessor.consumed_requests().to_vec(),
            execution,
            consumed_requests,
            decoded,
            issue_wake_generation,
            issue_ready: false,
        }
    }

    pub(crate) const fn execution(&self) -> &RiscvCpuExecutionEvent {
        &self.execution
    }

    pub(crate) fn execution_mut(&mut self) -> &mut RiscvCpuExecutionEvent {
        &mut self.execution
    }

    pub(crate) fn owns_fetch(&self, fetch_request: MemoryRequestId) -> bool {
        self.execution.fetch().request_id() == fetch_request
    }

    pub(crate) const fn issue_ready(&self) -> bool {
        self.issue_ready
    }

    fn mark_issue_ready(&mut self) {
        self.issue_ready = true;
    }

    fn owns_issue_wake(&self, fetch: MemoryRequestId, generation: u64) -> bool {
        self.owns_fetch(fetch) && self.issue_wake_generation == generation
    }

    fn issue_wake_identity(&self) -> (MemoryRequestId, u64) {
        (
            self.execution.fetch().request_id(),
            self.issue_wake_generation,
        )
    }

    pub(crate) fn follows(
        &self,
        predecessor: &RiscvCpuExecutionEvent,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        self.predecessor_pc == predecessor.fetch_pc()
            && self.predecessor_instruction == predecessor.instruction()
            && self.predecessor_consumed_requests == consumed_requests
    }

    fn has_predecessor(
        &self,
        request: MemoryRequestId,
        pc: Address,
        instruction: RiscvInstruction,
    ) -> bool {
        self.predecessor_pc == pc
            && self.predecessor_instruction == instruction
            && self.predecessor_consumed_requests.first().copied() == Some(request)
    }

    pub(crate) const fn decoded(&self) -> RiscvDecodedInstruction {
        self.decoded
    }

    pub(crate) fn consumed_requests(&self) -> &[MemoryRequestId] {
        &self.consumed_requests
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
        if state.o3_runtime.has_pending_data_address()
            && crate::riscv_fetch_ahead::hart_has_enabled_pending_interrupt(&state.hart)
        {
            let retire_tick = match gate_scheduler.as_mut() {
                Some((scheduler, _)) => scheduler
                    .partition_now(self.partition())
                    .map_err(RiscvCpuError::Scheduler)?,
                None => window.fetch_tick,
            };
            return Ok(Some(retire_tick));
        }
        if detailed_scalar_memory_blocks_execution(state, &window)? {
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
                if let Some(identity) =
                    provision_terminal_memory_result_behind_live_fu(state, &window)?
                {
                    self.schedule_terminal_memory_result_issue_wake(
                        state, scheduler, *kind, now, identity,
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
                if let Some(identity) =
                    provision_terminal_memory_result_behind_live_fu(state, &window)?
                {
                    self.schedule_terminal_memory_result_issue_wake(
                        state, scheduler, *kind, now, identity,
                    )?;
                }
                Ok(None)
            }
        }
    }

    fn schedule_terminal_memory_result_issue_wake(
        &self,
        state: &mut RiscvCoreState,
        scheduler: &mut PartitionedScheduler,
        kind: RiscvLiveRetireGateWakeKind,
        now: u64,
        identity: (MemoryRequestId, u64),
    ) -> Result<(), RiscvCpuError> {
        let Some(issue_tick) = now.checked_add(2) else {
            rollback_terminal_memory_result_provision(state, identity);
            return Err(RiscvCpuError::Scheduler(
                rem6_kernel::SchedulerError::TickOverflow { now, delay: 2 },
            ));
        };
        let core = self.clone();
        let scheduled = match kind {
            RiscvLiveRetireGateWakeKind::Serial => scheduler
                .schedule_at(self.partition(), issue_tick, move |_| {
                    core.mark_terminal_memory_result_issue_ready(identity);
                })
                .map(|_| ()),
            RiscvLiveRetireGateWakeKind::Parallel => scheduler
                .schedule_parallel_at(self.partition(), issue_tick, move |_| {
                    core.mark_terminal_memory_result_issue_ready(identity);
                })
                .map(|_| ()),
        };
        if let Err(error) = scheduled {
            rollback_terminal_memory_result_provision(state, identity);
            return Err(RiscvCpuError::Scheduler(error));
        }
        Ok(())
    }

    fn mark_terminal_memory_result_issue_ready(&self, identity: (MemoryRequestId, u64)) {
        let mut state = self.state.lock().expect("riscv core lock");
        if let Some(pending) = state
            .pending_terminal_memory_result
            .as_mut()
            .filter(|pending| pending.owns_issue_wake(identity.0, identity.1))
        {
            pending.mark_issue_ready();
        }
    }
}

fn rollback_terminal_memory_result_provision(
    state: &mut RiscvCoreState,
    identity: (MemoryRequestId, u64),
) {
    if !state
        .pending_terminal_memory_result
        .as_ref()
        .is_some_and(|pending| pending.owns_issue_wake(identity.0, identity.1))
    {
        return;
    }
    assert!(
        state.abort_deferred_o3_live_data_access_execution(identity.0),
        "terminal result wake rollback owns deferred data access"
    );
}

fn provision_terminal_memory_result_behind_live_fu(
    state: &mut RiscvCoreState,
    window: &RiscvLiveRetireWindowRequest<'_>,
) -> Result<Option<(MemoryRequestId, u64)>, RiscvCpuError> {
    if state.pending_terminal_memory_result.is_some()
        || state.data_translation.is_some()
        || state.pending_trap.is_some()
        || state.hart.machine_interrupt_enable() != 0
        || state.live_retire_gate.pending_ready_tick().is_none()
        || !state.live_retire_gate.detailed_policy_enabled()
        || !state.o3_runtime.has_live_retirement_authority()
        || state.o3_runtime.has_live_data_access()
        || !state.outstanding_data.is_empty()
        || !state.pending_data_translations.is_empty()
        || !state.ready_translated_data.is_empty()
    {
        return Ok(None);
    }
    let decoded = RiscvInstruction::decode_with_length(window.raw).map_err(RiscvCpuError::Isa)?;
    if crate::riscv_fu_latency::riscv_execute_wait_cycles(decoded.instruction()) == 0 {
        return Ok(None);
    }
    let Some(head_fetch) = window.fetch_events.iter().find(|event| {
        event.kind() == CpuFetchEventKind::Completed
            && event.request_id() == window.request
            && event.pc() == window.pc
    }) else {
        return Ok(None);
    };
    let Some(head) = completed_fetch_instruction_starting_with(
        &state.executed_fetches,
        window.fetch_events,
        head_fetch,
    ) else {
        return Ok(None);
    };
    if head.decoded() != decoded {
        return Ok(None);
    }
    let Some((_, head_execution)) = live_speculative_fu(state, window)? else {
        return Ok(None);
    };
    let successor_pc = Address::new(
        head.pc()
            .get()
            .wrapping_add(u64::from(head.decoded().bytes())),
    );
    let Some(successor) = completed_fetch_instruction_at(
        state,
        window.fetch_events,
        head.last_consumed_request(),
        successor_pc,
    ) else {
        return Ok(None);
    };
    if crate::riscv_fu_latency::riscv_pipeline_fu_writes_vector_state(head.decoded().instruction())
        && matches!(
            successor.decoded().instruction(),
            RiscvInstruction::VectorMemory(_)
        )
    {
        return Ok(None);
    }
    if head_execution.register_writes().iter().any(|write| {
        !write.register().is_zero()
            && terminal_memory_result_reads_register(
                successor.decoded().instruction(),
                write.register(),
            )
    }) {
        return Ok(None);
    }

    let mut hart = state.hart.clone();
    hart.set_pc(successor.pc().get());
    let execution = hart
        .execute_decoded(successor.decoded())
        .map_err(RiscvCpuError::Isa)?;
    let sequential_next_pc = successor
        .pc()
        .get()
        .wrapping_add(u64::from(successor.decoded().bytes()));
    if execution.trap().is_some()
        || execution.system_event().is_some()
        || execution.next_pc() != sequential_next_pc
        || execution
            .memory_access()
            .and_then(o3_memory_result_destination)
            .is_none()
    {
        return Ok(None);
    }
    let event = RiscvCpuExecutionEvent::new(
        successor.fetch().clone(),
        successor.decoded().instruction(),
        execution,
    );
    if !state.o3_runtime.defer_live_data_access_execution(&event) {
        return Ok(None);
    }
    let generation = state.next_terminal_memory_result_issue_wake_generation;
    state.next_terminal_memory_result_issue_wake_generation = generation.wrapping_add(1);
    state.pending_terminal_memory_result = Some(RiscvPendingTerminalMemoryResult::new(
        &head,
        event,
        successor.consumed_requests().to_vec(),
        successor.decoded(),
        generation,
    ));
    Ok(state
        .pending_terminal_memory_result
        .as_ref()
        .map(RiscvPendingTerminalMemoryResult::issue_wake_identity))
}

fn terminal_memory_result_reads_register(
    instruction: RiscvInstruction,
    register: Register,
) -> bool {
    if o3_scalar_integer_source_registers(&instruction).contains(&register) {
        return true;
    }
    let RiscvInstruction::VectorMemory(memory) = instruction else {
        return false;
    };
    match memory {
        RiscvVectorMemoryInstruction::LoadUnitStride { rs1, .. }
        | RiscvVectorMemoryInstruction::LoadUnitStrideFaultOnly { rs1, .. }
        | RiscvVectorMemoryInstruction::LoadSegmentUnitStride { rs1, .. }
        | RiscvVectorMemoryInstruction::LoadIndexedUnordered { rs1, .. }
        | RiscvVectorMemoryInstruction::StoreUnitStride { rs1, .. }
        | RiscvVectorMemoryInstruction::StoreSegmentUnitStride { rs1, .. }
        | RiscvVectorMemoryInstruction::StoreIndexedUnordered { rs1, .. } => rs1 == register,
        RiscvVectorMemoryInstruction::LoadStrided { rs1, rs2, .. }
        | RiscvVectorMemoryInstruction::StoreStrided { rs1, rs2, .. } => {
            rs1 == register || rs2 == register
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
    Ok(live_speculative_fu(state, window)?.map(|(ready_tick, _)| ready_tick))
}

fn live_speculative_fu(
    state: &RiscvCoreState,
    window: &RiscvLiveRetireWindowRequest<'_>,
) -> Result<Option<(u64, RiscvExecutionRecord)>, RiscvCpuError> {
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
    let Some(ready_tick) = state
        .o3_runtime
        .live_speculative_execution_ready_tick(&instruction.consumed_requests, &execution)
    else {
        return Ok(None);
    };
    Ok(Some((ready_tick, execution)))
}

fn detailed_scalar_memory_blocks_execution(
    state: &RiscvCoreState,
    window: &RiscvLiveRetireWindowRequest<'_>,
) -> Result<bool, RiscvCpuError> {
    if !state.live_retire_gate.detailed_policy_enabled()
        || !state.o3_runtime.has_pending_live_data_access_retirement()
    {
        return Ok(false);
    }
    let instruction = RiscvInstruction::decode_with_length(window.raw)
        .map_err(RiscvCpuError::Isa)?
        .instruction();
    if state
        .pending_terminal_memory_result
        .as_ref()
        .is_some_and(|pending| pending.has_predecessor(window.request, window.pc, instruction))
    {
        return Ok(false);
    }
    if state.can_overlap_detailed_scalar_memory_instruction(instruction) {
        return Ok(false);
    }
    Ok(!state.can_overlap_detailed_memory_result_instruction(window.request, instruction))
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
    if !state.o3_runtime.bind_live_staged_issue_packet(
        pc,
        decoded,
        &head_instruction.consumed_requests,
        head_issue_tick,
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
    schedule_o3_live_speculative_younger_executions(state, &younger, earliest_tick)?;
    Ok(Some(admitted_tick))
}

pub(crate) fn stage_o3_data_access_younger_window(
    state: &mut RiscvCoreState,
    execution: &RiscvCpuExecutionEvent,
    issue_tick: u64,
    fetch_events: &[CpuFetchEvent],
) {
    if !state.live_retire_gate.detailed_policy_enabled()
        && !state
            .o3_runtime
            .owns_pending_live_data_access_retirement(execution.fetch().request_id())
    {
        return;
    }
    if dependent_result_address::stage_dependent_result_address_window(
        state,
        execution,
        issue_tick,
        fetch_events,
    ) {
        return;
    }
    let Some(window) = state
        .o3_runtime
        .data_access_integer_window(execution.fetch().request_id())
    else {
        return;
    };
    let remaining_rows = window.remaining_rows();
    let younger = completed_scalar_integer_younger_window(
        state,
        fetch_events,
        execution.fetch().request_id(),
        Address::new(execution.execution().next_pc()),
        window,
        remaining_rows,
    );
    let staged_rows = state.o3_runtime.stage_live_data_access_younger_window(
        execution.fetch().request_id(),
        younger
            .iter()
            .map(|younger| (younger.pc, younger.decoded.instruction())),
    );
    schedule_o3_live_speculative_younger_executions(
        state,
        &younger[..staged_rows.min(younger.len())],
        issue_tick,
    )
    .expect("live data-access younger writeback reservation");
}

pub(crate) fn stage_o3_producer_forwarded_scalar_return_descendant(
    state: &mut RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> bool {
    let Some((scalar_chain, _head, retirement_tick)) = state
        .o3_runtime
        .producer_forwarded_scalar_return_issue_context()
    else {
        return false;
    };
    let parent = scalar_chain.parent();
    let Some(scalar) = scalar_chain.last() else {
        return false;
    };
    if crate::riscv_fetch_ahead::recorded_predicted_pc(
        state,
        parent.fetch_request(),
        parent.sequential_pc(),
        &crate::riscv_fetch_ahead::PredictedControlTargetAuthority::ProducerForwarded(parent),
    ) != crate::riscv_fetch_ahead::RecordedPredictedPc::Ready(parent.target())
        || state.branch_speculations.len() >= state.branch_lookahead
        || crate::riscv_fetch_ahead::detailed_o3::unconsumed_ras_required_target(
            state,
            parent.fetch_request().sequence(),
            parent.sequential_pc(),
            crate::riscv_fetch_ahead::detailed_o3::RequiredRasConsumer::Pop,
        ) != Some(parent.sequential_pc())
    {
        return false;
    }
    let Some(returned) = completed_fetch_instruction_at(
        state,
        fetch_events,
        scalar.last_fetch_request(),
        scalar.sequential_pc(),
    ) else {
        return false;
    };
    let decoded = returned.decoded();
    let instruction = decoded.instruction();
    let issue_tick = returned.fetch().tick().max(retirement_tick);
    if crate::o3_runtime::o3_exact_link_return_source(instruction) != parent.link_destination()
        || state
            .o3_runtime
            .append_producer_forwarded_scalar_return_descendant(
                &scalar_chain,
                returned.pc(),
                decoded,
                returned.consumed_requests(),
                issue_tick,
            )
            .is_none()
    {
        return false;
    }
    schedule_o3_live_speculative_younger_executions(
        state,
        std::slice::from_ref(&returned),
        issue_tick,
    )
    .expect("producer-forwarded scalar-return writeback reservation");
    state.producer_forwarded_scalar_continuation = None;
    true
}

pub(crate) fn wake_o3_data_access_younger_window(
    state: &mut RiscvCoreState,
    issue_tick: u64,
    fetch_events: &[CpuFetchEvent],
) {
    let pending_window = state.o3_runtime.has_pending_data_address();
    let (tail_request, younger_pcs, _head) =
        if let Some(seed) = state.o3_runtime.pending_data_address_wake_seed() {
            (
                seed.fetch_predecessor_request(),
                seed.younger_pcs().to_vec(),
                seed.head_reservation(),
            )
        } else {
            let Some((tail_request, younger_pcs)) =
                state.o3_runtime.live_data_access_younger_wakeup_seed()
            else {
                if pending_window {
                    state.o3_runtime.discard_pending_data_address();
                }
                return;
            };
            let Some(head) = state
                .o3_runtime
                .live_data_access_head_reservation(tail_request)
            else {
                if pending_window {
                    state.o3_runtime.discard_pending_data_address();
                }
                return;
            };
            (tail_request, younger_pcs, head)
        };
    let mut current_request = tail_request;
    let mut younger = Vec::with_capacity(younger_pcs.len());
    for pc in younger_pcs {
        let Some(instruction) =
            completed_fetch_instruction_at(state, fetch_events, current_request, pc)
        else {
            if pending_window {
                state.o3_runtime.discard_pending_data_address();
            }
            return;
        };
        current_request = instruction.last_consumed_request();
        younger.push(instruction);
    }
    schedule_o3_live_speculative_younger_executions(state, &younger, issue_tick)
        .expect("live data-access wake writeback reservation");
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
    younger: &[RiscvCompletedFetchInstruction],
    issue_tick: u64,
) -> Result<bool, RiscvCpuError> {
    let pending_window = state.o3_runtime.has_pending_data_address();
    for younger in younger {
        if !state.o3_runtime.bind_live_staged_issue_packet(
            younger.pc,
            younger.decoded,
            &younger.consumed_requests,
            issue_tick,
        ) {
            if pending_window {
                state.o3_runtime.discard_pending_data_address();
            }
            return Ok(false);
        }
    }
    state.refresh_o3_writeback_wake(issue_tick);
    Ok(true)
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
            let Some(target_authority) =
                crate::riscv_fetch_ahead::predicted_control_target_authority(
                    instruction.decoded.instruction(),
                    sequential_pc,
                    classification,
                    &sequenced_return_addresses,
                )
            else {
                break;
            };
            let crate::riscv_fetch_ahead::RecordedPredictedPc::Ready(next_pc) =
                crate::riscv_fetch_ahead::recorded_predicted_pc(
                    state,
                    prediction_request,
                    sequential_pc,
                    &target_authority,
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
    let (raw, fetch) = match data {
        [low, high] if low & 0x3 != 0x3 => {
            (u32::from(u16::from_le_bytes([*low, *high])), event.clone())
        }
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
            let raw = u32::from_le_bytes([*low, *high, *suffix_low, *suffix_high]);
            let fetch = CpuFetchEvent::completed(
                CpuFetchRecord::new(
                    event.tick(),
                    event.partition(),
                    event.route(),
                    event.endpoint().clone(),
                    event.request_id(),
                    event.pc(),
                    AccessSize::new(4).expect("RISC-V word fetch width is nonzero"),
                ),
                raw.to_le_bytes().to_vec(),
            );
            (raw, fetch)
        }
        [a, b, c, d] => (u32::from_le_bytes([*a, *b, *c, *d]), event.clone()),
        _ => return None,
    };
    let decoded = RiscvInstruction::decode_with_length(raw).ok()?;
    Some(RiscvCompletedFetchInstruction {
        fetch,
        pc,
        consumed_requests,
        decoded,
    })
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::{
        Immediate, MemoryAccessKind, MemoryWidth, Register, RiscvExecutionRecord, RiscvInstruction,
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
            let seed_raw = 0x0000_0013_u32;
            let seed_instruction = RiscvInstruction::decode_with_length(seed_raw)
                .unwrap()
                .instruction();
            let seed = RiscvCpuExecutionEvent::new(
                completed_fetch_with_data(
                    7,
                    9,
                    Address::new(0x7ffc),
                    seed_raw.to_le_bytes().to_vec(),
                ),
                seed_instruction,
                RiscvExecutionRecord::new(seed_instruction, 0x7ffc, 0x8000, Vec::new(), None),
            );
            state.o3_runtime.record_retired_instruction(&seed);
            state
                .o3_runtime
                .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(0, 12)])
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
            let head = state.o3_runtime.writeback_reservation(1).unwrap();
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
        assert!(state.o3_runtime.stage_live_data_access_issue_for_test(
            &execution,
            request(7, 20),
            31
        ));

        stage_o3_data_access_younger_window(&mut state, &execution, 10, &events);

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
        assert!(state.o3_runtime.stage_live_data_access_issue_for_test(
            &execution,
            request(7, 20),
            31
        ));

        stage_o3_data_access_younger_window(&mut state, &execution, 10, &events);

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
        let raw = 0x0220_83b3_u32;
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
        assert!(state.o3_runtime.bind_live_staged_issue_packet(
            Address::new(0x8004),
            RiscvInstruction::decode_with_length(raw).unwrap(),
            &[request(7, 11), request(7, 12)],
            0,
        ));
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

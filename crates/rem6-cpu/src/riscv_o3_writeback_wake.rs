use rem6_kernel::{PendingEventSnapshot, SchedulerInstanceId, Tick};

use crate::{CpuFetchEvent, RiscvCore, RiscvCoreState, RiscvCpuError};

impl RiscvCoreState {
    pub(crate) fn wake_ready_o3_data_access_younger_window(
        &mut self,
        tick: Tick,
        fetch_events: &[CpuFetchEvent],
    ) {
        crate::riscv_live_retire_window::wake_o3_data_access_younger_window(
            self,
            tick,
            fetch_events,
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct RiscvO3WritebackWake {
    scheduler: SchedulerInstanceId,
    event: PendingEventSnapshot,
}

impl RiscvO3WritebackWake {
    pub(crate) const fn new(scheduler: SchedulerInstanceId, event: PendingEventSnapshot) -> Self {
        Self { scheduler, event }
    }

    pub(crate) fn tick(self) -> Tick {
        self.event.tick()
    }

    pub(crate) const fn scheduler(self) -> SchedulerInstanceId {
        self.scheduler
    }

    pub(crate) const fn event(self) -> PendingEventSnapshot {
        self.event
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RiscvO3WritebackWakeState {
    desired_tick: Option<Tick>,
    scheduled: Option<RiscvO3WritebackWake>,
    detached: Vec<RiscvO3WritebackWake>,
    fired_through: Option<Tick>,
}

impl RiscvO3WritebackWakeState {
    pub(crate) fn set_desired_tick(&mut self, desired: Option<Tick>, now: Tick) {
        self.prune(now);
        if self.desired_tick == desired {
            return;
        }
        if let Some(wake) = self.scheduled.take() {
            if !self.detached.contains(&wake) {
                self.detached.push(wake);
            }
        }
        self.desired_tick = desired;
    }

    pub(crate) fn requested_tick(&mut self, now: Tick) -> Option<Tick> {
        self.requested_tick_with_current(now, false)
    }

    fn requested_tick_with_current(&mut self, now: Tick, allow_current: bool) -> Option<Tick> {
        self.prune(now);
        if self.scheduled.is_some() {
            return None;
        }
        self.desired_tick
            .filter(|tick| *tick > now || (allow_current && *tick == now))
    }

    pub(crate) fn mark_scheduled(
        &mut self,
        scheduler: SchedulerInstanceId,
        event: PendingEventSnapshot,
    ) {
        let wake = RiscvO3WritebackWake::new(scheduler, event);
        if self.desired_tick == Some(wake.tick()) && self.scheduled.is_none() {
            self.scheduled = Some(wake);
        }
    }

    pub(crate) fn mark_fired(&mut self, now: Tick) {
        let fired_tick = self
            .scheduled
            .filter(|wake| wake.tick() <= now)
            .map(RiscvO3WritebackWake::tick);
        if fired_tick.is_some() {
            self.scheduled = None;
        }
        if self.desired_tick == fired_tick {
            self.desired_tick = None;
        }
        self.fired_through = Some(self.fired_through.map_or(now, |tick| tick.max(now)));
        self.prune(now);
    }

    pub(crate) fn owned_wakes(&self) -> Vec<RiscvO3WritebackWake> {
        self.scheduled
            .into_iter()
            .chain(self.detached.iter().copied())
            .collect()
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(crate) const fn has_desired_tick(&self) -> bool {
        self.desired_tick.is_some()
    }

    pub(crate) fn has_scheduled_wake_authority(&self) -> bool {
        self.scheduled.is_some()
            || self.detached.iter().any(|wake| {
                self.fired_through
                    .is_none_or(|fired_through| wake.tick() > fired_through)
            })
    }

    pub(crate) fn has_pending_checkpoint_authority(&self) -> bool {
        self.desired_tick.is_some() || self.scheduled.is_some() || !self.detached.is_empty()
    }

    fn finalize_fired_detached_wakes(&mut self) {
        let Some(fired_through) = self.fired_through else {
            return;
        };
        self.detached.retain(|wake| wake.tick() > fired_through);
        if self.detached.is_empty() && self.scheduled.is_none() {
            self.fired_through = None;
        }
    }

    fn prune(&mut self, now: Tick) {
        self.detached.retain(|wake| wake.tick() >= now);
    }
}

impl RiscvCore {
    pub fn requested_o3_writeback_wake_tick(&self, now: Tick) -> Option<Tick> {
        let mut state = self.state.lock().expect("riscv core lock");
        state.o3_runtime.prune_writeback_calendar_before(now);
        let memory_result = state
            .o3_runtime
            .earliest_unpublished_memory_result_writeback_tick();
        let live_gate_ready_tick = state.live_retire_gate.pending_ready_tick();
        let live_gate_wakes = state.live_retire_gate.owned_scheduler_wakes();
        let restored_live_gate = live_gate_wakes
            .is_empty()
            .then_some(live_gate_ready_tick)
            .flatten();
        let desired = match (memory_result, restored_live_gate) {
            (Some(memory_result), Some(live_gate)) => Some(memory_result.min(live_gate)),
            (Some(tick), None) | (None, Some(tick)) => Some(tick),
            (None, None) => None,
        };
        state.o3_writeback_wake.set_desired_tick(desired, now);
        if restored_live_gate == Some(now) {
            state
                .o3_writeback_wake
                .requested_tick_with_current(now, true)
        } else {
            state.o3_writeback_wake.requested_tick(now)
        }
    }

    pub fn mark_o3_writeback_wake_scheduled(
        &self,
        scheduler: SchedulerInstanceId,
        event: PendingEventSnapshot,
    ) {
        self.state
            .lock()
            .expect("riscv core lock")
            .o3_writeback_wake
            .mark_scheduled(scheduler, event);
    }

    pub fn mark_o3_writeback_wake_fired(&self, now: Tick) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.o3_runtime.prune_writeback_calendar_before(now);
        state.o3_writeback_wake.mark_fired(now);
    }

    pub fn owned_o3_writeback_wakes(&self) -> Vec<(SchedulerInstanceId, PendingEventSnapshot)> {
        self.state
            .lock()
            .expect("riscv core lock")
            .o3_writeback_wake
            .owned_wakes()
            .into_iter()
            .map(|wake| (wake.scheduler(), wake.event()))
            .collect()
    }

    pub fn finalize_quiescent_o3_writeback_for_checkpoint(&self) {
        let mut state = self.state.lock().expect("riscv core lock");
        if state.o3_runtime.has_live_writeback_owner()
            || state
                .o3_runtime
                .earliest_unpublished_memory_result_writeback_tick()
                .is_some()
            || state.o3_writeback_wake.has_desired_tick()
            || state.o3_writeback_wake.has_scheduled_wake_authority()
        {
            return;
        }
        if let Err(error) = state.o3_runtime.finalize_all_writeback_reservations() {
            state
                .pending_callback_error
                .get_or_insert(RiscvCpuError::O3Runtime(error));
            return;
        }
        state.o3_writeback_wake.finalize_fired_detached_wakes();
    }
}

impl RiscvCoreState {
    pub(crate) fn refresh_o3_writeback_wake(&mut self, now: Tick) {
        let desired = self
            .o3_runtime
            .earliest_unpublished_memory_result_writeback_tick();
        self.o3_writeback_wake.set_desired_tick(desired, now);
    }
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::{
        Immediate, MemoryAccessKind, MemoryWidth, Register, RiscvExecutionRecord, RiscvInstruction,
    };
    use rem6_kernel::{PartitionId, PartitionedScheduler};
    use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryRequestId};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    use super::*;
    use crate::{
        o3_runtime::O3LiveRetireGateCheckpointPayload, CpuCore, CpuFetchConfig, CpuFetchEvent,
        CpuFetchRecord, CpuId, CpuResetState, O3RuntimeError, RiscvCpuExecutionEvent,
        RiscvDataAccessEventKind,
    };

    #[test]
    fn o3_writeback_wake_identical_request_deduplicates() {
        let mut state = RiscvO3WritebackWakeState::default();
        state.set_desired_tick(Some(20), 10);
        let (scheduler, event) = wake(20);
        state.mark_scheduled(scheduler, event);

        state.set_desired_tick(Some(20), 11);

        assert_eq!(state.requested_tick(11), None);
        assert_eq!(state.owned_wakes().len(), 1);
    }

    #[test]
    fn o3_writeback_wake_earlier_request_detaches_later_schedule() {
        let mut state = RiscvO3WritebackWakeState::default();
        state.set_desired_tick(Some(20), 10);
        let (scheduler, event) = wake(20);
        state.mark_scheduled(scheduler, event);

        state.set_desired_tick(Some(15), 11);

        assert_eq!(state.requested_tick(11), Some(15));
        assert_eq!(state.owned_wakes().len(), 1);
    }

    #[test]
    fn o3_writeback_wake_fired_schedule_clears_ownership() {
        let mut state = RiscvO3WritebackWakeState::default();
        state.set_desired_tick(Some(20), 10);
        let (scheduler, event) = wake(20);
        state.mark_scheduled(scheduler, event);

        state.mark_fired(20);

        assert!(state.owned_wakes().is_empty());
        assert!(!state.has_desired_tick());
    }

    #[test]
    fn o3_writeback_wake_detached_schedule_prunes_after_later_tick() {
        let mut state = RiscvO3WritebackWakeState::default();
        state.set_desired_tick(Some(20), 10);
        let (scheduler, event) = wake(20);
        state.mark_scheduled(scheduler, event);
        state.set_desired_tick(Some(15), 11);

        assert_eq!(state.owned_wakes().len(), 1);
        assert_eq!(state.requested_tick(20), None);
        assert_eq!(state.owned_wakes().len(), 1);
        assert_eq!(state.requested_tick(21), None);
        assert!(state.owned_wakes().is_empty());
    }

    #[test]
    fn checkpoint_finalization_clears_consumed_calendar_history() {
        let core = core();
        core.reserve_test_fixed_fu_writeback(4, 20).unwrap();
        assert!(!core.data_access_lifecycle_is_quiescent());

        core.finalize_quiescent_o3_writeback_for_checkpoint();

        let state = core.state.lock().expect("riscv core lock");
        assert!(state.o3_runtime.writeback_reservation(4).is_none());
        assert!(state.o3_writeback_wake.owned_wakes().is_empty());
        drop(state);
        assert!(core.data_access_lifecycle_is_quiescent());
        assert_eq!(
            core.reserve_test_fixed_fu_writeback(5, 20).unwrap_err(),
            O3RuntimeError::WritebackReservationTickClosed {
                sequence: 5,
                raw_ready_tick: 20,
                closed_before_tick: 21,
            }
        );
    }

    #[test]
    fn checkpoint_finalization_keeps_scheduled_writeback_wake_nonquiescent() {
        let core = core();
        core.reserve_test_fixed_fu_writeback(4, 20).unwrap();
        let (scheduler, event) = wake(20);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            state.o3_writeback_wake.set_desired_tick(Some(20), 10);
            state.o3_writeback_wake.mark_scheduled(scheduler, event);
        }

        core.finalize_quiescent_o3_writeback_for_checkpoint();

        let state = core.state.lock().expect("riscv core lock");
        assert!(state.o3_runtime.writeback_reservation(4).is_some());
        assert_eq!(state.o3_writeback_wake.owned_wakes().len(), 1);
        drop(state);
        assert!(!core.data_access_lifecycle_is_quiescent());
    }

    #[test]
    fn checkpoint_finalization_reports_max_tick_seal_error_without_clearing() {
        let core = core();
        core.reserve_test_fixed_fu_writeback(4, u64::MAX).unwrap();

        core.finalize_quiescent_o3_writeback_for_checkpoint();

        assert_eq!(
            core.pending_callback_error(),
            Some(RiscvCpuError::O3Runtime(
                O3RuntimeError::WritebackClosureTickOverflow { tick: u64::MAX }
            ))
        );
        assert_eq!(core.o3_runtime_writeback_reservations().len(), 1);
    }

    #[test]
    fn scheduled_writeback_wakes_advance_through_publication_and_finalize() {
        let core = core_with_completed_scalar_loads();
        assert_eq!(core.requested_o3_writeback_wake_tick(10), Some(20));
        let (first_scheduler, first_event) = wake(20);
        core.mark_o3_writeback_wake_scheduled(first_scheduler, first_event);

        core.mark_o3_writeback_wake_fired(20);
        assert!(core
            .record_ready_o3_data_access_event_with_trace(20, false)
            .is_some());
        {
            let state = core.state.lock().expect("riscv core lock");
            assert_eq!(state.o3_writeback_wake.desired_tick, Some(22));
        }
        assert_eq!(core.requested_o3_writeback_wake_tick(20), Some(22));

        let (second_scheduler, second_event) = wake(22);
        core.mark_o3_writeback_wake_scheduled(second_scheduler, second_event);
        core.mark_o3_writeback_wake_fired(22);
        assert!(core
            .record_ready_o3_data_access_event_with_trace(22, false)
            .is_some());
        {
            let state = core.state.lock().expect("riscv core lock");
            assert_eq!(state.o3_writeback_wake.desired_tick, None);
        }

        core.finalize_quiescent_o3_writeback_for_checkpoint();

        let state = core.state.lock().expect("riscv core lock");
        assert!(!state.o3_writeback_wake.has_pending_checkpoint_authority());
        drop(state);
        assert!(core.o3_runtime_writeback_reservations().is_empty());
        assert!(core.data_access_lifecycle_is_quiescent());
    }

    #[test]
    fn restored_live_retire_gate_without_owned_scheduler_wake_requests_gate_tick() {
        let core = core();
        let request = memory_request(31);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            state
                .live_retire_gate
                .restore_checkpoint(Some(O3LiveRetireGateCheckpointPayload::new(request, 31)));
        }
        assert!(core.checkpoint_owned_live_retire_gate_wakes().is_empty());

        assert_eq!(core.requested_o3_writeback_wake_tick(28), Some(31));
        let (scheduler, event) = wake(31);
        core.mark_o3_writeback_wake_scheduled(scheduler, event);

        assert_eq!(core.requested_o3_writeback_wake_tick(29), None);
    }

    #[test]
    fn restored_live_retire_gate_due_now_requests_current_tick() {
        let core = core();
        let request = memory_request(31);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            state
                .live_retire_gate
                .restore_checkpoint(Some(O3LiveRetireGateCheckpointPayload::new(request, 31)));
        }
        assert!(core.checkpoint_owned_live_retire_gate_wakes().is_empty());

        assert_eq!(core.requested_o3_writeback_wake_tick(31), Some(31));
        let (scheduler, event) = wake(31);
        core.mark_o3_writeback_wake_scheduled(scheduler, event);

        assert_eq!(core.requested_o3_writeback_wake_tick(31), None);
    }

    fn core() -> RiscvCore {
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

    fn core_with_completed_scalar_loads() -> RiscvCore {
        let older = scalar_load_event(0x8000, 10, 12, 0x9000);
        let younger = scalar_load_event(0x8004, 11, 13, 0x9040);
        let mut runtime = crate::o3_runtime::O3RuntimeState::default();
        assert!(runtime.stage_live_data_access_issue_for_test(&older, memory_request(20), 10));
        assert!(runtime.stage_live_data_access_issue_for_test(&younger, memory_request(21), 11));
        complete_scalar_load(&mut runtime, &older, memory_request(20), 19, 0x2a);
        complete_scalar_load(&mut runtime, &younger, memory_request(21), 21, 0x63);
        let core = core();
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .issued_data_for_fetches
            .extend([older.fetch().request_id(), younger.fetch().request_id()]);
        state.events.extend([older, younger]);
        state.o3_runtime = runtime;
        drop(state);
        core
    }

    fn complete_scalar_load(
        runtime: &mut crate::o3_runtime::O3RuntimeState,
        execution: &RiscvCpuExecutionEvent,
        request: MemoryRequestId,
        response_tick: u64,
        value: u8,
    ) {
        let mut completed = execution.clone();
        completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(runtime
            .complete_live_data_access_response(
                &completed,
                request,
                response_tick,
                response_tick.saturating_sub(10),
                Some(&[value, 0, 0, 0]),
            )
            .unwrap());
    }

    fn scalar_load_event(
        pc: u64,
        sequence: u64,
        destination: u8,
        address: u64,
    ) -> RiscvCpuExecutionEvent {
        let instruction = RiscvInstruction::Load {
            rd: register(destination),
            rs1: register(2),
            offset: Immediate::new(0),
            width: MemoryWidth::Word,
            signed: false,
        };
        let access = MemoryAccessKind::Load {
            rd: register(destination),
            address,
            width: MemoryWidth::Word,
            signed: false,
        };
        RiscvCpuExecutionEvent::new(
            CpuFetchEvent::completed(
                CpuFetchRecord::new(
                    sequence,
                    PartitionId::new(0),
                    MemoryRouteId::new(0),
                    TransportEndpointId::new("cpu0.ifetch").unwrap(),
                    memory_request(sequence),
                    Address::new(pc),
                    AccessSize::new(4).unwrap(),
                ),
                0x0000_0073_u32.to_le_bytes().to_vec(),
            ),
            instruction,
            RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
        )
    }

    fn memory_request(sequence: u64) -> MemoryRequestId {
        MemoryRequestId::new(AgentId::new(7), sequence)
    }

    fn register(index: u8) -> Register {
        Register::new(index).unwrap()
    }

    fn wake(
        tick: u64,
    ) -> (
        rem6_kernel::SchedulerInstanceId,
        rem6_kernel::PendingEventSnapshot,
    ) {
        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        let event = scheduler
            .schedule_at(PartitionId::new(0), tick, |_| {})
            .unwrap();
        (
            scheduler.instance_id(),
            scheduler.pending_event_snapshot(event).unwrap(),
        )
    }
}

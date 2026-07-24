use rem6_isa_riscv::RiscvInstruction;
use rem6_kernel::{PendingEventSnapshot, SchedulerError, SchedulerInstanceId, Tick};
use rem6_memory::MemoryRequestId;

use crate::o3_runtime::{
    O3LiveRetireGateCheckpointPayload, O3RuntimeCheckpointPayload, O3RuntimeError,
};
use crate::{riscv_fu_latency::riscv_execute_wait_cycles, RiscvCore, RiscvCpuError};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct RiscvLiveRetireGatePolicy {
    detailed: bool,
}

impl RiscvLiveRetireGatePolicy {
    pub(crate) const fn disabled() -> Self {
        Self { detailed: false }
    }

    pub(crate) const fn detailed() -> Self {
        Self { detailed: true }
    }

    const fn creates_gates(self) -> bool {
        self.detailed
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct RiscvLiveRetireGateWake {
    scheduler: SchedulerInstanceId,
    event: PendingEventSnapshot,
}

impl RiscvLiveRetireGateWake {
    pub(crate) const fn new(scheduler: SchedulerInstanceId, event: PendingEventSnapshot) -> Self {
        Self { scheduler, event }
    }

    pub const fn scheduler(self) -> SchedulerInstanceId {
        self.scheduler
    }

    pub const fn event(self) -> PendingEventSnapshot {
        self.event
    }

    pub fn tick(self) -> Tick {
        self.event.tick()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvLiveRetireGatePending {
    request: MemoryRequestId,
    ready_tick: Tick,
    scheduler_wake: Option<RiscvLiveRetireGateWake>,
    rebind_on_next_request: bool,
}

impl RiscvLiveRetireGatePending {
    const fn new(request: MemoryRequestId, ready_tick: Tick) -> Self {
        Self {
            request,
            ready_tick,
            scheduler_wake: None,
            rebind_on_next_request: false,
        }
    }

    const fn restored(request: MemoryRequestId, ready_tick: Tick) -> Self {
        Self {
            request,
            ready_tick,
            scheduler_wake: None,
            rebind_on_next_request: true,
        }
    }

    pub(crate) const fn request(self) -> MemoryRequestId {
        self.request
    }

    const fn ready_tick(self) -> Tick {
        self.ready_tick
    }

    const fn is_scheduled(self) -> bool {
        self.scheduler_wake.is_some()
    }

    const fn scheduler_wake(self) -> Option<RiscvLiveRetireGateWake> {
        self.scheduler_wake
    }

    const fn blocks_new_work(self) -> bool {
        !self.rebind_on_next_request
    }

    fn rebind(&mut self, request: MemoryRequestId) {
        self.request = request;
        self.rebind_on_next_request = false;
    }

    fn rebind_to_next_request(&mut self) {
        self.rebind_on_next_request = true;
    }

    fn mark_scheduled(&mut self, wake: RiscvLiveRetireGateWake) {
        self.scheduler_wake = Some(wake);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RiscvLiveRetireGateDecision {
    Ready,
    Blocked,
    Schedule {
        ready_tick: Tick,
        created_wait_ticks: Option<Tick>,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RiscvLiveRetireGateState {
    policy: RiscvLiveRetireGatePolicy,
    pending: Option<RiscvLiveRetireGatePending>,
    detached_scheduler_wakes: Vec<RiscvLiveRetireGateWake>,
}

impl RiscvLiveRetireGateState {
    pub(crate) fn set_policy(&mut self, policy: RiscvLiveRetireGatePolicy) {
        self.policy = policy;
    }

    pub(crate) fn checkpoint(&self) -> Option<O3LiveRetireGateCheckpointPayload> {
        self.pending.map(|pending| {
            O3LiveRetireGateCheckpointPayload::new(pending.request(), pending.ready_tick())
        })
    }

    pub(crate) fn restore_checkpoint(
        &mut self,
        payload: Option<O3LiveRetireGateCheckpointPayload>,
    ) {
        self.detached_scheduler_wakes.clear();
        self.pending = payload.map(|payload| {
            RiscvLiveRetireGatePending::restored(payload.request(), payload.ready_tick())
        });
    }

    pub(crate) fn blocks_new_work(&self) -> bool {
        self.pending
            .is_some_and(RiscvLiveRetireGatePending::blocks_new_work)
    }

    pub(crate) fn pending_ready_tick(&self) -> Option<Tick> {
        self.pending.map(RiscvLiveRetireGatePending::ready_tick)
    }

    pub(crate) fn awaits_rebind_to_next_request(&self) -> bool {
        self.pending
            .is_some_and(|pending| pending.rebind_on_next_request)
    }

    pub(crate) fn owned_scheduler_wakes(&self) -> Vec<RiscvLiveRetireGateWake> {
        self.detached_scheduler_wakes
            .iter()
            .copied()
            .chain(
                self.pending
                    .and_then(RiscvLiveRetireGatePending::scheduler_wake),
            )
            .collect()
    }

    pub(crate) const fn detailed_policy_enabled(&self) -> bool {
        self.policy.creates_gates()
    }

    pub(crate) fn rebind_pending_to_next_request(&mut self) {
        if let Some(pending) = &mut self.pending {
            pending.rebind_to_next_request();
        }
    }

    pub(crate) fn clear_pending_for_pc_redirect(&mut self) {
        self.detach_pending_scheduler_wake();
        self.pending = None;
    }

    pub(crate) fn before_retire(
        &mut self,
        request: MemoryRequestId,
        raw: u32,
        now: Tick,
        ready_base_tick: Tick,
    ) -> Result<RiscvLiveRetireGateDecision, RiscvCpuError> {
        self.prune_detached_scheduler_wakes(now);
        if let Some(decision) = self.pending_before_retire(request, now) {
            return Ok(decision);
        }

        if !self.policy.creates_gates() {
            return Ok(RiscvLiveRetireGateDecision::Ready);
        }
        let decoded = RiscvInstruction::decode_with_length(raw).map_err(RiscvCpuError::Isa)?;
        let wait_ticks = riscv_execute_wait_cycles(decoded.instruction());
        if wait_ticks == 0 {
            return Ok(RiscvLiveRetireGateDecision::Ready);
        }
        let ready_tick =
            ready_base_tick
                .checked_add(wait_ticks)
                .ok_or(RiscvCpuError::Scheduler(SchedulerError::TickOverflow {
                    now: ready_base_tick,
                    delay: wait_ticks,
                }))?;
        self.pending = Some(RiscvLiveRetireGatePending::new(request, ready_tick));
        Ok(RiscvLiveRetireGateDecision::Schedule {
            ready_tick,
            created_wait_ticks: Some(wait_ticks),
        })
    }

    pub(crate) fn before_retire_at_known_ready_tick(
        &mut self,
        request: MemoryRequestId,
        now: Tick,
        ready_tick: Tick,
    ) -> RiscvLiveRetireGateDecision {
        self.prune_detached_scheduler_wakes(now);
        if let Some(decision) = self.pending_before_retire(request, now) {
            return decision;
        }
        if now >= ready_tick {
            return RiscvLiveRetireGateDecision::Ready;
        }
        self.pending = Some(RiscvLiveRetireGatePending::new(request, ready_tick));
        RiscvLiveRetireGateDecision::Schedule {
            ready_tick,
            created_wait_ticks: Some(ready_tick - now),
        }
    }

    fn pending_before_retire(
        &mut self,
        request: MemoryRequestId,
        now: Tick,
    ) -> Option<RiscvLiveRetireGateDecision> {
        if let Some(mut pending) = self.pending {
            if pending.request() != request {
                if !pending.rebind_on_next_request {
                    return Some(RiscvLiveRetireGateDecision::Blocked);
                }
                pending.rebind(request);
            } else if pending.rebind_on_next_request {
                pending.rebind(request);
            }
            if now >= pending.ready_tick() {
                self.remember_detached_scheduler_wake(pending.scheduler_wake());
                self.pending = None;
                return Some(RiscvLiveRetireGateDecision::Ready);
            }
            self.pending = Some(pending);
            return Some(if pending.is_scheduled() {
                RiscvLiveRetireGateDecision::Blocked
            } else {
                RiscvLiveRetireGateDecision::Schedule {
                    ready_tick: pending.ready_tick(),
                    created_wait_ticks: None,
                }
            });
        }
        None
    }

    pub(crate) fn blocks_without_scheduler(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn mark_scheduled(
        &mut self,
        request: MemoryRequestId,
        wake: RiscvLiveRetireGateWake,
    ) {
        if let Some(pending) = &mut self.pending {
            if pending.request() == request {
                pending.mark_scheduled(wake);
            }
        }
    }

    fn detach_pending_scheduler_wake(&mut self) {
        self.remember_detached_scheduler_wake(
            self.pending
                .and_then(RiscvLiveRetireGatePending::scheduler_wake),
        );
    }

    fn remember_detached_scheduler_wake(&mut self, wake: Option<RiscvLiveRetireGateWake>) {
        if let Some(wake) = wake {
            if !self.detached_scheduler_wakes.contains(&wake) {
                self.detached_scheduler_wakes.push(wake);
            }
        }
    }

    fn prune_detached_scheduler_wakes(&mut self, now: Tick) {
        self.detached_scheduler_wakes
            .retain(|wake| wake.tick() >= now);
    }
}

impl RiscvCore {
    pub fn set_detailed_live_retire_gate_enabled(&self, detailed: bool) {
        let policy = if detailed {
            RiscvLiveRetireGatePolicy::detailed()
        } else {
            RiscvLiveRetireGatePolicy::disabled()
        };
        let mut state = self.state.lock().expect("riscv core lock");
        state.live_retire_gate.set_policy(policy);
        if !detailed {
            state.o3_runtime.discard_all_live_issue_transient_state();
            let younger_result_fetches = state
                .memory_result_window_authorizations
                .iter()
                .filter_map(|(request, authorization)| {
                    authorization.role().is_younger().then_some(*request)
                })
                .collect::<Vec<_>>();
            for fetch_request in younger_result_fetches {
                state.discard_translated_result_pair_from(fetch_request);
            }
            let buffered_memory_results = state
                .buffered_o3_effects
                .values()
                .filter_map(crate::riscv_data_issue::BufferedO3Effect::memory_result_requests)
                .collect::<Vec<_>>();
            for (data_request, fetch_request) in buffered_memory_results {
                assert!(
                    state
                        .o3_runtime
                        .discard_live_data_access_suffix(fetch_request, data_request),
                    "buffered O3 memory-result effect owns a live suffix"
                );
                state.buffered_o3_effects.remove(&data_request);
                state.outstanding_data.remove(&data_request);
                state.issued_data_for_fetches.remove(&fetch_request);
                state
                    .memory_result_window_authorizations
                    .remove(&fetch_request);
                if let Some(event) = state.data_access_execution_mut(fetch_request) {
                    event.clear_data_access_retirement();
                }
            }
            let pending_address_wake = state.o3_runtime.pending_data_address_wake_tick().is_some();
            state.o3_runtime.discard_pending_data_address();
            if pending_address_wake {
                state.o3_writeback_wake.clear();
            }
            if state.o3_runtime.has_live_retirement_authority() {
                return;
            }
            state.memory_result_window_authorizations.clear();
            if state.o3_runtime.has_live_data_access_window() {
                state.o3_runtime.discard_live_retire_window();
            } else {
                state.o3_runtime.discard_live_data_access_lifecycle();
                state.o3_runtime.discard_live_speculative_executions();
            }
            state.o3_writeback_wake.clear();
        }
    }

    pub(crate) fn detailed_o3_window_prefers_fetch_ahead(&self) -> bool {
        let state = self.state.lock().expect("riscv core lock");
        let draining_normal_execute_wait = state
            .in_order_pipeline
            .in_flight()
            .iter()
            .any(|instruction| instruction.execute_wait_total_cycles().is_some());
        let producer_forwarded_control = state
            .o3_runtime
            .producer_forwarded_control_target()
            .is_some()
            || state
                .o3_runtime
                .retained_producer_forwarded_control_target()
                .is_some();
        (state.live_retire_gate.detailed_policy_enabled()
            || state.o3_runtime.has_live_control_window())
            && (!draining_normal_execute_wait || producer_forwarded_control)
    }

    pub(crate) fn o3_retirement_suppresses_normal_pipeline(&self) -> bool {
        let state = self.state.lock().expect("riscv core lock");
        let draining_normal_execute_wait = state
            .in_order_pipeline
            .in_flight()
            .iter()
            .any(|instruction| instruction.execute_wait_total_cycles().is_some());
        state.o3_runtime.has_pending_retirement_authority() && !draining_normal_execute_wait
    }

    pub(crate) fn live_retire_gate_blocks_new_work(&self) -> bool {
        let state = self.state.lock().expect("riscv core lock");
        state.live_retire_gate.blocks_new_work()
            || state.o3_runtime.has_pending_live_data_access_retirement()
    }

    pub(crate) fn live_retire_gate_awaits_rebind(&self) -> bool {
        self.state
            .lock()
            .expect("riscv core lock")
            .live_retire_gate
            .awaits_rebind_to_next_request()
    }

    pub fn o3_runtime_checkpoint_payload(&self) -> O3RuntimeCheckpointPayload {
        let state = self.state.lock().expect("riscv core lock");
        state
            .o3_runtime
            .checkpoint_payload()
            .with_live_retire_gate(state.live_retire_gate.checkpoint())
    }

    #[doc(hidden)]
    pub fn checkpoint_owned_live_retire_gate_wakes(
        &self,
    ) -> Vec<(SchedulerInstanceId, PendingEventSnapshot)> {
        self.state
            .lock()
            .expect("riscv core lock")
            .live_retire_gate
            .owned_scheduler_wakes()
            .into_iter()
            .map(|wake| (wake.scheduler(), wake.event()))
            .collect()
    }

    pub fn restore_o3_runtime_checkpoint_payload(
        &self,
        payload: O3RuntimeCheckpointPayload,
    ) -> Result<(), O3RuntimeError> {
        let live_retire_gate = payload.live_retire_gate();
        let mut state = self.state.lock().expect("riscv core lock");
        state.o3_runtime.restore_checkpoint_payload(payload)?;
        state.live_retire_gate.restore_checkpoint(live_retire_gate);
        state.o3_writeback_wake.clear();
        state.pending_callback_error = None;
        state.producer_forwarded_scalar_continuation = None;
        state.memory_result_window_authorizations.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rem6_kernel::{PartitionId, PartitionedScheduler};
    use rem6_memory::{AgentId, MemoryRequestId};

    use super::*;

    fn div_raw() -> u32 {
        (1 << 25) | (2 << 20) | (1 << 15) | (4 << 12) | (3 << 7) | 0x33
    }

    fn wake(tick: Tick) -> RiscvLiveRetireGateWake {
        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        let event_id = scheduler
            .schedule_at(PartitionId::new(0), tick, |_| {})
            .unwrap();
        RiscvLiveRetireGateWake::new(
            scheduler.instance_id(),
            scheduler.pending_event_snapshot(event_id).unwrap(),
        )
    }

    #[test]
    fn restored_pending_gate_blocks_without_scheduler() {
        let request = MemoryRequestId::new(AgentId::new(1), 42);
        let mut gate = RiscvLiveRetireGateState::default();
        gate.restore_checkpoint(Some(O3LiveRetireGateCheckpointPayload::new(request, 99)));

        assert!(gate.blocks_without_scheduler());
    }

    #[test]
    fn restored_pending_gate_rebinds_to_refetched_request_and_schedules_once() {
        let captured_request = MemoryRequestId::new(AgentId::new(1), 42);
        let refetched_request = MemoryRequestId::new(AgentId::new(1), 43);
        let mut gate = RiscvLiveRetireGateState::default();
        gate.restore_checkpoint(Some(O3LiveRetireGateCheckpointPayload::new(
            captured_request,
            99,
        )));

        assert_eq!(
            gate.before_retire(refetched_request, div_raw(), 10, 10),
            Ok(RiscvLiveRetireGateDecision::Schedule {
                ready_tick: 99,
                created_wait_ticks: None,
            })
        );
        gate.mark_scheduled(refetched_request, wake(99));
        assert_eq!(
            gate.before_retire(refetched_request, div_raw(), 10, 10),
            Ok(RiscvLiveRetireGateDecision::Blocked)
        );
        assert_eq!(
            gate.before_retire(refetched_request, div_raw(), 99, 99),
            Ok(RiscvLiveRetireGateDecision::Ready)
        );
    }

    #[test]
    fn known_speculative_ready_tick_arms_only_the_remaining_wait() {
        let mut gate = RiscvLiveRetireGateState::default();
        gate.set_policy(RiscvLiveRetireGatePolicy::detailed());
        let request = MemoryRequestId::new(AgentId::new(7), 11);

        assert_eq!(
            gate.before_retire_at_known_ready_tick(request, 10, 12),
            RiscvLiveRetireGateDecision::Schedule {
                ready_tick: 12,
                created_wait_ticks: Some(2),
            }
        );
        gate.mark_scheduled(request, wake(12));
        assert_eq!(
            gate.before_retire_at_known_ready_tick(request, 11, 12),
            RiscvLiveRetireGateDecision::Blocked
        );
        assert_eq!(
            gate.before_retire_at_known_ready_tick(request, 12, 12),
            RiscvLiveRetireGateDecision::Ready
        );
    }

    #[test]
    fn scheduled_gate_exposes_exact_owned_scheduler_wake_until_ready() {
        let request = MemoryRequestId::new(AgentId::new(1), 42);
        let mut scheduler = PartitionedScheduler::new(4).unwrap();
        let event_id = scheduler
            .schedule_parallel_at(PartitionId::new(3), 29, |_| {})
            .unwrap();
        let wake = RiscvLiveRetireGateWake::new(
            scheduler.instance_id(),
            scheduler.pending_event_snapshot(event_id).unwrap(),
        );
        let mut gate = RiscvLiveRetireGateState::default();
        gate.set_policy(RiscvLiveRetireGatePolicy::detailed());
        assert!(matches!(
            gate.before_retire(request, div_raw(), 10, 10),
            Ok(RiscvLiveRetireGateDecision::Schedule { ready_tick: 29, .. })
        ));

        gate.mark_scheduled(request, wake);

        assert_eq!(gate.owned_scheduler_wakes(), vec![wake]);
        assert_eq!(
            gate.before_retire(request, div_raw(), 28, 28),
            Ok(RiscvLiveRetireGateDecision::Blocked)
        );
        assert_eq!(gate.owned_scheduler_wakes(), vec![wake]);
        assert_eq!(
            gate.before_retire(request, div_raw(), 29, 29),
            Ok(RiscvLiveRetireGateDecision::Ready)
        );
        assert_eq!(gate.owned_scheduler_wakes(), vec![wake]);
        gate.prune_detached_scheduler_wakes(30);
        assert!(gate.owned_scheduler_wakes().is_empty());
    }

    #[test]
    fn restored_gate_has_no_owned_scheduler_wake_before_rearming() {
        let request = MemoryRequestId::new(AgentId::new(1), 42);
        let mut gate = RiscvLiveRetireGateState::default();
        gate.restore_checkpoint(Some(O3LiveRetireGateCheckpointPayload::new(request, 99)));

        assert!(gate.owned_scheduler_wakes().is_empty());
    }

    #[test]
    fn detailed_nonzero_gate_does_not_start_without_scheduler() {
        let mut gate = RiscvLiveRetireGateState::default();
        gate.set_policy(RiscvLiveRetireGatePolicy::detailed());

        assert!(gate.detailed_policy_enabled());
        assert!(!gate.blocks_without_scheduler());
        assert_eq!(gate.checkpoint(), None);

        gate.set_policy(RiscvLiveRetireGatePolicy::disabled());
        assert!(!gate.detailed_policy_enabled());
    }

    #[test]
    fn pending_gate_rebinds_after_fetch_stream_reset_without_duplicate_schedule() {
        let original = MemoryRequestId::new(AgentId::new(1), 42);
        let refetched = MemoryRequestId::new(AgentId::new(1), 43);
        let mut gate = RiscvLiveRetireGateState::default();
        gate.set_policy(RiscvLiveRetireGatePolicy::detailed());
        assert_eq!(
            gate.before_retire(original, div_raw(), 10, 10),
            Ok(RiscvLiveRetireGateDecision::Schedule {
                ready_tick: 29,
                created_wait_ticks: Some(19),
            })
        );
        gate.mark_scheduled(original, wake(29));

        gate.rebind_pending_to_next_request();

        assert!(!gate.blocks_new_work());
        assert_eq!(
            gate.before_retire(refetched, div_raw(), 28, 28),
            Ok(RiscvLiveRetireGateDecision::Blocked)
        );
        assert_eq!(
            gate.before_retire(refetched, div_raw(), 29, 29),
            Ok(RiscvLiveRetireGateDecision::Ready)
        );
    }

    #[test]
    fn pending_gate_clears_for_pc_redirect() {
        let request = MemoryRequestId::new(AgentId::new(1), 42);
        let mut gate = RiscvLiveRetireGateState::default();
        gate.set_policy(RiscvLiveRetireGatePolicy::detailed());
        assert!(matches!(
            gate.before_retire(request, div_raw(), 10, 10),
            Ok(RiscvLiveRetireGateDecision::Schedule { .. })
        ));
        let wake = wake(29);
        gate.mark_scheduled(request, wake);

        gate.clear_pending_for_pc_redirect();

        assert!(!gate.blocks_new_work());
        assert_eq!(gate.checkpoint(), None);
        assert_eq!(gate.owned_scheduler_wakes(), vec![wake]);
    }

    #[test]
    fn o3_checkpoint_payload_round_trips_pending_gate() {
        let request = MemoryRequestId::new(AgentId::new(2), 7);
        let pending = O3LiveRetireGateCheckpointPayload::new(request, 123);
        let payload =
            RiscvCore::default_o3_runtime_checkpoint_payload().with_live_retire_gate(Some(pending));

        let decoded = O3RuntimeCheckpointPayload::decode(&payload.encode()).unwrap();

        assert_eq!(decoded.live_retire_gate(), Some(pending));
        assert_eq!(decoded.pending_live_retire_gate(), Some((request, 123)));
    }
}

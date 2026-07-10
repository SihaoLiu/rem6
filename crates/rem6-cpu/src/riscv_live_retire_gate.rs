use rem6_isa_riscv::RiscvInstruction;
use rem6_kernel::{SchedulerError, Tick};
use rem6_memory::MemoryRequestId;

use crate::o3_runtime::{
    O3LiveRetireGateCheckpointPayload, O3RuntimeCheckpointPayload, O3RuntimeError,
};
use crate::{riscv_execute::in_order_execute_wait_cycles, RiscvCore, RiscvCpuError};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvLiveRetireGatePending {
    request: MemoryRequestId,
    ready_tick: Tick,
    scheduled: bool,
    rebind_on_next_request: bool,
}

impl RiscvLiveRetireGatePending {
    const fn new(request: MemoryRequestId, ready_tick: Tick) -> Self {
        Self {
            request,
            ready_tick,
            scheduled: false,
            rebind_on_next_request: false,
        }
    }

    const fn restored(request: MemoryRequestId, ready_tick: Tick) -> Self {
        Self {
            request,
            ready_tick,
            scheduled: false,
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
        self.scheduled
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

    fn mark_scheduled(&mut self) {
        self.scheduled = true;
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
        self.pending = payload.map(|payload| {
            RiscvLiveRetireGatePending::restored(payload.request(), payload.ready_tick())
        });
    }

    pub(crate) fn blocks_new_work(&self) -> bool {
        self.pending
            .is_some_and(RiscvLiveRetireGatePending::blocks_new_work)
    }

    pub(crate) fn rebind_pending_to_next_request(&mut self) {
        if let Some(pending) = &mut self.pending {
            pending.rebind_to_next_request();
        }
    }

    pub(crate) fn clear_pending_for_pc_redirect(&mut self) {
        self.pending = None;
    }

    pub(crate) fn before_retire(
        &mut self,
        request: MemoryRequestId,
        raw: u32,
        now: Tick,
        ready_base_tick: Tick,
    ) -> Result<RiscvLiveRetireGateDecision, RiscvCpuError> {
        if let Some(mut pending) = self.pending {
            if pending.request() != request {
                if !pending.rebind_on_next_request {
                    return Ok(RiscvLiveRetireGateDecision::Blocked);
                }
                pending.rebind(request);
            } else if pending.rebind_on_next_request {
                pending.rebind(request);
            }
            if now >= pending.ready_tick() {
                self.pending = None;
                return Ok(RiscvLiveRetireGateDecision::Ready);
            }
            self.pending = Some(pending);
            return Ok(if pending.is_scheduled() {
                RiscvLiveRetireGateDecision::Blocked
            } else {
                RiscvLiveRetireGateDecision::Schedule {
                    ready_tick: pending.ready_tick(),
                    created_wait_ticks: None,
                }
            });
        }

        if !self.policy.creates_gates() {
            return Ok(RiscvLiveRetireGateDecision::Ready);
        }
        let decoded = RiscvInstruction::decode_with_length(raw).map_err(RiscvCpuError::Isa)?;
        let wait_ticks = in_order_execute_wait_cycles(decoded.instruction());
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

    pub(crate) fn blocks_without_scheduler(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn mark_scheduled(&mut self, request: MemoryRequestId) {
        if let Some(pending) = &mut self.pending {
            if pending.request() == request {
                pending.mark_scheduled();
            }
        }
    }
}

impl RiscvCore {
    pub fn set_detailed_live_retire_gate_enabled(&self, detailed: bool) {
        let policy = if detailed {
            RiscvLiveRetireGatePolicy::detailed()
        } else {
            RiscvLiveRetireGatePolicy::disabled()
        };
        self.state
            .lock()
            .expect("riscv core lock")
            .live_retire_gate
            .set_policy(policy);
    }

    pub(crate) fn live_retire_gate_blocks_new_work(&self) -> bool {
        self.state
            .lock()
            .expect("riscv core lock")
            .live_retire_gate
            .blocks_new_work()
    }

    pub fn o3_runtime_checkpoint_payload(&self) -> O3RuntimeCheckpointPayload {
        let state = self.state.lock().expect("riscv core lock");
        state
            .o3_runtime
            .checkpoint_payload()
            .with_live_retire_gate(state.live_retire_gate.checkpoint())
    }

    pub fn restore_o3_runtime_checkpoint_payload(
        &self,
        payload: O3RuntimeCheckpointPayload,
    ) -> Result<(), O3RuntimeError> {
        self.validate_o3_runtime_checkpoint_payload(&payload)?;
        let live_retire_gate = payload.live_retire_gate();
        let mut state = self.state.lock().expect("riscv core lock");
        state.o3_runtime.restore_checkpoint_payload(payload)?;
        state.live_retire_gate.restore_checkpoint(live_retire_gate);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rem6_memory::{AgentId, MemoryRequestId};

    use super::*;

    fn div_raw() -> u32 {
        (1 << 25) | (2 << 20) | (1 << 15) | (4 << 12) | (3 << 7) | 0x33
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
        gate.mark_scheduled(refetched_request);
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
    fn detailed_nonzero_gate_does_not_start_without_scheduler() {
        let mut gate = RiscvLiveRetireGateState::default();
        gate.set_policy(RiscvLiveRetireGatePolicy::detailed());

        assert!(!gate.blocks_without_scheduler());
        assert_eq!(gate.checkpoint(), None);
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
        gate.mark_scheduled(original);

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

        gate.clear_pending_for_pc_redirect();

        assert!(!gate.blocks_new_work());
        assert_eq!(gate.checkpoint(), None);
    }

    #[test]
    fn o3_checkpoint_payload_round_trips_pending_gate() {
        let request = MemoryRequestId::new(AgentId::new(2), 7);
        let pending = O3LiveRetireGateCheckpointPayload::new(request, 123);
        let payload =
            RiscvCore::default_o3_runtime_checkpoint_payload().with_live_retire_gate(Some(pending));

        let decoded = O3RuntimeCheckpointPayload::decode(&payload.encode()).unwrap();

        assert_eq!(decoded.live_retire_gate(), Some(pending));
    }
}

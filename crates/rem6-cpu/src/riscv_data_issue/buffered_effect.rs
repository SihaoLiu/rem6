use rem6_kernel::{ParallelSchedulerContext, PartitionEventId, PartitionedScheduler};
use rem6_memory::{MemoryRequest, MemoryRequestId};
use rem6_transport::{
    MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery, TargetOutcome,
};

use super::{BufferedO3EffectAdmission, OutstandingDataAccess, PreparedDataParallelAccess};
use crate::{
    o3_runtime::o3_memory_result_younger_buffered_effect_destination,
    riscv_execution_mode_handoff::RiscvIssuedScalarMemoryHandoff, RiscvCore, RiscvCoreState,
    RiscvCpuError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BufferedO3Effect {
    pub(super) predecessor: MemoryRequestId,
    pub(super) issue: OutstandingDataAccess,
    pub(super) request: MemoryRequest,
}

impl BufferedO3Effect {
    pub(crate) fn scalar_memory_handoff(
        &self,
    ) -> Option<(RiscvIssuedScalarMemoryHandoff, MemoryRequestId)> {
        Some((self.issue.scalar_memory_handoff()?, self.predecessor))
    }

    pub(crate) fn memory_result_requests(&self) -> Option<(MemoryRequestId, MemoryRequestId)> {
        o3_memory_result_younger_buffered_effect_destination(&self.issue.access)?;
        Some((self.issue.request_id, self.issue.fetch_request))
    }
}

pub(super) enum PreparedDataAccess {
    BufferedEffect(BufferedO3Effect),
    New(OutstandingDataAccess),
}

impl RiscvCoreState {
    pub(crate) fn has_ready_buffered_o3_effect(&self) -> bool {
        self.buffered_o3_effects
            .values()
            .any(|effect| !self.outstanding_data.contains_key(&effect.predecessor))
    }

    pub(crate) fn ready_buffered_o3_effect(&self) -> Option<BufferedO3Effect> {
        self.buffered_o3_effects
            .values()
            .find(|effect| !self.outstanding_data.contains_key(&effect.predecessor))
            .cloned()
    }
}

pub(super) fn buffered_o3_effect_admission(
    state: &RiscvCoreState,
    issue: &OutstandingDataAccess,
) -> BufferedO3EffectAdmission {
    use BufferedO3EffectAdmission::{Blocked, Buffered, NotBuffered};

    let Some(execution) = state.data_access_execution(issue.fetch_request) else {
        return Blocked;
    };
    if let Some(predecessor) = state.o3_runtime.scalar_store_predecessor(execution) {
        return Buffered(predecessor);
    }
    if o3_memory_result_younger_buffered_effect_destination(&issue.access).is_none()
        || !state.o3_runtime.has_live_data_access()
    {
        return NotBuffered;
    }
    if !state.can_overlap_detailed_memory_result_event(execution) {
        return Blocked;
    }
    state
        .o3_runtime
        .memory_result_effect_predecessor(execution)
        .map_or(Blocked, Buffered)
}

impl RiscvCore {
    pub(super) fn o3_buffered_effect_predecessor(
        &self,
        issue: &OutstandingDataAccess,
    ) -> BufferedO3EffectAdmission {
        let state = self.state.lock().expect("riscv core lock");
        buffered_o3_effect_admission(&state, issue)
    }

    pub(super) fn ready_buffered_o3_effect(&self) -> Option<BufferedO3Effect> {
        self.state
            .lock()
            .expect("riscv core lock")
            .ready_buffered_o3_effect()
    }

    pub(super) fn submit_buffered_o3_effect<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        buffered: BufferedO3Effect,
        responder: F,
    ) -> Result<PartitionEventId, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut rem6_kernel::SchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let request_id = buffered.issue.request_id;
        let responder_core = self.clone();
        let core = self.clone();
        let event = transport
            .submit(
                scheduler,
                buffered.issue.memory_route(),
                buffered.request,
                trace,
                move |delivery, context| {
                    if responder_core.owns_outstanding_data_request(request_id) {
                        responder(delivery, context)
                    } else {
                        TargetOutcome::NoResponse
                    }
                },
                move |delivery| core.record_data_response(delivery),
            )
            .map_err(RiscvCpuError::Transport)?;
        self.record_buffered_o3_effect_submission(request_id);
        Ok(event)
    }

    pub(super) fn prepare_buffered_o3_effect_parallel<F>(
        &self,
        buffered: BufferedO3Effect,
        trace: MemoryTrace,
        responder: F,
    ) -> PreparedDataParallelAccess
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let request_id = buffered.issue.request_id;
        let responder_core = self.clone();
        let core = self.clone();
        let transaction = ParallelMemoryTransaction::new(
            buffered.issue.memory_route(),
            buffered.request,
            trace,
            move |delivery, context| {
                if responder_core.owns_outstanding_data_request(request_id) {
                    responder(delivery, context)
                } else {
                    TargetOutcome::NoResponse
                }
            },
            move |delivery| core.record_data_response(delivery),
        );
        PreparedDataParallelAccess::buffered_transaction(request_id, transaction)
    }

    pub(crate) fn schedule_prepared_buffered_o3_effect_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        issue: OutstandingDataAccess,
        request: MemoryRequest,
        predecessor: MemoryRequestId,
    ) -> Result<Option<PartitionEventId>, RiscvCpuError> {
        let event = scheduler
            .schedule_parallel_at(self.partition(), scheduler.now(), |_context| {})
            .map_err(RiscvCpuError::Scheduler)?;
        if !self.record_buffered_o3_effect_issue_state(issue, request, predecessor) {
            scheduler
                .cancel_event(event)
                .map_err(RiscvCpuError::Scheduler)?;
            return Ok(None);
        }
        Ok(Some(event))
    }

    pub(super) fn schedule_buffered_o3_effect(
        &self,
        scheduler: &mut PartitionedScheduler,
        issue: OutstandingDataAccess,
        request: MemoryRequest,
        predecessor: MemoryRequestId,
    ) -> Result<Option<PartitionEventId>, RiscvCpuError> {
        let event = scheduler
            .schedule_at(self.partition(), scheduler.now(), |_context| {})
            .map_err(RiscvCpuError::Scheduler)?;
        let recorded = self.record_buffered_o3_effect_issue_state(issue, request, predecessor);
        if !recorded {
            scheduler
                .cancel_event(event)
                .map_err(RiscvCpuError::Scheduler)?;
            self.clear_deferred_o3_live_data_access_execution();
        }
        Ok(recorded.then_some(event))
    }

    pub(crate) fn record_buffered_o3_effect_submission(&self, request_id: MemoryRequestId) {
        let removed = self
            .state
            .lock()
            .expect("riscv core lock")
            .buffered_o3_effects
            .remove(&request_id);
        assert!(removed.is_some(), "submitted O3 effect was buffered");
    }
}

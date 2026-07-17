use rem6_kernel::{ParallelSchedulerContext, PartitionEventId, PartitionedScheduler};
use rem6_memory::{MemoryRequest, MemoryRequestId};
use rem6_transport::{
    MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery, TargetOutcome,
};

use super::{OutstandingDataAccess, PreparedDataParallelAccess};
use crate::riscv_execution_mode_handoff::RiscvIssuedScalarMemoryHandoff;
use crate::{RiscvCore, RiscvCoreState, RiscvCpuError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BufferedO3Store {
    predecessor: MemoryRequestId,
    issue: OutstandingDataAccess,
    request: MemoryRequest,
}

impl BufferedO3Store {
    pub(crate) fn scalar_memory_handoff(
        &self,
    ) -> Option<(RiscvIssuedScalarMemoryHandoff, MemoryRequestId)> {
        Some((self.issue.scalar_memory_handoff()?, self.predecessor))
    }
}

pub(super) enum PreparedDataAccess {
    BufferedStore(BufferedO3Store),
    New(OutstandingDataAccess),
}

impl RiscvCoreState {
    pub(crate) fn has_ready_buffered_o3_store(&self) -> bool {
        self.buffered_o3_stores
            .values()
            .any(|store| !self.outstanding_data.contains_key(&store.predecessor))
    }

    pub(crate) fn ready_buffered_o3_store(&self) -> Option<BufferedO3Store> {
        self.buffered_o3_stores
            .values()
            .find(|store| !self.outstanding_data.contains_key(&store.predecessor))
            .cloned()
    }
}

impl RiscvCore {
    pub(super) fn o3_store_predecessor(
        &self,
        issue: &OutstandingDataAccess,
    ) -> Option<MemoryRequestId> {
        let state = self.state.lock().expect("riscv core lock");
        let execution = state.data_access_execution(issue.fetch_request)?;
        state.o3_runtime.scalar_store_predecessor(execution)
    }

    pub(super) fn ready_buffered_o3_store(&self) -> Option<BufferedO3Store> {
        self.state
            .lock()
            .expect("riscv core lock")
            .ready_buffered_o3_store()
    }

    pub(super) fn submit_buffered_o3_store<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        buffered: BufferedO3Store,
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
        self.record_buffered_o3_store_submission(request_id);
        Ok(event)
    }

    pub(super) fn prepare_buffered_o3_store_parallel<F>(
        &self,
        buffered: BufferedO3Store,
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

    pub(crate) fn schedule_prepared_buffered_o3_store_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        issue: OutstandingDataAccess,
        request: MemoryRequest,
        predecessor: MemoryRequestId,
    ) -> Result<PartitionEventId, RiscvCpuError> {
        let event = scheduler
            .schedule_parallel_at(self.partition(), scheduler.now(), |_context| {})
            .map_err(RiscvCpuError::Scheduler)?;
        self.record_buffered_o3_store_issue(issue, request, predecessor);
        Ok(event)
    }

    pub(super) fn schedule_buffered_o3_store(
        &self,
        scheduler: &mut PartitionedScheduler,
        issue: OutstandingDataAccess,
        request: MemoryRequest,
        predecessor: MemoryRequestId,
    ) -> Result<PartitionEventId, RiscvCpuError> {
        let event = scheduler
            .schedule_at(self.partition(), scheduler.now(), |_context| {})
            .map_err(RiscvCpuError::Scheduler)?;
        self.record_buffered_o3_store_issue(issue, request, predecessor);
        Ok(event)
    }

    fn record_buffered_o3_store_issue(
        &self,
        issue: OutstandingDataAccess,
        request: MemoryRequest,
        predecessor: MemoryRequestId,
    ) {
        let request_id = issue.request_id;
        let buffered = BufferedO3Store {
            predecessor,
            issue: issue.clone(),
            request,
        };
        self.record_data_issue(issue);
        let replaced = self
            .state
            .lock()
            .expect("riscv core lock")
            .buffered_o3_stores
            .insert(request_id, buffered);
        assert!(replaced.is_none(), "buffered O3 store request is unique");
    }

    pub(crate) fn record_buffered_o3_store_submission(&self, request_id: MemoryRequestId) {
        let removed = self
            .state
            .lock()
            .expect("riscv core lock")
            .buffered_o3_stores
            .remove(&request_id);
        assert!(removed.is_some(), "submitted O3 store was buffered");
    }
}

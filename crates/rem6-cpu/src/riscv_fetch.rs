use rem6_isa_riscv::{RiscvPmpAccessKind, RiscvPrivilegeMode};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionedScheduler, SchedulerContext, Tick,
};
use rem6_memory::{AccessSize, Address, MemoryRequest};
use rem6_transport::{
    MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery, TargetOutcome,
};

use crate::{OutstandingFetch, RiscvCore, RiscvCpuError};

impl RiscvCore {
    pub fn issue_next_fetch<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<PartitionEventId, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        let issue = self.prepare_fetch(scheduler.now(), transport)?;
        let request = MemoryRequest::instruction_fetch(
            issue.request_id,
            issue.pc,
            issue.size,
            issue.line_layout,
        )
        .map_err(RiscvCpuError::Memory)?;

        let core = self.inner();
        let event = transport
            .submit(
                scheduler,
                issue.route,
                request,
                trace,
                responder,
                move |delivery| core.record_response(delivery),
            )
            .map_err(RiscvCpuError::Transport)?;

        self.inner().record_issue(issue);
        Ok(event)
    }

    pub fn issue_next_fetch_parallel<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<PartitionEventId, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let (issue, transaction) =
            self.prepare_fetch_parallel_transaction(scheduler.now(), transport, trace, responder)?;
        let event = transport
            .submit_parallel_batch(scheduler, [transaction])
            .map_err(RiscvCpuError::Transport)?
            .into_iter()
            .next()
            .expect("single fetch transaction returns one event");

        self.inner().record_issue(issue);
        Ok(event)
    }

    pub(crate) fn prepare_fetch_parallel_transaction<F>(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<(OutstandingFetch, ParallelMemoryTransaction), RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let issue = self.prepare_fetch(tick, transport)?;
        let request = MemoryRequest::instruction_fetch(
            issue.request_id,
            issue.pc,
            issue.size,
            issue.line_layout,
        )
        .map_err(RiscvCpuError::Memory)?;

        let core = self.inner();
        let transaction = ParallelMemoryTransaction::new(
            issue.route,
            request,
            trace,
            responder,
            move |delivery| core.record_response(delivery),
        );
        Ok((issue, transaction))
    }

    pub(crate) fn record_prepared_fetch_issue(&self, issue: OutstandingFetch) {
        self.inner().record_issue(issue);
    }

    fn prepare_fetch(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
    ) -> Result<OutstandingFetch, RiscvCpuError> {
        let issue = self
            .inner()
            .prepare_fetch(tick, transport)
            .map_err(RiscvCpuError::Cpu)?;
        self.check_pmp_fetch_access(issue.pc, issue.size)?;
        Ok(issue)
    }

    fn check_pmp_fetch_access(&self, pc: Address, size: AccessSize) -> Result<(), RiscvCpuError> {
        self.state
            .lock()
            .expect("riscv core lock")
            .pmp
            .check_access(
                pc.get(),
                size.bytes(),
                RiscvPmpAccessKind::Execute,
                RiscvPrivilegeMode::Machine,
            )
            .map_err(|error| RiscvCpuError::FetchPmpAccess { pc, error })
    }
}

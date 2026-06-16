use rem6_isa_riscv::{RiscvPmpAccessKind, RiscvPrivilegeMode};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionedScheduler, SchedulerContext, Tick,
};
use rem6_memory::{AccessSize, Address, MemoryRequest, MemoryRequestId};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery,
    ResponseDelivery, TargetOutcome, TransportEndpointId,
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
        let request = self.apply_pma_fetch_request_attributes(issue.pc, issue.size, request)?;

        let core = self.clone();
        let event = transport
            .submit(
                scheduler,
                issue.route,
                request,
                trace,
                responder,
                move |delivery| core.record_fetch_response(delivery),
            )
            .map_err(RiscvCpuError::Transport)?;

        self.inner().record_issue(issue);
        self.sync_in_order_fetch_state()?;
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
        self.sync_in_order_fetch_state()?;
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
        let request = self.apply_pma_fetch_request_attributes(issue.pc, issue.size, request)?;

        let core = self.clone();
        let transaction = ParallelMemoryTransaction::new(
            issue.route,
            request,
            trace,
            responder,
            move |delivery| core.record_fetch_response(delivery),
        );
        Ok((issue, transaction))
    }

    pub(crate) fn record_prepared_fetch_issue(
        &self,
        issue: OutstandingFetch,
    ) -> Result<(), RiscvCpuError> {
        self.inner().record_issue(issue);
        self.sync_in_order_fetch_state()
    }

    pub fn record_fetch_failure(
        &self,
        request_id: MemoryRequestId,
        tick: Tick,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
    ) {
        self.inner()
            .record_fetch_failure(request_id, tick, route, endpoint);
        self.sync_in_order_fetch_state()
            .expect("fetch failure sync preserves canonical pipeline state");
    }

    fn record_fetch_response(&self, delivery: ResponseDelivery) {
        self.inner().record_response(delivery);
        self.sync_in_order_fetch_state()
            .expect("fetch response sync preserves canonical pipeline state");
    }

    fn prepare_fetch(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
    ) -> Result<OutstandingFetch, RiscvCpuError> {
        let needs_fetch_suffix = self
            .state
            .lock()
            .expect("riscv core lock")
            .pending_fetch_prefix
            .is_some();
        let issue = if needs_fetch_suffix {
            self.inner()
                .prepare_fetch_with_explicit_size(
                    tick,
                    transport,
                    AccessSize::new(2).expect("RISC-V fetch suffix width is nonzero"),
                )
                .map_err(RiscvCpuError::Cpu)?
        } else {
            match self.inner().prepare_fetch(tick, transport) {
                Ok(issue) => issue,
                Err(crate::CpuError::FetchCrossesLine { size, .. }) if size.bytes() > 2 => self
                    .inner()
                    .prepare_fetch_with_explicit_size(
                        tick,
                        transport,
                        AccessSize::new(2).expect("RISC-V compressed fetch width is nonzero"),
                    )
                    .map_err(RiscvCpuError::Cpu)?,
                Err(error) => return Err(RiscvCpuError::Cpu(error)),
            }
        };
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

    fn apply_pma_fetch_request_attributes(
        &self,
        pc: Address,
        size: AccessSize,
        request: MemoryRequest,
    ) -> Result<MemoryRequest, RiscvCpuError> {
        let is_uncacheable = self
            .state
            .lock()
            .expect("riscv core lock")
            .pma
            .is_uncacheable(pc.get(), size.bytes())
            .map_err(|error| RiscvCpuError::FetchPmaAccess { pc, error })?;
        if is_uncacheable {
            Ok(request.with_uncacheable_strict_order())
        } else {
            Ok(request)
        }
    }
}

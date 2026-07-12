use rem6_kernel::{PartitionEventId, PartitionedScheduler};
use rem6_memory::{MemoryRequest, MemoryRequestId};
use rem6_transport::{MemoryTransport, ParallelMemoryTransaction};

use super::OutstandingDataAccess;
use crate::{RiscvCore, RiscvCpuError};

#[allow(clippy::large_enum_variant)]
pub(crate) enum PreparedDataParallelAccess {
    Transaction {
        issue: OutstandingDataAccess,
        transaction: ParallelMemoryTransaction,
        cleanup: PreparedDataIssueCleanup,
    },
    ConditionalFailed {
        issue: OutstandingDataAccess,
        cleanup: PreparedDataIssueCleanup,
    },
    Forwarded {
        issue: OutstandingDataAccess,
        cleanup: PreparedDataIssueCleanup,
    },
    BufferedStore {
        issue: OutstandingDataAccess,
        request: MemoryRequest,
        predecessor: MemoryRequestId,
        cleanup: PreparedDataIssueCleanup,
    },
    BufferedTransaction {
        request_id: MemoryRequestId,
        transaction: ParallelMemoryTransaction,
    },
}

impl PreparedDataParallelAccess {
    pub(crate) fn transaction(
        core: &RiscvCore,
        issue: OutstandingDataAccess,
        transaction: ParallelMemoryTransaction,
    ) -> Self {
        let cleanup = PreparedDataIssueCleanup::new(core, issue.fetch_request);
        Self::Transaction {
            issue,
            transaction,
            cleanup,
        }
    }

    pub(crate) fn conditional_failed(core: &RiscvCore, issue: OutstandingDataAccess) -> Self {
        let cleanup = PreparedDataIssueCleanup::new(core, issue.fetch_request);
        Self::ConditionalFailed { issue, cleanup }
    }

    pub(crate) fn forwarded(core: &RiscvCore, issue: OutstandingDataAccess) -> Self {
        let cleanup = PreparedDataIssueCleanup::new(core, issue.fetch_request);
        Self::Forwarded { issue, cleanup }
    }

    pub(crate) fn buffered_store(
        core: &RiscvCore,
        issue: OutstandingDataAccess,
        request: MemoryRequest,
        predecessor: MemoryRequestId,
    ) -> Self {
        let cleanup = PreparedDataIssueCleanup::new(core, issue.fetch_request);
        Self::BufferedStore {
            issue,
            request,
            predecessor,
            cleanup,
        }
    }

    pub(crate) fn buffered_transaction(
        request_id: MemoryRequestId,
        transaction: ParallelMemoryTransaction,
    ) -> Self {
        Self::BufferedTransaction {
            request_id,
            transaction,
        }
    }
}

impl RiscvCore {
    pub(super) fn submit_prepared_data_parallel_access(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        prepared: PreparedDataParallelAccess,
    ) -> Result<PartitionEventId, RiscvCpuError> {
        match prepared {
            PreparedDataParallelAccess::Transaction {
                issue,
                transaction,
                cleanup,
            } => {
                let event = submit_single_parallel_data(scheduler, transport, transaction)?;
                self.record_data_issue(issue);
                cleanup.disarm();
                Ok(event)
            }
            PreparedDataParallelAccess::ConditionalFailed { issue, cleanup } => {
                let event = self.schedule_store_conditional_failure_parallel(scheduler, issue)?;
                cleanup.disarm();
                Ok(event)
            }
            PreparedDataParallelAccess::Forwarded { issue, cleanup } => {
                let event = self.schedule_forwarded_load_completion_parallel(scheduler, issue)?;
                cleanup.disarm();
                Ok(event)
            }
            PreparedDataParallelAccess::BufferedStore {
                issue,
                request,
                predecessor,
                cleanup,
            } => {
                let event = self.schedule_prepared_buffered_o3_store_parallel(
                    scheduler,
                    issue,
                    request,
                    predecessor,
                )?;
                cleanup.disarm();
                Ok(event)
            }
            PreparedDataParallelAccess::BufferedTransaction {
                request_id,
                transaction,
            } => {
                let event = submit_single_parallel_data(scheduler, transport, transaction)?;
                self.record_buffered_o3_store_submission(request_id);
                Ok(event)
            }
        }
    }
}

fn submit_single_parallel_data(
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    transaction: ParallelMemoryTransaction,
) -> Result<PartitionEventId, RiscvCpuError> {
    Ok(transport
        .submit_parallel_batch(scheduler, [transaction])
        .map_err(RiscvCpuError::Transport)?
        .into_iter()
        .next()
        .expect("single data transaction returns one event"))
}

pub(crate) struct PreparedDataIssueCleanup {
    core: RiscvCore,
    fetch_request: MemoryRequestId,
    armed: bool,
}

impl PreparedDataIssueCleanup {
    fn new(core: &RiscvCore, fetch_request: MemoryRequestId) -> Self {
        Self {
            core: core.clone(),
            fetch_request,
            armed: true,
        }
    }

    pub(crate) fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for PreparedDataIssueCleanup {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.core
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .o3_runtime
            .abort_deferred_scalar_memory_execution(self.fetch_request);
    }
}

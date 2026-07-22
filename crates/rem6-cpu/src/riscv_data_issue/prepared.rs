use rem6_kernel::{PartitionEventId, PartitionedScheduler};
use rem6_memory::{MemoryRequest, MemoryRequestId};
use rem6_transport::{MemoryTransport, ParallelMemoryTransaction};

use super::OutstandingDataAccess;
use crate::{RiscvCore, RiscvCoreState, RiscvCpuError};

impl RiscvCoreState {
    pub(crate) fn abort_prepared_data_issue(&mut self, fetch_request: MemoryRequestId) -> bool {
        let pending = self
            .o3_runtime
            .discard_pending_data_address_for_fetch(fetch_request);
        self.abort_deferred_o3_live_data_access_execution(fetch_request) || pending
    }
}

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
    BufferedEffect {
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

    pub(crate) fn buffered_effect(
        core: &RiscvCore,
        issue: OutstandingDataAccess,
        request: MemoryRequest,
        predecessor: MemoryRequestId,
    ) -> Self {
        let cleanup = PreparedDataIssueCleanup::new(core, issue.fetch_request);
        Self::BufferedEffect {
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
    ) -> Result<Option<PartitionEventId>, RiscvCpuError> {
        match prepared {
            PreparedDataParallelAccess::Transaction {
                issue,
                transaction,
                cleanup,
            } => {
                if !cleanup.is_current() {
                    return Ok(None);
                }
                let event = submit_single_parallel_data(scheduler, transport, transaction)?;
                self.record_data_issue(issue);
                cleanup.disarm();
                Ok(Some(event))
            }
            PreparedDataParallelAccess::ConditionalFailed { issue, cleanup } => {
                if !cleanup.is_current() {
                    return Ok(None);
                }
                let event = self.schedule_store_conditional_failure_parallel(scheduler, issue)?;
                cleanup.disarm();
                Ok(Some(event))
            }
            PreparedDataParallelAccess::Forwarded { issue, cleanup } => {
                if !cleanup.is_current() {
                    return Ok(None);
                }
                let event = self.schedule_forwarded_load_completion_parallel(scheduler, issue)?;
                cleanup.disarm();
                Ok(Some(event))
            }
            PreparedDataParallelAccess::BufferedEffect {
                issue,
                request,
                predecessor,
                cleanup,
            } => {
                let event = self.schedule_prepared_buffered_o3_effect_parallel(
                    scheduler,
                    issue,
                    request,
                    predecessor,
                )?;
                if event.is_some() {
                    cleanup.disarm();
                }
                Ok(event)
            }
            PreparedDataParallelAccess::BufferedTransaction {
                request_id,
                transaction,
            } => {
                if !self.owns_ready_buffered_o3_effect(request_id) {
                    return Ok(None);
                }
                let event = submit_single_parallel_data(scheduler, transport, transaction)?;
                self.record_buffered_o3_effect_submission(request_id);
                Ok(Some(event))
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
    owner: PreparedDataIssueOwner,
    armed: bool,
}

impl PreparedDataIssueCleanup {
    fn new(core: &RiscvCore, fetch_request: MemoryRequestId) -> Self {
        let state = core
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let owner = if state
            .o3_runtime
            .pending_data_address_execution_for_fetch(fetch_request)
            .is_some()
        {
            PreparedDataIssueOwner::PendingAddress
        } else if state.o3_runtime.deferred_live_data_access_execution() == Some(fetch_request) {
            PreparedDataIssueOwner::DeferredLiveDataAccess
        } else {
            PreparedDataIssueOwner::Execution
        };
        drop(state);
        Self {
            core: core.clone(),
            fetch_request,
            owner,
            armed: true,
        }
    }

    pub(crate) fn is_current(&self) -> bool {
        let state = self
            .core
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match self.owner {
            PreparedDataIssueOwner::PendingAddress => state
                .o3_runtime
                .pending_data_address_execution_for_fetch(self.fetch_request)
                .is_some(),
            PreparedDataIssueOwner::DeferredLiveDataAccess => {
                state.o3_runtime.deferred_live_data_access_execution() == Some(self.fetch_request)
            }
            PreparedDataIssueOwner::Execution => {
                !state.issued_data_for_fetches.contains(&self.fetch_request)
                    && state.data_access_execution(self.fetch_request).is_some()
            }
        }
    }

    pub(crate) fn disarm(mut self) {
        self.armed = false;
    }
}

#[derive(Clone, Copy)]
enum PreparedDataIssueOwner {
    PendingAddress,
    DeferredLiveDataAccess,
    Execution,
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
            .abort_prepared_data_issue(self.fetch_request);
    }
}

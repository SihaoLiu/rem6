use rem6_memory::MemoryRequestId;
use rem6_transport::ParallelMemoryTransaction;

use super::OutstandingDataAccess;
use crate::RiscvCore;

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

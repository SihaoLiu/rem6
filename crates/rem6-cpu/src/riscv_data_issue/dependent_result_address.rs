use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PendingAddressPreSubmit {
    NotPending,
    Ready,
    Replay,
}

pub(super) fn pending_address_preparation_error_replays(error: &RiscvCpuError) -> bool {
    matches!(
        error,
        RiscvCpuError::Transport(TransportError::UnknownRoute { .. })
            | RiscvCpuError::DataRoutePartitionMismatch { .. }
            | RiscvCpuError::DataRouteEndpointMismatch { .. }
            | RiscvCpuError::DataPmpAccess { .. }
            | RiscvCpuError::DataPmaAccess { .. }
            | RiscvCpuError::DataAccessCrossesLine { .. }
    )
}

impl RiscvCore {
    pub(super) fn pending_address_preparation_failure_is_replay(
        &self,
        fetch_request: MemoryRequestId,
        error: &RiscvCpuError,
    ) -> bool {
        pending_address_preparation_error_replays(error)
            && self
                .state
                .lock()
                .expect("riscv core lock")
                .o3_runtime
                .pending_data_address_owns_fetch(fetch_request)
    }

    pub(super) fn pending_address_owns_fetch(&self, fetch_request: MemoryRequestId) -> bool {
        self.state
            .lock()
            .expect("riscv core lock")
            .o3_runtime
            .pending_data_address_owns_fetch(fetch_request)
    }

    pub(super) fn validate_pending_address_pre_submit(
        &self,
        issue: &OutstandingDataAccess,
    ) -> PendingAddressPreSubmit {
        let state = self.state.lock().expect("riscv core lock");
        if !state
            .o3_runtime
            .pending_data_address_owns_fetch(issue.fetch_request)
        {
            return PendingAddressPreSubmit::NotPending;
        }
        if !state.o3_runtime.pending_data_address_issue_matches(
            issue.fetch_request,
            &issue.access,
            issue.physical_address,
            issue.size,
            issue.tick,
        ) || !matches!(issue.target, RiscvDataAccessTarget::Memory { .. })
            || issue.request_byte_offset != 0
            || state
                .events
                .iter()
                .any(|event| event.fetch().request_id() == issue.fetch_request)
        {
            return PendingAddressPreSubmit::Replay;
        }
        let Some(line_layout) = issue.line_layout else {
            return PendingAddressPreSubmit::Replay;
        };
        if line_layout.line_offset(issue.physical_address) + issue.size.bytes()
            > line_layout.bytes()
        {
            return PendingAddressPreSubmit::Replay;
        }
        if !matches!(
            state
                .pma
                .is_uncacheable(issue.physical_address.get(), issue.size.bytes(),),
            Ok(false)
        ) {
            return PendingAddressPreSubmit::Replay;
        }
        PendingAddressPreSubmit::Ready
    }

    pub(super) fn replay_pending_address_before_submit(&self, fetch_request: MemoryRequestId) {
        let mut state = self.state.lock().expect("riscv core lock");
        if state
            .o3_runtime
            .pending_data_address_owns_fetch(fetch_request)
        {
            state.o3_runtime.discard_pending_data_address();
        }
    }
}

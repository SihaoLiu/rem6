use super::*;

impl RiscvCoreState {
    pub(crate) fn abort_deferred_o3_live_data_access_execution(
        &mut self,
        fetch_request: MemoryRequestId,
    ) -> bool {
        let abort_pair = self
            .memory_result_window_authorizations
            .get(&fetch_request)
            .is_some_and(|authorization| authorization.role() == O3MemoryResultWindowRole::Head);
        let mut requests = vec![fetch_request];
        if abort_pair {
            requests.extend(self.memory_result_window_authorizations.iter().filter_map(
                |(request, authorization)| authorization.role().is_younger().then_some(*request),
            ));
        }
        let mut requested_aborted = false;
        for request in requests {
            self.memory_result_window_authorizations.remove(&request);
            let aborted = self
                .o3_runtime
                .abort_deferred_live_data_access_execution(request);
            if self
                .pending_terminal_memory_result
                .as_ref()
                .is_some_and(|pending| pending.owns_fetch(request))
            {
                assert!(
                    aborted,
                    "pending terminal result owns deferred O3 data execution"
                );
                self.pending_terminal_memory_result = None;
            }
            if aborted {
                if let Some(event) = self.data_access_execution_mut(request) {
                    event.clear_data_access_retirement();
                }
            }
            if request == fetch_request {
                requested_aborted = aborted;
            }
        }
        requested_aborted
    }
}

pub(super) fn mark_data_access_event_kind(
    state: &mut RiscvCoreState,
    access: &IssuedDataAccess,
    kind: RiscvDataAccessEventKind,
) -> Option<RiscvCpuExecutionEvent> {
    let event = state.data_access_execution_mut(access.fetch_request)?;
    event.set_data_access_event_kind(kind);
    Some(event.clone())
}

pub(super) fn cloned_data_access_event_with_kind(
    state: &RiscvCoreState,
    access: &IssuedDataAccess,
    kind: RiscvDataAccessEventKind,
) -> Option<RiscvCpuExecutionEvent> {
    let mut event = state.data_access_execution(access.fetch_request)?.clone();
    event.set_data_access_event_kind(kind);
    Some(event)
}

pub(super) fn record_callback_error(state: &mut RiscvCoreState, error: O3RuntimeError) {
    if state.pending_callback_error.is_none() {
        state.pending_callback_error = Some(RiscvCpuError::O3Runtime(error));
    }
}

pub(super) fn record_o3_data_access_outcome(
    state: &mut RiscvCoreState,
    access: &IssuedDataAccess,
    execution: Option<RiscvCpuExecutionEvent>,
    response_tick: Tick,
    completion: Option<RiscvDataCompletion>,
    forwarding_plan: Option<O3StoreLoadForwardingPlan>,
) -> Result<bool, O3RuntimeError> {
    let Some(execution) = execution else {
        state.buffered_o3_effects.remove(&access.request);
        state
            .o3_runtime
            .discard_data_access_outcome(access.fetch_request);
        return Ok(false);
    };
    let latency_ticks = response_tick.saturating_sub(access.tick);
    let squash_younger_requests = matches!(
        execution.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::Retry | RiscvDataAccessEventKind::Failed)
    )
    .then(|| {
        state
            .o3_runtime
            .younger_live_scalar_memory_requests(access.fetch_request, access.request)
    })
    .unwrap_or_default();
    let discard_translated_suffix = matches!(
        execution.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::Retry | RiscvDataAccessEventKind::Failed)
    );
    let completed_live_data_access = if let Some(forwarding_plan) = forwarding_plan {
        match completion {
            Some(completion) => state
                .o3_runtime
                .complete_live_scalar_memory_forwarding_completion(
                    &execution,
                    access.request,
                    response_tick,
                    latency_ticks,
                    (
                        access.physical_address,
                        access.size,
                        access.request_byte_offset,
                    ),
                    completion,
                    forwarding_plan,
                )?,
            None => false,
        }
    } else {
        state.o3_runtime.complete_live_data_access_completion(
            &execution,
            access.request,
            response_tick,
            latency_ticks,
            (
                access.physical_address,
                access.size,
                access.request_byte_offset,
            ),
            completion,
        )?
    };
    state.refresh_o3_writeback_wake(response_tick);
    if completed_live_data_access {
        state.buffered_o3_effects.remove(&access.request);
        if discard_translated_suffix {
            state.discard_translated_result_pair_from(access.fetch_request);
        }
        for (request, fetch_request) in squash_younger_requests {
            state.outstanding_data.remove(&request);
            state.buffered_o3_effects.remove(&request);
            state.issued_data_for_fetches.remove(&fetch_request);
            if let Some(event) = state.data_access_execution_mut(fetch_request) {
                event.clear_data_access_retirement();
            }
        }
        return Ok(true);
    }
    if discard_translated_suffix {
        state.discard_translated_result_pair_from(access.fetch_request);
        state
            .o3_runtime
            .discard_data_access_outcome(access.fetch_request);
    } else {
        state
            .o3_runtime
            .record_data_access_outcome(&execution, response_tick, latency_ticks);
    }
    Ok(false)
}

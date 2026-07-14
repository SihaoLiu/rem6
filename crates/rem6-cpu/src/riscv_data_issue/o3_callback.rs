use super::*;

pub(super) fn mark_data_access_event_kind(
    state: &mut RiscvCoreState,
    access: &IssuedDataAccess,
    kind: RiscvDataAccessEventKind,
) -> Option<RiscvCpuExecutionEvent> {
    let event = state
        .events
        .iter_mut()
        .find(|event| event.fetch().request_id() == access.fetch_request)?;
    event.set_data_access_event_kind(kind);
    Some(event.clone())
}

pub(super) fn cloned_data_access_event_with_kind(
    state: &RiscvCoreState,
    access: &IssuedDataAccess,
    kind: RiscvDataAccessEventKind,
) -> Option<RiscvCpuExecutionEvent> {
    let mut event = state
        .events
        .iter()
        .find(|event| event.fetch().request_id() == access.fetch_request)?
        .clone();
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
    load_data: Option<&[u8]>,
    forwarding_plan: Option<O3StoreLoadForwardingPlan>,
) -> Result<bool, O3RuntimeError> {
    let Some(execution) = execution else {
        state.buffered_o3_stores.remove(&access.request);
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
    let completed_live_scalar_memory = if let Some(forwarding_plan) = forwarding_plan {
        match load_data {
            Some(data) => state.o3_runtime.complete_live_scalar_memory_forwarding(
                &execution,
                access.request,
                response_tick,
                latency_ticks,
                data,
                forwarding_plan,
            )?,
            None => false,
        }
    } else {
        state.o3_runtime.complete_live_scalar_memory_response(
            &execution,
            access.request,
            response_tick,
            latency_ticks,
            load_data,
        )?
    };
    if completed_live_scalar_memory {
        state.buffered_o3_stores.remove(&access.request);
        for (request, fetch_request) in squash_younger_requests {
            state.outstanding_data.remove(&request);
            state.buffered_o3_stores.remove(&request);
            state.issued_data_for_fetches.remove(&fetch_request);
            if let Some(event) = state
                .events
                .iter_mut()
                .find(|event| event.fetch().request_id() == fetch_request)
            {
                event.clear_data_access_retirement();
            }
        }
        return Ok(true);
    }
    if matches!(
        execution.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::Retry | RiscvDataAccessEventKind::Failed)
    ) {
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

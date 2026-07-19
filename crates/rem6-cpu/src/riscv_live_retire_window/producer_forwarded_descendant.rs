use super::*;

pub(crate) fn stage_o3_producer_forwarded_control_descendant(
    state: &mut RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> bool {
    stage_o3_producer_forwarded_control_descendant_inner(state, fetch_events, None)
}

pub(crate) fn stage_o3_producer_forwarded_control_descendant_for_response(
    state: &mut RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    completed_request: MemoryRequestId,
) -> bool {
    stage_o3_producer_forwarded_control_descendant_inner(
        state,
        fetch_events,
        Some(completed_request),
    )
}

fn stage_o3_producer_forwarded_control_descendant_inner(
    state: &mut RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    completed_request: Option<MemoryRequestId>,
) -> bool {
    let Some((authority, head)) = state
        .o3_runtime
        .producer_forwarded_descendant_issue_context()
    else {
        return false;
    };
    let crate::riscv_fetch_ahead::RecordedPredictedPc::Ready(target) =
        crate::riscv_fetch_ahead::recorded_predicted_pc(
            state,
            authority.fetch_request(),
            authority.sequential_pc(),
            &crate::riscv_fetch_ahead::PredictedControlTargetAuthority::ProducerForwarded(
                authority,
            ),
        )
    else {
        return false;
    };
    let Some(descendant) =
        completed_fetch_instruction_at(state, fetch_events, authority.last_fetch_request(), target)
    else {
        return false;
    };
    if completed_request.is_some_and(|request| !descendant.consumed_requests().contains(&request)) {
        return false;
    }
    let descendant_instruction = descendant.decoded().instruction();
    if crate::o3_runtime::o3_exact_link_return_source(descendant_instruction).is_some()
        && state.branch_speculations.len() >= state.branch_lookahead
    {
        return false;
    }
    if state
        .o3_runtime
        .append_producer_forwarded_control_descendant(
            authority,
            descendant.pc(),
            descendant_instruction,
            descendant.consumed_requests(),
        )
        .is_none()
    {
        return false;
    }
    let issue_tick = descendant
        .consumed_requests()
        .iter()
        .filter_map(|request| {
            fetch_events
                .iter()
                .find(|event| {
                    event.kind() == CpuFetchEventKind::Completed && event.request_id() == *request
                })
                .map(CpuFetchEvent::tick)
        })
        .max()
        .unwrap_or_else(|| descendant.fetch().tick());
    schedule_o3_live_speculative_younger_executions(
        state,
        head,
        std::slice::from_ref(&descendant),
        issue_tick,
    )
    .expect("producer-forwarded control descendant writeback reservation");
    if let Some(scalar_chain) = state.o3_runtime.producer_forwarded_scalar_chain() {
        let continuation = crate::riscv_fetch_ahead::ProducerForwardedScalarContinuation::capture(
            state,
            scalar_chain,
        );
        state.producer_forwarded_scalar_continuation = continuation;
    }
    true
}

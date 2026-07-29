use super::*;

pub(super) struct OperationalFetchProjection {
    pub(super) fetches: Vec<crate::CpuFetchEvent>,
    pub(super) requests: BTreeSet<MemoryRequestId>,
}

pub(super) fn operational_fetch_projection(
    core: &RiscvCore,
    live: &RiscvO3LiveCheckpointPayload,
) -> Result<OperationalFetchProjection, RiscvCoreCheckpointRestoreError> {
    let fetches = match live.profile {
        crate::RiscvO3LiveCheckpointProfile::PendingDataAddress => {
            let pending = live.pending_addresses.as_slice();
            let Some(first) = pending.first() else {
                return Err(live_error(
                    "pending-address profile lacks its pending fetches",
                ));
            };
            let next_pc = pending
                .last()
                .and_then(|row| row.fetch.pc().get().checked_add(4));
            let stable_cpu_pc = core.inner().pc().get();
            let stable_hart_pc = core.state.lock().expect("riscv core lock").hart.pc();
            if !live.events.is_empty()
                || stable_hart_pc != first.fetch.pc().get()
                || stable_cpu_pc != stable_hart_pc
                || next_pc != Some(live.next_fetch_pc.get())
            {
                return Err(live_error(
                    "pending-address fetch membership is inconsistent",
                ));
            }
            let mut fetches = Vec::with_capacity(pending.len());
            for (index, row) in pending.iter().enumerate() {
                let expected_pc = first
                    .fetch
                    .pc()
                    .get()
                    .checked_add(4 * index as u64)
                    .ok_or(live_error("pending-address fetch PC overflows"))?;
                if row.fetch.partition() != core.partition()
                    || row.fetch.request_id().agent() != core.agent()
                    || row.fetch.route() != core.inner().fetch_route()
                    || row.fetch.endpoint() != &core.inner().fetch_endpoint()
                    || row.fetch.request_id().sequence() >= live.next_fetch_request_sequence
                    || row.fetch.pc().get() != expected_pc
                    || pending.get(index + 1).is_some_and(|next| {
                        row.fetch.request_id().sequence() >= next.fetch.request_id().sequence()
                    })
                {
                    return Err(live_error(
                        "pending-address fetch membership is inconsistent",
                    ));
                }
                fetches.push(row.fetch.clone());
            }
            fetches
        }
        crate::RiscvO3LiveCheckpointProfile::ComputeQueue
        | crate::RiscvO3LiveCheckpointProfile::CompletedFpLoad => {
            let requests = live
                .events
                .iter()
                .map(|event| event.fetch.request_id())
                .collect::<BTreeSet<_>>();
            if requests.len() != live.events.len()
                || requests
                    .iter()
                    .any(|request| request.sequence() >= live.next_fetch_request_sequence)
                || live.events.iter().any(|event| {
                    event.fetch.partition() != core.partition()
                        || event.fetch.request_id().agent() != core.agent()
                        || event.fetch.route() != core.inner().fetch_route()
                        || event.fetch.endpoint() != &core.inner().fetch_endpoint()
                        || event.fetch.pc().get() != event.execution_pc
                })
                || live.events.windows(2).any(|events| {
                    events[0].next_pc != events[1].execution_pc
                        || events[0].fetch.request_id().sequence()
                            >= events[1].fetch.request_id().sequence()
                })
                || live.events.last().map(|event| event.next_pc) != Some(live.next_fetch_pc.get())
            {
                return Err(live_error("live fetch membership is inconsistent"));
            }
            live.events
                .iter()
                .map(|event| event.fetch.clone())
                .collect()
        }
    };
    let requests = fetches
        .iter()
        .map(crate::CpuFetchEvent::request_id)
        .collect::<BTreeSet<_>>();
    if requests.len() != fetches.len() {
        return Err(live_error("operational fetch membership repeats a request"));
    }
    Ok(OperationalFetchProjection { fetches, requests })
}

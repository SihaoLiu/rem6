use std::collections::{BTreeMap, BTreeSet};

use rem6_memory::MemoryRequestId;

use super::{invalid, RiscvO3LiveCheckpointError};

pub(super) fn select_live_completed_fetches<'a>(
    cpu_events: &'a [crate::CpuFetchEvent],
    executed_fetches: &BTreeSet<MemoryRequestId>,
    expected_requests: &[MemoryRequestId],
    pending_suffix_after: Option<MemoryRequestId>,
) -> Result<Vec<&'a crate::CpuFetchEvent>, RiscvO3LiveCheckpointError> {
    let expected = expected_requests.iter().copied().collect::<BTreeSet<_>>();
    if expected.len() != expected_requests.len() {
        return Err(invalid("live issue rows repeat a fetch request"));
    }

    let mut issued = BTreeSet::new();
    let mut completed = BTreeMap::new();
    for event in cpu_events {
        let request = event.request_id();
        let permitted_pending_suffix = pending_suffix_after.is_some_and(|pending| {
            request.agent() == pending.agent() && request.sequence() > pending.sequence()
        });
        if !expected.contains(&request)
            && !executed_fetches.contains(&request)
            && !permitted_pending_suffix
        {
            return Err(invalid(
                "operational fetch stream contains an unexecuted fetch",
            ));
        }
        match event.kind() {
            crate::CpuFetchEventKind::Issued => {
                if !issued.insert(request) {
                    return Err(invalid("operational fetch stream repeats an issued fetch"));
                }
            }
            crate::CpuFetchEventKind::Completed => {
                if !matches!(event.data().map(<[u8]>::len), Some(2 | 4))
                    || event.data().map(|bytes| bytes.len() as u64) != Some(event.size().bytes())
                    || completed.insert(request, event).is_some()
                {
                    return Err(invalid(
                        "operational fetch stream contains an invalid completed instruction",
                    ));
                }
            }
            crate::CpuFetchEventKind::Retry | crate::CpuFetchEventKind::Failed => {
                return Err(invalid(
                    "operational fetch stream contains an unsuccessful fetch",
                ));
            }
        }
    }
    if issued
        .iter()
        .any(|request| !completed.contains_key(request))
    {
        return Err(invalid("operational fetch stream retains a pending fetch"));
    }

    expected_requests
        .iter()
        .map(|request| {
            completed
                .get(request)
                .copied()
                .ok_or(invalid("live issue row has no completed fetch"))
        })
        .collect()
}

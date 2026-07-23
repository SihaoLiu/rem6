use rem6_memory::Address;

use crate::{
    riscv_live_retire_window::completed_fetch_instruction_starting_with, CpuFetchEvent,
    CpuFetchEventKind, RiscvCoreState,
};

use super::{
    data_access_result::data_access_result_window_candidate, DetailedFetchAheadCandidate,
    TranslatedMemoryFetchAhead,
};
use crate::riscv_fetch_ahead::O3MemoryResultWindowRole;

pub(in crate::riscv_fetch_ahead) fn retained_data_access_result_window_candidate(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    translated: TranslatedMemoryFetchAhead,
) -> Option<DetailedFetchAheadCandidate> {
    let matching_mode = matches!(
        (translated, state.data_translation.is_some()),
        (TranslatedMemoryFetchAhead::Disabled, false)
            | (TranslatedMemoryFetchAhead::CachedMemory, true)
    );
    if !matching_mode {
        return None;
    }
    let (&head_request, &head_authorization) = state
        .memory_result_window_authorizations
        .iter()
        .find(|(request, authorization)| {
            authorization.role() == O3MemoryResultWindowRole::Head
                && state.executed_fetches.contains(request)
                && !state.issued_data_for_fetches.contains(request)
        })?;
    let has_pending_execution = state
        .events
        .iter()
        .any(|event| event.fetch().request_id() == head_request)
        || state
            .pending_terminal_memory_result
            .as_ref()
            .is_some_and(|pending| pending.owns_fetch(head_request));
    if !has_pending_execution {
        return None;
    }
    let head_event = fetch_events.iter().find(|event| {
        event.kind() == CpuFetchEventKind::Completed && event.request_id() == head_request
    })?;
    let head = completed_fetch_instruction_starting_with(
        &std::collections::BTreeSet::new(),
        fetch_events,
        head_event,
    )?;
    let sequential_pc = Address::new(
        head.pc()
            .get()
            .wrapping_add(u64::from(head.decoded().bytes())),
    );
    if sequential_pc != Address::new(state.hart.pc()) {
        return None;
    }
    Some(data_access_result_window_candidate(
        state,
        fetch_events,
        &head,
        head_authorization,
        translated,
    ))
}

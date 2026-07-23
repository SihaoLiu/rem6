use super::*;

impl RiscvCoreState {
    pub(crate) fn has_issuable_pending_data_address(&self) -> bool {
        self.next_unissued_data_access()
            .is_some_and(|(fetch_request, _)| {
                self.o3_runtime
                    .pending_data_address_owns_fetch(fetch_request)
            })
    }

    pub(crate) fn next_unissued_data_access(
        &self,
    ) -> Option<(MemoryRequestId, rem6_isa_riscv::MemoryAccessKind)> {
        let pending_terminal = self
            .pending_terminal_memory_result
            .as_ref()
            .filter(|pending| pending.issue_ready())
            .filter(|_| !crate::riscv_fetch_ahead::hart_has_enabled_pending_interrupt(&self.hart))
            .map(|pending| pending.execution());
        let pending_address = self
            .o3_runtime
            .oldest_pending_data_address_execution()
            .filter(|_| !crate::riscv_fetch_ahead::hart_has_enabled_pending_interrupt(&self.hart))
            .filter(|event| {
                !self
                    .issued_data_for_fetches
                    .contains(&event.fetch().request_id())
            });
        let candidate = self
            .events
            .iter()
            .chain(pending_terminal)
            .chain(pending_address)
            .find_map(|event| {
                let fetch_request = event.fetch().request_id();
                if self.issued_data_for_fetches.contains(&fetch_request) {
                    return None;
                }
                if self
                    .pending_data_translations
                    .values()
                    .any(|pending| pending.fetch_request() == fetch_request)
                {
                    return None;
                }
                if self.ready_translated_data.contains_key(&fetch_request) {
                    return None;
                }
                event
                    .execution()
                    .memory_access()
                    .map(|access| (event, fetch_request, access.clone()))
            });
        let (event, fetch_request, access) = candidate?;
        if self.outstanding_data.is_empty() && !self.o3_runtime.has_live_data_access() {
            return Some((fetch_request, access));
        }
        (self
            .o3_runtime
            .pending_data_address_can_issue(fetch_request, &access)
            || (self.can_overlap_detailed_scalar_memory_instruction(event.instruction())
                && self.o3_runtime.can_stage_scalar_memory(event))
            || self.can_overlap_detailed_memory_result_event(event))
        .then_some((fetch_request, access))
    }
}

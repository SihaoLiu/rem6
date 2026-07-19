use super::*;

impl O3RuntimeState {
    pub(crate) fn retire_producer_forwarded_data_head_for_test(
        &mut self,
        retire_tick: u64,
    ) -> bool {
        if self.live_data_accesses.len() != 1
            || self.producer_forwarded_scalar_chain().is_none()
            || self
                .snapshot
                .reorder_buffer
                .first()
                .map(|entry| entry.sequence())
                != self.live_data_accesses.first().map(|head| head.sequence)
        {
            return false;
        }
        self.live_data_accesses.clear();
        self.snapshot.reorder_buffer.remove(0);
        self.last_live_commit_tick = Some(retire_tick);
        true
    }

    pub(crate) fn producer_forwarded_scalar_return_issue_tick_for_test(&self) -> Option<u64> {
        let sequence = self.producer_forwarded_return_descendant()?.sequence();
        self.live_speculative_executions
            .iter()
            .find(|issued| issued.sequence == sequence)
            .map(|issued| issued.issue_tick)
    }

    pub(crate) fn producer_forwarded_scalar_issue_tick_for_test(&self) -> Option<u64> {
        let sequence = self.producer_forwarded_scalar_chain()?.last()?.sequence();
        self.live_speculative_executions
            .iter()
            .find(|issued| issued.sequence == sequence)
            .map(|issued| issued.issue_tick)
    }

    pub(crate) fn replace_producer_forwarded_chain_fetch_identity_for_test(
        &mut self,
        sequence: u64,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        let Some(issued) = self
            .live_speculative_executions
            .iter_mut()
            .find(|issued| issued.sequence == sequence)
        else {
            return false;
        };
        issued.consumed_requests = consumed_requests.to_vec();
        true
    }
}

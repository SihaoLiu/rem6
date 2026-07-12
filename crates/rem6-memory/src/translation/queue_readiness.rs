use super::{TranslationQueue, TranslationRequestId};

impl TranslationQueue {
    pub fn pending_request_ids(&self) -> Vec<TranslationRequestId> {
        self.service_order_ids(None)
    }

    pub fn ready_request_ids(&self, tick: u64) -> Vec<TranslationRequestId> {
        self.service_order_ids(Some(tick))
    }

    pub fn next_ready_tick(&self) -> Option<u64> {
        self.entries.values().map(|entry| entry.ready_tick).min()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QueuedPrefetchStatsSnapshot {
    issued_prefetches: u64,
    useful_prefetches: u64,
    unused_prefetches: u64,
    useful_but_miss_prefetches: u64,
    demand_mshr_misses: u64,
    prefetch_hits_in_cache: u64,
    prefetch_hits_in_mshr: u64,
    prefetch_hits_in_write_buffer: u64,
    identified_prefetches: u64,
    buffer_hits: u64,
    in_cache_drops: u64,
    removed_by_demand: u64,
    removed_by_full_queue: u64,
    span_page_prefetches: u64,
    useful_span_page_prefetches: u64,
}

impl QueuedPrefetchStatsSnapshot {
    pub const fn issued_prefetches(&self) -> u64 {
        self.issued_prefetches
    }

    pub const fn useful_prefetches(&self) -> u64 {
        self.useful_prefetches
    }

    pub const fn unused_prefetches(&self) -> u64 {
        self.unused_prefetches
    }

    pub const fn useful_but_miss_prefetches(&self) -> u64 {
        self.useful_but_miss_prefetches
    }

    pub const fn demand_mshr_misses(&self) -> u64 {
        self.demand_mshr_misses
    }

    pub const fn prefetch_hits_in_cache(&self) -> u64 {
        self.prefetch_hits_in_cache
    }

    pub const fn prefetch_hits_in_mshr(&self) -> u64 {
        self.prefetch_hits_in_mshr
    }

    pub const fn prefetch_hits_in_write_buffer(&self) -> u64 {
        self.prefetch_hits_in_write_buffer
    }

    pub fn late_prefetches(&self) -> u64 {
        self.prefetch_hits_in_cache
            .saturating_add(self.prefetch_hits_in_mshr)
            .saturating_add(self.prefetch_hits_in_write_buffer)
    }

    pub const fn identified_prefetches(&self) -> u64 {
        self.identified_prefetches
    }

    pub const fn buffer_hits(&self) -> u64 {
        self.buffer_hits
    }

    pub const fn in_cache_drops(&self) -> u64 {
        self.in_cache_drops
    }

    pub const fn removed_by_demand(&self) -> u64 {
        self.removed_by_demand
    }

    pub const fn removed_by_full_queue(&self) -> u64 {
        self.removed_by_full_queue
    }

    pub const fn span_page_prefetches(&self) -> u64 {
        self.span_page_prefetches
    }

    pub const fn useful_span_page_prefetches(&self) -> u64 {
        self.useful_span_page_prefetches
    }

    pub(crate) fn record_issued(&mut self, delta: u64) {
        self.issued_prefetches = self.issued_prefetches.saturating_add(delta);
    }

    pub(crate) fn record_useful(&mut self, missed_usable_state: bool) {
        self.useful_prefetches = self.useful_prefetches.saturating_add(1);
        if missed_usable_state {
            self.useful_but_miss_prefetches = self.useful_but_miss_prefetches.saturating_add(1);
        }
    }

    pub(crate) fn record_unused(&mut self) {
        self.unused_prefetches = self.unused_prefetches.saturating_add(1);
    }

    pub(crate) fn record_demand_mshr_miss(&mut self) {
        self.demand_mshr_misses = self.demand_mshr_misses.saturating_add(1);
    }

    pub(crate) fn record_hit_in_cache(&mut self) {
        self.prefetch_hits_in_cache = self.prefetch_hits_in_cache.saturating_add(1);
    }

    pub(crate) fn record_hit_in_mshr(&mut self) {
        self.prefetch_hits_in_mshr = self.prefetch_hits_in_mshr.saturating_add(1);
    }

    pub(crate) fn record_hit_in_write_buffer(&mut self) {
        self.prefetch_hits_in_write_buffer = self.prefetch_hits_in_write_buffer.saturating_add(1);
    }

    pub(crate) fn record_identified(&mut self, delta: u64) {
        self.identified_prefetches = self.identified_prefetches.saturating_add(delta);
    }

    pub(crate) fn record_buffer_hit(&mut self, delta: u64) {
        self.buffer_hits = self.buffer_hits.saturating_add(delta);
    }

    pub(crate) fn record_in_cache_drop(&mut self, delta: u64) {
        self.in_cache_drops = self.in_cache_drops.saturating_add(delta);
    }

    pub(crate) fn record_removed_by_demand(&mut self, delta: u64) {
        self.removed_by_demand = self.removed_by_demand.saturating_add(delta);
    }

    pub(crate) fn record_removed_by_full_queue(&mut self, delta: u64) {
        self.removed_by_full_queue = self.removed_by_full_queue.saturating_add(delta);
    }

    pub(crate) fn record_span_page(&mut self, delta: u64) {
        self.span_page_prefetches = self.span_page_prefetches.saturating_add(delta);
    }

    pub(crate) fn record_useful_span_page(&mut self, delta: u64) {
        self.useful_span_page_prefetches = self.useful_span_page_prefetches.saturating_add(delta);
    }
}

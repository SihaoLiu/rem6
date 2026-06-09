use super::{WorkloadExpectedTrafficTraceReplaySummary, WorkloadTrafficTraceReplaySummary};

impl WorkloadTrafficTraceReplaySummary {
    pub fn with_trace_read_response_count(mut self, trace_read_response_count: usize) -> Self {
        self.trace_read_response_count = trace_read_response_count;
        self
    }

    pub fn with_trace_write_response_count(mut self, trace_write_response_count: usize) -> Self {
        self.trace_write_response_count = trace_write_response_count;
        self
    }

    pub fn with_trace_prefetch_response_count(
        mut self,
        trace_prefetch_response_count: usize,
    ) -> Self {
        self.trace_prefetch_response_count = trace_prefetch_response_count;
        self
    }

    pub fn with_trace_invalidate_response_count(
        mut self,
        trace_invalidate_response_count: usize,
    ) -> Self {
        self.trace_invalidate_response_count = trace_invalidate_response_count;
        self
    }

    pub fn with_trace_clean_response_count(mut self, trace_clean_response_count: usize) -> Self {
        self.trace_clean_response_count = trace_clean_response_count;
        self
    }

    pub fn with_trace_upgrade_response_count(
        mut self,
        trace_upgrade_response_count: usize,
    ) -> Self {
        self.trace_upgrade_response_count = trace_upgrade_response_count;
        self
    }

    pub fn with_trace_llsc_response_count(mut self, trace_llsc_response_count: usize) -> Self {
        self.trace_llsc_response_count = trace_llsc_response_count;
        self
    }

    pub fn with_trace_locked_rmw_response_count(
        mut self,
        trace_locked_rmw_response_count: usize,
    ) -> Self {
        self.trace_locked_rmw_response_count = trace_locked_rmw_response_count;
        self
    }

    pub fn with_trace_writable_intent_response_count(
        mut self,
        trace_writable_intent_response_count: usize,
    ) -> Self {
        self.trace_writable_intent_response_count = trace_writable_intent_response_count;
        self
    }

    pub const fn trace_read_response_count(&self) -> usize {
        self.trace_read_response_count
    }

    pub const fn trace_write_response_count(&self) -> usize {
        self.trace_write_response_count
    }

    pub const fn trace_prefetch_response_count(&self) -> usize {
        self.trace_prefetch_response_count
    }

    pub const fn trace_invalidate_response_count(&self) -> usize {
        self.trace_invalidate_response_count
    }

    pub const fn trace_clean_response_count(&self) -> usize {
        self.trace_clean_response_count
    }

    pub const fn trace_upgrade_response_count(&self) -> usize {
        self.trace_upgrade_response_count
    }

    pub const fn trace_llsc_response_count(&self) -> usize {
        self.trace_llsc_response_count
    }

    pub const fn trace_locked_rmw_response_count(&self) -> usize {
        self.trace_locked_rmw_response_count
    }

    pub const fn trace_writable_intent_response_count(&self) -> usize {
        self.trace_writable_intent_response_count
    }
}

impl WorkloadExpectedTrafficTraceReplaySummary {
    pub fn with_minimum_trace_read_response_count(
        mut self,
        minimum_trace_read_response_count: usize,
    ) -> Self {
        self.minimum_trace_read_response_count = minimum_trace_read_response_count;
        self
    }

    pub fn with_minimum_trace_write_response_count(
        mut self,
        minimum_trace_write_response_count: usize,
    ) -> Self {
        self.minimum_trace_write_response_count = minimum_trace_write_response_count;
        self
    }

    pub fn with_minimum_trace_prefetch_response_count(
        mut self,
        minimum_trace_prefetch_response_count: usize,
    ) -> Self {
        self.minimum_trace_prefetch_response_count = minimum_trace_prefetch_response_count;
        self
    }

    pub fn with_minimum_trace_invalidate_response_count(
        mut self,
        minimum_trace_invalidate_response_count: usize,
    ) -> Self {
        self.minimum_trace_invalidate_response_count = minimum_trace_invalidate_response_count;
        self
    }

    pub fn with_minimum_trace_clean_response_count(
        mut self,
        minimum_trace_clean_response_count: usize,
    ) -> Self {
        self.minimum_trace_clean_response_count = minimum_trace_clean_response_count;
        self
    }

    pub fn with_minimum_trace_upgrade_response_count(
        mut self,
        minimum_trace_upgrade_response_count: usize,
    ) -> Self {
        self.minimum_trace_upgrade_response_count = minimum_trace_upgrade_response_count;
        self
    }

    pub fn with_minimum_trace_llsc_response_count(
        mut self,
        minimum_trace_llsc_response_count: usize,
    ) -> Self {
        self.minimum_trace_llsc_response_count = minimum_trace_llsc_response_count;
        self
    }

    pub fn with_minimum_trace_locked_rmw_response_count(
        mut self,
        minimum_trace_locked_rmw_response_count: usize,
    ) -> Self {
        self.minimum_trace_locked_rmw_response_count = minimum_trace_locked_rmw_response_count;
        self
    }

    pub fn with_minimum_trace_writable_intent_response_count(
        mut self,
        minimum_trace_writable_intent_response_count: usize,
    ) -> Self {
        self.minimum_trace_writable_intent_response_count =
            minimum_trace_writable_intent_response_count;
        self
    }

    pub const fn minimum_trace_read_response_count(&self) -> usize {
        self.minimum_trace_read_response_count
    }

    pub const fn minimum_trace_write_response_count(&self) -> usize {
        self.minimum_trace_write_response_count
    }

    pub const fn minimum_trace_prefetch_response_count(&self) -> usize {
        self.minimum_trace_prefetch_response_count
    }

    pub const fn minimum_trace_invalidate_response_count(&self) -> usize {
        self.minimum_trace_invalidate_response_count
    }

    pub const fn minimum_trace_clean_response_count(&self) -> usize {
        self.minimum_trace_clean_response_count
    }

    pub const fn minimum_trace_upgrade_response_count(&self) -> usize {
        self.minimum_trace_upgrade_response_count
    }

    pub const fn minimum_trace_llsc_response_count(&self) -> usize {
        self.minimum_trace_llsc_response_count
    }

    pub const fn minimum_trace_locked_rmw_response_count(&self) -> usize {
        self.minimum_trace_locked_rmw_response_count
    }

    pub const fn minimum_trace_writable_intent_response_count(&self) -> usize {
        self.minimum_trace_writable_intent_response_count
    }
}

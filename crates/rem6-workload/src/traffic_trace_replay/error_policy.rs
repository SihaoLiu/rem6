use super::{WorkloadExpectedTrafficTraceReplaySummary, WorkloadTrafficTraceReplaySummary};

impl WorkloadTrafficTraceReplaySummary {
    pub fn with_trace_error_invalid_destination_count(
        mut self,
        trace_error_invalid_destination_count: usize,
    ) -> Self {
        self.trace_error_invalid_destination_count = trace_error_invalid_destination_count;
        self
    }

    pub fn with_trace_error_bad_address_count(
        mut self,
        trace_error_bad_address_count: usize,
    ) -> Self {
        self.trace_error_bad_address_count = trace_error_bad_address_count;
        self
    }

    pub fn with_trace_error_read_count(mut self, trace_error_read_count: usize) -> Self {
        self.trace_error_read_count = trace_error_read_count;
        self
    }

    pub fn with_trace_error_write_count(mut self, trace_error_write_count: usize) -> Self {
        self.trace_error_write_count = trace_error_write_count;
        self
    }

    pub fn with_trace_error_functional_read_count(
        mut self,
        trace_error_functional_read_count: usize,
    ) -> Self {
        self.trace_error_functional_read_count = trace_error_functional_read_count;
        self
    }

    pub fn with_trace_error_functional_write_count(
        mut self,
        trace_error_functional_write_count: usize,
    ) -> Self {
        self.trace_error_functional_write_count = trace_error_functional_write_count;
        self
    }

    pub const fn trace_error_invalid_destination_count(&self) -> usize {
        self.trace_error_invalid_destination_count
    }

    pub const fn trace_error_bad_address_count(&self) -> usize {
        self.trace_error_bad_address_count
    }

    pub const fn trace_error_read_count(&self) -> usize {
        self.trace_error_read_count
    }

    pub const fn trace_error_write_count(&self) -> usize {
        self.trace_error_write_count
    }

    pub const fn trace_error_functional_read_count(&self) -> usize {
        self.trace_error_functional_read_count
    }

    pub const fn trace_error_functional_write_count(&self) -> usize {
        self.trace_error_functional_write_count
    }
}

impl WorkloadExpectedTrafficTraceReplaySummary {
    pub fn with_minimum_trace_error_invalid_destination_count(
        mut self,
        minimum_trace_error_invalid_destination_count: usize,
    ) -> Self {
        self.minimum_trace_error_invalid_destination_count =
            minimum_trace_error_invalid_destination_count;
        self
    }

    pub fn with_minimum_trace_error_bad_address_count(
        mut self,
        minimum_trace_error_bad_address_count: usize,
    ) -> Self {
        self.minimum_trace_error_bad_address_count = minimum_trace_error_bad_address_count;
        self
    }

    pub fn with_minimum_trace_error_read_count(
        mut self,
        minimum_trace_error_read_count: usize,
    ) -> Self {
        self.minimum_trace_error_read_count = minimum_trace_error_read_count;
        self
    }

    pub fn with_minimum_trace_error_write_count(
        mut self,
        minimum_trace_error_write_count: usize,
    ) -> Self {
        self.minimum_trace_error_write_count = minimum_trace_error_write_count;
        self
    }

    pub fn with_minimum_trace_error_functional_read_count(
        mut self,
        minimum_trace_error_functional_read_count: usize,
    ) -> Self {
        self.minimum_trace_error_functional_read_count = minimum_trace_error_functional_read_count;
        self
    }

    pub fn with_minimum_trace_error_functional_write_count(
        mut self,
        minimum_trace_error_functional_write_count: usize,
    ) -> Self {
        self.minimum_trace_error_functional_write_count =
            minimum_trace_error_functional_write_count;
        self
    }

    pub const fn minimum_trace_error_invalid_destination_count(&self) -> usize {
        self.minimum_trace_error_invalid_destination_count
    }

    pub const fn minimum_trace_error_bad_address_count(&self) -> usize {
        self.minimum_trace_error_bad_address_count
    }

    pub const fn minimum_trace_error_read_count(&self) -> usize {
        self.minimum_trace_error_read_count
    }

    pub const fn minimum_trace_error_write_count(&self) -> usize {
        self.minimum_trace_error_write_count
    }

    pub const fn minimum_trace_error_functional_read_count(&self) -> usize {
        self.minimum_trace_error_functional_read_count
    }

    pub const fn minimum_trace_error_functional_write_count(&self) -> usize {
        self.minimum_trace_error_functional_write_count
    }
}

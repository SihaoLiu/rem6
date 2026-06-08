use crate::{WorkloadError, WorkloadReplayPlan, WorkloadResult, WorkloadRouteId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadTrafficTraceReplaySummary {
    route: WorkloadRouteId,
    scheduled_count: usize,
    response_delivery_count: usize,
    trace_completed_response_count: usize,
    trace_retry_response_count: usize,
    trace_store_conditional_failed_response_count: usize,
    memory_trace_event_count: usize,
    memory_write_completion_count: usize,
    trace_data_cache_response_count: usize,
    trace_data_cache_error_count: usize,
    memory_failure_count: usize,
    memory_failure_invalid_destination_count: usize,
    memory_failure_bad_address_count: usize,
    memory_failure_read_count: usize,
    memory_failure_write_count: usize,
    memory_failure_functional_read_count: usize,
    memory_failure_functional_write_count: usize,
    trace_error_count: usize,
    trace_htm_access_count: usize,
    control_ack_count: usize,
    sync_control_ack_count: usize,
    htm_control_ack_count: usize,
    control_failure_count: usize,
    control_failure_invalid_destination_count: usize,
    control_failure_bad_address_count: usize,
    control_failure_read_count: usize,
    control_failure_write_count: usize,
    control_failure_functional_read_count: usize,
    control_failure_functional_write_count: usize,
    sync_control_failure_count: usize,
    tlb_control_failure_count: usize,
    cache_control_failure_count: usize,
    htm_control_failure_count: usize,
    diagnostic_control_failure_count: usize,
    sideband_event_count: usize,
    tlb_sync_event_count: usize,
    trace_tlb_sync_count: usize,
    cache_flush_event_count: usize,
    trace_cache_flush_count: usize,
    trace_l1_invalidation_count: usize,
    diagnostic_print_event_count: usize,
    trace_diagnostic_count: usize,
    htm_abort_event_count: usize,
}

impl WorkloadTrafficTraceReplaySummary {
    pub fn new(route: WorkloadRouteId, scheduled_count: usize) -> Self {
        Self {
            route,
            scheduled_count,
            response_delivery_count: 0,
            trace_completed_response_count: 0,
            trace_retry_response_count: 0,
            trace_store_conditional_failed_response_count: 0,
            memory_trace_event_count: 0,
            memory_write_completion_count: 0,
            trace_data_cache_response_count: 0,
            trace_data_cache_error_count: 0,
            memory_failure_count: 0,
            memory_failure_invalid_destination_count: 0,
            memory_failure_bad_address_count: 0,
            memory_failure_read_count: 0,
            memory_failure_write_count: 0,
            memory_failure_functional_read_count: 0,
            memory_failure_functional_write_count: 0,
            trace_error_count: 0,
            trace_htm_access_count: 0,
            control_ack_count: 0,
            sync_control_ack_count: 0,
            htm_control_ack_count: 0,
            control_failure_count: 0,
            control_failure_invalid_destination_count: 0,
            control_failure_bad_address_count: 0,
            control_failure_read_count: 0,
            control_failure_write_count: 0,
            control_failure_functional_read_count: 0,
            control_failure_functional_write_count: 0,
            sync_control_failure_count: 0,
            tlb_control_failure_count: 0,
            cache_control_failure_count: 0,
            htm_control_failure_count: 0,
            diagnostic_control_failure_count: 0,
            sideband_event_count: 0,
            tlb_sync_event_count: 0,
            trace_tlb_sync_count: 0,
            cache_flush_event_count: 0,
            trace_cache_flush_count: 0,
            trace_l1_invalidation_count: 0,
            diagnostic_print_event_count: 0,
            trace_diagnostic_count: 0,
            htm_abort_event_count: 0,
        }
    }

    pub fn with_response_delivery_count(mut self, response_delivery_count: usize) -> Self {
        self.response_delivery_count = response_delivery_count;
        self
    }

    pub fn with_trace_completed_response_count(
        mut self,
        trace_completed_response_count: usize,
    ) -> Self {
        self.trace_completed_response_count = trace_completed_response_count;
        self
    }

    pub fn with_trace_retry_response_count(mut self, trace_retry_response_count: usize) -> Self {
        self.trace_retry_response_count = trace_retry_response_count;
        self
    }

    pub fn with_trace_store_conditional_failed_response_count(
        mut self,
        trace_store_conditional_failed_response_count: usize,
    ) -> Self {
        self.trace_store_conditional_failed_response_count =
            trace_store_conditional_failed_response_count;
        self
    }

    pub fn with_memory_trace_event_count(mut self, memory_trace_event_count: usize) -> Self {
        self.memory_trace_event_count = memory_trace_event_count;
        self
    }

    pub fn with_memory_write_completion_count(
        mut self,
        memory_write_completion_count: usize,
    ) -> Self {
        self.memory_write_completion_count = memory_write_completion_count;
        self
    }

    pub fn with_trace_data_cache_response_count(
        mut self,
        trace_data_cache_response_count: usize,
    ) -> Self {
        self.trace_data_cache_response_count = trace_data_cache_response_count;
        self
    }

    pub fn with_trace_data_cache_error_count(
        mut self,
        trace_data_cache_error_count: usize,
    ) -> Self {
        self.trace_data_cache_error_count = trace_data_cache_error_count;
        self
    }

    pub fn with_memory_failure_count(mut self, memory_failure_count: usize) -> Self {
        self.memory_failure_count = memory_failure_count;
        self
    }

    pub fn with_memory_failure_invalid_destination_count(
        mut self,
        memory_failure_invalid_destination_count: usize,
    ) -> Self {
        self.memory_failure_invalid_destination_count = memory_failure_invalid_destination_count;
        self
    }

    pub fn with_memory_failure_bad_address_count(
        mut self,
        memory_failure_bad_address_count: usize,
    ) -> Self {
        self.memory_failure_bad_address_count = memory_failure_bad_address_count;
        self
    }

    pub fn with_memory_failure_read_count(mut self, memory_failure_read_count: usize) -> Self {
        self.memory_failure_read_count = memory_failure_read_count;
        self
    }

    pub fn with_memory_failure_write_count(mut self, memory_failure_write_count: usize) -> Self {
        self.memory_failure_write_count = memory_failure_write_count;
        self
    }

    pub fn with_memory_failure_functional_read_count(
        mut self,
        memory_failure_functional_read_count: usize,
    ) -> Self {
        self.memory_failure_functional_read_count = memory_failure_functional_read_count;
        self
    }

    pub fn with_memory_failure_functional_write_count(
        mut self,
        memory_failure_functional_write_count: usize,
    ) -> Self {
        self.memory_failure_functional_write_count = memory_failure_functional_write_count;
        self
    }

    pub fn with_trace_error_count(mut self, trace_error_count: usize) -> Self {
        self.trace_error_count = trace_error_count;
        self
    }

    pub fn with_trace_htm_access_count(mut self, trace_htm_access_count: usize) -> Self {
        self.trace_htm_access_count = trace_htm_access_count;
        self
    }

    pub fn with_control_ack_count(mut self, control_ack_count: usize) -> Self {
        self.control_ack_count = control_ack_count;
        self
    }

    pub fn with_sync_control_ack_count(mut self, sync_control_ack_count: usize) -> Self {
        self.sync_control_ack_count = sync_control_ack_count;
        self
    }

    pub fn with_htm_control_ack_count(mut self, htm_control_ack_count: usize) -> Self {
        self.htm_control_ack_count = htm_control_ack_count;
        self
    }

    pub fn with_control_failure_count(mut self, control_failure_count: usize) -> Self {
        self.control_failure_count = control_failure_count;
        self
    }

    pub fn with_control_failure_invalid_destination_count(
        mut self,
        control_failure_invalid_destination_count: usize,
    ) -> Self {
        self.control_failure_invalid_destination_count = control_failure_invalid_destination_count;
        self
    }

    pub fn with_control_failure_bad_address_count(
        mut self,
        control_failure_bad_address_count: usize,
    ) -> Self {
        self.control_failure_bad_address_count = control_failure_bad_address_count;
        self
    }

    pub fn with_control_failure_read_count(mut self, control_failure_read_count: usize) -> Self {
        self.control_failure_read_count = control_failure_read_count;
        self
    }

    pub fn with_control_failure_write_count(mut self, control_failure_write_count: usize) -> Self {
        self.control_failure_write_count = control_failure_write_count;
        self
    }

    pub fn with_control_failure_functional_read_count(
        mut self,
        control_failure_functional_read_count: usize,
    ) -> Self {
        self.control_failure_functional_read_count = control_failure_functional_read_count;
        self
    }

    pub fn with_control_failure_functional_write_count(
        mut self,
        control_failure_functional_write_count: usize,
    ) -> Self {
        self.control_failure_functional_write_count = control_failure_functional_write_count;
        self
    }

    pub fn with_sync_control_failure_count(mut self, sync_control_failure_count: usize) -> Self {
        self.sync_control_failure_count = sync_control_failure_count;
        self
    }

    pub fn with_tlb_control_failure_count(mut self, tlb_control_failure_count: usize) -> Self {
        self.tlb_control_failure_count = tlb_control_failure_count;
        self
    }

    pub fn with_cache_control_failure_count(mut self, cache_control_failure_count: usize) -> Self {
        self.cache_control_failure_count = cache_control_failure_count;
        self
    }

    pub fn with_htm_control_failure_count(mut self, htm_control_failure_count: usize) -> Self {
        self.htm_control_failure_count = htm_control_failure_count;
        self
    }

    pub fn with_diagnostic_control_failure_count(
        mut self,
        diagnostic_control_failure_count: usize,
    ) -> Self {
        self.diagnostic_control_failure_count = diagnostic_control_failure_count;
        self
    }

    pub fn with_sideband_event_count(mut self, sideband_event_count: usize) -> Self {
        self.sideband_event_count = sideband_event_count;
        self
    }

    pub fn with_tlb_sync_event_count(mut self, tlb_sync_event_count: usize) -> Self {
        self.tlb_sync_event_count = tlb_sync_event_count;
        self
    }

    pub fn with_trace_tlb_sync_count(mut self, trace_tlb_sync_count: usize) -> Self {
        self.trace_tlb_sync_count = trace_tlb_sync_count;
        self
    }

    pub fn with_cache_flush_event_count(mut self, cache_flush_event_count: usize) -> Self {
        self.cache_flush_event_count = cache_flush_event_count;
        self
    }

    pub fn with_trace_cache_flush_count(mut self, trace_cache_flush_count: usize) -> Self {
        self.trace_cache_flush_count = trace_cache_flush_count;
        self
    }

    pub fn with_trace_l1_invalidation_count(mut self, trace_l1_invalidation_count: usize) -> Self {
        self.trace_l1_invalidation_count = trace_l1_invalidation_count;
        self
    }

    pub fn with_diagnostic_print_event_count(
        mut self,
        diagnostic_print_event_count: usize,
    ) -> Self {
        self.diagnostic_print_event_count = diagnostic_print_event_count;
        self
    }

    pub fn with_trace_diagnostic_count(mut self, trace_diagnostic_count: usize) -> Self {
        self.trace_diagnostic_count = trace_diagnostic_count;
        self
    }

    pub fn with_htm_abort_event_count(mut self, htm_abort_event_count: usize) -> Self {
        self.htm_abort_event_count = htm_abort_event_count;
        self
    }

    pub const fn route(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub const fn scheduled_count(&self) -> usize {
        self.scheduled_count
    }

    pub const fn response_delivery_count(&self) -> usize {
        self.response_delivery_count
    }

    pub const fn trace_completed_response_count(&self) -> usize {
        self.trace_completed_response_count
    }

    pub const fn trace_retry_response_count(&self) -> usize {
        self.trace_retry_response_count
    }

    pub const fn trace_store_conditional_failed_response_count(&self) -> usize {
        self.trace_store_conditional_failed_response_count
    }

    pub const fn memory_trace_event_count(&self) -> usize {
        self.memory_trace_event_count
    }

    pub const fn memory_write_completion_count(&self) -> usize {
        self.memory_write_completion_count
    }

    pub const fn trace_data_cache_response_count(&self) -> usize {
        self.trace_data_cache_response_count
    }

    pub const fn trace_data_cache_error_count(&self) -> usize {
        self.trace_data_cache_error_count
    }

    pub const fn memory_failure_count(&self) -> usize {
        self.memory_failure_count
    }

    pub const fn memory_failure_invalid_destination_count(&self) -> usize {
        self.memory_failure_invalid_destination_count
    }

    pub const fn memory_failure_bad_address_count(&self) -> usize {
        self.memory_failure_bad_address_count
    }

    pub const fn memory_failure_read_count(&self) -> usize {
        self.memory_failure_read_count
    }

    pub const fn memory_failure_write_count(&self) -> usize {
        self.memory_failure_write_count
    }

    pub const fn memory_failure_functional_read_count(&self) -> usize {
        self.memory_failure_functional_read_count
    }

    pub const fn memory_failure_functional_write_count(&self) -> usize {
        self.memory_failure_functional_write_count
    }

    pub const fn trace_error_count(&self) -> usize {
        self.trace_error_count
    }

    pub const fn trace_htm_access_count(&self) -> usize {
        self.trace_htm_access_count
    }

    pub const fn control_ack_count(&self) -> usize {
        self.control_ack_count
    }

    pub const fn sync_control_ack_count(&self) -> usize {
        self.sync_control_ack_count
    }

    pub const fn htm_control_ack_count(&self) -> usize {
        self.htm_control_ack_count
    }

    pub const fn control_failure_count(&self) -> usize {
        self.control_failure_count
    }

    pub const fn control_failure_invalid_destination_count(&self) -> usize {
        self.control_failure_invalid_destination_count
    }

    pub const fn control_failure_bad_address_count(&self) -> usize {
        self.control_failure_bad_address_count
    }

    pub const fn control_failure_read_count(&self) -> usize {
        self.control_failure_read_count
    }

    pub const fn control_failure_write_count(&self) -> usize {
        self.control_failure_write_count
    }

    pub const fn control_failure_functional_read_count(&self) -> usize {
        self.control_failure_functional_read_count
    }

    pub const fn control_failure_functional_write_count(&self) -> usize {
        self.control_failure_functional_write_count
    }

    pub const fn sync_control_failure_count(&self) -> usize {
        self.sync_control_failure_count
    }

    pub const fn tlb_control_failure_count(&self) -> usize {
        self.tlb_control_failure_count
    }

    pub const fn cache_control_failure_count(&self) -> usize {
        self.cache_control_failure_count
    }

    pub const fn htm_control_failure_count(&self) -> usize {
        self.htm_control_failure_count
    }

    pub const fn diagnostic_control_failure_count(&self) -> usize {
        self.diagnostic_control_failure_count
    }

    pub const fn sideband_event_count(&self) -> usize {
        self.sideband_event_count
    }

    pub const fn tlb_sync_event_count(&self) -> usize {
        self.tlb_sync_event_count
    }

    pub const fn trace_tlb_sync_count(&self) -> usize {
        self.trace_tlb_sync_count
    }

    pub const fn cache_flush_event_count(&self) -> usize {
        self.cache_flush_event_count
    }

    pub const fn trace_cache_flush_count(&self) -> usize {
        self.trace_cache_flush_count
    }

    pub const fn trace_l1_invalidation_count(&self) -> usize {
        self.trace_l1_invalidation_count
    }

    pub const fn diagnostic_print_event_count(&self) -> usize {
        self.diagnostic_print_event_count
    }

    pub const fn trace_diagnostic_count(&self) -> usize {
        self.trace_diagnostic_count
    }

    pub const fn htm_abort_event_count(&self) -> usize {
        self.htm_abort_event_count
    }

    pub(crate) fn sort_key(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub(crate) fn merged(&self, other: &Self) -> Self {
        debug_assert_eq!(self.route(), other.route());
        Self {
            route: self.route.clone(),
            scheduled_count: self.scheduled_count + other.scheduled_count,
            response_delivery_count: self.response_delivery_count + other.response_delivery_count,
            trace_completed_response_count: self.trace_completed_response_count
                + other.trace_completed_response_count,
            trace_retry_response_count: self.trace_retry_response_count
                + other.trace_retry_response_count,
            trace_store_conditional_failed_response_count: self
                .trace_store_conditional_failed_response_count
                + other.trace_store_conditional_failed_response_count,
            memory_trace_event_count: self.memory_trace_event_count
                + other.memory_trace_event_count,
            memory_write_completion_count: self.memory_write_completion_count
                + other.memory_write_completion_count,
            trace_data_cache_response_count: self.trace_data_cache_response_count
                + other.trace_data_cache_response_count,
            trace_data_cache_error_count: self.trace_data_cache_error_count
                + other.trace_data_cache_error_count,
            memory_failure_count: self.memory_failure_count + other.memory_failure_count,
            memory_failure_invalid_destination_count: self.memory_failure_invalid_destination_count
                + other.memory_failure_invalid_destination_count,
            memory_failure_bad_address_count: self.memory_failure_bad_address_count
                + other.memory_failure_bad_address_count,
            memory_failure_read_count: self.memory_failure_read_count
                + other.memory_failure_read_count,
            memory_failure_write_count: self.memory_failure_write_count
                + other.memory_failure_write_count,
            memory_failure_functional_read_count: self.memory_failure_functional_read_count
                + other.memory_failure_functional_read_count,
            memory_failure_functional_write_count: self.memory_failure_functional_write_count
                + other.memory_failure_functional_write_count,
            trace_error_count: self.trace_error_count + other.trace_error_count,
            trace_htm_access_count: self.trace_htm_access_count + other.trace_htm_access_count,
            control_ack_count: self.control_ack_count + other.control_ack_count,
            sync_control_ack_count: self.sync_control_ack_count + other.sync_control_ack_count,
            htm_control_ack_count: self.htm_control_ack_count + other.htm_control_ack_count,
            control_failure_count: self.control_failure_count + other.control_failure_count,
            control_failure_invalid_destination_count: self
                .control_failure_invalid_destination_count
                + other.control_failure_invalid_destination_count,
            control_failure_bad_address_count: self.control_failure_bad_address_count
                + other.control_failure_bad_address_count,
            control_failure_read_count: self.control_failure_read_count
                + other.control_failure_read_count,
            control_failure_write_count: self.control_failure_write_count
                + other.control_failure_write_count,
            control_failure_functional_read_count: self.control_failure_functional_read_count
                + other.control_failure_functional_read_count,
            control_failure_functional_write_count: self.control_failure_functional_write_count
                + other.control_failure_functional_write_count,
            sync_control_failure_count: self.sync_control_failure_count
                + other.sync_control_failure_count,
            tlb_control_failure_count: self.tlb_control_failure_count
                + other.tlb_control_failure_count,
            cache_control_failure_count: self.cache_control_failure_count
                + other.cache_control_failure_count,
            htm_control_failure_count: self.htm_control_failure_count
                + other.htm_control_failure_count,
            diagnostic_control_failure_count: self.diagnostic_control_failure_count
                + other.diagnostic_control_failure_count,
            sideband_event_count: self.sideband_event_count + other.sideband_event_count,
            tlb_sync_event_count: self.tlb_sync_event_count + other.tlb_sync_event_count,
            trace_tlb_sync_count: self.trace_tlb_sync_count + other.trace_tlb_sync_count,
            cache_flush_event_count: self.cache_flush_event_count + other.cache_flush_event_count,
            trace_cache_flush_count: self.trace_cache_flush_count + other.trace_cache_flush_count,
            trace_l1_invalidation_count: self.trace_l1_invalidation_count
                + other.trace_l1_invalidation_count,
            diagnostic_print_event_count: self.diagnostic_print_event_count
                + other.diagnostic_print_event_count,
            trace_diagnostic_count: self.trace_diagnostic_count + other.trace_diagnostic_count,
            htm_abort_event_count: self.htm_abort_event_count + other.htm_abort_event_count,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadExpectedTrafficTraceReplaySummary {
    route: WorkloadRouteId,
    minimum_scheduled_count: usize,
    minimum_response_delivery_count: usize,
    minimum_trace_completed_response_count: usize,
    minimum_trace_retry_response_count: usize,
    minimum_trace_store_conditional_failed_response_count: usize,
    minimum_memory_trace_event_count: usize,
    minimum_memory_write_completion_count: usize,
    minimum_trace_data_cache_response_count: usize,
    minimum_trace_data_cache_error_count: usize,
    minimum_memory_failure_count: usize,
    minimum_memory_failure_invalid_destination_count: usize,
    minimum_memory_failure_bad_address_count: usize,
    minimum_memory_failure_read_count: usize,
    minimum_memory_failure_write_count: usize,
    minimum_memory_failure_functional_read_count: usize,
    minimum_memory_failure_functional_write_count: usize,
    minimum_trace_error_count: usize,
    minimum_trace_htm_access_count: usize,
    minimum_control_ack_count: usize,
    minimum_sync_control_ack_count: usize,
    minimum_htm_control_ack_count: usize,
    minimum_control_failure_count: usize,
    minimum_control_failure_invalid_destination_count: usize,
    minimum_control_failure_bad_address_count: usize,
    minimum_control_failure_read_count: usize,
    minimum_control_failure_write_count: usize,
    minimum_control_failure_functional_read_count: usize,
    minimum_control_failure_functional_write_count: usize,
    minimum_sync_control_failure_count: usize,
    minimum_tlb_control_failure_count: usize,
    minimum_cache_control_failure_count: usize,
    minimum_htm_control_failure_count: usize,
    minimum_diagnostic_control_failure_count: usize,
    minimum_sideband_event_count: usize,
    minimum_tlb_sync_event_count: usize,
    minimum_trace_tlb_sync_count: usize,
    minimum_cache_flush_event_count: usize,
    minimum_trace_cache_flush_count: usize,
    minimum_trace_l1_invalidation_count: usize,
    minimum_diagnostic_print_event_count: usize,
    minimum_trace_diagnostic_count: usize,
    minimum_htm_abort_event_count: usize,
}

impl WorkloadExpectedTrafficTraceReplaySummary {
    pub fn new(route: WorkloadRouteId) -> Self {
        Self {
            route,
            minimum_scheduled_count: 0,
            minimum_response_delivery_count: 0,
            minimum_trace_completed_response_count: 0,
            minimum_trace_retry_response_count: 0,
            minimum_trace_store_conditional_failed_response_count: 0,
            minimum_memory_trace_event_count: 0,
            minimum_memory_write_completion_count: 0,
            minimum_trace_data_cache_response_count: 0,
            minimum_trace_data_cache_error_count: 0,
            minimum_memory_failure_count: 0,
            minimum_memory_failure_invalid_destination_count: 0,
            minimum_memory_failure_bad_address_count: 0,
            minimum_memory_failure_read_count: 0,
            minimum_memory_failure_write_count: 0,
            minimum_memory_failure_functional_read_count: 0,
            minimum_memory_failure_functional_write_count: 0,
            minimum_trace_error_count: 0,
            minimum_trace_htm_access_count: 0,
            minimum_control_ack_count: 0,
            minimum_sync_control_ack_count: 0,
            minimum_htm_control_ack_count: 0,
            minimum_control_failure_count: 0,
            minimum_control_failure_invalid_destination_count: 0,
            minimum_control_failure_bad_address_count: 0,
            minimum_control_failure_read_count: 0,
            minimum_control_failure_write_count: 0,
            minimum_control_failure_functional_read_count: 0,
            minimum_control_failure_functional_write_count: 0,
            minimum_sync_control_failure_count: 0,
            minimum_tlb_control_failure_count: 0,
            minimum_cache_control_failure_count: 0,
            minimum_htm_control_failure_count: 0,
            minimum_diagnostic_control_failure_count: 0,
            minimum_sideband_event_count: 0,
            minimum_tlb_sync_event_count: 0,
            minimum_trace_tlb_sync_count: 0,
            minimum_cache_flush_event_count: 0,
            minimum_trace_cache_flush_count: 0,
            minimum_trace_l1_invalidation_count: 0,
            minimum_diagnostic_print_event_count: 0,
            minimum_trace_diagnostic_count: 0,
            minimum_htm_abort_event_count: 0,
        }
    }

    pub fn with_minimum_scheduled_count(mut self, minimum_scheduled_count: usize) -> Self {
        self.minimum_scheduled_count = minimum_scheduled_count;
        self
    }

    pub fn with_minimum_response_delivery_count(
        mut self,
        minimum_response_delivery_count: usize,
    ) -> Self {
        self.minimum_response_delivery_count = minimum_response_delivery_count;
        self
    }

    pub fn with_minimum_trace_completed_response_count(
        mut self,
        minimum_trace_completed_response_count: usize,
    ) -> Self {
        self.minimum_trace_completed_response_count = minimum_trace_completed_response_count;
        self
    }

    pub fn with_minimum_trace_retry_response_count(
        mut self,
        minimum_trace_retry_response_count: usize,
    ) -> Self {
        self.minimum_trace_retry_response_count = minimum_trace_retry_response_count;
        self
    }

    pub fn with_minimum_trace_store_conditional_failed_response_count(
        mut self,
        minimum_trace_store_conditional_failed_response_count: usize,
    ) -> Self {
        self.minimum_trace_store_conditional_failed_response_count =
            minimum_trace_store_conditional_failed_response_count;
        self
    }

    pub fn with_minimum_memory_trace_event_count(
        mut self,
        minimum_memory_trace_event_count: usize,
    ) -> Self {
        self.minimum_memory_trace_event_count = minimum_memory_trace_event_count;
        self
    }

    pub fn with_minimum_memory_write_completion_count(
        mut self,
        minimum_memory_write_completion_count: usize,
    ) -> Self {
        self.minimum_memory_write_completion_count = minimum_memory_write_completion_count;
        self
    }

    pub fn with_minimum_trace_data_cache_response_count(
        mut self,
        minimum_trace_data_cache_response_count: usize,
    ) -> Self {
        self.minimum_trace_data_cache_response_count = minimum_trace_data_cache_response_count;
        self
    }

    pub fn with_minimum_trace_data_cache_error_count(
        mut self,
        minimum_trace_data_cache_error_count: usize,
    ) -> Self {
        self.minimum_trace_data_cache_error_count = minimum_trace_data_cache_error_count;
        self
    }

    pub fn with_minimum_memory_failure_count(
        mut self,
        minimum_memory_failure_count: usize,
    ) -> Self {
        self.minimum_memory_failure_count = minimum_memory_failure_count;
        self
    }

    pub fn with_minimum_memory_failure_invalid_destination_count(
        mut self,
        minimum_memory_failure_invalid_destination_count: usize,
    ) -> Self {
        self.minimum_memory_failure_invalid_destination_count =
            minimum_memory_failure_invalid_destination_count;
        self
    }

    pub fn with_minimum_memory_failure_bad_address_count(
        mut self,
        minimum_memory_failure_bad_address_count: usize,
    ) -> Self {
        self.minimum_memory_failure_bad_address_count = minimum_memory_failure_bad_address_count;
        self
    }

    pub fn with_minimum_memory_failure_read_count(
        mut self,
        minimum_memory_failure_read_count: usize,
    ) -> Self {
        self.minimum_memory_failure_read_count = minimum_memory_failure_read_count;
        self
    }

    pub fn with_minimum_memory_failure_write_count(
        mut self,
        minimum_memory_failure_write_count: usize,
    ) -> Self {
        self.minimum_memory_failure_write_count = minimum_memory_failure_write_count;
        self
    }

    pub fn with_minimum_memory_failure_functional_read_count(
        mut self,
        minimum_memory_failure_functional_read_count: usize,
    ) -> Self {
        self.minimum_memory_failure_functional_read_count =
            minimum_memory_failure_functional_read_count;
        self
    }

    pub fn with_minimum_memory_failure_functional_write_count(
        mut self,
        minimum_memory_failure_functional_write_count: usize,
    ) -> Self {
        self.minimum_memory_failure_functional_write_count =
            minimum_memory_failure_functional_write_count;
        self
    }

    pub fn with_minimum_trace_error_count(mut self, minimum_trace_error_count: usize) -> Self {
        self.minimum_trace_error_count = minimum_trace_error_count;
        self
    }

    pub fn with_minimum_trace_htm_access_count(
        mut self,
        minimum_trace_htm_access_count: usize,
    ) -> Self {
        self.minimum_trace_htm_access_count = minimum_trace_htm_access_count;
        self
    }

    pub fn with_minimum_control_ack_count(mut self, minimum_control_ack_count: usize) -> Self {
        self.minimum_control_ack_count = minimum_control_ack_count;
        self
    }

    pub fn with_minimum_sync_control_ack_count(
        mut self,
        minimum_sync_control_ack_count: usize,
    ) -> Self {
        self.minimum_sync_control_ack_count = minimum_sync_control_ack_count;
        self
    }

    pub fn with_minimum_htm_control_ack_count(
        mut self,
        minimum_htm_control_ack_count: usize,
    ) -> Self {
        self.minimum_htm_control_ack_count = minimum_htm_control_ack_count;
        self
    }

    pub fn with_minimum_control_failure_count(
        mut self,
        minimum_control_failure_count: usize,
    ) -> Self {
        self.minimum_control_failure_count = minimum_control_failure_count;
        self
    }

    pub fn with_minimum_control_failure_invalid_destination_count(
        mut self,
        minimum_control_failure_invalid_destination_count: usize,
    ) -> Self {
        self.minimum_control_failure_invalid_destination_count =
            minimum_control_failure_invalid_destination_count;
        self
    }

    pub fn with_minimum_control_failure_bad_address_count(
        mut self,
        minimum_control_failure_bad_address_count: usize,
    ) -> Self {
        self.minimum_control_failure_bad_address_count = minimum_control_failure_bad_address_count;
        self
    }

    pub fn with_minimum_control_failure_read_count(
        mut self,
        minimum_control_failure_read_count: usize,
    ) -> Self {
        self.minimum_control_failure_read_count = minimum_control_failure_read_count;
        self
    }

    pub fn with_minimum_control_failure_write_count(
        mut self,
        minimum_control_failure_write_count: usize,
    ) -> Self {
        self.minimum_control_failure_write_count = minimum_control_failure_write_count;
        self
    }

    pub fn with_minimum_control_failure_functional_read_count(
        mut self,
        minimum_control_failure_functional_read_count: usize,
    ) -> Self {
        self.minimum_control_failure_functional_read_count =
            minimum_control_failure_functional_read_count;
        self
    }

    pub fn with_minimum_control_failure_functional_write_count(
        mut self,
        minimum_control_failure_functional_write_count: usize,
    ) -> Self {
        self.minimum_control_failure_functional_write_count =
            minimum_control_failure_functional_write_count;
        self
    }

    pub fn with_minimum_sync_control_failure_count(
        mut self,
        minimum_sync_control_failure_count: usize,
    ) -> Self {
        self.minimum_sync_control_failure_count = minimum_sync_control_failure_count;
        self
    }

    pub fn with_minimum_tlb_control_failure_count(
        mut self,
        minimum_tlb_control_failure_count: usize,
    ) -> Self {
        self.minimum_tlb_control_failure_count = minimum_tlb_control_failure_count;
        self
    }

    pub fn with_minimum_cache_control_failure_count(
        mut self,
        minimum_cache_control_failure_count: usize,
    ) -> Self {
        self.minimum_cache_control_failure_count = minimum_cache_control_failure_count;
        self
    }

    pub fn with_minimum_htm_control_failure_count(
        mut self,
        minimum_htm_control_failure_count: usize,
    ) -> Self {
        self.minimum_htm_control_failure_count = minimum_htm_control_failure_count;
        self
    }

    pub fn with_minimum_diagnostic_control_failure_count(
        mut self,
        minimum_diagnostic_control_failure_count: usize,
    ) -> Self {
        self.minimum_diagnostic_control_failure_count = minimum_diagnostic_control_failure_count;
        self
    }

    pub fn with_minimum_sideband_event_count(
        mut self,
        minimum_sideband_event_count: usize,
    ) -> Self {
        self.minimum_sideband_event_count = minimum_sideband_event_count;
        self
    }

    pub fn with_minimum_tlb_sync_event_count(
        mut self,
        minimum_tlb_sync_event_count: usize,
    ) -> Self {
        self.minimum_tlb_sync_event_count = minimum_tlb_sync_event_count;
        self
    }

    pub fn with_minimum_trace_tlb_sync_count(
        mut self,
        minimum_trace_tlb_sync_count: usize,
    ) -> Self {
        self.minimum_trace_tlb_sync_count = minimum_trace_tlb_sync_count;
        self
    }

    pub fn with_minimum_cache_flush_event_count(
        mut self,
        minimum_cache_flush_event_count: usize,
    ) -> Self {
        self.minimum_cache_flush_event_count = minimum_cache_flush_event_count;
        self
    }

    pub fn with_minimum_trace_cache_flush_count(
        mut self,
        minimum_trace_cache_flush_count: usize,
    ) -> Self {
        self.minimum_trace_cache_flush_count = minimum_trace_cache_flush_count;
        self
    }

    pub fn with_minimum_trace_l1_invalidation_count(
        mut self,
        minimum_trace_l1_invalidation_count: usize,
    ) -> Self {
        self.minimum_trace_l1_invalidation_count = minimum_trace_l1_invalidation_count;
        self
    }

    pub fn with_minimum_diagnostic_print_event_count(
        mut self,
        minimum_diagnostic_print_event_count: usize,
    ) -> Self {
        self.minimum_diagnostic_print_event_count = minimum_diagnostic_print_event_count;
        self
    }

    pub fn with_minimum_trace_diagnostic_count(
        mut self,
        minimum_trace_diagnostic_count: usize,
    ) -> Self {
        self.minimum_trace_diagnostic_count = minimum_trace_diagnostic_count;
        self
    }

    pub fn with_minimum_htm_abort_event_count(
        mut self,
        minimum_htm_abort_event_count: usize,
    ) -> Self {
        self.minimum_htm_abort_event_count = minimum_htm_abort_event_count;
        self
    }

    pub const fn route(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub const fn minimum_scheduled_count(&self) -> usize {
        self.minimum_scheduled_count
    }

    pub const fn minimum_response_delivery_count(&self) -> usize {
        self.minimum_response_delivery_count
    }

    pub const fn minimum_trace_completed_response_count(&self) -> usize {
        self.minimum_trace_completed_response_count
    }

    pub const fn minimum_trace_retry_response_count(&self) -> usize {
        self.minimum_trace_retry_response_count
    }

    pub const fn minimum_trace_store_conditional_failed_response_count(&self) -> usize {
        self.minimum_trace_store_conditional_failed_response_count
    }

    pub const fn minimum_memory_trace_event_count(&self) -> usize {
        self.minimum_memory_trace_event_count
    }

    pub const fn minimum_memory_write_completion_count(&self) -> usize {
        self.minimum_memory_write_completion_count
    }

    pub const fn minimum_trace_data_cache_response_count(&self) -> usize {
        self.minimum_trace_data_cache_response_count
    }

    pub const fn minimum_trace_data_cache_error_count(&self) -> usize {
        self.minimum_trace_data_cache_error_count
    }

    pub const fn minimum_memory_failure_count(&self) -> usize {
        self.minimum_memory_failure_count
    }

    pub const fn minimum_memory_failure_invalid_destination_count(&self) -> usize {
        self.minimum_memory_failure_invalid_destination_count
    }

    pub const fn minimum_memory_failure_bad_address_count(&self) -> usize {
        self.minimum_memory_failure_bad_address_count
    }

    pub const fn minimum_memory_failure_read_count(&self) -> usize {
        self.minimum_memory_failure_read_count
    }

    pub const fn minimum_memory_failure_write_count(&self) -> usize {
        self.minimum_memory_failure_write_count
    }

    pub const fn minimum_memory_failure_functional_read_count(&self) -> usize {
        self.minimum_memory_failure_functional_read_count
    }

    pub const fn minimum_memory_failure_functional_write_count(&self) -> usize {
        self.minimum_memory_failure_functional_write_count
    }

    pub const fn minimum_trace_error_count(&self) -> usize {
        self.minimum_trace_error_count
    }

    pub const fn minimum_trace_htm_access_count(&self) -> usize {
        self.minimum_trace_htm_access_count
    }

    pub const fn minimum_control_ack_count(&self) -> usize {
        self.minimum_control_ack_count
    }

    pub const fn minimum_sync_control_ack_count(&self) -> usize {
        self.minimum_sync_control_ack_count
    }

    pub const fn minimum_htm_control_ack_count(&self) -> usize {
        self.minimum_htm_control_ack_count
    }

    pub const fn minimum_control_failure_count(&self) -> usize {
        self.minimum_control_failure_count
    }

    pub const fn minimum_control_failure_invalid_destination_count(&self) -> usize {
        self.minimum_control_failure_invalid_destination_count
    }

    pub const fn minimum_control_failure_bad_address_count(&self) -> usize {
        self.minimum_control_failure_bad_address_count
    }

    pub const fn minimum_control_failure_read_count(&self) -> usize {
        self.minimum_control_failure_read_count
    }

    pub const fn minimum_control_failure_write_count(&self) -> usize {
        self.minimum_control_failure_write_count
    }

    pub const fn minimum_control_failure_functional_read_count(&self) -> usize {
        self.minimum_control_failure_functional_read_count
    }

    pub const fn minimum_control_failure_functional_write_count(&self) -> usize {
        self.minimum_control_failure_functional_write_count
    }

    pub const fn minimum_sync_control_failure_count(&self) -> usize {
        self.minimum_sync_control_failure_count
    }

    pub const fn minimum_tlb_control_failure_count(&self) -> usize {
        self.minimum_tlb_control_failure_count
    }

    pub const fn minimum_cache_control_failure_count(&self) -> usize {
        self.minimum_cache_control_failure_count
    }

    pub const fn minimum_htm_control_failure_count(&self) -> usize {
        self.minimum_htm_control_failure_count
    }

    pub const fn minimum_diagnostic_control_failure_count(&self) -> usize {
        self.minimum_diagnostic_control_failure_count
    }

    pub const fn minimum_sideband_event_count(&self) -> usize {
        self.minimum_sideband_event_count
    }

    pub const fn minimum_tlb_sync_event_count(&self) -> usize {
        self.minimum_tlb_sync_event_count
    }

    pub const fn minimum_trace_tlb_sync_count(&self) -> usize {
        self.minimum_trace_tlb_sync_count
    }

    pub const fn minimum_cache_flush_event_count(&self) -> usize {
        self.minimum_cache_flush_event_count
    }

    pub const fn minimum_trace_cache_flush_count(&self) -> usize {
        self.minimum_trace_cache_flush_count
    }

    pub const fn minimum_trace_l1_invalidation_count(&self) -> usize {
        self.minimum_trace_l1_invalidation_count
    }

    pub const fn minimum_diagnostic_print_event_count(&self) -> usize {
        self.minimum_diagnostic_print_event_count
    }

    pub const fn minimum_trace_diagnostic_count(&self) -> usize {
        self.minimum_trace_diagnostic_count
    }

    pub const fn minimum_htm_abort_event_count(&self) -> usize {
        self.minimum_htm_abort_event_count
    }

    pub(crate) fn sort_key(&self) -> &WorkloadRouteId {
        &self.route
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkloadTrafficTraceReplaySummaryExpectationError {
    Missing(WorkloadExpectedTrafficTraceReplaySummary),
    BelowMinimum {
        expected: WorkloadExpectedTrafficTraceReplaySummary,
        actual: WorkloadTrafficTraceReplaySummary,
    },
}

pub(crate) fn verify_expected_traffic_trace_replay_summaries(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    for expected in plan.expected_traffic_trace_replay_summaries() {
        let actual = result
            .traffic_trace_replay_summary(expected.route())
            .cloned()
            .ok_or_else(|| {
                WorkloadError::TrafficTraceReplaySummaryExpectation(Box::new(
                    WorkloadTrafficTraceReplaySummaryExpectationError::Missing(expected.clone()),
                ))
            })?;
        if !traffic_trace_replay_summary_meets_minimum(expected, &actual) {
            return Err(WorkloadError::TrafficTraceReplaySummaryExpectation(
                Box::new(
                    WorkloadTrafficTraceReplaySummaryExpectationError::BelowMinimum {
                        expected: expected.clone(),
                        actual,
                    },
                ),
            ));
        }
    }

    Ok(())
}

fn traffic_trace_replay_summary_meets_minimum(
    expected: &WorkloadExpectedTrafficTraceReplaySummary,
    actual: &WorkloadTrafficTraceReplaySummary,
) -> bool {
    actual.scheduled_count() >= expected.minimum_scheduled_count()
        && actual.response_delivery_count() >= expected.minimum_response_delivery_count()
        && actual.trace_completed_response_count()
            >= expected.minimum_trace_completed_response_count()
        && actual.trace_retry_response_count() >= expected.minimum_trace_retry_response_count()
        && actual.trace_store_conditional_failed_response_count()
            >= expected.minimum_trace_store_conditional_failed_response_count()
        && actual.memory_trace_event_count() >= expected.minimum_memory_trace_event_count()
        && actual.memory_write_completion_count()
            >= expected.minimum_memory_write_completion_count()
        && actual.trace_data_cache_response_count()
            >= expected.minimum_trace_data_cache_response_count()
        && actual.trace_data_cache_error_count() >= expected.minimum_trace_data_cache_error_count()
        && actual.memory_failure_count() >= expected.minimum_memory_failure_count()
        && actual.memory_failure_invalid_destination_count()
            >= expected.minimum_memory_failure_invalid_destination_count()
        && actual.memory_failure_bad_address_count()
            >= expected.minimum_memory_failure_bad_address_count()
        && actual.memory_failure_read_count() >= expected.minimum_memory_failure_read_count()
        && actual.memory_failure_write_count() >= expected.minimum_memory_failure_write_count()
        && actual.memory_failure_functional_read_count()
            >= expected.minimum_memory_failure_functional_read_count()
        && actual.memory_failure_functional_write_count()
            >= expected.minimum_memory_failure_functional_write_count()
        && actual.trace_error_count() >= expected.minimum_trace_error_count()
        && actual.trace_htm_access_count() >= expected.minimum_trace_htm_access_count()
        && actual.control_ack_count() >= expected.minimum_control_ack_count()
        && actual.sync_control_ack_count() >= expected.minimum_sync_control_ack_count()
        && actual.htm_control_ack_count() >= expected.minimum_htm_control_ack_count()
        && actual.control_failure_count() >= expected.minimum_control_failure_count()
        && actual.control_failure_invalid_destination_count()
            >= expected.minimum_control_failure_invalid_destination_count()
        && actual.control_failure_bad_address_count()
            >= expected.minimum_control_failure_bad_address_count()
        && actual.control_failure_read_count() >= expected.minimum_control_failure_read_count()
        && actual.control_failure_write_count() >= expected.minimum_control_failure_write_count()
        && actual.control_failure_functional_read_count()
            >= expected.minimum_control_failure_functional_read_count()
        && actual.control_failure_functional_write_count()
            >= expected.minimum_control_failure_functional_write_count()
        && actual.sync_control_failure_count() >= expected.minimum_sync_control_failure_count()
        && actual.tlb_control_failure_count() >= expected.minimum_tlb_control_failure_count()
        && actual.cache_control_failure_count() >= expected.minimum_cache_control_failure_count()
        && actual.htm_control_failure_count() >= expected.minimum_htm_control_failure_count()
        && actual.diagnostic_control_failure_count()
            >= expected.minimum_diagnostic_control_failure_count()
        && actual.sideband_event_count() >= expected.minimum_sideband_event_count()
        && actual.tlb_sync_event_count() >= expected.minimum_tlb_sync_event_count()
        && actual.trace_tlb_sync_count() >= expected.minimum_trace_tlb_sync_count()
        && actual.cache_flush_event_count() >= expected.minimum_cache_flush_event_count()
        && actual.trace_cache_flush_count() >= expected.minimum_trace_cache_flush_count()
        && actual.trace_l1_invalidation_count() >= expected.minimum_trace_l1_invalidation_count()
        && actual.diagnostic_print_event_count() >= expected.minimum_diagnostic_print_event_count()
        && actual.trace_diagnostic_count() >= expected.minimum_trace_diagnostic_count()
        && actual.htm_abort_event_count() >= expected.minimum_htm_abort_event_count()
}

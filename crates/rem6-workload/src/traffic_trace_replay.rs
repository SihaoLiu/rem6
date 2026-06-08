mod merge;
mod verify;

pub use verify::WorkloadTrafficTraceReplaySummaryExpectationError;

pub(crate) use verify::verify_expected_traffic_trace_replay_summaries;

use crate::WorkloadRouteId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadTrafficTraceReplaySummary {
    pub(in crate::traffic_trace_replay) route: WorkloadRouteId,
    pub(in crate::traffic_trace_replay) scheduled_count: usize,
    pub(in crate::traffic_trace_replay) response_delivery_count: usize,
    pub(in crate::traffic_trace_replay) trace_completed_response_count: usize,
    pub(in crate::traffic_trace_replay) trace_retry_response_count: usize,
    pub(in crate::traffic_trace_replay) trace_store_conditional_failed_response_count: usize,
    pub(in crate::traffic_trace_replay) trace_read_response_count: usize,
    pub(in crate::traffic_trace_replay) trace_write_response_count: usize,
    pub(in crate::traffic_trace_replay) trace_prefetch_response_count: usize,
    pub(in crate::traffic_trace_replay) trace_upgrade_response_count: usize,
    pub(in crate::traffic_trace_replay) trace_llsc_response_count: usize,
    pub(in crate::traffic_trace_replay) trace_locked_rmw_response_count: usize,
    pub(in crate::traffic_trace_replay) trace_writable_intent_response_count: usize,
    pub(in crate::traffic_trace_replay) trace_response_data_byte_count: u64,
    pub(in crate::traffic_trace_replay) trace_response_fill_data_byte_count: u64,
    pub(in crate::traffic_trace_replay) memory_trace_event_count: usize,
    pub(in crate::traffic_trace_replay) memory_write_completion_count: usize,
    pub(in crate::traffic_trace_replay) trace_data_cache_response_count: usize,
    pub(in crate::traffic_trace_replay) trace_data_cache_maintenance_response_count: usize,
    pub(in crate::traffic_trace_replay) trace_data_cache_error_count: usize,
    pub(in crate::traffic_trace_replay) trace_data_cache_invalid_destination_error_count: usize,
    pub(in crate::traffic_trace_replay) trace_data_cache_bad_address_error_count: usize,
    pub(in crate::traffic_trace_replay) trace_data_cache_read_error_count: usize,
    pub(in crate::traffic_trace_replay) trace_data_cache_write_error_count: usize,
    pub(in crate::traffic_trace_replay) trace_data_cache_functional_read_error_count: usize,
    pub(in crate::traffic_trace_replay) trace_data_cache_functional_write_error_count: usize,
    pub(in crate::traffic_trace_replay) memory_failure_count: usize,
    pub(in crate::traffic_trace_replay) memory_failure_invalid_destination_count: usize,
    pub(in crate::traffic_trace_replay) memory_failure_bad_address_count: usize,
    pub(in crate::traffic_trace_replay) memory_failure_read_count: usize,
    pub(in crate::traffic_trace_replay) memory_failure_write_count: usize,
    pub(in crate::traffic_trace_replay) memory_failure_functional_read_count: usize,
    pub(in crate::traffic_trace_replay) memory_failure_functional_write_count: usize,
    pub(in crate::traffic_trace_replay) trace_error_count: usize,
    pub(in crate::traffic_trace_replay) trace_htm_access_count: usize,
    pub(in crate::traffic_trace_replay) trace_htm_begin_count: usize,
    pub(in crate::traffic_trace_replay) control_ack_count: usize,
    pub(in crate::traffic_trace_replay) sync_control_ack_count: usize,
    pub(in crate::traffic_trace_replay) htm_control_ack_count: usize,
    pub(in crate::traffic_trace_replay) control_failure_count: usize,
    pub(in crate::traffic_trace_replay) control_failure_invalid_destination_count: usize,
    pub(in crate::traffic_trace_replay) control_failure_bad_address_count: usize,
    pub(in crate::traffic_trace_replay) control_failure_read_count: usize,
    pub(in crate::traffic_trace_replay) control_failure_write_count: usize,
    pub(in crate::traffic_trace_replay) control_failure_functional_read_count: usize,
    pub(in crate::traffic_trace_replay) control_failure_functional_write_count: usize,
    pub(in crate::traffic_trace_replay) sync_control_failure_count: usize,
    pub(in crate::traffic_trace_replay) tlb_control_failure_count: usize,
    pub(in crate::traffic_trace_replay) cache_control_failure_count: usize,
    pub(in crate::traffic_trace_replay) htm_control_failure_count: usize,
    pub(in crate::traffic_trace_replay) diagnostic_control_failure_count: usize,
    pub(in crate::traffic_trace_replay) sideband_event_count: usize,
    pub(in crate::traffic_trace_replay) trace_sideband_failure_count: usize,
    pub(in crate::traffic_trace_replay) tlb_sync_event_count: usize,
    pub(in crate::traffic_trace_replay) trace_tlb_sync_count: usize,
    pub(in crate::traffic_trace_replay) cache_flush_event_count: usize,
    pub(in crate::traffic_trace_replay) trace_cache_flush_count: usize,
    pub(in crate::traffic_trace_replay) trace_cache_flush_data_byte_count: u64,
    pub(in crate::traffic_trace_replay) trace_l1_invalidation_count: usize,
    pub(in crate::traffic_trace_replay) diagnostic_print_event_count: usize,
    pub(in crate::traffic_trace_replay) trace_diagnostic_count: usize,
    pub(in crate::traffic_trace_replay) htm_abort_event_count: usize,
    pub(in crate::traffic_trace_replay) trace_htm_abort_count: usize,
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
            trace_read_response_count: 0,
            trace_write_response_count: 0,
            trace_prefetch_response_count: 0,
            trace_upgrade_response_count: 0,
            trace_llsc_response_count: 0,
            trace_locked_rmw_response_count: 0,
            trace_writable_intent_response_count: 0,
            trace_response_data_byte_count: 0,
            trace_response_fill_data_byte_count: 0,
            memory_trace_event_count: 0,
            memory_write_completion_count: 0,
            trace_data_cache_response_count: 0,
            trace_data_cache_maintenance_response_count: 0,
            trace_data_cache_error_count: 0,
            trace_data_cache_invalid_destination_error_count: 0,
            trace_data_cache_bad_address_error_count: 0,
            trace_data_cache_read_error_count: 0,
            trace_data_cache_write_error_count: 0,
            trace_data_cache_functional_read_error_count: 0,
            trace_data_cache_functional_write_error_count: 0,
            memory_failure_count: 0,
            memory_failure_invalid_destination_count: 0,
            memory_failure_bad_address_count: 0,
            memory_failure_read_count: 0,
            memory_failure_write_count: 0,
            memory_failure_functional_read_count: 0,
            memory_failure_functional_write_count: 0,
            trace_error_count: 0,
            trace_htm_access_count: 0,
            trace_htm_begin_count: 0,
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
            trace_sideband_failure_count: 0,
            tlb_sync_event_count: 0,
            trace_tlb_sync_count: 0,
            cache_flush_event_count: 0,
            trace_cache_flush_count: 0,
            trace_cache_flush_data_byte_count: 0,
            trace_l1_invalidation_count: 0,
            diagnostic_print_event_count: 0,
            trace_diagnostic_count: 0,
            htm_abort_event_count: 0,
            trace_htm_abort_count: 0,
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

    pub fn with_trace_response_data_byte_count(
        mut self,
        trace_response_data_byte_count: u64,
    ) -> Self {
        self.trace_response_data_byte_count = trace_response_data_byte_count;
        self
    }

    pub fn with_trace_response_fill_data_byte_count(
        mut self,
        trace_response_fill_data_byte_count: u64,
    ) -> Self {
        self.trace_response_fill_data_byte_count = trace_response_fill_data_byte_count;
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

    pub fn with_trace_data_cache_maintenance_response_count(
        mut self,
        trace_data_cache_maintenance_response_count: usize,
    ) -> Self {
        self.trace_data_cache_maintenance_response_count =
            trace_data_cache_maintenance_response_count;
        self
    }

    pub fn with_trace_data_cache_error_count(
        mut self,
        trace_data_cache_error_count: usize,
    ) -> Self {
        self.trace_data_cache_error_count = trace_data_cache_error_count;
        self
    }

    pub fn with_trace_data_cache_invalid_destination_error_count(
        mut self,
        trace_data_cache_invalid_destination_error_count: usize,
    ) -> Self {
        self.trace_data_cache_invalid_destination_error_count =
            trace_data_cache_invalid_destination_error_count;
        self
    }

    pub fn with_trace_data_cache_bad_address_error_count(
        mut self,
        trace_data_cache_bad_address_error_count: usize,
    ) -> Self {
        self.trace_data_cache_bad_address_error_count = trace_data_cache_bad_address_error_count;
        self
    }

    pub fn with_trace_data_cache_read_error_count(
        mut self,
        trace_data_cache_read_error_count: usize,
    ) -> Self {
        self.trace_data_cache_read_error_count = trace_data_cache_read_error_count;
        self
    }

    pub fn with_trace_data_cache_write_error_count(
        mut self,
        trace_data_cache_write_error_count: usize,
    ) -> Self {
        self.trace_data_cache_write_error_count = trace_data_cache_write_error_count;
        self
    }

    pub fn with_trace_data_cache_functional_read_error_count(
        mut self,
        trace_data_cache_functional_read_error_count: usize,
    ) -> Self {
        self.trace_data_cache_functional_read_error_count =
            trace_data_cache_functional_read_error_count;
        self
    }

    pub fn with_trace_data_cache_functional_write_error_count(
        mut self,
        trace_data_cache_functional_write_error_count: usize,
    ) -> Self {
        self.trace_data_cache_functional_write_error_count =
            trace_data_cache_functional_write_error_count;
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

    pub fn with_trace_htm_begin_count(mut self, trace_htm_begin_count: usize) -> Self {
        self.trace_htm_begin_count = trace_htm_begin_count;
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

    pub fn with_trace_sideband_failure_count(
        mut self,
        trace_sideband_failure_count: usize,
    ) -> Self {
        self.trace_sideband_failure_count = trace_sideband_failure_count;
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

    pub fn with_trace_cache_flush_data_byte_count(
        mut self,
        trace_cache_flush_data_byte_count: u64,
    ) -> Self {
        self.trace_cache_flush_data_byte_count = trace_cache_flush_data_byte_count;
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

    pub fn with_trace_htm_abort_count(mut self, trace_htm_abort_count: usize) -> Self {
        self.trace_htm_abort_count = trace_htm_abort_count;
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

    pub const fn trace_read_response_count(&self) -> usize {
        self.trace_read_response_count
    }

    pub const fn trace_write_response_count(&self) -> usize {
        self.trace_write_response_count
    }

    pub const fn trace_prefetch_response_count(&self) -> usize {
        self.trace_prefetch_response_count
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

    pub const fn trace_response_data_byte_count(&self) -> u64 {
        self.trace_response_data_byte_count
    }

    pub const fn trace_response_fill_data_byte_count(&self) -> u64 {
        self.trace_response_fill_data_byte_count
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

    pub const fn trace_data_cache_maintenance_response_count(&self) -> usize {
        self.trace_data_cache_maintenance_response_count
    }

    pub const fn trace_data_cache_error_count(&self) -> usize {
        self.trace_data_cache_error_count
    }

    pub const fn trace_data_cache_invalid_destination_error_count(&self) -> usize {
        self.trace_data_cache_invalid_destination_error_count
    }

    pub const fn trace_data_cache_bad_address_error_count(&self) -> usize {
        self.trace_data_cache_bad_address_error_count
    }

    pub const fn trace_data_cache_read_error_count(&self) -> usize {
        self.trace_data_cache_read_error_count
    }

    pub const fn trace_data_cache_write_error_count(&self) -> usize {
        self.trace_data_cache_write_error_count
    }

    pub const fn trace_data_cache_functional_read_error_count(&self) -> usize {
        self.trace_data_cache_functional_read_error_count
    }

    pub const fn trace_data_cache_functional_write_error_count(&self) -> usize {
        self.trace_data_cache_functional_write_error_count
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

    pub const fn trace_htm_begin_count(&self) -> usize {
        self.trace_htm_begin_count
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

    pub const fn trace_sideband_failure_count(&self) -> usize {
        self.trace_sideband_failure_count
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

    pub const fn trace_cache_flush_data_byte_count(&self) -> u64 {
        self.trace_cache_flush_data_byte_count
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

    pub const fn trace_htm_abort_count(&self) -> usize {
        self.trace_htm_abort_count
    }

    pub(crate) fn sort_key(&self) -> &WorkloadRouteId {
        &self.route
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
    minimum_trace_read_response_count: usize,
    minimum_trace_write_response_count: usize,
    minimum_trace_prefetch_response_count: usize,
    minimum_trace_upgrade_response_count: usize,
    minimum_trace_llsc_response_count: usize,
    minimum_trace_locked_rmw_response_count: usize,
    minimum_trace_writable_intent_response_count: usize,
    minimum_trace_response_data_byte_count: u64,
    minimum_trace_response_fill_data_byte_count: u64,
    minimum_memory_trace_event_count: usize,
    minimum_memory_write_completion_count: usize,
    minimum_trace_data_cache_response_count: usize,
    minimum_trace_data_cache_maintenance_response_count: usize,
    minimum_trace_data_cache_error_count: usize,
    minimum_trace_data_cache_invalid_destination_error_count: usize,
    minimum_trace_data_cache_bad_address_error_count: usize,
    minimum_trace_data_cache_read_error_count: usize,
    minimum_trace_data_cache_write_error_count: usize,
    minimum_trace_data_cache_functional_read_error_count: usize,
    minimum_trace_data_cache_functional_write_error_count: usize,
    minimum_memory_failure_count: usize,
    minimum_memory_failure_invalid_destination_count: usize,
    minimum_memory_failure_bad_address_count: usize,
    minimum_memory_failure_read_count: usize,
    minimum_memory_failure_write_count: usize,
    minimum_memory_failure_functional_read_count: usize,
    minimum_memory_failure_functional_write_count: usize,
    minimum_trace_error_count: usize,
    minimum_trace_htm_access_count: usize,
    minimum_trace_htm_begin_count: usize,
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
    minimum_trace_sideband_failure_count: usize,
    minimum_tlb_sync_event_count: usize,
    minimum_trace_tlb_sync_count: usize,
    minimum_cache_flush_event_count: usize,
    minimum_trace_cache_flush_count: usize,
    minimum_trace_cache_flush_data_byte_count: u64,
    minimum_trace_l1_invalidation_count: usize,
    minimum_diagnostic_print_event_count: usize,
    minimum_trace_diagnostic_count: usize,
    minimum_htm_abort_event_count: usize,
    minimum_trace_htm_abort_count: usize,
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
            minimum_trace_read_response_count: 0,
            minimum_trace_write_response_count: 0,
            minimum_trace_prefetch_response_count: 0,
            minimum_trace_upgrade_response_count: 0,
            minimum_trace_llsc_response_count: 0,
            minimum_trace_locked_rmw_response_count: 0,
            minimum_trace_writable_intent_response_count: 0,
            minimum_trace_response_data_byte_count: 0,
            minimum_trace_response_fill_data_byte_count: 0,
            minimum_memory_trace_event_count: 0,
            minimum_memory_write_completion_count: 0,
            minimum_trace_data_cache_response_count: 0,
            minimum_trace_data_cache_maintenance_response_count: 0,
            minimum_trace_data_cache_error_count: 0,
            minimum_trace_data_cache_invalid_destination_error_count: 0,
            minimum_trace_data_cache_bad_address_error_count: 0,
            minimum_trace_data_cache_read_error_count: 0,
            minimum_trace_data_cache_write_error_count: 0,
            minimum_trace_data_cache_functional_read_error_count: 0,
            minimum_trace_data_cache_functional_write_error_count: 0,
            minimum_memory_failure_count: 0,
            minimum_memory_failure_invalid_destination_count: 0,
            minimum_memory_failure_bad_address_count: 0,
            minimum_memory_failure_read_count: 0,
            minimum_memory_failure_write_count: 0,
            minimum_memory_failure_functional_read_count: 0,
            minimum_memory_failure_functional_write_count: 0,
            minimum_trace_error_count: 0,
            minimum_trace_htm_access_count: 0,
            minimum_trace_htm_begin_count: 0,
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
            minimum_trace_sideband_failure_count: 0,
            minimum_tlb_sync_event_count: 0,
            minimum_trace_tlb_sync_count: 0,
            minimum_cache_flush_event_count: 0,
            minimum_trace_cache_flush_count: 0,
            minimum_trace_cache_flush_data_byte_count: 0,
            minimum_trace_l1_invalidation_count: 0,
            minimum_diagnostic_print_event_count: 0,
            minimum_trace_diagnostic_count: 0,
            minimum_htm_abort_event_count: 0,
            minimum_trace_htm_abort_count: 0,
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

    pub fn with_minimum_trace_response_data_byte_count(
        mut self,
        minimum_trace_response_data_byte_count: u64,
    ) -> Self {
        self.minimum_trace_response_data_byte_count = minimum_trace_response_data_byte_count;
        self
    }

    pub fn with_minimum_trace_response_fill_data_byte_count(
        mut self,
        minimum_trace_response_fill_data_byte_count: u64,
    ) -> Self {
        self.minimum_trace_response_fill_data_byte_count =
            minimum_trace_response_fill_data_byte_count;
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

    pub fn with_minimum_trace_data_cache_maintenance_response_count(
        mut self,
        minimum_trace_data_cache_maintenance_response_count: usize,
    ) -> Self {
        self.minimum_trace_data_cache_maintenance_response_count =
            minimum_trace_data_cache_maintenance_response_count;
        self
    }

    pub fn with_minimum_trace_data_cache_error_count(
        mut self,
        minimum_trace_data_cache_error_count: usize,
    ) -> Self {
        self.minimum_trace_data_cache_error_count = minimum_trace_data_cache_error_count;
        self
    }

    pub fn with_minimum_trace_data_cache_invalid_destination_error_count(
        mut self,
        minimum_trace_data_cache_invalid_destination_error_count: usize,
    ) -> Self {
        self.minimum_trace_data_cache_invalid_destination_error_count =
            minimum_trace_data_cache_invalid_destination_error_count;
        self
    }

    pub fn with_minimum_trace_data_cache_bad_address_error_count(
        mut self,
        minimum_trace_data_cache_bad_address_error_count: usize,
    ) -> Self {
        self.minimum_trace_data_cache_bad_address_error_count =
            minimum_trace_data_cache_bad_address_error_count;
        self
    }

    pub fn with_minimum_trace_data_cache_read_error_count(
        mut self,
        minimum_trace_data_cache_read_error_count: usize,
    ) -> Self {
        self.minimum_trace_data_cache_read_error_count = minimum_trace_data_cache_read_error_count;
        self
    }

    pub fn with_minimum_trace_data_cache_write_error_count(
        mut self,
        minimum_trace_data_cache_write_error_count: usize,
    ) -> Self {
        self.minimum_trace_data_cache_write_error_count =
            minimum_trace_data_cache_write_error_count;
        self
    }

    pub fn with_minimum_trace_data_cache_functional_read_error_count(
        mut self,
        minimum_trace_data_cache_functional_read_error_count: usize,
    ) -> Self {
        self.minimum_trace_data_cache_functional_read_error_count =
            minimum_trace_data_cache_functional_read_error_count;
        self
    }

    pub fn with_minimum_trace_data_cache_functional_write_error_count(
        mut self,
        minimum_trace_data_cache_functional_write_error_count: usize,
    ) -> Self {
        self.minimum_trace_data_cache_functional_write_error_count =
            minimum_trace_data_cache_functional_write_error_count;
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

    pub fn with_minimum_trace_htm_begin_count(
        mut self,
        minimum_trace_htm_begin_count: usize,
    ) -> Self {
        self.minimum_trace_htm_begin_count = minimum_trace_htm_begin_count;
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

    pub fn with_minimum_trace_sideband_failure_count(
        mut self,
        minimum_trace_sideband_failure_count: usize,
    ) -> Self {
        self.minimum_trace_sideband_failure_count = minimum_trace_sideband_failure_count;
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

    pub fn with_minimum_trace_cache_flush_data_byte_count(
        mut self,
        minimum_trace_cache_flush_data_byte_count: u64,
    ) -> Self {
        self.minimum_trace_cache_flush_data_byte_count = minimum_trace_cache_flush_data_byte_count;
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

    pub fn with_minimum_trace_htm_abort_count(
        mut self,
        minimum_trace_htm_abort_count: usize,
    ) -> Self {
        self.minimum_trace_htm_abort_count = minimum_trace_htm_abort_count;
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

    pub const fn minimum_trace_read_response_count(&self) -> usize {
        self.minimum_trace_read_response_count
    }

    pub const fn minimum_trace_write_response_count(&self) -> usize {
        self.minimum_trace_write_response_count
    }

    pub const fn minimum_trace_prefetch_response_count(&self) -> usize {
        self.minimum_trace_prefetch_response_count
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

    pub const fn minimum_trace_response_data_byte_count(&self) -> u64 {
        self.minimum_trace_response_data_byte_count
    }

    pub const fn minimum_trace_response_fill_data_byte_count(&self) -> u64 {
        self.minimum_trace_response_fill_data_byte_count
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

    pub const fn minimum_trace_data_cache_maintenance_response_count(&self) -> usize {
        self.minimum_trace_data_cache_maintenance_response_count
    }

    pub const fn minimum_trace_data_cache_error_count(&self) -> usize {
        self.minimum_trace_data_cache_error_count
    }

    pub const fn minimum_trace_data_cache_invalid_destination_error_count(&self) -> usize {
        self.minimum_trace_data_cache_invalid_destination_error_count
    }

    pub const fn minimum_trace_data_cache_bad_address_error_count(&self) -> usize {
        self.minimum_trace_data_cache_bad_address_error_count
    }

    pub const fn minimum_trace_data_cache_read_error_count(&self) -> usize {
        self.minimum_trace_data_cache_read_error_count
    }

    pub const fn minimum_trace_data_cache_write_error_count(&self) -> usize {
        self.minimum_trace_data_cache_write_error_count
    }

    pub const fn minimum_trace_data_cache_functional_read_error_count(&self) -> usize {
        self.minimum_trace_data_cache_functional_read_error_count
    }

    pub const fn minimum_trace_data_cache_functional_write_error_count(&self) -> usize {
        self.minimum_trace_data_cache_functional_write_error_count
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

    pub const fn minimum_trace_htm_begin_count(&self) -> usize {
        self.minimum_trace_htm_begin_count
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

    pub const fn minimum_trace_sideband_failure_count(&self) -> usize {
        self.minimum_trace_sideband_failure_count
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

    pub const fn minimum_trace_cache_flush_data_byte_count(&self) -> u64 {
        self.minimum_trace_cache_flush_data_byte_count
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

    pub const fn minimum_trace_htm_abort_count(&self) -> usize {
        self.minimum_trace_htm_abort_count
    }

    pub(crate) fn sort_key(&self) -> &WorkloadRouteId {
        &self.route
    }
}

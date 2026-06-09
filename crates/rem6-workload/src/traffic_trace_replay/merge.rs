use crate::WorkloadTrafficTraceReplaySummary;

impl WorkloadTrafficTraceReplaySummary {
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
            trace_read_response_count: self.trace_read_response_count
                + other.trace_read_response_count,
            trace_write_response_count: self.trace_write_response_count
                + other.trace_write_response_count,
            trace_prefetch_response_count: self.trace_prefetch_response_count
                + other.trace_prefetch_response_count,
            trace_invalidate_response_count: self.trace_invalidate_response_count
                + other.trace_invalidate_response_count,
            trace_upgrade_response_count: self.trace_upgrade_response_count
                + other.trace_upgrade_response_count,
            trace_llsc_response_count: self.trace_llsc_response_count
                + other.trace_llsc_response_count,
            trace_locked_rmw_response_count: self.trace_locked_rmw_response_count
                + other.trace_locked_rmw_response_count,
            trace_writable_intent_response_count: self.trace_writable_intent_response_count
                + other.trace_writable_intent_response_count,
            trace_response_data_byte_count: self.trace_response_data_byte_count
                + other.trace_response_data_byte_count,
            trace_response_fill_data_byte_count: self.trace_response_fill_data_byte_count
                + other.trace_response_fill_data_byte_count,
            memory_trace_event_count: self.memory_trace_event_count
                + other.memory_trace_event_count,
            memory_write_completion_count: self.memory_write_completion_count
                + other.memory_write_completion_count,
            trace_data_cache_response_count: self.trace_data_cache_response_count
                + other.trace_data_cache_response_count,
            trace_data_cache_maintenance_response_count: self
                .trace_data_cache_maintenance_response_count
                + other.trace_data_cache_maintenance_response_count,
            trace_data_cache_clean_maintenance_response_count: self
                .trace_data_cache_clean_maintenance_response_count
                + other.trace_data_cache_clean_maintenance_response_count,
            trace_data_cache_invalidate_maintenance_response_count: self
                .trace_data_cache_invalidate_maintenance_response_count
                + other.trace_data_cache_invalidate_maintenance_response_count,
            trace_data_cache_error_count: self.trace_data_cache_error_count
                + other.trace_data_cache_error_count,
            trace_data_cache_invalid_destination_error_count: self
                .trace_data_cache_invalid_destination_error_count
                + other.trace_data_cache_invalid_destination_error_count,
            trace_data_cache_bad_address_error_count: self.trace_data_cache_bad_address_error_count
                + other.trace_data_cache_bad_address_error_count,
            trace_data_cache_read_error_count: self.trace_data_cache_read_error_count
                + other.trace_data_cache_read_error_count,
            trace_data_cache_write_error_count: self.trace_data_cache_write_error_count
                + other.trace_data_cache_write_error_count,
            trace_data_cache_functional_read_error_count: self
                .trace_data_cache_functional_read_error_count
                + other.trace_data_cache_functional_read_error_count,
            trace_data_cache_functional_write_error_count: self
                .trace_data_cache_functional_write_error_count
                + other.trace_data_cache_functional_write_error_count,
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
            trace_htm_begin_count: self.trace_htm_begin_count + other.trace_htm_begin_count,
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
            trace_sideband_failure_count: self.trace_sideband_failure_count
                + other.trace_sideband_failure_count,
            tlb_sync_event_count: self.tlb_sync_event_count + other.tlb_sync_event_count,
            trace_tlb_sync_count: self.trace_tlb_sync_count + other.trace_tlb_sync_count,
            cache_flush_event_count: self.cache_flush_event_count + other.cache_flush_event_count,
            trace_cache_flush_count: self.trace_cache_flush_count + other.trace_cache_flush_count,
            trace_cache_flush_data_byte_count: self.trace_cache_flush_data_byte_count
                + other.trace_cache_flush_data_byte_count,
            trace_l1_invalidation_count: self.trace_l1_invalidation_count
                + other.trace_l1_invalidation_count,
            diagnostic_print_event_count: self.diagnostic_print_event_count
                + other.diagnostic_print_event_count,
            trace_diagnostic_count: self.trace_diagnostic_count + other.trace_diagnostic_count,
            htm_abort_event_count: self.htm_abort_event_count + other.htm_abort_event_count,
            trace_htm_abort_count: self.trace_htm_abort_count + other.trace_htm_abort_count,
        }
    }
}

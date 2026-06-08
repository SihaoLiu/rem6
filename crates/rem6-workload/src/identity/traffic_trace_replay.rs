use crate::WorkloadExpectedTrafficTraceReplaySummary;

use super::{hash_str, hash_u64};

pub(super) fn hash_expected_traffic_trace_replay_summary(
    hash: &mut u64,
    expected: &WorkloadExpectedTrafficTraceReplaySummary,
) {
    hash_str(hash, expected.route().as_str());
    for count in [
        expected.minimum_scheduled_count(),
        expected.minimum_response_delivery_count(),
        expected.minimum_trace_completed_response_count(),
        expected.minimum_trace_retry_response_count(),
        expected.minimum_trace_store_conditional_failed_response_count(),
        expected.minimum_trace_read_response_count(),
        expected.minimum_trace_write_response_count(),
        expected.minimum_trace_prefetch_response_count(),
        expected.minimum_trace_upgrade_response_count(),
        expected.minimum_trace_llsc_response_count(),
        expected.minimum_trace_locked_rmw_response_count(),
        expected.minimum_trace_writable_intent_response_count(),
    ] {
        hash_u64(hash, count as u64);
    }
    hash_u64(hash, expected.minimum_trace_response_data_byte_count());
    hash_u64(hash, expected.minimum_trace_response_fill_data_byte_count());
    for count in [
        expected.minimum_memory_trace_event_count(),
        expected.minimum_memory_write_completion_count(),
        expected.minimum_trace_data_cache_response_count(),
        expected.minimum_trace_data_cache_maintenance_response_count(),
        expected.minimum_trace_data_cache_clean_maintenance_response_count(),
        expected.minimum_trace_data_cache_invalidate_maintenance_response_count(),
        expected.minimum_trace_data_cache_error_count(),
        expected.minimum_trace_data_cache_invalid_destination_error_count(),
        expected.minimum_trace_data_cache_bad_address_error_count(),
        expected.minimum_trace_data_cache_read_error_count(),
        expected.minimum_trace_data_cache_write_error_count(),
        expected.minimum_trace_data_cache_functional_read_error_count(),
        expected.minimum_trace_data_cache_functional_write_error_count(),
        expected.minimum_memory_failure_count(),
        expected.minimum_memory_failure_invalid_destination_count(),
        expected.minimum_memory_failure_bad_address_count(),
        expected.minimum_memory_failure_read_count(),
        expected.minimum_memory_failure_write_count(),
        expected.minimum_memory_failure_functional_read_count(),
        expected.minimum_memory_failure_functional_write_count(),
    ] {
        hash_u64(hash, count as u64);
    }
    hash_u64(hash, expected.minimum_trace_error_count() as u64);
    hash_u64(hash, expected.minimum_trace_htm_access_count() as u64);
    hash_u64(hash, expected.minimum_trace_htm_begin_count() as u64);
    hash_u64(hash, expected.minimum_control_ack_count() as u64);
    hash_u64(hash, expected.minimum_sync_control_ack_count() as u64);
    hash_u64(hash, expected.minimum_htm_control_ack_count() as u64);
    hash_u64(hash, expected.minimum_control_failure_count() as u64);
    hash_u64(
        hash,
        expected.minimum_control_failure_invalid_destination_count() as u64,
    );
    hash_u64(
        hash,
        expected.minimum_control_failure_bad_address_count() as u64,
    );
    hash_u64(hash, expected.minimum_control_failure_read_count() as u64);
    hash_u64(hash, expected.minimum_control_failure_write_count() as u64);
    hash_u64(
        hash,
        expected.minimum_control_failure_functional_read_count() as u64,
    );
    hash_u64(
        hash,
        expected.minimum_control_failure_functional_write_count() as u64,
    );
    hash_u64(hash, expected.minimum_sync_control_failure_count() as u64);
    hash_u64(hash, expected.minimum_tlb_control_failure_count() as u64);
    hash_u64(hash, expected.minimum_cache_control_failure_count() as u64);
    hash_u64(hash, expected.minimum_htm_control_failure_count() as u64);
    hash_u64(
        hash,
        expected.minimum_diagnostic_control_failure_count() as u64,
    );
    hash_u64(hash, expected.minimum_sideband_event_count() as u64);
    hash_u64(hash, expected.minimum_trace_sideband_failure_count() as u64);
    hash_u64(hash, expected.minimum_tlb_sync_event_count() as u64);
    hash_u64(hash, expected.minimum_trace_tlb_sync_count() as u64);
    hash_u64(hash, expected.minimum_cache_flush_event_count() as u64);
    hash_u64(hash, expected.minimum_trace_cache_flush_count() as u64);
    hash_u64(hash, expected.minimum_trace_cache_flush_data_byte_count());
    hash_u64(hash, expected.minimum_trace_l1_invalidation_count() as u64);
    hash_u64(hash, expected.minimum_diagnostic_print_event_count() as u64);
    hash_u64(hash, expected.minimum_trace_diagnostic_count() as u64);
    hash_u64(hash, expected.minimum_htm_abort_event_count() as u64);
    hash_u64(hash, expected.minimum_trace_htm_abort_count() as u64);
}

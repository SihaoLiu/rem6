use crate::{
    WorkloadError, WorkloadExpectedTrafficTraceReplaySummary, WorkloadReplayPlan, WorkloadResult,
    WorkloadTrafficTraceReplaySummary,
};

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
        && actual.trace_read_response_count() >= expected.minimum_trace_read_response_count()
        && actual.trace_write_response_count() >= expected.minimum_trace_write_response_count()
        && actual.trace_prefetch_response_count()
            >= expected.minimum_trace_prefetch_response_count()
        && actual.trace_upgrade_response_count() >= expected.minimum_trace_upgrade_response_count()
        && actual.trace_llsc_response_count() >= expected.minimum_trace_llsc_response_count()
        && actual.trace_locked_rmw_response_count()
            >= expected.minimum_trace_locked_rmw_response_count()
        && actual.trace_writable_intent_response_count()
            >= expected.minimum_trace_writable_intent_response_count()
        && actual.trace_response_data_byte_count()
            >= expected.minimum_trace_response_data_byte_count()
        && actual.trace_response_fill_data_byte_count()
            >= expected.minimum_trace_response_fill_data_byte_count()
        && actual.memory_trace_event_count() >= expected.minimum_memory_trace_event_count()
        && actual.memory_write_completion_count()
            >= expected.minimum_memory_write_completion_count()
        && actual.trace_data_cache_response_count()
            >= expected.minimum_trace_data_cache_response_count()
        && actual.trace_data_cache_maintenance_response_count()
            >= expected.minimum_trace_data_cache_maintenance_response_count()
        && actual.trace_data_cache_error_count() >= expected.minimum_trace_data_cache_error_count()
        && actual.trace_data_cache_invalid_destination_error_count()
            >= expected.minimum_trace_data_cache_invalid_destination_error_count()
        && actual.trace_data_cache_bad_address_error_count()
            >= expected.minimum_trace_data_cache_bad_address_error_count()
        && actual.trace_data_cache_read_error_count()
            >= expected.minimum_trace_data_cache_read_error_count()
        && actual.trace_data_cache_write_error_count()
            >= expected.minimum_trace_data_cache_write_error_count()
        && actual.trace_data_cache_functional_read_error_count()
            >= expected.minimum_trace_data_cache_functional_read_error_count()
        && actual.trace_data_cache_functional_write_error_count()
            >= expected.minimum_trace_data_cache_functional_write_error_count()
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
        && actual.trace_htm_begin_count() >= expected.minimum_trace_htm_begin_count()
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
        && actual.trace_sideband_failure_count() >= expected.minimum_trace_sideband_failure_count()
        && actual.tlb_sync_event_count() >= expected.minimum_tlb_sync_event_count()
        && actual.trace_tlb_sync_count() >= expected.minimum_trace_tlb_sync_count()
        && actual.cache_flush_event_count() >= expected.minimum_cache_flush_event_count()
        && actual.trace_cache_flush_count() >= expected.minimum_trace_cache_flush_count()
        && actual.trace_cache_flush_data_byte_count()
            >= expected.minimum_trace_cache_flush_data_byte_count()
        && actual.trace_l1_invalidation_count() >= expected.minimum_trace_l1_invalidation_count()
        && actual.diagnostic_print_event_count() >= expected.minimum_diagnostic_print_event_count()
        && actual.trace_diagnostic_count() >= expected.minimum_trace_diagnostic_count()
        && actual.htm_abort_event_count() >= expected.minimum_htm_abort_event_count()
        && actual.trace_htm_abort_count() >= expected.minimum_trace_htm_abort_count()
}

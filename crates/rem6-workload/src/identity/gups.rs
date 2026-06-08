use crate::WorkloadExpectedGupsRunSummary;

use super::{hash_str, hash_u64};

pub(super) fn hash_expected_gups_run_summary(
    hash: &mut u64,
    expected: &WorkloadExpectedGupsRunSummary,
) {
    hash_str(hash, expected.route().as_str());
    match expected.maximum_final_tick() {
        Some(maximum_final_tick) => {
            hash_u64(hash, 1);
            hash_u64(hash, maximum_final_tick);
        }
        None => hash_u64(hash, 0),
    }
    for count in [
        expected.minimum_scheduled_count(),
        expected.minimum_response_count(),
        expected.minimum_completed_response_count(),
        expected.minimum_retry_response_count(),
        expected.minimum_store_conditional_failed_response_count(),
        expected.minimum_read_response_count(),
        expected.minimum_write_response_count(),
        expected.minimum_memory_trace_event_count(),
    ] {
        hash_u64(hash, count as u64);
    }
    hash_u64(hash, expected.minimum_response_data_byte_count());
}

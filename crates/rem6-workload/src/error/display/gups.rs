use std::fmt;

use crate::{WorkloadError, WorkloadGupsRunSummaryExpectationError};

pub(super) fn format_gups_error(
    error: &WorkloadError,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match error {
        WorkloadError::GupsRunSummaryExpectation(error) => match error.as_ref() {
            WorkloadGupsRunSummaryExpectationError::DuplicateExpected { route } => write!(
                formatter,
                "duplicate GUPS run summary expectation for route {}",
                route.as_str()
            ),
            WorkloadGupsRunSummaryExpectationError::Missing(expected) => write!(
                formatter,
                "GUPS run summary for route {} was not recorded; expected final tick at most {:?}, scheduled {}, responses {}, completed {}, retry {}, store-conditional failed {}, reads {}, writes {}, response data bytes {}, memory trace events {}",
                expected.route().as_str(),
                expected.maximum_final_tick(),
                expected.minimum_scheduled_count(),
                expected.minimum_response_count(),
                expected.minimum_completed_response_count(),
                expected.minimum_retry_response_count(),
                expected.minimum_store_conditional_failed_response_count(),
                expected.minimum_read_response_count(),
                expected.minimum_write_response_count(),
                expected.minimum_response_data_byte_count(),
                expected.minimum_memory_trace_event_count()
            ),
            WorkloadGupsRunSummaryExpectationError::OutsideBounds { expected, actual } => write!(
                formatter,
                "GUPS run summary for route {} has final tick {:?}/{:?}, scheduled {}/{}, responses {}/{}, completed {}/{}, retry {}/{}, store-conditional failed {}/{}, reads {}/{}, writes {}/{}, response data bytes {}/{}, memory trace events {}/{}",
                expected.route().as_str(),
                actual.final_tick(),
                expected.maximum_final_tick(),
                actual.scheduled_count(),
                expected.minimum_scheduled_count(),
                actual.response_count(),
                expected.minimum_response_count(),
                actual.completed_response_count(),
                expected.minimum_completed_response_count(),
                actual.retry_response_count(),
                expected.minimum_retry_response_count(),
                actual.store_conditional_failed_response_count(),
                expected.minimum_store_conditional_failed_response_count(),
                actual.read_response_count(),
                expected.minimum_read_response_count(),
                actual.write_response_count(),
                expected.minimum_write_response_count(),
                actual.response_data_byte_count(),
                expected.minimum_response_data_byte_count(),
                actual.memory_trace_event_count(),
                expected.minimum_memory_trace_event_count()
            ),
            WorkloadGupsRunSummaryExpectationError::AfterResultFinalTick {
                actual,
                result_final_tick,
            } => write!(
                formatter,
                "GUPS run summary for route {} has final tick {} after workload final tick {}",
                actual.route().as_str(),
                actual.final_tick(),
                result_final_tick
            ),
        },
        _ => unreachable!("GUPS error formatter only handles GUPS errors"),
    }
}

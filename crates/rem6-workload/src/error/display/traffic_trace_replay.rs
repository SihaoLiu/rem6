use std::fmt;

use crate::{WorkloadError, WorkloadTrafficTraceReplaySummaryExpectationError};

pub(super) fn format_traffic_trace_replay_error(
    error: &WorkloadError,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match error {
        WorkloadError::DuplicateExpectedTrafficTraceReplaySummary { route } => write!(
            formatter,
            "duplicate traffic trace replay summary expectation for route {}",
            route.as_str()
        ),
        WorkloadError::TrafficTraceReplaySummaryExpectation(error) => match error.as_ref() {
            WorkloadTrafficTraceReplaySummaryExpectationError::Missing(expected) => write!(
                formatter,
                "traffic trace replay summary for route {} was not recorded; expected scheduled {}, responses {}, memory trace events {}, memory write completions {}, memory failures {}, control acks {}, control failures {}, sideband events {}, tlb sync events {}, cache flush events {}, diagnostic print events {}, htm abort events {}",
                expected.route().as_str(),
                expected.minimum_scheduled_count(),
                expected.minimum_response_delivery_count(),
                expected.minimum_memory_trace_event_count(),
                expected.minimum_memory_write_completion_count(),
                expected.minimum_memory_failure_count(),
                expected.minimum_control_ack_count(),
                expected.minimum_control_failure_count(),
                expected.minimum_sideband_event_count(),
                expected.minimum_tlb_sync_event_count(),
                expected.minimum_cache_flush_event_count(),
                expected.minimum_diagnostic_print_event_count(),
                expected.minimum_htm_abort_event_count()
            ),
            WorkloadTrafficTraceReplaySummaryExpectationError::BelowMinimum {
                expected,
                actual,
            } => write!(
                formatter,
                "traffic trace replay summary for route {} has scheduled {}/{}, responses {}/{}, memory trace events {}/{}, memory write completions {}/{}, memory failures {}/{}, control acks {}/{}, control failures {}/{}, sideband events {}/{}, tlb sync events {}/{}, cache flush events {}/{}, diagnostic print events {}/{}, htm abort events {}/{}",
                expected.route().as_str(),
                actual.scheduled_count(),
                expected.minimum_scheduled_count(),
                actual.response_delivery_count(),
                expected.minimum_response_delivery_count(),
                actual.memory_trace_event_count(),
                expected.minimum_memory_trace_event_count(),
                actual.memory_write_completion_count(),
                expected.minimum_memory_write_completion_count(),
                actual.memory_failure_count(),
                expected.minimum_memory_failure_count(),
                actual.control_ack_count(),
                expected.minimum_control_ack_count(),
                actual.control_failure_count(),
                expected.minimum_control_failure_count(),
                actual.sideband_event_count(),
                expected.minimum_sideband_event_count(),
                actual.tlb_sync_event_count(),
                expected.minimum_tlb_sync_event_count(),
                actual.cache_flush_event_count(),
                expected.minimum_cache_flush_event_count(),
                actual.diagnostic_print_event_count(),
                expected.minimum_diagnostic_print_event_count(),
                actual.htm_abort_event_count(),
                expected.minimum_htm_abort_event_count()
            ),
        },
        _ => unreachable!("traffic trace replay formatter called with unrelated workload error"),
    }
}

use rem6_memory::ResponseStatus;
use rem6_traffic::{
    TrafficTraceCacheKind, TrafficTraceControlFailureSource, TrafficTraceDiagnosticKind,
    TrafficTraceErrorKind, TrafficTraceHtmKind, TrafficTraceTlbKind,
};

use crate::{
    RiscvTraceErrorRecord, TrafficTraceReplayScheduledControlFailure,
    TrafficTraceReplayScheduledSidebandEvent, TrafficTraceReplaySidebandEvent,
};

use super::{RiscvWorkloadTraceMemoryFailureRecord, RiscvWorkloadTraceMemoryResponseRecord};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TrafficTraceReplaySidebandCounts {
    pub(super) tlb_sync: usize,
    pub(super) cache_flush: usize,
    pub(super) diagnostic_print: usize,
    pub(super) htm_abort: usize,
}

pub(super) fn traffic_trace_replay_sideband_counts(
    events: &[TrafficTraceReplayScheduledSidebandEvent],
) -> TrafficTraceReplaySidebandCounts {
    events.iter().fold(
        TrafficTraceReplaySidebandCounts::default(),
        |mut counts, event| {
            match event.event() {
                TrafficTraceReplaySidebandEvent::Tlb(event) => match event.kind() {
                    TrafficTraceTlbKind::ExternalSync => counts.tlb_sync += 1,
                },
                TrafficTraceReplaySidebandEvent::Cache(event) => match event.kind() {
                    TrafficTraceCacheKind::Flush => counts.cache_flush += 1,
                },
                TrafficTraceReplaySidebandEvent::Diagnostic(event) => match event.kind() {
                    TrafficTraceDiagnosticKind::Print => counts.diagnostic_print += 1,
                },
                TrafficTraceReplaySidebandEvent::Htm(event) => match event.kind() {
                    TrafficTraceHtmKind::Request => {}
                    TrafficTraceHtmKind::Abort => counts.htm_abort += 1,
                },
            }
            counts
        },
    )
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TrafficTraceReplayControlFailureCounts {
    pub(super) sync: usize,
    pub(super) tlb: usize,
    pub(super) cache: usize,
    pub(super) htm: usize,
    pub(super) diagnostic: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TrafficTraceReplayResponseStatusCounts {
    pub(super) completed: usize,
    pub(super) retry: usize,
    pub(super) store_conditional_failed: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TrafficTraceReplayResponseClassCounts {
    pub(super) read: usize,
    pub(super) write: usize,
    pub(super) prefetch: usize,
    pub(super) invalidate: usize,
    pub(super) clean: usize,
    pub(super) upgrade: usize,
    pub(super) llsc: usize,
    pub(super) locked_rmw: usize,
    pub(super) writable_intent: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TrafficTraceReplayMemoryFailureKindCounts {
    pub(super) invalid_destination: usize,
    pub(super) bad_address: usize,
    pub(super) read: usize,
    pub(super) write: usize,
    pub(super) functional_read: usize,
    pub(super) functional_write: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TrafficTraceReplayControlFailureKindCounts {
    pub(super) invalid_destination: usize,
    pub(super) bad_address: usize,
    pub(super) read: usize,
    pub(super) write: usize,
    pub(super) functional_read: usize,
    pub(super) functional_write: usize,
}

pub(super) fn traffic_trace_replay_response_status_counts(
    records: &[RiscvWorkloadTraceMemoryResponseRecord],
) -> TrafficTraceReplayResponseStatusCounts {
    records.iter().fold(
        TrafficTraceReplayResponseStatusCounts::default(),
        |mut counts, record| {
            match record.status() {
                ResponseStatus::Completed => counts.completed += 1,
                ResponseStatus::Retry => counts.retry += 1,
                ResponseStatus::StoreConditionalFailed => counts.store_conditional_failed += 1,
            }
            counts
        },
    )
}

pub(super) fn traffic_trace_replay_response_class_counts(
    records: &[RiscvWorkloadTraceMemoryResponseRecord],
) -> TrafficTraceReplayResponseClassCounts {
    records.iter().fold(
        TrafficTraceReplayResponseClassCounts::default(),
        |mut counts, record| {
            let kind = record.kind();
            if kind.is_read() {
                counts.read += 1;
            }
            if kind.is_write() {
                counts.write += 1;
            }
            if kind.is_prefetch() {
                counts.prefetch += 1;
            }
            if kind.invalidates_line() {
                counts.invalidate += 1;
            }
            if kind.cleans_line() {
                counts.clean += 1;
            }
            if kind.is_upgrade() {
                counts.upgrade += 1;
            }
            if kind.is_llsc() {
                counts.llsc += 1;
            }
            if kind.is_locked_rmw() {
                counts.locked_rmw += 1;
            }
            if kind.carries_writable_intent() {
                counts.writable_intent += 1;
            }
            counts
        },
    )
}

pub(super) fn traffic_trace_replay_memory_failure_kind_counts(
    records: &[RiscvWorkloadTraceMemoryFailureRecord],
) -> TrafficTraceReplayMemoryFailureKindCounts {
    records.iter().fold(
        TrafficTraceReplayMemoryFailureKindCounts::default(),
        |mut counts, record| {
            match record.error() {
                TrafficTraceErrorKind::InvalidDestination => counts.invalid_destination += 1,
                TrafficTraceErrorKind::BadAddress => counts.bad_address += 1,
                TrafficTraceErrorKind::Read => counts.read += 1,
                TrafficTraceErrorKind::Write => counts.write += 1,
                TrafficTraceErrorKind::FunctionalRead => counts.functional_read += 1,
                TrafficTraceErrorKind::FunctionalWrite => counts.functional_write += 1,
            }
            counts
        },
    )
}

pub(super) fn traffic_trace_replay_data_cache_error_kind_counts(
    records: &[RiscvTraceErrorRecord],
) -> TrafficTraceReplayMemoryFailureKindCounts {
    records.iter().fold(
        TrafficTraceReplayMemoryFailureKindCounts::default(),
        |mut counts, record| {
            match record.error() {
                TrafficTraceErrorKind::InvalidDestination => counts.invalid_destination += 1,
                TrafficTraceErrorKind::BadAddress => counts.bad_address += 1,
                TrafficTraceErrorKind::Read => counts.read += 1,
                TrafficTraceErrorKind::Write => counts.write += 1,
                TrafficTraceErrorKind::FunctionalRead => counts.functional_read += 1,
                TrafficTraceErrorKind::FunctionalWrite => counts.functional_write += 1,
            }
            counts
        },
    )
}

pub(super) fn traffic_trace_replay_control_failure_kind_counts(
    failures: &[TrafficTraceReplayScheduledControlFailure],
) -> TrafficTraceReplayControlFailureKindCounts {
    failures.iter().fold(
        TrafficTraceReplayControlFailureKindCounts::default(),
        |mut counts, failure| {
            match failure.record().failure().error() {
                TrafficTraceErrorKind::InvalidDestination => counts.invalid_destination += 1,
                TrafficTraceErrorKind::BadAddress => counts.bad_address += 1,
                TrafficTraceErrorKind::Read => counts.read += 1,
                TrafficTraceErrorKind::Write => counts.write += 1,
                TrafficTraceErrorKind::FunctionalRead => counts.functional_read += 1,
                TrafficTraceErrorKind::FunctionalWrite => counts.functional_write += 1,
            }
            counts
        },
    )
}

pub(super) fn traffic_trace_replay_control_failure_counts(
    failures: &[TrafficTraceReplayScheduledControlFailure],
) -> TrafficTraceReplayControlFailureCounts {
    failures.iter().fold(
        TrafficTraceReplayControlFailureCounts::default(),
        |mut counts, failure| {
            match failure.record().source() {
                Some(TrafficTraceControlFailureSource::Sync(_)) => counts.sync += 1,
                Some(TrafficTraceControlFailureSource::Tlb(_)) => counts.tlb += 1,
                Some(TrafficTraceControlFailureSource::Cache(_)) => counts.cache += 1,
                Some(TrafficTraceControlFailureSource::Htm(_)) => counts.htm += 1,
                Some(TrafficTraceControlFailureSource::Diagnostic(_)) => counts.diagnostic += 1,
                None => {}
            }
            counts
        },
    )
}

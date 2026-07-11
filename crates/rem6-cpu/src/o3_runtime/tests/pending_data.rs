//! Pending O3 data-response ownership, reset, discard, and restore behavior.

use super::*;

#[test]
fn failed_store_conditional_trace_mark_uses_dynamic_sequence_identity() {
    let mut runtime = O3RuntimeState::default();
    let mut first = store_conditional_event(0x8000, 10);
    let second = store_conditional_event(0x8000, 11);

    runtime.record_retired_instruction_with_trace(&first, true);
    runtime.record_retired_instruction_with_trace(&second, true);
    first.set_data_access_event_kind(RiscvDataAccessEventKind::ConditionalFailed);
    runtime.record_data_access_outcome(&first, 41, 7);

    let trace = runtime.trace_records();
    assert_eq!(trace.len(), 2);
    assert!(trace[0].lsq_store_conditional_failed());
    assert_eq!(trace[0].lsq_data_response_tick(), 41);
    assert_eq!(trace[0].lsq_data_latency_ticks(), 7);
    assert!(!trace[1].lsq_store_conditional_failed());
    assert_eq!(trace[1].lsq_data_response_tick(), 0);
    assert_eq!(trace[1].lsq_data_latency_ticks(), 0);
}

#[test]
fn data_response_trace_updates_current_instruction_commit_tick() {
    let mut runtime = O3RuntimeState::default();
    let event = store_conditional_event(0x8000, 10);
    runtime.record_retired_instruction_with_trace(&event, true);
    runtime.record_data_access_outcome(&event, 41, 7);
    let record = runtime.trace_records()[0];
    assert_eq!((record.writeback_tick(), record.commit_tick()), (41, 41));
}

#[test]
fn failed_store_conditional_pending_trace_identity_can_be_discarded() {
    let mut runtime = O3RuntimeState::default();
    let mut event = store_conditional_event(0x8000, 10);

    runtime.record_retired_instruction_with_trace(&event, true);
    assert_eq!(runtime.pending_trace_data_access_outcomes(), 1);

    runtime.discard_data_access_outcome(event.fetch().request_id());
    assert_eq!(runtime.pending_trace_data_access_outcomes(), 0);

    event.set_data_access_event_kind(RiscvDataAccessEventKind::ConditionalFailed);
    runtime.record_data_access_outcome(&event, 41, 7);
    assert!(!runtime.trace_records()[0].lsq_store_conditional_failed());
    assert_eq!(runtime.trace_records()[0].lsq_data_response_tick(), 0);
    assert_eq!(runtime.trace_records()[0].lsq_data_latency_ticks(), 0);
}

#[test]
fn stats_reset_preserves_pending_data_response_without_old_trace_row() {
    let mut runtime = O3RuntimeState::default();
    let event = store_conditional_event(0x8000, 10);

    runtime.record_retired_instruction_with_trace(&event, true);

    assert_eq!(
        runtime
            .stats()
            .lsq_operation_count(O3RuntimeLsqOperation::StoreConditional),
        1
    );
    assert_eq!(runtime.pending_data_accesses.len(), 1);
    assert_eq!(runtime.pending_trace_data_access_outcomes(), 1);
    assert_eq!(runtime.trace_records().len(), 1);

    runtime.reset_stats();
    runtime.reset_stats();

    assert_eq!(
        runtime
            .stats()
            .lsq_operation_count(O3RuntimeLsqOperation::StoreConditional),
        0
    );
    assert_eq!(runtime.pending_data_accesses.len(), 1);
    assert_eq!(runtime.pending_trace_data_access_outcomes(), 0);
    assert!(runtime.trace_records().is_empty());

    runtime.record_data_access_outcome(&event, 41, 7);

    assert!(runtime.pending_data_accesses.is_empty());
    assert_eq!(runtime.stats().lsq_data_latency_samples(), 1);
    assert_eq!(runtime.stats().lsq_data_latency_ticks(), 7);
    assert_eq!(
        runtime
            .stats()
            .lsq_operation_latency_samples(O3RuntimeLsqOperation::StoreConditional),
        1
    );
    assert_eq!(
        runtime
            .stats()
            .lsq_operation_count(O3RuntimeLsqOperation::StoreConditional),
        0
    );
    assert!(runtime.trace_records().is_empty());
}

#[test]
fn discard_after_stats_reset_prevents_late_data_response_outcome() {
    let mut runtime = O3RuntimeState::default();
    let event = store_conditional_event(0x8000, 10);

    runtime.record_retired_instruction_with_trace(&event, true);
    runtime.reset_stats();
    runtime.discard_data_access_outcome(event.fetch().request_id());
    runtime.record_data_access_outcome(&event, 41, 7);

    assert!(runtime.pending_data_accesses.is_empty());
    assert_eq!(runtime.stats().lsq_data_latency_samples(), 0);
    assert_eq!(runtime.stats().lsq_data_latency_ticks(), 0);
    assert!(runtime.trace_records().is_empty());
}

#[test]
fn stats_reset_preserves_untraced_pending_data_response() {
    let mut runtime = O3RuntimeState::default();
    let event = store_conditional_event(0x8000, 10);

    runtime.record_retired_instruction(&event);
    assert_eq!(runtime.pending_data_accesses.len(), 1);
    assert_eq!(runtime.pending_trace_data_access_outcomes(), 0);

    runtime.reset_stats();
    runtime.record_data_access_outcome(&event, 41, 7);

    assert!(runtime.pending_data_accesses.is_empty());
    assert_eq!(runtime.stats().lsq_data_latency_samples(), 1);
    assert_eq!(runtime.stats().lsq_data_latency_ticks(), 7);
    assert_eq!(
        runtime
            .stats()
            .lsq_operation_count(O3RuntimeLsqOperation::StoreConditional),
        0
    );
}

#[test]
fn snapshot_restore_discards_pending_data_response_identity() {
    let mut runtime = O3RuntimeState::default();
    let event = store_conditional_event(0x8000, 10);

    runtime.record_retired_instruction_with_trace(&event, true);
    let snapshot = runtime.snapshot();
    runtime.restore(snapshot).unwrap();

    assert!(runtime.pending_data_accesses.is_empty());
    assert_eq!(runtime.pending_trace_data_access_outcomes(), 0);
    assert!(runtime.trace_records().is_empty());
    runtime.record_data_access_outcome(&event, 41, 7);
    assert_eq!(runtime.stats().lsq_data_latency_samples(), 0);
}

use super::o3_runtime_memory::*;
use super::*;

use rem6_isa_riscv::{
    Immediate, MemoryWidth, Register, RegisterWrite, RiscvExecutionRecord, RiscvInstruction,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, AgentId, CacheLineLayout};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

use crate::{
    CpuCore, CpuFetchConfig, CpuFetchEvent, CpuFetchRecord, CpuId, CpuResetState, RiscvCore,
};

#[test]
fn scalar_load_issue_allocates_same_sequence_rob_and_lsq_rows() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);

    assert!(runtime.stage_live_data_access_issue_for_test(&execution, memory_request(20), 31));

    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.reorder_buffer().len(), 1);
    assert_eq!(snapshot.load_store_queue().len(), 1);
    let rob = snapshot.reorder_buffer()[0];
    let lsq = snapshot.load_store_queue()[0];
    assert_eq!(rob.sequence(), lsq.sequence());
    assert!(!rob.is_ready());
    assert!(!lsq.is_completed());
    assert_eq!(runtime.stats().max_rob_occupancy(), 1);
    assert_eq!(runtime.stats().max_lsq_occupancy(), 1);
    assert_eq!(runtime.live_data_accesses.first().unwrap().issue_tick, 31);
}

#[test]
fn scalar_store_issue_records_real_issue_tick_and_single_occupancy() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_store_event(0x8004, 11);

    assert!(runtime.stage_live_data_access_issue_for_test(&execution, memory_request(21), 37));

    let live = runtime.live_data_accesses.first().unwrap();
    assert_eq!(live.fetch_request, execution.fetch().request_id());
    assert_eq!(live.data_request, memory_request(21));
    assert_eq!(live.issue_tick, 37);
    assert_eq!(live.issue_rob_occupancy, 1);
    assert_eq!(live.issue_lsq_occupancy, 1);
    assert_eq!(live.outcome, O3LiveDataAccessOutcome::Resident);
    assert_eq!(runtime.snapshot().load_store_queue().len(), 1);
}

#[test]
fn excluded_memory_kinds_do_not_stage_live_scalar_rows() {
    let mut runtime = O3RuntimeState::default();
    let execution = store_conditional_event(0x8008, 12);

    assert!(!runtime.stage_live_data_access_issue_for_test(&execution, memory_request(22), 41));
    assert!(runtime.snapshot().reorder_buffer().is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
    assert!(runtime.live_data_accesses.is_empty());
}

#[test]
fn completed_response_marks_only_matching_rows_ready() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);
    let data_request = memory_request(20);
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, data_request, 31));
    let live_sequence = runtime.live_data_accesses.first().unwrap().sequence;
    let unrelated_sequence = runtime.allocate_sequence();
    runtime.snapshot.reorder_buffer.insert(
        0,
        O3ReorderBufferEntry::new(unrelated_sequence, Address::new(0x7ffc), None),
    );
    runtime.snapshot.load_store_queue.insert(
        0,
        O3LoadStoreQueueEntry::store(unrelated_sequence, Some(Address::new(0xa000)), 4),
    );
    let mut completed = execution.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);

    assert!(runtime
        .complete_live_data_access_response(
            &completed,
            data_request,
            41,
            10,
            Some(&[0x2a, 0, 0, 0]),
        )
        .unwrap());

    let snapshot = runtime.snapshot();
    let unrelated_rob = snapshot
        .reorder_buffer()
        .iter()
        .find(|entry| entry.sequence() == unrelated_sequence)
        .unwrap();
    let live_rob = snapshot
        .reorder_buffer()
        .iter()
        .find(|entry| entry.sequence() == live_sequence)
        .unwrap();
    let unrelated_lsq = snapshot
        .load_store_queue()
        .iter()
        .find(|entry| entry.sequence() == unrelated_sequence)
        .unwrap();
    let live_lsq = snapshot
        .load_store_queue()
        .iter()
        .find(|entry| entry.sequence() == live_sequence)
        .unwrap();
    assert!(!unrelated_rob.is_ready());
    assert!(!unrelated_lsq.is_completed());
    assert!(!live_rob.is_ready());
    assert!(live_lsq.is_completed());
    let live = runtime.live_data_accesses.first().unwrap();
    assert_eq!(live.outcome, O3LiveDataAccessOutcome::Completed);
    assert_eq!(live.response_tick, Some(41));
    assert_eq!(live.admitted_writeback_tick, Some(42));
    assert_eq!(live.latency_ticks, Some(10));
    assert_eq!(live.load_data.as_deref(), Some(&[0x2a, 0, 0, 0][..]));
    assert_eq!(
        runtime.take_ready_live_data_access_event(u64::MAX),
        Some(completed)
    );
    let snapshot = runtime.snapshot();
    let live_rob = snapshot
        .reorder_buffer()
        .iter()
        .find(|entry| entry.sequence() == live_sequence)
        .unwrap();
    assert!(live_rob.is_ready());
    assert_eq!(live_rob.ready_tick(), 42);
    assert!(runtime
        .take_ready_live_data_access_event(u64::MAX)
        .is_none());
}

#[test]
fn completed_scalar_load_reserves_writeback_before_marking_rob_ready() {
    let runtime = completed_live_load_runtime(41);
    let live = &runtime.live_data_accesses[0];
    assert_eq!(live.raw_ready_tick, Some(42));
    assert_eq!(live.admitted_writeback_tick, Some(42));
    assert!(!runtime.snapshot().reorder_buffer()[0].is_ready());
    assert!(runtime.snapshot().load_store_queue()[0].is_completed());
}

#[test]
fn scalar_load_publication_waits_until_admitted_tick() {
    let mut runtime = completed_live_load_runtime(41);
    assert!(runtime.take_ready_live_data_access_event(41).is_none());
    assert!(!runtime.snapshot().reorder_buffer()[0].is_ready());
    assert!(runtime.take_ready_live_data_access_event(42).is_some());
    assert!(runtime.snapshot().reorder_buffer()[0].is_ready());
    assert_eq!(runtime.snapshot().reorder_buffer()[0].ready_tick(), 42);
}

#[test]
fn older_memory_result_replans_younger_fixed_fu_reservation() {
    let mut runtime = O3RuntimeState::default();
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(8, 42)])
        .unwrap();
    let load = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::memory_result(4, 42)])
        .unwrap()[0];
    let fixed = runtime.writeback_reservation(8).unwrap();

    assert_eq!(load.admitted_tick(), 42);
    assert_eq!(load.slot(), 0);
    assert_eq!(fixed.admitted_tick(), 43);
    assert_eq!(fixed.slot(), 0);
    let stats = runtime.stats();
    assert_eq!(stats.writeback_port_admitted_rows(), 2);
    assert_eq!(stats.writeback_port_deferred_rows(), 1);
    assert_eq!(stats.writeback_port_deferred_row_cycles(), 1);
    assert_eq!(stats.writeback_port_max_ready_rows_per_cycle(), 2);
    assert_eq!(stats.writeback_port_max_deferred_rows(), 1);
}

#[test]
fn scalar_load_reservation_failure_does_not_partially_commit_response() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);
    let data_request = memory_request(20);
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, data_request, 31));
    let sequence = runtime.live_data_accesses[0].sequence;
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(sequence, 40)])
        .unwrap();
    let mut completed = execution.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(matches!(
        runtime.complete_live_data_access_response(
            &completed,
            data_request,
            41,
            10,
            Some(&[0x2a, 0, 0, 0]),
        ),
        Err(O3RuntimeError::WritebackReservationMismatch { .. })
    ));
    let live = &runtime.live_data_accesses[0];
    assert_eq!(live.outcome, O3LiveDataAccessOutcome::Resident);
    assert_eq!(live.response_tick, None);
    assert_eq!(live.raw_ready_tick, None);
    assert_eq!(live.admitted_writeback_tick, None);
    assert!(!runtime.snapshot().load_store_queue()[0].is_completed());
    assert!(!runtime.snapshot().reorder_buffer()[0].is_ready());
}

#[test]
fn two_live_scalar_loads_complete_out_of_order_and_retire_in_order() {
    let mut runtime = O3RuntimeState::default();
    let older = scalar_load_event_with(0x8000, 10, 12, 10, 0x9000);
    let younger = scalar_load_event_with(0x8004, 11, 13, 10, 0x9040);
    let older_data_request = memory_request(20);
    let younger_data_request = memory_request(21);

    assert!(runtime.stage_live_data_access_issue_for_test(&older, older_data_request, 31));
    assert!(runtime.stage_live_data_access_issue_for_test(&younger, younger_data_request, 32));

    let sequences = runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .map(|entry| entry.sequence())
        .collect::<Vec<_>>();
    assert_eq!(sequences.len(), 2);
    let mut younger_completed = younger.clone();
    younger_completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &younger_completed,
            younger_data_request,
            40,
            8,
            Some(&[0x63, 0, 0, 0]),
        )
        .unwrap());

    let snapshot = runtime.snapshot();
    assert!(!snapshot.reorder_buffer()[0].is_ready());
    assert!(!snapshot.reorder_buffer()[1].is_ready());
    assert!(!snapshot.load_store_queue()[0].is_completed());
    assert!(snapshot.load_store_queue()[1].is_completed());
    assert!(runtime
        .take_ready_live_data_access_event(u64::MAX)
        .is_none());

    let mut older_completed = older.clone();
    older_completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &older_completed,
            older_data_request,
            45,
            14,
            Some(&[0x2a, 0, 0, 0]),
        )
        .unwrap());

    assert_eq!(
        runtime.take_ready_live_data_access_event(u64::MAX),
        Some(older_completed.clone())
    );
    runtime.record_retired_instruction_with_trace(&older_completed, true);
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);
    assert_eq!(runtime.snapshot().load_store_queue().len(), 1);
    assert_eq!(
        runtime.take_ready_live_data_access_event(u64::MAX),
        Some(younger_completed.clone())
    );
    runtime.record_retired_instruction_with_trace(&younger_completed, true);

    assert!(runtime.snapshot().reorder_buffer().is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
    let trace = runtime.trace_records();
    assert_eq!(trace.len(), 2);
    assert_eq!(trace[0].sequence(), sequences[0]);
    assert_eq!(trace[1].sequence(), sequences[1]);
    assert_eq!(trace[0].lsq_data_response_tick(), 45);
    assert_eq!(trace[1].lsq_data_response_tick(), 40);
    assert_eq!(trace[0].commit_tick(), 46);
    assert_eq!(trace[1].commit_tick(), 46);
}

#[test]
fn writeback_wake_tracks_oldest_unpublished_scalar_load() {
    let mut runtime = O3RuntimeState::default();
    let older = scalar_load_event_with(0x8000, 10, 12, 10, 0x9000);
    let younger = scalar_load_event_with(0x8004, 11, 13, 10, 0x9040);
    let older_data_request = memory_request(20);
    let younger_data_request = memory_request(21);
    assert!(runtime.stage_live_data_access_issue_for_test(&older, older_data_request, 31));
    assert!(runtime.stage_live_data_access_issue_for_test(&younger, younger_data_request, 32));

    let mut younger_completed = younger.clone();
    younger_completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &younger_completed,
            younger_data_request,
            40,
            8,
            Some(&[0x63, 0, 0, 0]),
        )
        .unwrap());
    assert_eq!(
        runtime.earliest_unpublished_memory_result_writeback_tick(),
        None
    );

    let mut older_completed = older.clone();
    older_completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &older_completed,
            older_data_request,
            45,
            14,
            Some(&[0x2a, 0, 0, 0]),
        )
        .unwrap());
    assert_eq!(
        runtime.earliest_unpublished_memory_result_writeback_tick(),
        Some(46)
    );
}

#[test]
fn retry_response_removes_load_head_younger_rows_and_readies_one_abort_event() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8004, 11);
    let data_request = memory_request(21);
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, data_request, 37));
    stage_independent_younger(&mut runtime, &execution);
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
    let mut retry = execution.clone();
    retry.set_data_access_event_kind(RiscvDataAccessEventKind::Retry);

    assert!(runtime
        .complete_live_data_access_response(&retry, data_request, 44, 7, None,)
        .unwrap());

    assert!(runtime.snapshot().reorder_buffer().is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
    assert_eq!(
        runtime.live_data_accesses.first().unwrap().outcome,
        O3LiveDataAccessOutcome::Retried
    );
    assert_eq!(
        runtime.take_ready_live_data_access_event(u64::MAX),
        Some(retry)
    );
    assert!(runtime
        .take_ready_live_data_access_event(u64::MAX)
        .is_none());
}

#[test]
fn failed_response_drains_rows_and_never_counts_o3_retirement() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);
    let data_request = memory_request(20);
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, data_request, 31));
    stage_independent_younger(&mut runtime, &execution);
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
    let mut failed = execution.clone();
    failed.set_data_access_event_kind(RiscvDataAccessEventKind::Failed);

    assert!(runtime
        .complete_live_data_access_response(&failed, data_request, 43, 12, None,)
        .unwrap());

    assert!(runtime.snapshot().reorder_buffer().is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
    assert_eq!(
        runtime.live_data_accesses.first().unwrap().outcome,
        O3LiveDataAccessOutcome::Failed
    );
    assert_eq!(
        runtime.take_ready_live_data_access_event(u64::MAX),
        Some(failed.clone())
    );
    assert!(runtime
        .take_ready_live_data_access_event(u64::MAX)
        .is_none());

    runtime.record_retired_instruction_with_trace(&failed, true);

    assert!(runtime.live_data_accesses.is_empty());
    assert_eq!(runtime.stats().instructions(), 0);
    assert_eq!(runtime.stats().lsq_loads(), 0);
    assert!(runtime.trace_records().is_empty());
}

#[test]
fn pending_retirement_tracks_deferred_and_live_data_access() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);
    assert!(!runtime.has_pending_live_data_access_retirement());
    assert!(runtime.defer_live_data_access_execution(&execution));
    assert!(runtime.has_pending_live_data_access_retirement());
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, memory_request(20), 31));
    assert!(runtime.has_pending_live_data_access_retirement());
    runtime.discard_live_data_access_lifecycle();
    assert!(!runtime.has_pending_live_data_access_retirement());
}

#[test]
fn stats_reset_preserves_live_rows_and_request_identity() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_store_event(0x8004, 11);
    let data_request = memory_request(21);
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, data_request, 37));

    runtime.reset_stats();

    assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);
    assert_eq!(runtime.snapshot().load_store_queue().len(), 1);
    let live = runtime.live_data_accesses.first().unwrap();
    assert_eq!(live.fetch_request, execution.fetch().request_id());
    assert_eq!(live.data_request, data_request);
    assert_eq!(runtime.stats().max_rob_occupancy(), 1);
    assert_eq!(runtime.stats().max_lsq_occupancy(), 1);
}

#[test]
fn stats_reset_preserves_completed_scalar_younger_window_provenance() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);
    let data_request = memory_request(20);
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, data_request, 31));
    stage_independent_younger(&mut runtime, &execution);
    let mut completed = execution.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed,
            data_request,
            41,
            10,
            Some(&[0x2a, 0, 0, 0]),
        )
        .unwrap());
    assert_eq!(
        runtime.take_ready_live_data_access_event(u64::MAX),
        Some(completed.clone())
    );
    runtime.record_retired_instruction_with_trace(&completed, true);
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);

    runtime.reset_stats();

    assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);
    assert!(runtime.snapshot().load_store_queue().is_empty());
    assert!(!runtime.live_data_access_lifecycle_is_quiescent());
    assert_eq!(runtime.stats().max_rob_occupancy(), 1);
    assert_eq!(runtime.stats().max_lsq_occupancy(), 0);
}

#[test]
fn completed_retirement_uses_issue_and_response_ticks_then_drains_rows() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);
    let data_request = memory_request(20);
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, data_request, 31));
    let sequence = runtime.snapshot().reorder_buffer()[0].sequence();
    let destination = runtime.snapshot().reorder_buffer()[0].destination();
    let mut completed = execution.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed,
            data_request,
            41,
            10,
            Some(&[0x2a, 0, 0, 0]),
        )
        .unwrap());
    assert_eq!(
        runtime.take_ready_live_data_access_event(u64::MAX),
        Some(completed.clone())
    );

    runtime.record_retired_instruction_with_trace(&completed, true);

    assert!(runtime.live_data_accesses.is_empty());
    assert!(runtime.snapshot().reorder_buffer().is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
    assert_eq!(runtime.stats().instructions(), 1);
    assert_eq!(runtime.stats().lsq_loads(), 1);
    assert_eq!(runtime.stats().lsq_data_latency_samples(), 1);
    assert_eq!(runtime.stats().lsq_data_latency_ticks(), 10);
    assert_eq!(runtime.stats().max_rob_occupancy(), 1);
    assert_eq!(runtime.stats().max_lsq_occupancy(), 1);
    let trace = runtime.trace_records()[0];
    assert_eq!(trace.sequence(), sequence);
    assert_eq!(trace.issue_tick(), 31);
    assert_eq!(trace.writeback_tick(), 42);
    assert_eq!(trace.commit_tick(), 42);
    assert_eq!(trace.rob_occupancy(), 1);
    assert_eq!(trace.lsq_occupancy(), 1);
    assert_eq!(trace.lsq_data_response_tick(), 41);
    assert_eq!(trace.lsq_data_latency_ticks(), 10);
    assert!(runtime.snapshot().rename_map().iter().any(|entry| {
        entry.register_class() == O3RegisterClass::Integer
            && entry.architectural() == 12
            && Some(entry.physical()) == destination
    }));
}

#[test]
fn completed_load_retirement_preserves_staged_younger_row() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);
    let data_request = memory_request(20);
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, data_request, 31));
    stage_independent_younger(&mut runtime, &execution);
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
    let mut completed = execution.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed,
            data_request,
            41,
            10,
            Some(&[0x2a, 0, 0, 0]),
        )
        .unwrap());
    assert_eq!(
        runtime.take_ready_live_data_access_event(u64::MAX),
        Some(completed.clone())
    );

    runtime.record_retired_instruction_with_trace(&completed, true);

    assert!(runtime.live_data_accesses.is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);
    assert_eq!(
        runtime.snapshot().reorder_buffer()[0].pc(),
        Address::new(0x8004)
    );
    assert!(runtime.snapshot().reorder_buffer()[0].is_live_staged());
    assert_eq!(runtime.stats().max_rob_occupancy(), 2);
    assert!(!runtime.live_data_access_lifecycle_is_quiescent());

    let younger = independent_younger_event();
    runtime.retire_live_staged_instruction(&younger, &[younger.fetch().request_id()], 42);
    runtime.record_retired_instruction_with_trace(&younger, true);
    assert!(runtime.live_data_access_lifecycle_is_quiescent());
}

#[test]
fn mode_disable_preserves_completed_scalar_younger_window_until_retirement() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);
    let data_request = memory_request(20);
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, data_request, 31));
    stage_independent_younger(&mut runtime, &execution);
    let mut completed = execution.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed,
            data_request,
            41,
            10,
            Some(&[0x2a, 0, 0, 0]),
        )
        .unwrap());
    assert_eq!(
        runtime.take_ready_live_data_access_event(u64::MAX),
        Some(completed.clone())
    );
    runtime.record_retired_instruction_with_trace(&completed, true);
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);
    let core = core_with_runtime(runtime);
    core.set_detailed_live_retire_gate_enabled(true);
    assert!(!core.data_access_lifecycle_is_quiescent());

    core.set_detailed_live_retire_gate_enabled(false);

    assert!(core.has_pending_o3_runtime_retirement());
    let younger = independent_younger_event();
    let mut state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 1);
    assert!(!state.o3_runtime.live_data_access_lifecycle_is_quiescent());

    state
        .o3_runtime
        .retire_live_staged_instruction(&younger, &[younger.fetch().request_id()], 42);
    state
        .o3_runtime
        .record_retired_instruction_with_trace(&younger, true);

    assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
    assert!(state.o3_runtime.live_data_access_lifecycle_is_quiescent());
    assert_eq!(state.o3_runtime.writeback_reservations().len(), 1);
    assert!(!state.o3_runtime.has_pending_retirement_authority());
    state.o3_runtime.prune_writeback_calendar_before(43);
    assert!(state.o3_runtime.writeback_reservations().is_empty());
    assert!(!state.o3_runtime.has_pending_retirement_authority());
}

#[test]
fn retry_retirement_clears_lifecycle_without_counting_instruction() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_store_event(0x8004, 11);
    let data_request = memory_request(21);
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, data_request, 37));
    let mut retry = execution.clone();
    retry.set_data_access_event_kind(RiscvDataAccessEventKind::Retry);
    assert!(runtime
        .complete_live_data_access_response(&retry, data_request, 44, 7, None,)
        .unwrap());
    assert_eq!(
        runtime.take_ready_live_data_access_event(u64::MAX),
        Some(retry.clone())
    );

    runtime.record_retired_instruction_with_trace(&retry, true);

    assert!(runtime.live_data_accesses.is_empty());
    assert!(runtime.snapshot().reorder_buffer().is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
    assert_eq!(runtime.stats().instructions(), 0);
    assert_eq!(runtime.stats().lsq_stores(), 0);
    assert!(runtime.trace_records().is_empty());
}

#[test]
fn cleanup_after_ready_event_prevents_stale_terminal_retirement() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);
    let data_request = memory_request(20);
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, data_request, 31));
    let mut completed = execution.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed,
            data_request,
            41,
            10,
            Some(&[0x2a, 0, 0, 0]),
        )
        .unwrap());
    assert_eq!(
        runtime.take_ready_live_data_access_event(u64::MAX),
        Some(completed.clone())
    );

    runtime.discard_live_data_access_lifecycle();
    runtime.record_retired_instruction_with_trace(&completed, true);

    assert_eq!(runtime.stats().instructions(), 0);
    assert!(runtime.snapshot().reorder_buffer().is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
    assert!(runtime.trace_records().is_empty());
}

#[test]
fn cleanup_discard_removes_resident_scalar_rows_and_identity() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, memory_request(20), 31));

    runtime.discard_live_staged_instructions();

    assert!(runtime.live_data_accesses.is_empty());
    assert!(runtime.snapshot().reorder_buffer().is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
}

#[test]
fn cleanup_pc_redirect_removes_resident_scalar_rows_and_identity() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, memory_request(20), 31));
    stage_independent_younger(&mut runtime, &execution);
    let core = core_with_runtime(runtime);

    core.redirect_pc(Address::new(0x9000));

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.o3_runtime.live_data_accesses.is_empty());
    assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
    assert!(state.o3_runtime.snapshot().load_store_queue().is_empty());
}

#[test]
fn cleanup_hart_reset_removes_scalar_lifecycle_without_reissuing_stale_event() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, memory_request(20), 31));
    let core = core_with_runtime(runtime);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.events.push(execution.clone());
        state
            .issued_data_for_fetches
            .insert(execution.fetch().request_id());
    }

    core.resume_nonretentive_supervisor_hart(Address::new(0x9000), 0);

    assert!(!core.has_unissued_data_access());
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.o3_runtime.live_data_access_lifecycle_is_quiescent());
    assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
    assert!(state.o3_runtime.snapshot().load_store_queue().is_empty());
}

fn scalar_load_event(pc: u64, sequence: u64) -> RiscvCpuExecutionEvent {
    scalar_load_event_with(pc, sequence, 12, 10, 0x9000)
}

fn completed_live_load_runtime(response_tick: u64) -> O3RuntimeState {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);
    let data_request = memory_request(20);
    assert!(runtime.stage_live_data_access_issue_for_test(&execution, data_request, 31));
    let mut completed = execution.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed,
            data_request,
            response_tick,
            response_tick - 31,
            Some(&[0x2a, 0, 0, 0]),
        )
        .unwrap());
    runtime
}

fn scalar_load_event_with(
    pc: u64,
    sequence: u64,
    rd: u8,
    rs1: u8,
    address: u64,
) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Load {
        rd: reg(rd),
        rs1: reg(rs1),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    let access = MemoryAccessKind::Load {
        rd: reg(rd),
        address,
        width: MemoryWidth::Word,
        signed: false,
    };
    execution_event(pc, sequence, instruction, access)
}

fn stage_independent_younger(runtime: &mut O3RuntimeState, execution: &RiscvCpuExecutionEvent) {
    runtime.stage_live_data_access_younger_window(
        execution.fetch().request_id(),
        [(
            Address::new(execution.execution().next_pc()),
            RiscvInstruction::Addi {
                rd: reg(13),
                rs1: reg(0),
                imm: Immediate::new(7),
            },
        )],
    );
}

fn independent_younger_event() -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Addi {
        rd: reg(13),
        rs1: reg(0),
        imm: Immediate::new(7),
    };
    RiscvCpuExecutionEvent::new(
        fetch_event(0x8004, 11),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            0x8004,
            0x8008,
            vec![RegisterWrite::new(reg(13), 7)],
            None,
        ),
    )
}

fn scalar_store_event(pc: u64, sequence: u64) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Store {
        rs1: reg(10),
        rs2: reg(11),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
    };
    let access = MemoryAccessKind::Store {
        address: 0x9000,
        width: MemoryWidth::Word,
        value: 0x2a,
    };
    execution_event(pc, sequence, instruction, access)
}

fn store_conditional_event(pc: u64, sequence: u64) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::StoreConditional {
        rd: reg(7),
        rs1: reg(10),
        rs2: reg(11),
        width: MemoryWidth::Word,
        acquire: false,
        release: false,
    };
    let access = MemoryAccessKind::StoreConditional {
        rd: reg(7),
        address: 0x9000,
        width: MemoryWidth::Word,
        value: 0x2a,
        acquire: false,
        release: false,
    };
    execution_event(pc, sequence, instruction, access)
}

fn execution_event(
    pc: u64,
    sequence: u64,
    instruction: RiscvInstruction,
    access: MemoryAccessKind,
) -> RiscvCpuExecutionEvent {
    RiscvCpuExecutionEvent::new(
        fetch_event(pc, sequence),
        instruction,
        RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
    )
}

fn fetch_event(pc: u64, sequence: u64) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            memory_request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        0x0000_0073_u32.to_le_bytes().to_vec(),
    )
}

fn memory_request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn core_with_runtime(runtime: O3RuntimeState) -> RiscvCore {
    let core = RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                MemoryRouteId::new(0),
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    );
    core.state.lock().expect("riscv core lock").o3_runtime = runtime;
    core
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

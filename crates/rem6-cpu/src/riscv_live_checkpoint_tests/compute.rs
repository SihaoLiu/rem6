use rem6_isa_riscv::{
    Immediate, RiscvCounterSnapshot, RiscvFloatStatus, RiscvInstruction, RiscvPmpSnapshot,
    RiscvPrivilegeMode, RiscvStatusWord, RiscvTrap, RiscvTrapKind,
};
use rem6_kernel::PartitionedScheduler;
use rem6_memory::CacheLineLayout;

use super::*;
use crate::o3_runtime::{O3LiveRetireGateCheckpointPayload, O3LiveWritebackReady};
use crate::{
    CpuCore, CpuFetchConfig, CpuId, CpuResetState, O3LoadStoreQueueEntry, O3RuntimeSnapshot,
    RiscvCore, RiscvCoreCheckpointRestoreInput, RiscvLoadReservation, RiscvO3CheckpointProjection,
};

#[test]
#[rustfmt::skip]
fn capture_o3_live_checkpoint_returns_compute_profile() {
    let fixture = ComputeFixture::new();
    fixture.core.state.lock().expect("riscv core lock").issued_data_for_fetches.insert(request(0));
    let projection = fixture.core.capture_checkpoint_projection(100);
    let live = captured_live(&projection);
    assert_eq!((live.captured_tick, live.service.requested_tick), (100, 100));
    assert_eq!(live.profile, RiscvO3LiveCheckpointProfile::ComputeQueue);
    assert_eq!(live.resident_sequences, fixture.sequences);
    assert_eq!(live.issue_rows.iter().map(|row| row.fetch_request).collect::<Vec<_>>(), vec![request(1), request(2)]);
    assert_eq!(live.events.len(), 2);
    assert_eq!(live.executed_fetch_requests, vec![request(1), request(2)]);
    assert!(live.issued_fetch_requests.is_empty());
    assert_eq!(live.wake.tick, 100);
    assert_eq!(live.wake.scheduler_instance_raw, fixture.scheduler_instance_raw);
    assert_eq!(captured_live(&fixture.core.capture_checkpoint_projection(100)), live);
    let mut state = fixture.core.state.lock().expect("riscv core lock");
    state.events.retain(|event| event.fetch().request_id() == request(0));
    state.executed_fetches.retain(|request_id| *request_id == request(0));
    drop(state);
    let mut pending = live.clone();
    pending.executed_fetch_requests.clear();
    assert_eq!(captured_live(&fixture.core.capture_checkpoint_projection(100)), &pending);
}

#[test]
fn compute_capture_accepts_real_fetch_stream_with_retired_history() {
    let fixture = ComputeFixture::new();
    let resident_completions = fixture.core.inner().fetch_events();
    fixture
        .core
        .core
        .state
        .lock()
        .expect("cpu core lock")
        .events = real_fetch_stream(&fixture);

    let projection = fixture.core.capture_checkpoint_projection(100);
    let live = captured_live(&projection);
    assert_eq!(
        live.events
            .iter()
            .map(|event| event.fetch.clone())
            .collect::<Vec<_>>(),
        resident_completions,
    );
}

#[test]
fn compute_capture_preserves_pending_execution_membership() {
    let fixture = ComputeFixture::new();
    {
        let mut state = fixture.core.state.lock().expect("riscv core lock");
        state
            .events
            .retain(|event| event.fetch().request_id() == request(0));
        state
            .executed_fetches
            .retain(|request_id| *request_id == request(0));
    }

    let projection = fixture.core.capture_checkpoint_projection(100);
    let live = captured_live(&projection);
    assert_eq!(live.events.len(), 2);
    assert!(live.executed_fetch_requests.is_empty());

    let destination = core();
    install_projection(&destination, &fixture.core, &projection);
    assert!(destination.execution_events().is_empty());
    let state = destination.state.lock().expect("riscv core lock");
    assert!(!state.executed_fetches.contains(&request(1)));
    assert!(!state.executed_fetches.contains(&request(2)));
}

#[test]
fn compute_capture_rejects_nonresident_unexecuted_fetch_records() {
    let fixture = ComputeFixture::new();
    let unexecuted = compute_event(
        0x8008,
        3,
        i_type(0, 0, 0, 0, 0x13),
        Vec::new(),
        Vec::new(),
        None,
        None,
    )
    .fetch;
    let mut events = real_fetch_stream(&fixture);
    events.extend(fetch_pair(&unexecuted));
    fixture
        .core
        .core
        .state
        .lock()
        .expect("cpu core lock")
        .events = events;
    fixture.core.inner().advance_sequence_past(request(3));

    assert!(matches!(
        fixture
            .core
            .capture_checkpoint_projection(100)
            .live_capture(),
        RiscvO3LiveCheckpointCapture::Rejected
    ));
}

#[test]
fn compute_capture_requires_one_valid_completion_per_resident_fetch() {
    let fixture = ComputeFixture::new();
    let valid = real_fetch_stream(&fixture);

    let mut missing = valid.clone();
    missing.retain(|event| {
        event.request_id() != request(2) || event.kind() != CpuFetchEventKind::Completed
    });
    assert_compute_fetch_stream_rejected(&fixture, missing);

    let mut duplicate = valid.clone();
    let completion = duplicate
        .iter()
        .find(|event| {
            event.request_id() == request(1) && event.kind() == CpuFetchEventKind::Completed
        })
        .unwrap()
        .clone();
    duplicate.push(completion);
    assert_compute_fetch_stream_rejected(&fixture, duplicate);

    let resident = fixture
        .core
        .inner()
        .fetch_events()
        .into_iter()
        .find(|event| event.request_id() == request(1))
        .unwrap();
    for terminal in [
        CpuFetchEvent::completed(fetch_record(&resident), vec![0; 3]),
        CpuFetchEvent::retry(fetch_record(&resident)),
        CpuFetchEvent::failed(fetch_record(&resident)),
    ] {
        let mut invalid = valid.clone();
        invalid.retain(|event| {
            event.request_id() != request(1) || event.kind() == CpuFetchEventKind::Issued
        });
        invalid.push(terminal);
        assert_compute_fetch_stream_rejected(&fixture, invalid);
    }
}

#[test]
#[rustfmt::skip]
fn capture_allows_retired_history_but_rejects_unissued_data_universally() {
    let retired = ComputeFixture::new().core.execution_events().into_iter().next().unwrap(); let core = core(); { let mut state = core.state.lock().expect("riscv core lock"); state.events.push(retired); state.executed_fetches.insert(request(0)); state.issued_data_for_fetches.insert(request(0)); }
    assert!(matches!(core.capture_checkpoint_projection(100).live_capture(), RiscvO3LiveCheckpointCapture::Absent));
    let instruction = RiscvInstruction::Load { rd: reg(3), rs1: reg(1), offset: Immediate::new(0), width: MemoryWidth::Word, signed: true };
    let unissued = RiscvCpuExecutionEvent::new(compute_event(0x8010, 3, i_type(0, 1, 2, 3, 0x03), Vec::new(), Vec::new(), None, None).fetch, instruction, rem6_isa_riscv::RiscvExecutionRecord::new(instruction, 0x8010, 0x8014, Vec::new(), Some(MemoryAccessKind::Load { rd: reg(3), address: 0x9000, width: MemoryWidth::Word, signed: true })));
    { let mut state = core.state.lock().expect("riscv core lock"); state.events.push(unissued.clone()); assert_eq!(state.next_unissued_data_access().unwrap().0, request(3)); }
    assert!(matches!(core.capture_checkpoint_projection(100).live_capture(), RiscvO3LiveCheckpointCapture::Rejected));
    let fixture = ComputeFixture::new(); fixture.core.state.lock().expect("riscv core lock").events.push(unissued); assert!(matches!(fixture.core.capture_checkpoint_projection(100).live_capture(), RiscvO3LiveCheckpointCapture::Rejected));
}

#[test]
#[rustfmt::skip]
fn compute_capture_rejects_live_issued_membership() {
    let fixture = ComputeFixture::new(); fixture.core.state.lock().expect("riscv core lock").issued_data_for_fetches.insert(request(1));
    assert!(matches!(fixture.core.capture_checkpoint_projection(100).live_capture(), RiscvO3LiveCheckpointCapture::Rejected));
}

#[test]
#[rustfmt::skip]
fn checkpoint_capture_holds_cpu_then_riscv_state_for_one_projection() {
    let (_fixture, projection) = fixture_projection();
    assert!(matches!(projection.live_capture(), RiscvO3LiveCheckpointCapture::Captured(_)));
    assert_eq!(projection.replay().hart, _fixture.core.checkpoint_hart_state());
    _fixture.core.write_register(reg(2), 99);
    assert_ne!(projection.replay().hart.read(reg(2)), _fixture.core.read_register(reg(2)));
    let source = include_str!("../riscv_live_checkpoint.rs");
    let capture = source.split("pub fn capture_checkpoint_projection").nth(1).expect("bundled capture API");
    let cpu_lock = capture.find("cpu core lock").expect("CPU lock in capture");
    let riscv_lock = capture.find("riscv core lock").expect("RISC-V lock in capture");
    let guarded = capture.find("capture_checkpoint_projection_from_guards").expect("guard-taking projection helper");
    assert!(cpu_lock < riscv_lock && riscv_lock < guarded);
    assert!(!capture[..guarded].contains("o3_runtime_checkpoint_payload("));
    let guarded = source.split("fn capture_checkpoint_projection_from_guards").nth(1).unwrap();
    assert!(guarded.contains("state: Arc::new(Mutex::new(state.clone()))"));
    assert!(guarded.contains("RiscvCoreCheckpointRestoreInput::new"));
}

#[test]
#[rustfmt::skip]
fn capture_rejects_pending_fetch_and_in_order_pipeline_authority() {
    let drained = core();
    drained.core.state.lock().expect("cpu core lock").events.push(CpuFetchEvent::issued(CpuFetchRecord::new(0, PartitionId::new(0), MemoryRouteId::new(0), TransportEndpointId::new("cpu0.ifetch").unwrap(), request(0), Address::new(0x8000), AccessSize::new(4).unwrap())));
    assert!(matches!(drained.capture_checkpoint_projection(0).live_capture(), RiscvO3LiveCheckpointCapture::Rejected));

    let fetch = ComputeFixture::new();
    fetch.core.core.state.lock().expect("cpu core lock").events.push(CpuFetchEvent::issued(CpuFetchRecord::new(3, PartitionId::new(0), MemoryRouteId::new(0), TransportEndpointId::new("cpu0.ifetch").unwrap(), request(3), Address::new(0x8008), AccessSize::new(4).unwrap())));
    assert!(matches!(fetch.core.capture_checkpoint_projection(100).live_capture(), RiscvO3LiveCheckpointCapture::Rejected));

    let wake = ComputeFixture::new();
    wake.core.core.state.lock().expect("cpu core lock").events.push(compute_event(0x8000, 3, i_type(0, 0, 0, 0, 0x13), Vec::new(), Vec::new(), None, None).fetch);
    wake.core.inner().advance_sequence_past(request(3));
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    schedule_pipeline_wake(&wake.core, &mut scheduler);
    { let state = wake.core.state.lock().expect("riscv core lock"); assert!(state.pending_in_order_pipeline_wake.is_some()); assert!(state.pending_in_order_pipeline_advance.is_some()); }
    assert!(matches!(wake.core.capture_checkpoint_projection(100).live_capture(), RiscvO3LiveCheckpointCapture::Rejected));

    let detached = ComputeFixture::new();
    detached.core.core.state.lock().expect("cpu core lock").events.push(compute_event(0x8000, 3, i_type(0, 0, 0, 0, 0x13), Vec::new(), Vec::new(), None, None).fetch);
    detached.core.inner().advance_sequence_past(request(3));
    schedule_pipeline_wake(&detached.core, &mut scheduler);
    detached.core.restore_in_order_pipeline_snapshot(RiscvCore::default_in_order_pipeline_snapshot()).unwrap();
    assert!(!detached.core.state.lock().expect("riscv core lock").detached_in_order_pipeline_wakes.is_empty());
    assert!(matches!(detached.core.capture_checkpoint_projection(100).live_capture(), RiscvO3LiveCheckpointCapture::Rejected));
}

#[test]
#[rustfmt::skip]
fn compute_capture_normalizes_projected_issue_decision_once() {
    let fixture = ComputeFixture::new();
    {
        let mut state = fixture.core.state.lock().expect("riscv core lock");
        state.o3_runtime.observe_live_issue_decision_for_test(100, &[fixture.sequences[0]], &[], &[fixture.sequences[1]], 1);
        assert!(state.o3_runtime.has_active_live_issue_decision_for_test());
    }
    let first = fixture.core.capture_checkpoint_projection(100);
    let second = fixture.core.capture_checkpoint_projection(100);
    assert_eq!(first.stable(), second.stable());
    assert_eq!(first.stable().stats().issue_cycles(), 1);
    assert_eq!(first.stable().stats().issued_rows(), 1);
    let state = fixture.core.state.lock().expect("riscv core lock");
    assert!(state.o3_runtime.has_active_live_issue_decision_for_test());
    assert_eq!(state.o3_runtime.stats(), first.stable().stats());
}

#[test]
fn compute_capture_normalizes_finalized_writeback_and_retired_data_provenance() {
    let fixture = ComputeFixture::new();
    let source_stats = {
        let mut state = fixture.core.state.lock().expect("riscv core lock");
        state
            .o3_runtime
            .checkpoint_publish_fixed_writeback_for_test(0, 99);
        state
            .o3_runtime
            .checkpoint_mark_data_younger_for_test(fixture.sequences.iter().copied());
        state
            .o3_runtime
            .checkpoint_complete_service_and_request_for_test(99, 100);
        state.o3_runtime.stats()
    };

    let projection = fixture.core.capture_checkpoint_projection(100);
    let live = captured_live(&projection);
    assert_eq!(projection.stable().stats(), source_stats);
    assert!(live.writeback_published_sequences.is_empty());
    assert!(live.writeback_counted_sequences.is_empty());

    let destination = core();
    install_projection(&destination, &fixture.core, &projection);
    assert_eq!(destination.o3_runtime_stats(), source_stats);
}

#[test]
fn compute_capture_rejects_current_writeback_or_unrelated_data_provenance() {
    let current = ComputeFixture::new();
    current
        .core
        .state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .checkpoint_publish_fixed_writeback_for_test(0, 100);
    assert!(matches!(
        current
            .core
            .capture_checkpoint_projection(100)
            .live_capture(),
        RiscvO3LiveCheckpointCapture::Rejected
    ));

    let unrelated = ComputeFixture::new();
    unrelated
        .core
        .state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .checkpoint_mark_data_younger_for_test([999]);
    assert!(matches!(
        unrelated
            .core
            .capture_checkpoint_projection(100)
            .live_capture(),
        RiscvO3LiveCheckpointCapture::Rejected
    ));
}

#[test]
#[rustfmt::skip]
fn compute_prepare_rebuilds_live_rename_queue_and_telemetry() {
    let (fixture, projection) = fixture_projection();
    let live = captured_live(&projection).clone();
    let destination = core();
    install_projection(&destination, &fixture.core, &projection);
    assert_eq!(destination.o3_runtime_snapshot().rename_map(), live.rename_rows);
    assert_eq!(destination.o3_runtime_live_issue_telemetry(), restore_telemetry(live.service.telemetry));
    let state = destination.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.live_issue_resident_sequences_for_checkpoint(), live.resident_sequences);
    assert_eq!(state.o3_runtime.live_issue_service_tick(), Some(live.service.requested_tick));
    assert!(state.o3_runtime.live_issue_queue_materializes_for_checkpoint());
    assert!(!state.o3_writeback_wake.has_scheduled_wake_authority());
    assert!(state.o3_writeback_wake.has_desired_tick());
}

#[test]
#[rustfmt::skip]
fn compute_restore_preserves_service_identity_and_production_dependency_plan() {
    let (fixture, projection) = fixture_projection();
    let live = captured_live(&projection);
    assert_eq!(live.service.last_service_generation, Some((99, live.service.mutation_generation)));
    let destination = core();
    install_projection(&destination, &fixture.core, &projection);
    let mut state = destination.state.lock().expect("riscv core lock");
    let (selected, dependency_blocked) = state.o3_runtime.checkpoint_issue_plan_at_for_test(100).unwrap();
    assert_eq!(selected, vec![fixture.sequences[0]]);
    assert_eq!(dependency_blocked, vec![fixture.sequences[1]]);
    let hart = state.hart.clone();
    let before = state.o3_runtime.stats();
    state.o3_runtime.service_live_issue_queue_at(&hart, 100).unwrap();
    let after = state.o3_runtime.stats();
    assert_eq!(after.issued_rows() - before.issued_rows(), 1);
    assert_eq!(state.o3_runtime.live_issue_resident_sequences_for_checkpoint(), vec![fixture.sequences[1]]);
}

#[test]
#[rustfmt::skip]
fn compute_install_replaces_fetch_frontier_and_next_sequence() {
    let (fixture, projection) = fixture_projection();
    let destination = progressed_destination();
    assert_eq!(destination.inner().next_sequence(), 51);
    install_projection(&destination, &fixture.core, &projection);
    assert_eq!(destination.inner().pc(), Address::new(0x8008));
    assert_eq!(destination.inner().next_sequence(), 3);
    assert_eq!(destination.inner().fetch_events().iter().map(CpuFetchEvent::request_id).collect::<Vec<_>>(), vec![request(1), request(2)]);
    assert_eq!(destination.execution_events().iter().map(|event| event.fetch().request_id()).collect::<Vec<_>>(), vec![request(1), request(2)]);
    assert_eq!(destination.checkpoint_hart_state(), fixture.core.checkpoint_hart_state());
    assert_eq!(destination.load_reservation(), None);
    let state = destination.state.lock().expect("riscv core lock");
    assert!(state.executed_fetches.contains(&request(0)));
    assert!(!state.issued_data_for_fetches.contains(&request(1)));
    drop(state);
    attach_checkpoint_wake(&destination, 100);
    let recaptured = destination.capture_checkpoint_projection(100);
    assert_eq!(captured_live(&recaptured).issued_fetch_requests, Vec::new());
    assert_eq!(captured_live(&recaptured).executed_fetch_requests, vec![request(1), request(2)]);
}

#[test]
#[rustfmt::skip]
fn compute_restore_scrubs_destination_transient_authority() {
    let (fixture, projection) = fixture_projection();
    let destination = progressed_destination();
    assert_eq!(checkpoint_restore_transients(&destination), [true; 13]);
    install_projection(&destination, &fixture.core, &projection);
    assert_eq!(checkpoint_restore_transients(&destination), [false; 13]);
    assert!(destination.state.lock().expect("riscv core lock").o3_writeback_wake.has_desired_tick());
    let quiescent = core();
    let transaction = quiescent.begin_htm_transaction().unwrap();
    quiescent.abort_htm_transaction(transaction.uid(), HtmFailureCause::Explicit).unwrap();
    let history = quiescent.htm_transaction_snapshot();
    assert!(history.active().is_none() && history.last_abort().is_some());
    install_projection(&quiescent, &fixture.core, &projection);
    assert_eq!(quiescent.htm_transaction_snapshot(), history);
    assert_eq!(quiescent.begin_htm_transaction().unwrap().uid().get(), history.next_uid());
}

#[test]
#[rustfmt::skip]
fn compute_capture_rejects_architectural_load_reservation() {
    let fixture = ComputeFixture::new();
    let mut state = fixture.core.state.lock().expect("riscv core lock");
    state.reservation = Some(RiscvLoadReservation::new(Address::new(0x9100), AccessSize::new(8).unwrap()));
    drop(state);
    assert!(matches!(fixture.core.capture_checkpoint_projection(100).live_capture(), RiscvO3LiveCheckpointCapture::Rejected));
}

#[test]
#[rustfmt::skip]
fn compute_restore_recomposes_finalized_and_live_writeback_stats() {
    let (fixture, projection) = fixture_projection();
    let expected = projection.stable().stats();
    let expected_finalized = captured_live(&projection).finalized_writeback.clone();
    assert_eq!(expected.writeback_port_cycles(), 1);
    assert_eq!(expected.writeback_port_admitted_rows(), 1);
    assert_eq!(expected_finalized.cycles, 1);
    assert_eq!(expected_finalized.admitted_rows, 1);
    assert!(expected_finalized.partial_cycle_ticks.is_empty());
    assert!(expected_finalized.partial_ready_rows_by_tick.is_empty());
    assert_eq!(expected_finalized.closed_before_tick, 100);
    let destination = core();
    install_projection(&destination, &fixture.core, &projection);
    assert_eq!(destination.o3_runtime_stats(), expected);
    let state = destination.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.checkpoint_finalized_writeback(), expected_finalized);
    assert!(state.o3_runtime.writeback_reservations().is_empty());
}

#[test]
#[rustfmt::skip]
fn compute_prepare_rejects_cross_reference_without_mutating_destination() {
    let (fixture, projection) = fixture_projection();
    let mut live = captured_live(&projection).clone();
    live.issue_rows[0].sequence = 999;
    let destination = progressed_destination();
    let before = sentinel(&destination);
    let input = checkpoint_input_with_live(&fixture.core, projection.stable().clone(), live);
    assert!(destination.prepare_checkpoint_restore(input).is_err());
    assert_eq!(sentinel(&destination), before);
}

#[test]
#[rustfmt::skip]
fn full_core_prepare_rejects_stable_or_live_error_without_mutation() {
    let (fixture, projection) = fixture_projection();
    let destination = progressed_destination();
    let before = sentinel(&destination);
    let invalid_pmp = RiscvPmpSnapshot::new(Vec::new()).unwrap();
    let stable_invalid = checkpoint_input(&fixture.core, &projection).with_pmp_snapshot(invalid_pmp);
    assert!(destination.prepare_checkpoint_restore(stable_invalid).is_err());
    assert_eq!(sentinel(&destination), before);
    let mut invalid_live = captured_live(&projection).clone();
    invalid_live.rename_rows.clear();
    let live_invalid = checkpoint_input_with_live(&fixture.core, projection.stable().clone(), invalid_live);
    assert!(destination.prepare_checkpoint_restore(live_invalid).is_err());
    assert_eq!(sentinel(&destination), before);
}

macro_rules! corruption_tests {
    ($($name:ident => $corruption:ident),+ $(,)?) => {$(
        #[test]
        fn $name() { assert_compute_corruption(ComputeCorruption::$corruption); }
    )+};
}

corruption_tests! {
    compute_prepare_rejects_duplicate_physical_rename_without_mutation => DuplicatePhysicalRename,
    compute_prepare_rejects_nonempty_stable_lsq => NonemptyStableLsq,
    compute_prepare_rejects_incomplete_live_rob_closure => IncompleteRobClosure,
    compute_prepare_rejects_self_suppressing_service_identity => SelfSuppressingService,
    compute_prepare_rejects_foreign_executed_replay_membership => ForeignExecutedMembership,
    compute_prepare_rejects_stable_live_retire_gate => StableRetireGate,
    compute_prepare_rejects_ready_resident_row => ReadyResident,
    compute_prepare_rejects_future_finalized_writeback_ownership => FutureFinalizedTick,
    compute_prepare_rejects_writeback_watermark_after_capture => FutureClosedBefore,
}

#[rustfmt::skip]
fn fixture_projection() -> (ComputeFixture, RiscvO3CheckpointProjection) { let fixture = ComputeFixture::new(); let projection = fixture.core.capture_checkpoint_projection(100); (fixture, projection) }

#[rustfmt::skip]
fn install_projection(destination: &RiscvCore, source: &RiscvCore, projection: &RiscvO3CheckpointProjection) {
    let prepared = destination.prepare_checkpoint_restore(checkpoint_input(source, projection)).unwrap();
    destination.install_prepared_checkpoint_restore(prepared);
}

struct ComputeFixture {
    core: RiscvCore,
    sequences: Vec<u64>,
    scheduler_instance_raw: u64,
}

impl ComputeFixture {
    #[rustfmt::skip]
    fn new() -> Self {
        let core = core();
        let producer = RiscvInstruction::Addi { rd: reg(3), rs1: reg(0), imm: Immediate::new(13) };
        let dependent = RiscvInstruction::Mul { rd: reg(4), rs1: reg(3), rs2: reg(2) };
        let producer_raw = i_type(13, 0, 0, 3, 0x13);
        let dependent_raw = r_type(1, 2, 3, 0, 4, 0x33);
        let projected_events = vec![
            compute_event(0x8000, 1, producer_raw, vec![RegisterWrite::new(reg(3), 13)], Vec::new(), None, None),
            compute_event(0x8004, 2, dependent_raw, vec![RegisterWrite::new(reg(4), 65)], Vec::new(), None, None),
        ];
        let mut execution_events = vec![compute_event(0x7ffc, 0, i_type(1, 0, 0, 1, 0x13), vec![RegisterWrite::new(reg(1), 1)], Vec::new(), None, None).rebuild().unwrap()];
        execution_events.extend(projected_events.iter().map(RiscvO3LiveCheckpointEvent::rebuild).collect::<Result<Vec<_>, _>>().unwrap());
        let mut state = core.state.lock().expect("riscv core lock");
        assert!(state.o3_runtime.set_issue_width(2));
        state.hart.restore_counter_snapshot(&RiscvCounterSnapshot::with_time(41, 42, 43));
        state.hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
        state.hart.set_status(RiscvStatusWord::new(0x8_000a));
        state.hart.set_float_status(RiscvFloatStatus::new(0xa5));
        state.hart.set_supervisor_scratch(0x1234_5678);
        state.hart.set_machine_trap_value(0x8765_4321);
        state.hart.set_translation_satp(0x8000_0000_0000_0042);
        state.hart.write(reg(2), 5);
        state.o3_runtime.reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(90, 90)]).unwrap();
        state.o3_runtime.finalize_all_writeback_reservations().unwrap();
        state.o3_runtime.checkpoint_reopen_finalized_writeback_for_test(90);
        let first = state.o3_runtime.stage_live_retire_window(Address::new(0x8000), producer, 99, [(Address::new(0x8004), dependent)]).unwrap();
        assert!(state.o3_runtime.bind_live_staged_issue_packet(Address::new(0x8000), RiscvInstruction::decode_with_length(producer_raw).unwrap(), &[request(1)], 100));
        assert!(state.o3_runtime.bind_live_staged_issue_packet(Address::new(0x8004), RiscvInstruction::decode_with_length(dependent_raw).unwrap(), &[request(2)], 100));
        let sequences = state.o3_runtime.live_issue_resident_sequences_for_checkpoint();
        assert_eq!(sequences, vec![first, first + 1]);
        state.o3_runtime.checkpoint_complete_service_and_request_for_test(99, 100);
        state.events = execution_events;
        state.executed_fetches.extend([request(0), request(1), request(2)]);
        state.refresh_o3_writeback_wake(99);
        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        let wake = scheduler.schedule_at(PartitionId::new(0), 100, |_| {}).unwrap();
        let scheduler_instance_raw = scheduler.instance_id().checkpoint_raw();
        state.o3_writeback_wake.mark_scheduled(scheduler.instance_id(), scheduler.pending_event_snapshot(wake).unwrap());
        drop(state);
        core.core.state.lock().expect("cpu core lock").events = projected_events.iter().map(|event| event.fetch.clone()).collect();
        core.inner().set_pc(Address::new(0x8008));
        core.inner().advance_sequence_past(request(2));
        Self { core, sequences, scheduler_instance_raw }
    }
}

fn core() -> RiscvCore {
    core_with_id(CpuId::new(0))
}

#[rustfmt::skip]
fn core_with_id(cpu: CpuId) -> RiscvCore {
    let reset = CpuResetState::new(cpu, PartitionId::new(0), AgentId::new(7), Address::new(0x8000));
    let fetch = CpuFetchConfig::new(TransportEndpointId::new("cpu0.ifetch").unwrap(), MemoryRouteId::new(0), CacheLineLayout::new(16).unwrap(), AccessSize::new(4).unwrap());
    RiscvCore::new(CpuCore::new(reset, fetch).unwrap())
}

#[rustfmt::skip]
fn progressed_destination() -> RiscvCore {
    let core = core_with_id(CpuId::new(9));
    let sentinel = compute_event(0x9000, 50, i_type(1, 0, 0, 8, 0x13), vec![RegisterWrite::new(reg(8), 1)], Vec::new(), None, None);
    let execution = sentinel.rebuild().unwrap();
    let pipeline_fetch = compute_event(0x9004, 49, i_type(0, 0, 0, 0, 0x13), Vec::new(), Vec::new(), None, None).fetch;
    core.inner().set_pc(Address::new(0x9004));
    core.inner().advance_sequence_past(request(50));
    core.core.state.lock().expect("cpu core lock").events = vec![sentinel.fetch, pipeline_fetch];
    let mut state = core.state.lock().expect("riscv core lock");
    state.hart.set_pc(0x9004);
    state.hart.restore_counter_snapshot(&RiscvCounterSnapshot::with_time(91, 92, 93));
    state.hart.set_privilege_mode(RiscvPrivilegeMode::User);
    state.hart.set_status(RiscvStatusWord::new(0x8_0008));
    state.hart.set_float_status(RiscvFloatStatus::new(0x5a));
    state.hart.set_supervisor_scratch(0xdead_beef);
    state.hart.set_machine_trap_value(0xfeed_face);
    state.hart.set_translation_satp(0x8000_0000_0000_0024);
    state.hart.write(reg(9), 0xfeed_face);
    state.reservation = Some(RiscvLoadReservation::new(Address::new(0x9200), AccessSize::new(8).unwrap()));
    state.pending_trap = Some(RiscvTrap::new(RiscvTrapKind::Breakpoint, 0x9000));
    state.pending_trap_event = Some(execution.clone());
    state.events = vec![execution];
    state.executed_fetches.extend([request(0), request(50)]);
    state.issued_data_for_fetches.insert(request(1));
    drop(state);
    core.begin_htm_transaction().unwrap();
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let o3_wake = scheduler.schedule_at(PartitionId::new(0), 77, |_| {}).unwrap();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.pending_callback_error = Some(RiscvCpuError::O3Runtime(O3RuntimeError::WritebackTickOverflow { tick: 7 }));
        state.live_retire_gate.restore_checkpoint(Some(O3LiveRetireGateCheckpointPayload::new(request(50), 75)));
        state.o3_writeback_wake.restore_desired_unscheduled(77);
        state.o3_writeback_wake.mark_scheduled(scheduler.instance_id(), scheduler.pending_event_snapshot(o3_wake).unwrap());
    }
    schedule_pipeline_wake(&core, &mut scheduler);
    assert_eq!(core.checkpoint_owned_in_order_pipeline_wakes().len(), 1);
    core.restore_in_order_pipeline_snapshot(RiscvCore::default_in_order_pipeline_snapshot()).unwrap();
    schedule_pipeline_wake(&core, &mut scheduler);
    assert_eq!(core.checkpoint_owned_in_order_pipeline_wakes().len(), 2);
    let mut state = core.state.lock().expect("riscv core lock");
    state.rebound_in_order_execute_waits.insert(50);
    state.o3_force_normal_execute_fetches.insert(request(50));
    drop(state);
    core
}

#[rustfmt::skip]
fn schedule_pipeline_wake(core: &RiscvCore, scheduler: &mut PartitionedScheduler) {
    let status = core.schedule_next_completed_fetch_pipeline_cycle_serial(scheduler).unwrap();
    assert!(matches!(status, crate::riscv_in_order_drive::RiscvInOrderDriveStatus::Scheduled(_)), "{status:?}");
}

#[rustfmt::skip]
fn checkpoint_restore_transients(core: &RiscvCore) -> [bool; 13] {
    let state = core.state.lock().expect("riscv core lock");
    [
        state.pending_trap.is_some(), state.pending_trap_event.is_some(),
        state.htm.in_transaction(), state.htm_hart_checkpoint.is_some(),
        state.pending_callback_error.is_some(), state.reservation.is_some(),
        state.live_retire_gate.checkpoint().is_some(), state.pending_in_order_pipeline_advance.is_some(),
        state.pending_in_order_pipeline_wake.is_some(), !state.detached_in_order_pipeline_wakes.is_empty(),
        !state.rebound_in_order_execute_waits.is_empty(), !state.o3_force_normal_execute_fetches.is_empty(),
        state.o3_writeback_wake.has_scheduled_wake_authority(),
    ]
}

#[rustfmt::skip]
fn attach_checkpoint_wake(core: &RiscvCore, tick: u64) {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let wake = scheduler.schedule_at(PartitionId::new(0), tick, |_| {}).unwrap();
    core.state.lock().expect("riscv core lock").o3_writeback_wake.mark_scheduled(scheduler.instance_id(), scheduler.pending_event_snapshot(wake).unwrap());
}

#[rustfmt::skip]
fn checkpoint_input(source: &RiscvCore, projection: &RiscvO3CheckpointProjection) -> RiscvCoreCheckpointRestoreInput {
    checkpoint_input_with_live(source, projection.stable().clone(), captured_live(projection).clone())
}

#[rustfmt::skip]
fn checkpoint_input_with_live(source: &RiscvCore, stable: O3RuntimeCheckpointPayload, live: RiscvO3LiveCheckpointPayload) -> RiscvCoreCheckpointRestoreInput {
    RiscvCoreCheckpointRestoreInput::new(
        source.checkpoint_hart_state(), source.pmp_snapshot(), source.hart_run_state(),
        source.in_order_pipeline_snapshot(), source.branch_predictor_checkpoint_payload(),
        source.gshare_branch_predictor_checkpoint_payload(), source.bimode_branch_predictor_checkpoint_payload(),
        source.tournament_branch_predictor_checkpoint_payload(), source.tage_sc_l_branch_predictor_checkpoint_payload(),
        source.multiperspective_perceptron_checkpoint_payload(),
        stable, Some(live),
    )
}

#[rustfmt::skip]
fn rebuilt_stable(
    stable: &O3RuntimeCheckpointPayload, rob: Option<Vec<O3ReorderBufferEntry>>,
    lsq: Option<Vec<O3LoadStoreQueueEntry>>, rename: Option<Vec<O3RenameMapEntry>>,
) -> Result<O3RuntimeCheckpointPayload, O3RuntimeError> {
    let snapshot = stable.snapshot();
    O3RuntimeCheckpointPayload::from_snapshot_with_stats_and_dependency_producers(
        O3RuntimeSnapshot::new(
            rob.unwrap_or_else(|| snapshot.reorder_buffer().to_vec()), lsq.unwrap_or_else(|| snapshot.load_store_queue().to_vec()),
            rename.unwrap_or_else(|| snapshot.rename_map().to_vec()),
            snapshot.pending_state().clone(),
        )?,
        stable.stats(), stable.dependency_producers_with_consumers().clone(),
    )
}

#[derive(Clone, Copy, Debug)]
enum ComputeCorruption {
    DuplicatePhysicalRename,
    NonemptyStableLsq,
    IncompleteRobClosure,
    SelfSuppressingService,
    ForeignExecutedMembership,
    StableRetireGate,
    ReadyResident,
    FutureFinalizedTick,
    FutureClosedBefore,
}

fn corrupt_compute_projection(
    projection: &RiscvO3CheckpointProjection,
    corruption: ComputeCorruption,
) -> (O3RuntimeCheckpointPayload, RiscvO3LiveCheckpointPayload) {
    let mut stable = projection.stable().clone();
    let mut live = captured_live(projection).clone();
    match corruption {
        ComputeCorruption::DuplicatePhysicalRename => {
            let physical = O3PhysicalRegisterId::new(500);
            let duplicates = [10, 11].map(|architectural| {
                O3RenameMapEntry::new(O3RegisterClass::Integer, architectural, physical)
            });
            let mut rename = stable.snapshot().rename_map().to_vec();
            rename.extend(duplicates);
            stable = rebuilt_stable(&stable, None, None, Some(rename)).unwrap();
            live.rename_rows.extend(duplicates);
        }
        ComputeCorruption::NonemptyStableLsq => {
            let lsq = vec![O3LoadStoreQueueEntry::load(
                live.resident_sequences[0],
                Some(Address::new(0x9100)),
                4,
            )];
            stable = rebuilt_stable(&stable, None, Some(lsq), None).unwrap();
        }
        ComputeCorruption::IncompleteRobClosure => {
            live.events.truncate(1);
            live.issue_rows.truncate(1);
            live.resident_sequences.truncate(1);
            live.executed_fetch_requests.truncate(1);
            live.next_fetch_pc = Address::new(0x8004);
            live.service.telemetry.current_occupancy = 1;
        }
        ComputeCorruption::SelfSuppressingService => {
            live.service.mutation_generation = 7;
            live.service.last_service_generation = Some((100, 7));
        }
        ComputeCorruption::ForeignExecutedMembership => {
            live.executed_fetch_requests.push(request(99));
        }
        ComputeCorruption::StableRetireGate => {
            stable = stable.with_live_retire_gate(Some(O3LiveRetireGateCheckpointPayload::new(
                request(90),
                99,
            )));
        }
        ComputeCorruption::ReadyResident => {
            let mut rob = stable.snapshot().reorder_buffer().to_vec();
            rob[0] = rob[0].with_ready(true).with_ready_tick(99);
            stable = rebuilt_stable(&stable, Some(rob), None, None).unwrap();
        }
        ComputeCorruption::FutureFinalizedTick => {
            let finalized = &mut live.finalized_writeback;
            finalized.partial_cycle_ticks.insert(101);
            finalized.partial_ready_rows_by_tick.insert(101, 1);
        }
        ComputeCorruption::FutureClosedBefore => {
            let finalized = &mut live.finalized_writeback;
            finalized.cycles = 1;
            finalized.partial_cycle_ticks.clear();
            finalized.partial_ready_rows_by_tick.clear();
            finalized.partial_deferred_rows_by_tick.clear();
            finalized.closed_before_tick = 101;
        }
    }
    (stable, live)
}

fn assert_compute_corruption(corruption: ComputeCorruption) {
    let fixture = ComputeFixture::new();
    let projection = fixture.core.capture_checkpoint_projection(100);
    let (stable, live) = corrupt_compute_projection(&projection, corruption);
    assert_prepare_rejected_atomically(&fixture.core, stable, live);
}

fn assert_prepare_rejected_atomically(
    source: &RiscvCore,
    stable: O3RuntimeCheckpointPayload,
    live: RiscvO3LiveCheckpointPayload,
) {
    let destination = progressed_destination();
    let before = sentinel(&destination);
    assert!(destination
        .prepare_checkpoint_restore(checkpoint_input_with_live(source, stable, live))
        .is_err());
    assert_eq!(sentinel(&destination), before);
}

fn captured_live(projection: &RiscvO3CheckpointProjection) -> &RiscvO3LiveCheckpointPayload {
    match projection.live_capture() {
        RiscvO3LiveCheckpointCapture::Captured(payload) => payload,
        other => panic!("expected captured compute checkpoint, got {other:?}"),
    }
}

#[rustfmt::skip]
fn compute_event(
    pc: u64, sequence: u64, raw: u32,
    register_writes: Vec<RegisterWrite>, float_register_writes: Vec<FloatRegisterWrite>,
    memory_access: Option<MemoryAccessKind>, data_access_event_kind: Option<RiscvDataAccessEventKind>,
) -> RiscvO3LiveCheckpointEvent {
    let fetch = CpuFetchRecord::new(
        20 + sequence, PartitionId::new(0), MemoryRouteId::new(0),
        TransportEndpointId::new("cpu0.ifetch").unwrap(), request(sequence),
        Address::new(pc), AccessSize::new(4).unwrap(),
    );
    RiscvO3LiveCheckpointEvent {
        fetch: CpuFetchEvent::completed(fetch, raw.to_le_bytes().to_vec()),
        execution_pc: pc, next_pc: pc + 4, instruction_bytes: 4,
        register_writes, float_register_writes, memory_access, data_access_event_kind,
        counts_as_retired_instruction: true,
    }
}

fn real_fetch_stream(fixture: &ComputeFixture) -> Vec<CpuFetchEvent> {
    let retired = fixture
        .core
        .execution_events()
        .into_iter()
        .find(|event| event.fetch().request_id() == request(0))
        .unwrap()
        .fetch()
        .clone();
    std::iter::once(retired)
        .chain(fixture.core.inner().fetch_events())
        .flat_map(|completed| fetch_pair(&completed))
        .collect()
}

fn fetch_pair(completed: &CpuFetchEvent) -> [CpuFetchEvent; 2] {
    [
        CpuFetchEvent::issued(fetch_record(completed)),
        completed.clone(),
    ]
}

fn fetch_record(event: &CpuFetchEvent) -> CpuFetchRecord {
    CpuFetchRecord::new(
        event.tick().saturating_sub(1),
        event.partition(),
        event.route(),
        event.endpoint().clone(),
        event.request_id(),
        event.pc(),
        event.size(),
    )
}

fn assert_compute_fetch_stream_rejected(fixture: &ComputeFixture, events: Vec<CpuFetchEvent>) {
    fixture
        .core
        .core
        .state
        .lock()
        .expect("cpu core lock")
        .events = events;
    assert!(matches!(
        fixture
            .core
            .capture_checkpoint_projection(100)
            .live_capture(),
        RiscvO3LiveCheckpointCapture::Rejected
    ));
}

#[rustfmt::skip]
fn restore_telemetry(value: RiscvO3LiveCheckpointTelemetry) -> crate::O3LiveIssueTelemetry {
    crate::O3LiveIssueTelemetry::from_checkpoint_for_test([
        value.enqueued_rows, value.service_turns, value.wake_requests,
        value.current_occupancy, value.peak_occupancy,
        value.scalar_integer_issued_rows, value.integer_mul_div_issued_rows,
        value.memory_agu_issued_rows, value.control_issued_rows, value.scalar_float_issued_rows,
        value.vector_to_scalar_issued_rows,
    ])
}

#[derive(Debug, Eq, PartialEq)]
struct CoreSentinel {
    cpu: crate::cpu_core::CpuCoreCheckpointState,
    riscv: RiscvCoreState,
}

fn sentinel(core: &RiscvCore) -> CoreSentinel {
    CoreSentinel {
        cpu: core.core.checkpoint_state(),
        riscv: core.state.lock().expect("riscv core lock").clone(),
    }
}

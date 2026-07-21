use super::*;

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use crate::CpuDataConfig;
use rem6_isa_riscv::RiscvHartState;
use rem6_memory::MemoryResponse;
use rem6_transport::{MemoryRoute, MemoryTrace, MemoryTransport, TargetOutcome};

const HEAD_RESPONSE_TICK: u64 = 40;
const HEAD_WRITEBACK_TICK: u64 = 41;
const PRODUCER_VALUE: u64 = 0xa000;

fn staged_lifecycle_fixture() -> (O3RuntimeState, O3RenameMapEntry, O3RuntimeSnapshot) {
    let prior_x6 =
        O3RenameMapEntry::new(O3RegisterClass::Integer, 6, O3PhysicalRegisterId::new(42));
    let checkpoint = O3RuntimeSnapshot::new(
        [],
        [],
        [prior_x6],
        default_o3_runtime_snapshot().pending_state().clone(),
    )
    .unwrap();
    let mut fixture = PendingAddressFixture::new(4, 4);
    fixture.runtime.restore(checkpoint.clone()).unwrap();
    let head = load_event(HEAD_PC, 10, 5, 2, 0x9000);
    assert!(fixture.runtime.stage_live_data_access_issue(
        &head,
        request(20),
        31,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    ));
    assert_eq!(fixture.stage_default(), 3);
    assert!(fixture.runtime.has_pending_data_address());
    assert_ne!(
        integer_mapping(&fixture.runtime, 6),
        Some(prior_x6.physical())
    );
    assert_eq!(
        fixture
            .runtime
            .live_data_access_issue_identity_for_test(request(11)),
        None
    );
    (fixture.runtime, prior_x6, checkpoint)
}

fn core_with_runtime(runtime: O3RuntimeState) -> RiscvCore {
    let core = core_with_completed_fetches(std::iter::empty());
    core.state.lock().expect("riscv core lock").o3_runtime = runtime;
    core
}

fn core_with_interrupt_fetch(runtime: O3RuntimeState) -> RiscvCore {
    core_with_interrupt_fetch_raw(runtime, addi(9, 0, 1))
}

fn core_with_interrupt_fetch_raw(runtime: O3RuntimeState, raw: u32) -> RiscvCore {
    let core = core_with_completed_fetches([(30, HEAD_PC, raw)]);
    core.state.lock().expect("riscv core lock").o3_runtime = runtime;
    core
}

fn assert_no_younger_request(runtime: &O3RuntimeState) {
    assert_eq!(
        runtime.live_data_access_issue_identity_for_test(request(11)),
        None
    );
}

fn assert_pending_cleanup(runtime: &O3RuntimeState, prior_x6: O3RenameMapEntry) {
    assert!(!runtime.has_pending_data_address());
    assert!(runtime
        .snapshot()
        .load_store_queue()
        .iter()
        .all(|entry| entry.address().is_some()));
    assert!(runtime.live_data_access_lifecycle_is_quiescent());
    assert!(!runtime.has_pending_retirement_authority());
    assert!(runtime.pending_data_address_wake_tick().is_none());
    assert_eq!(integer_mapping(runtime, 6), Some(prior_x6.physical()));
    assert_eq!(integer_mapping(runtime, 7), None);
    assert_eq!(integer_mapping(runtime, 8), None);
    assert_no_younger_request(runtime);
}

fn assert_core_pending_cleanup(core: &RiscvCore, prior_x6: O3RenameMapEntry) {
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.outstanding_data.is_empty());
    assert!(state.buffered_o3_effects.is_empty());
    assert_pending_cleanup(&state.o3_runtime, prior_x6);
    drop(state);
    assert_eq!(core.data_access_event_count(), 0);
}

fn callback_lifecycle_fixture() -> (
    RiscvCore,
    O3RenameMapEntry,
    PartitionedScheduler,
    MemoryTransport,
) {
    let scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                PartitionId::new(0),
                TransportEndpointId::new("l1i0").unwrap(),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let data_route = transport
        .add_route(
            MemoryRoute::new(
                TransportEndpointId::new("cpu0.dmem").unwrap(),
                PartitionId::new(0),
                TransportEndpointId::new("l1d0").unwrap(),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = RiscvCore::with_data(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(HEAD_PC),
            ),
            CpuFetchConfig::new(
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                fetch_route,
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
        CpuDataConfig::new(
            TransportEndpointId::new("cpu0.dmem").unwrap(),
            data_route,
            CacheLineLayout::new(16).unwrap(),
        ),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_window_depths(4, 4);
    let prior_x6 =
        O3RenameMapEntry::new(O3RegisterClass::Integer, 6, O3PhysicalRegisterId::new(42));
    let checkpoint = O3RuntimeSnapshot::new(
        [],
        [],
        [prior_x6],
        default_o3_runtime_snapshot().pending_state().clone(),
    )
    .unwrap();
    core.state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .restore(checkpoint)
        .unwrap();
    let mut core_state = core.core.state.lock().expect("cpu core lock");
    for (sequence, pc, raw) in [
        (10, HEAD_PC, ld(5, 2, 0)),
        (11, PENDING_PC, ld(6, 5, 0)),
        (12, FIRST_SUFFIX_PC, addi(7, 5, 8)),
        (13, SECOND_SUFFIX_PC, add(8, 6, 7)),
    ] {
        core_state
            .events
            .push(fetch_event_with_raw(pc, sequence, raw));
    }
    drop(core_state);
    core.write_register(reg(2), 0x9000);
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    core.state
        .lock()
        .expect("riscv core lock")
        .events
        .push(load_event(HEAD_PC, 10, 5, 2, 0x9000));
    (core, prior_x6, scheduler, transport)
}

fn complete_callback_head_outcome(kind: RiscvDataAccessEventKind) {
    let (core, prior_x6, mut scheduler, transport) = callback_lifecycle_fixture();
    let target_calls = Arc::new(AtomicU64::new(0));
    let responder_calls = Arc::clone(&target_calls);
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            responder_calls.fetch_add(1, Ordering::SeqCst);
            if kind == RiscvDataAccessEventKind::Retry {
                TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
            } else {
                TargetOutcome::NoResponse
            }
        },
    )
    .unwrap()
    .expect("head load issues");
    let data_request = {
        let state = core.state.lock().expect("riscv core lock");
        assert!(state.o3_runtime.has_pending_data_address());
        assert_ne!(
            integer_mapping(&state.o3_runtime, 6),
            Some(prior_x6.physical())
        );
        *state
            .outstanding_data
            .keys()
            .next()
            .expect("head request is outstanding")
    };
    scheduler.run_until_idle_conservative();
    if kind == RiscvDataAccessEventKind::Failed {
        core.record_data_failure(data_request, scheduler.now());
    }
    let ready = core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("head outcome is ready for terminal retirement");
    assert_eq!(ready.data_access_event_kind(), Some(kind));

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.outstanding_data.is_empty());
    assert!(state.buffered_o3_effects.is_empty());
    assert!(!state.issued_data_for_fetches.contains(&request(11)));
    assert_pending_cleanup(&state.o3_runtime, prior_x6);
    drop(state);
    assert_eq!(target_calls.load(Ordering::SeqCst), 1);
}

fn stage_future_pending_wake_with_head(runtime: &mut O3RuntimeState) -> RiscvCpuExecutionEvent {
    assert!(runtime.set_issue_width(1));
    for (pc, raw, sequence) in [
        (FIRST_SUFFIX_PC, addi(7, 5, 8), 12),
        (SECOND_SUFFIX_PC, add(8, 6, 7), 13),
    ] {
        assert!(runtime.bind_live_staged_fetch_identity(
            Address::new(pc),
            decoded(raw).instruction(),
            &[request(sequence)],
        ));
    }
    let head_execution = load_event(HEAD_PC, 10, 5, 2, 0x9000);
    let mut completed = head_execution.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed,
            request(20),
            HEAD_RESPONSE_TICK,
            9,
            Some(&PRODUCER_VALUE.to_le_bytes()),
        )
        .unwrap());
    let ready = runtime
        .take_ready_live_data_access_event(HEAD_WRITEBACK_TICK)
        .expect("completed head is ready for retirement");
    let head_sequence = runtime.snapshot().reorder_buffer()[0].sequence();
    let head = O3LiveIssueHeadReservation::for_instruction(
        head_sequence,
        HEAD_WRITEBACK_TICK,
        head_execution.instruction(),
    );
    let requests = [
        (PENDING_PC, ld(6, 5, 0), 11),
        (FIRST_SUFFIX_PC, addi(7, 5, 8), 12),
        (SECOND_SUFFIX_PC, add(8, 6, 7), 13),
    ]
    .into_iter()
    .map(|(pc, raw, sequence)| {
        O3LiveIssueRequest::new(Address::new(pc), vec![request(sequence)], decoded(raw))
    })
    .collect::<Vec<_>>();
    let mut hart = RiscvHartState::new(HEAD_PC);
    hart.write(reg(5), 0xdead_beef);
    runtime
        .schedule_live_speculative_issues(&hart, head, HEAD_WRITEBACK_TICK, &requests)
        .unwrap();
    assert_eq!(
        runtime.pending_data_address_wake_tick(),
        Some(HEAD_WRITEBACK_TICK + 1)
    );
    assert_no_younger_request(runtime);
    ready
}

fn stage_future_pending_wake(runtime: &mut O3RuntimeState) {
    let ready = stage_future_pending_wake_with_head(runtime);
    runtime.record_retired_instruction_with_trace(&ready, true);
    assert!(runtime.has_pending_data_address());
}

fn register_o3_wake(core: &RiscvCore, tick: u64) {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let event = scheduler
        .schedule_at(PartitionId::new(0), tick, |_| {})
        .unwrap();
    core.mark_o3_writeback_wake_scheduled(
        scheduler.instance_id(),
        scheduler.pending_event_snapshot(event).unwrap(),
    );
}

#[test]
fn head_retry_discards_pending_address_and_suffix() {
    complete_callback_head_outcome(RiscvDataAccessEventKind::Retry);
}

#[test]
fn head_failure_discards_pending_address_and_suffix() {
    complete_callback_head_outcome(RiscvDataAccessEventKind::Failed);
}

#[test]
fn redirect_discards_pending_address_and_future_wake() {
    let (mut runtime, prior_x6, _) = staged_lifecycle_fixture();
    stage_future_pending_wake(&mut runtime);
    let core = core_with_runtime(runtime);
    assert_eq!(
        core.requested_o3_writeback_wake_tick(HEAD_WRITEBACK_TICK),
        Some(HEAD_WRITEBACK_TICK + 1)
    );
    register_o3_wake(&core, HEAD_WRITEBACK_TICK + 1);
    assert_eq!(core.owned_o3_writeback_wakes().len(), 1);

    core.redirect_pc(Address::new(0x9000));

    assert_core_pending_cleanup(&core, prior_x6);
    assert!(core.owned_o3_writeback_wakes().is_empty());
}

#[test]
fn interrupt_discards_pending_address_and_suffix() {
    let (invalid_runtime, invalid_prior_x6, _) = staged_lifecycle_fixture();
    let invalid_core = core_with_interrupt_fetch_raw(invalid_runtime, 0xffff_ffff);
    invalid_core.set_detailed_live_retire_gate_enabled(true);
    let interrupt_bit = 1_u64 << 1;
    invalid_core.set_status(rem6_isa_riscv::RiscvStatusWord::new(0).with_mie(true));
    invalid_core.set_machine_interrupt_enable(interrupt_bit);
    invalid_core.set_machine_interrupt_pending(interrupt_bit);
    let mut invalid_scheduler = PartitionedScheduler::new(1).unwrap();

    assert!(invalid_core
        .execute_next_completed_fetch_serial(&mut invalid_scheduler)
        .is_err());
    let invalid_state = invalid_core.state.lock().expect("riscv core lock");
    assert!(invalid_state.o3_runtime.has_pending_data_address());
    assert_ne!(
        integer_mapping(&invalid_state.o3_runtime, 6),
        Some(invalid_prior_x6.physical())
    );
    drop(invalid_state);

    let (runtime, prior_x6, _) = staged_lifecycle_fixture();
    let core = core_with_interrupt_fetch(runtime);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_status(rem6_isa_riscv::RiscvStatusWord::new(0).with_mie(true));
    core.set_machine_interrupt_enable(interrupt_bit);
    core.set_machine_interrupt_pending(interrupt_bit);
    let mut scheduler = PartitionedScheduler::new(1).unwrap();

    let interrupted = core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .expect("enabled interrupt redirects the live window");

    assert!(matches!(
        interrupted.execution().trap().map(|trap| trap.kind()),
        Some(rem6_isa_riscv::RiscvTrapKind::Interrupt { code: 1 })
    ));
    assert_core_pending_cleanup(&core, prior_x6);
}

#[test]
fn restart_discards_pending_address_and_suffix() {
    let (runtime, prior_x6, _) = staged_lifecycle_fixture();
    let core = core_with_runtime(runtime);

    core.resume_nonretentive_supervisor_hart(Address::new(0x9000), 0x1234);

    assert_core_pending_cleanup(&core, prior_x6);
}

#[test]
fn reset_and_restore_clear_pending_address_state() {
    let (mut runtime, prior_x6, _) = staged_lifecycle_fixture();
    stage_future_pending_wake(&mut runtime);
    let reset_core = core_with_runtime(runtime);
    assert_eq!(
        reset_core.requested_o3_writeback_wake_tick(HEAD_WRITEBACK_TICK),
        Some(HEAD_WRITEBACK_TICK + 1)
    );
    register_o3_wake(&reset_core, HEAD_WRITEBACK_TICK + 1);

    reset_core.reset_instruction_fetch_stream(37);

    assert_core_pending_cleanup(&reset_core, prior_x6);
    assert!(reset_core.owned_o3_writeback_wakes().is_empty());
    assert!(reset_core.data_access_lifecycle_is_quiescent());

    let (mut restored, restored_prior_x6, checkpoint) = staged_lifecycle_fixture();
    restored.restore(checkpoint).unwrap();

    assert_pending_cleanup(&restored, restored_prior_x6);
}

#[test]
fn detailed_mode_disable_discards_pending_address_state() {
    let (mut runtime, prior_x6, _) = staged_lifecycle_fixture();
    let ready = stage_future_pending_wake_with_head(&mut runtime);
    let core = core_with_runtime(runtime);
    core.set_detailed_live_retire_gate_enabled(true);
    assert_eq!(
        core.requested_o3_writeback_wake_tick(HEAD_WRITEBACK_TICK),
        Some(HEAD_WRITEBACK_TICK + 1)
    );
    register_o3_wake(&core, HEAD_WRITEBACK_TICK + 1);

    core.set_detailed_live_retire_gate_enabled(false);

    {
        let mut state = core.state.lock().expect("riscv core lock");
        assert!(!state.o3_runtime.has_pending_data_address());
        assert!(state.o3_runtime.has_live_retirement_authority());
        assert_eq!(
            integer_mapping(&state.o3_runtime, 6),
            Some(prior_x6.physical())
        );
        state
            .o3_runtime
            .record_retired_instruction_with_trace(&ready, true);
    }
    assert!(core.owned_o3_writeback_wakes().is_empty());
    assert_core_pending_cleanup(&core, prior_x6);
}

#[test]
fn pending_address_keeps_live_data_handoff_nonquiescent() {
    let (mut runtime, _, _) = staged_lifecycle_fixture();
    stage_future_pending_wake(&mut runtime);
    let core = core_with_runtime(runtime);

    assert!(!core.o3_live_data_access_lifecycle_is_quiescent());
    assert!(!core.data_access_lifecycle_is_quiescent());
    assert_eq!(
        core.capture_o3_live_data_handoff_status(),
        crate::RiscvO3LiveDataHandoffCapture::Rejected
    );
}

#[test]
fn pending_address_rejects_live_checkpoint_capture() {
    let (mut runtime, _, _) = staged_lifecycle_fixture();
    stage_future_pending_wake(&mut runtime);
    let core = core_with_runtime(runtime);

    assert!(core.has_pending_o3_runtime_retirement());
    assert!(!core.data_access_lifecycle_is_quiescent());
    core.finalize_quiescent_o3_writeback_for_checkpoint();
    assert!(core.has_pending_o3_runtime_retirement());
    assert!(!core.data_access_lifecycle_is_quiescent());
}

#[test]
fn drained_pending_address_restores_checkpoint_compatibility() {
    let (mut runtime, prior_x6, _) = staged_lifecycle_fixture();
    stage_future_pending_wake(&mut runtime);
    runtime.discard_pending_data_address();
    assert_pending_cleanup(&runtime, prior_x6);
    let core = core_with_runtime(runtime);

    assert!(!core.has_pending_o3_runtime_retirement());
    assert!(core.data_access_lifecycle_is_quiescent());
    let checkpoint = core.o3_runtime_checkpoint_payload();
    core.restore_o3_runtime_checkpoint_payload(checkpoint)
        .unwrap();
    assert!(core.data_access_lifecycle_is_quiescent());
}

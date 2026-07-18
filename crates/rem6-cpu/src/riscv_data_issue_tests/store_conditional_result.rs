use super::*;
use rem6_isa_riscv::RiscvInstruction;

const SC_ADDRESS: u64 = 0x9008;
const SC_VALUE: u64 = 0x1122_3344_5566_7788;

#[test]
fn detailed_local_sc_failure_waits_for_admitted_writeback() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_conditional_core(fetch_route, data_route, false, 7);
    let delivered_requests = Arc::new(AtomicU64::new(0));
    let delivered_requests_for_target = Arc::clone(&delivered_requests);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |_delivery, _context| {
            delivered_requests_for_target.fetch_add(1, Ordering::SeqCst);
            TargetOutcome::NoResponse
        },
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(delivered_requests.load(Ordering::SeqCst), 0);
    assert_eq!(core.read_register(reg(7)), 9);
    assert_eq!(core.store_conditional_failure_streak(), None);
    let (response_tick, admitted_tick) = pending_sc_result_timing(&core);
    assert_eq!(admitted_tick, response_tick + 1);
    assert!(core
        .record_ready_o3_data_access_event_with_trace(response_tick, true)
        .is_none());
    assert_eq!(core.read_register(reg(7)), 9);

    let failed = core
        .record_ready_o3_data_access_event_with_trace(admitted_tick, true)
        .expect("local SC failure publishes at admitted writeback");

    assert_eq!(
        failed.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::ConditionalFailed)
    );
    assert_eq!(core.read_register(reg(7)), 1);
    assert_eq!(core.load_reservation(), None);
    let streak = core.store_conditional_failure_streak().unwrap();
    assert_eq!(streak.first_failure_tick(), response_tick);
    assert_eq!(streak.last_failure_tick(), response_tick);
    assert_eq!(core.o3_runtime_stats().lsq_store_conditional_failures(), 1);
}

#[test]
fn detailed_target_sc_failure_waits_for_admitted_writeback() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_conditional_core(fetch_route, data_route, true, 7);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::store_conditional_failed(delivery.request()).unwrap(),
            )
        },
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(core.read_register(reg(7)), 9);
    assert_eq!(
        core.load_reservation(),
        Some(RiscvLoadReservation::new(
            Address::new(SC_ADDRESS),
            AccessSize::new(8).unwrap(),
        ))
    );
    assert_eq!(core.store_conditional_failure_streak(), None);
    let (response_tick, admitted_tick) = pending_sc_result_timing(&core);
    assert_eq!(admitted_tick, response_tick + 1);

    let failed = core
        .record_ready_o3_data_access_event_with_trace(admitted_tick, true)
        .expect("target SC failure publishes at admitted writeback");

    assert_eq!(
        failed.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::ConditionalFailed)
    );
    assert_eq!(core.read_register(reg(7)), 1);
    assert_eq!(core.load_reservation(), None);
    let streak = core.store_conditional_failure_streak().unwrap();
    assert_eq!(streak.first_failure_tick(), response_tick);
    assert_eq!(streak.last_failure_tick(), response_tick);
    assert_eq!(core.o3_runtime_stats().lsq_store_conditional_failures(), 1);
}

#[test]
fn sc_failure_retry_or_redirect_never_publishes_stale_status() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_conditional_core(fetch_route, data_route, false, 7);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(core.read_register(reg(7)), 9);
    assert_eq!(core.store_conditional_failure_streak(), None);
    assert_eq!(core.o3_runtime_stats().lsq_store_conditional_failures(), 0);
    assert!(!core
        .state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .writeback_reservations()
        .is_empty());

    core.redirect_pc(Address::new(0xa000));

    assert!(core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .is_none());
    assert_eq!(core.read_register(reg(7)), 9);
    assert_eq!(core.store_conditional_failure_streak(), None);
    assert_eq!(core.o3_runtime_stats().lsq_store_conditional_failures(), 0);
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.o3_runtime.writeback_reservations().is_empty());
    assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
    assert!(state.o3_runtime.snapshot().load_store_queue().is_empty());
    assert_eq!(state.events[0].data_access_event_kind(), None);
    assert!(state.events[0].in_order_pipeline_cycle().is_none());
    assert_eq!(state.events[0].in_order_pipeline_data_wait_cycles(), 0);
}

#[test]
fn zero_destination_sc_failure_redirect_suppresses_progress() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_conditional_core(fetch_route, data_route, false, 0);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(core.store_conditional_failure_streak(), None);
    assert_eq!(core.o3_runtime_stats().lsq_store_conditional_failures(), 0);
    {
        let state = core.state.lock().expect("riscv core lock");
        assert!(state.o3_runtime.writeback_reservations().is_empty());
        assert!(state
            .o3_runtime
            .ready_live_memory_result_completion()
            .is_some());
    }

    core.redirect_pc(Address::new(0xa000));

    assert!(core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .is_none());
    assert_eq!(core.store_conditional_failure_streak(), None);
    assert_eq!(core.o3_runtime_stats().lsq_store_conditional_failures(), 0);
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.o3_runtime.writeback_reservations().is_empty());
    assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
    assert!(state.o3_runtime.snapshot().load_store_queue().is_empty());
    assert_eq!(state.events[0].data_access_event_kind(), None);
}

#[test]
fn control_boundary_preserves_resolved_no_cycle_data_history() {
    let (_scheduler, _transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_conditional_core(fetch_route, data_route, false, 7);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.events[0].set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(state.events[0].in_order_pipeline_cycle().is_none());
        assert!(!state
            .o3_runtime
            .owns_pending_live_data_access_retirement(state.events[0].fetch().request_id(),));
    }

    core.redirect_pc(Address::new(0xa000));

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state.events[0].data_access_event_kind(),
        Some(RiscvDataAccessEventKind::Completed)
    );
}

fn detailed_store_conditional_core(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
    with_reservation: bool,
    rd: u8,
) -> RiscvCore {
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    let event = store_conditional_event(0x8000, 1, rd);
    let mut state = core.state.lock().expect("riscv core lock");
    if rd != 0 {
        state.hart.write(reg(rd), 9);
    }
    if with_reservation {
        state.reservation = Some(RiscvLoadReservation::new(
            Address::new(SC_ADDRESS),
            AccessSize::new(8).unwrap(),
        ));
    }
    state.events.push(event);
    drop(state);
    core
}

fn store_conditional_event(pc: u64, sequence: u64, rd: u8) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::StoreConditional {
        rd: reg(rd),
        rs1: reg(10),
        rs2: reg(11),
        width: MemoryWidth::Doubleword,
        acquire: false,
        release: false,
    };
    let access = MemoryAccessKind::StoreConditional {
        rd: reg(rd),
        address: SC_ADDRESS,
        width: MemoryWidth::Doubleword,
        value: SC_VALUE,
        acquire: false,
        release: false,
    };
    RiscvCpuExecutionEvent::new(
        fetch_event(pc, sequence),
        instruction,
        RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
    )
}

fn pending_sc_result_timing(core: &RiscvCore) -> (Tick, Tick) {
    let response_tick = core
        .data_access_events()
        .into_iter()
        .find(|event| event.kind() == RiscvDataAccessEventKind::ConditionalFailed)
        .expect("SC failure emits its typed data event")
        .tick();
    let state = core.state.lock().expect("riscv core lock");
    let reservations = state.o3_runtime.writeback_reservations();
    assert_eq!(reservations.len(), 1);
    (response_tick, reservations[0].admitted_tick())
}

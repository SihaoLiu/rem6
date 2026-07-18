use super::*;

#[test]
fn fixed_fu_head_can_provision_terminal_store_conditional_status() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = terminal_store_conditional_core(fetch_route, data_route, true);

    let gate_ready_tick = provision_terminal_store_conditional(&core, &mut scheduler);
    let issue_tick = gate_ready_tick - 6;
    while scheduler.now() < issue_tick {
        scheduler.run_next_epoch_until(issue_tick).unwrap();
    }
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(MemoryResponse::completed(delivery.request(), None).unwrap())
        },
    )
    .unwrap()
    .expect("terminal SC issues for a writeback collision");
    scheduler.run_until_idle_conservative();

    let result_admitted_tick = core
        .o3_runtime_writeback_reservations()
        .into_iter()
        .map(|reservation| reservation.admitted_tick())
        .max()
        .expect("DIV and SC own writeback reservations");
    assert_eq!(result_admitted_tick, gate_ready_tick + 1);
    assert_eq!(core.read_register(reg(5)), 9);
    assert_eq!(
        core.load_reservation(),
        Some(RiscvLoadReservation::new(
            Address::new(0x9000),
            AccessSize::new(8).unwrap(),
        ))
    );
    assert_eq!(
        core.execute_next_completed_fetch_serial(&mut scheduler)
            .unwrap()
            .expect("older DIV retires at its gate")
            .fetch_pc(),
        Address::new(0x8000)
    );
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .pending_terminal_memory_result
        .is_some());
    assert_eq!(core.read_register(reg(5)), 9);

    scheduler
        .schedule_at(core.partition(), result_admitted_tick, |_| {})
        .unwrap();
    scheduler.run_until_idle_conservative();
    assert_eq!(
        core.execute_next_completed_fetch_serial(&mut scheduler)
            .unwrap()
            .expect("terminal SC canonicalizes at its own admission")
            .fetch_pc(),
        Address::new(0x8004)
    );
    let completed = core
        .record_ready_o3_data_access_event_with_trace(result_admitted_tick, true)
        .expect("terminal SC publishes at its own admission");

    assert_eq!(
        completed.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::Completed)
    );
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.load_reservation(), None);
}

#[test]
fn failed_terminal_store_conditional_is_squashed_without_status_publication() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = terminal_store_conditional_core(fetch_route, data_route, false);
    let gate_ready_tick = provision_terminal_store_conditional(&core, &mut scheduler);
    let issue_tick = gate_ready_tick - 1;
    while scheduler.now() < issue_tick {
        scheduler.run_next_epoch_until(issue_tick).unwrap();
    }
    let target_calls = Arc::new(AtomicU64::new(0));
    let target_calls_for_callback = Arc::clone(&target_calls);
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |_delivery, _context| {
            target_calls_for_callback.fetch_add(1, Ordering::SeqCst);
            TargetOutcome::NoResponse
        },
    )
    .unwrap()
    .expect("terminal SC schedules a local failure");
    scheduler
        .run_next_epoch_until(gate_ready_tick)
        .expect("local SC failure records before writeback admission");

    assert_eq!(target_calls.load(Ordering::SeqCst), 0);
    assert_eq!(core.read_register(reg(5)), 9);
    assert_eq!(core.store_conditional_failure_streak(), None);
    let sc_sequence = {
        let state = core.state.lock().expect("riscv core lock");
        let pending = state
            .pending_terminal_memory_result
            .as_ref()
            .expect("failed SC remains owned by the terminal-result path");
        assert_eq!(pending.execution().fetch_pc(), Address::new(0x8004));
        state
            .o3_runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .find(|entry| entry.pc() == Address::new(0x8004))
            .map(|entry| entry.sequence())
            .expect("terminal SC owns one ROB row")
    };
    let reservations = core.o3_runtime_writeback_reservations();
    assert_eq!(reservations.len(), 2);
    let sc_reservation = reservations
        .into_iter()
        .find(|reservation| reservation.sequence() == sc_sequence)
        .expect("failed terminal SC owns one deferred status reservation");
    assert_eq!(sc_reservation.raw_ready_tick(), gate_ready_tick);
    assert_eq!(sc_reservation.admitted_tick(), gate_ready_tick + 1);
    assert!(scheduler.now() < sc_reservation.admitted_tick());

    core.redirect_pc(Address::new(0xa000));

    assert!(core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .is_none());
    assert_eq!(core.read_register(reg(5)), 9);
    assert_eq!(core.store_conditional_failure_streak(), None);
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.pending_terminal_memory_result.is_none());
    assert!(state.o3_runtime.live_data_access_lifecycle_is_quiescent());
    assert!(state.o3_runtime.writeback_reservations().is_empty());
    assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
    assert!(state.o3_runtime.snapshot().load_store_queue().is_empty());
}

fn terminal_store_conditional_core(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
    with_reservation: bool,
) -> RiscvCore {
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.set_o3_writeback_width(1);
    core.write_register(reg(1), 84);
    core.write_register(reg(2), 6);
    core.write_register(reg(5), 9);
    core.write_register(reg(10), 0x9000);
    core.write_register(reg(11), 0x1122_3344_5566_7788);
    let div = r_type(1, 2, 1, 0x4, 3, 0x33);
    let sc = atomic_type(0x03, 11, 10, 0x3, 5);
    if with_reservation {
        core.state.lock().expect("riscv core lock").reservation = Some(RiscvLoadReservation::new(
            Address::new(0x9000),
            AccessSize::new(8).unwrap(),
        ));
    }
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(1, 0x8000, div),
            completed_fetch_with_raw(2, 0x8004, sc),
        ]);
    core
}

fn provision_terminal_store_conditional(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
) -> Tick {
    assert!(core
        .execute_next_completed_fetch_serial(scheduler)
        .unwrap()
        .is_none());
    core.checkpoint_owned_live_retire_gate_wakes()
        .first()
        .map(|(_, event)| event.tick())
        .expect("older DIV head owns a live-retire gate wake")
}

#[test]
fn unscheduled_execute_cannot_bypass_terminal_result_writeback_admission() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.set_o3_writeback_width(1);
    core.write_register(reg(1), 84);
    core.write_register(reg(2), 6);
    core.write_register(reg(10), 0x9000);
    let div = r_type(1, 2, 1, 0x4, 3, 0x33);
    let load = i_type(0, 10, 0b011, 5, 0x03);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(1, 0x8000, div),
            completed_fetch_with_raw(2, 0x8004, load),
        ]);

    assert!(core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .is_none());
    let gate_ready_tick = core
        .checkpoint_owned_live_retire_gate_wakes()
        .first()
        .map(|(_, event)| event.tick())
        .expect("older DIV head owns a live-retire gate wake");
    let issue_tick = gate_ready_tick - 6;
    while scheduler.now() < issue_tick {
        scheduler.run_next_epoch_until(issue_tick).unwrap();
    }
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(delivery.request(), Some(42_u64.to_le_bytes().to_vec()))
                    .unwrap(),
            )
        },
    )
    .unwrap()
    .expect("younger load issues for a writeback collision");
    scheduler.run_until_idle_conservative();
    assert_eq!(scheduler.now(), gate_ready_tick);
    let result_admitted_tick = core
        .o3_runtime_writeback_reservations()
        .into_iter()
        .map(|reservation| reservation.admitted_tick())
        .max()
        .expect("DIV and load own writeback reservations");
    assert_eq!(result_admitted_tick, gate_ready_tick + 1);
    assert_eq!(
        core.execute_next_completed_fetch_serial(&mut scheduler)
            .unwrap()
            .expect("older DIV retires at its gate")
            .fetch_pc(),
        Address::new(0x8000)
    );

    assert!(core.execute_next_completed_fetch().unwrap().is_none());
    assert_eq!(core.read_register(reg(5)), 0);
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .pending_terminal_memory_result
        .is_some());

    scheduler
        .schedule_at(core.partition(), result_admitted_tick, |_| {})
        .unwrap();
    scheduler.run_until_idle_conservative();
    assert_eq!(
        core.execute_next_completed_fetch_serial(&mut scheduler)
            .unwrap()
            .expect("scheduler authority admits the terminal result")
            .fetch_pc(),
        Address::new(0x8004)
    );
}

#[test]
fn dropped_prepared_terminal_result_clears_pending_and_runtime_owners() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(1), 84);
    core.write_register(reg(2), 6);
    core.write_register(reg(10), 0x9000);
    let div = r_type(1, 2, 1, 0x4, 3, 0x33);
    let load = i_type(0, 10, 0b011, 5, 0x03);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(1, 0x8000, div),
            completed_fetch_with_raw(2, 0x8004, load),
        ]);

    assert!(core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .is_none());
    scheduler.run_next_epoch();
    let prepared = core
        .prepare_data_parallel_access(
            scheduler.now(),
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("ready terminal result prepares a parallel data access");
    drop(prepared);

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.pending_terminal_memory_result.is_none());
    assert!(state.o3_runtime.live_data_access_lifecycle_is_quiescent());
}

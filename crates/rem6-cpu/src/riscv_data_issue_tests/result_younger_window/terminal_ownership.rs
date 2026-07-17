use super::*;

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

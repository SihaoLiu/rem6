use super::*;

#[test]
fn completed_scalar_load_blocks_younger_retirement_until_o3_event_is_consumed() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.write_register(reg(2), 0x9000);
    let load = i_type(0, 2, 0b010, 5, 0x03);
    core.issue_next_fetch(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(delivery.request(), Some(load.to_le_bytes().to_vec()))
                    .unwrap(),
            )
        },
    )
    .unwrap();
    scheduler.run_until_idle_conservative();
    let executed = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(executed.fetch_pc(), Address::new(0x8000));

    let independent = i_type(7, 0, 0b000, 6, 0x13);
    core.issue_next_fetch(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(
                    delivery.request(),
                    Some(independent.to_le_bytes().to_vec()),
                )
                .unwrap(),
            )
        },
    )
    .unwrap();
    scheduler.run_until_idle_conservative();

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(delivery.request(), Some(vec![0x2a, 0, 0, 0])).unwrap(),
            )
        },
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert!(core
        .drive_next_action(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
            |_delivery, _context| panic!("ready scalar-memory event must block another issue"),
        )
        .unwrap()
        .is_none());
    let wake_tick = core
        .requested_o3_writeback_wake_tick(scheduler.now())
        .expect("completed load requests an O3 issue wake");
    let wake_core = core.clone();
    let wake_event = scheduler
        .schedule_at(core.partition(), wake_tick, move |context| {
            wake_core.mark_o3_writeback_wake_fired(context.now());
        })
        .unwrap();
    core.mark_o3_writeback_wake_scheduled(
        scheduler.instance_id(),
        scheduler.pending_event_snapshot(wake_event).unwrap(),
    );
    scheduler.run_until_idle_conservative();
    assert!(core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .is_none());
    assert_eq!(core.o3_runtime_snapshot().reorder_buffer().len(), 2);
    assert!(core
        .record_ready_o3_data_access_event_with_trace(wake_tick, true)
        .is_some());

    let younger = core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .unwrap();
    assert_eq!(younger.fetch_pc(), Address::new(0x8004));
    core.record_o3_retired_instruction_with_trace(&younger, true);
    assert!(core.o3_runtime_snapshot().reorder_buffer().is_empty());
    assert!(core.o3_live_data_access_lifecycle_is_quiescent());
}

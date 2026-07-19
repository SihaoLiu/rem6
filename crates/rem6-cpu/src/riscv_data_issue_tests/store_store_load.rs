use super::*;
use crate::{O3LoadStoreQueueKind, RiscvCoreDriveAction};

#[test]
fn detailed_store_store_load_forwards_from_the_youngest_store_without_transport() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_store_load_core(fetch_route, data_route);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| TargetOutcome::RespondAfter {
            delay: 20,
            response: MemoryResponse::completed(delivery.request(), None).unwrap(),
        },
    )
    .unwrap()
    .expect("leading store should issue through transport");
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| panic!("younger store must remain buffered"),
    )
    .unwrap()
    .expect("younger store should occupy the detailed O3 window");
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| panic!("youngest-store forwarded load must not reach transport"),
    )
    .unwrap()
    .expect("forwarded load should schedule a local completion");

    {
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.outstanding_data.len(), 3);
        assert_eq!(
            state
                .o3_runtime
                .snapshot()
                .load_store_queue()
                .iter()
                .map(|entry| entry.kind())
                .collect::<Vec<_>>(),
            vec![
                O3LoadStoreQueueKind::Store,
                O3LoadStoreQueueKind::Store,
                O3LoadStoreQueueKind::Load,
            ]
        );
    }

    scheduler.run_next_epoch();
    {
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.outstanding_data.len(), 2);
        let snapshot = state.o3_runtime.snapshot();
        let lsq = snapshot.load_store_queue();
        assert!(!lsq[0].is_completed());
        assert!(!lsq[1].is_completed());
        assert!(lsq[2].is_completed());
    }
    assert_eq!(core.read_register(reg(6)), 0);
    assert!(core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .is_none());

    scheduler.run_until_idle_conservative();
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(MemoryResponse::completed(delivery.request(), None).unwrap())
        },
    )
    .unwrap()
    .expect("younger store should drain after the leading response");
    scheduler.run_until_idle_conservative();
    for expected_pc in [0x8000, 0x8004] {
        let store = core
            .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
            .expect("stores should retire oldest-first");
        assert_eq!(store.fetch_pc(), Address::new(expected_pc));
        assert_eq!(core.read_register(reg(6)), 0);
    }
    let load = core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("forwarded load should retire after both stores");
    assert_eq!(load.fetch_pc(), Address::new(0x8008));
    assert_eq!(core.read_register(reg(6)), 0x63);
}

#[test]
fn leading_store_retry_cancels_the_younger_store_before_transport_side_effects() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_store_load_core(fetch_route, data_route);
    let visible_store = Arc::new(AtomicU64::new(0));

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| TargetOutcome::RespondAfter {
            delay: 20,
            response: MemoryResponse::retry(delivery.request()),
        },
    )
    .unwrap()
    .expect("leading store should issue through transport");
    core.issue_next_data_access(&mut scheduler, &transport, MemoryTrace::new(), {
        let visible_store = Arc::clone(&visible_store);
        move |delivery, _context| {
            visible_store.store(0x63, Ordering::SeqCst);
            TargetOutcome::Respond(MemoryResponse::completed(delivery.request(), None).unwrap())
        }
    })
    .unwrap()
    .expect("younger store should occupy the detailed O3 window");
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| panic!("forwarded load must not reach transport"),
    )
    .unwrap()
    .expect("forwarded load should schedule a local completion");

    scheduler.run_until_idle_conservative();

    assert_eq!(visible_store.load(Ordering::SeqCst), 0);
    let retry = core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("leading retry should retire before replay");
    assert_eq!(
        retry.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::Retry)
    );
    assert!(core.has_unissued_data_access());

    core.issue_next_data_access(&mut scheduler, &transport, MemoryTrace::new(), {
        let visible_store = Arc::clone(&visible_store);
        move |delivery, _context| {
            visible_store.store(0x63, Ordering::SeqCst);
            TargetOutcome::Respond(MemoryResponse::completed(delivery.request(), None).unwrap())
        }
    })
    .unwrap()
    .expect("cancelled younger store should replay");
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| panic!("replayed load should forward from the younger store"),
    )
    .unwrap()
    .expect("cancelled forwarded load should replay");
    scheduler.run_until_idle_conservative();

    assert_eq!(visible_store.load(Ordering::SeqCst), 0x63);
    core.record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("replayed younger store should retire");
    core.record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("replayed forwarded load should retire");
    assert_eq!(core.read_register(reg(6)), 0x63);
}

#[test]
fn middle_store_retry_cancels_the_forwarded_load_after_buffer_drain() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_store_load_core(fetch_route, data_route);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| TargetOutcome::RespondAfter {
            delay: 20,
            response: MemoryResponse::completed(delivery.request(), None).unwrap(),
        },
    )
    .unwrap()
    .expect("leading store should issue through transport");
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| panic!("younger store must remain buffered"),
    )
    .unwrap()
    .expect("younger store should occupy the detailed O3 window");
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| panic!("forwarded load must not reach transport"),
    )
    .unwrap()
    .expect("forwarded load should schedule a local completion");
    scheduler.run_until_idle_conservative();

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| TargetOutcome::Respond(MemoryResponse::retry(delivery.request())),
    )
    .unwrap()
    .expect("younger store should drain after its predecessor completes");
    scheduler.run_until_idle_conservative();

    let first = core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("leading store should retire before the retry");
    assert_eq!(first.fetch_pc(), Address::new(0x8000));
    let retry = core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("middle retry should retire before load replay");
    assert_eq!(retry.fetch_pc(), Address::new(0x8004));
    assert_eq!(
        retry.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::Retry)
    );
    assert!(core.has_unissued_data_access());

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(delivery.request(), Some(vec![0x63, 0, 0, 0])).unwrap(),
            )
        },
    )
    .unwrap()
    .expect("cancelled load should replay through transport");
    scheduler.run_until_idle_conservative();
    core.record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("replayed load should retire");
    assert_eq!(core.read_register(reg(6)), 0x63);
}

#[test]
fn cluster_batch_buffers_and_drains_the_younger_store_in_two_phases() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_store_load_core(fetch_route, data_route);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();

    let actions = cluster
        .drive_ready_cores_parallel(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            |_cpu| {
                |delivery, _context| TargetOutcome::RespondAfter {
                    delay: 20,
                    response: MemoryResponse::completed(delivery.request(), None).unwrap(),
                }
            },
        )
        .unwrap();
    assert_eq!(actions.len(), 1);

    let actions = cluster
        .drive_ready_cores_parallel(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            |_cpu| |_delivery, _context| panic!("younger store must remain buffered"),
        )
        .unwrap();
    assert_eq!(actions.len(), 1);

    let actions = cluster
        .drive_ready_cores_parallel(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            |_cpu| |_delivery, _context| panic!("forwarded load must not reach transport"),
        )
        .unwrap();
    assert_eq!(actions.len(), 1);
    scheduler.run_until_idle_parallel().unwrap();

    let actions = cluster
        .drive_ready_cores_parallel(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            |_cpu| {
                |delivery, _context| {
                    TargetOutcome::Respond(
                        MemoryResponse::completed(delivery.request(), None).unwrap(),
                    )
                }
            },
        )
        .unwrap();
    assert_eq!(actions.len(), 1);
    scheduler.run_until_idle_parallel().unwrap();

    for expected_pc in [0x8000, 0x8004, 0x8008] {
        let retired = core
            .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
            .expect("cluster-batch store-store-load rows should retire in order");
        assert_eq!(retired.fetch_pc(), Address::new(expected_pc));
    }
    assert_eq!(core.read_register(reg(6)), 0x63);
}

#[test]
fn ready_buffered_store_drains_before_a_younger_mmio_access() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_store_load_core(fetch_route, data_route);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.hart.write(reg(3), 0xa000);
        state
            .events
            .push(scalar_load_event_with_base(0x800c, 4, 7, 3, 0xa000));
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
    .expect("leading store should issue through transport");
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| panic!("younger store must remain buffered"),
    )
    .unwrap()
    .expect("younger store should occupy the detailed O3 window");
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| panic!("forwarded load must not reach transport"),
    )
    .unwrap()
    .expect("forwarded load should schedule a local completion");
    scheduler.run_until_idle_conservative();

    let bus = test_mmio_bus(0xa000, vec![0x7b, 0, 0, 0]);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let actions = cluster
        .drive_ready_cores_parallel_with_mmio(
            &mut scheduler,
            &transport,
            &bus,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            |_cpu| {
                |delivery, _context| {
                    TargetOutcome::Respond(
                        MemoryResponse::completed(delivery.request(), None).unwrap(),
                    )
                }
            },
        )
        .unwrap();

    assert_eq!(actions.len(), 1);
    assert!(matches!(
        actions[0].action(),
        RiscvCoreDriveAction::DataAccessIssued { .. }
    ));
    scheduler.run_until_idle_parallel().unwrap();
    for expected_pc in [0x8000, 0x8004, 0x8008] {
        let retired = core
            .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
            .expect("store-store-load rows should retire after the buffer drains");
        assert_eq!(retired.fetch_pc(), Address::new(expected_pc));
    }

    let actions = cluster
        .drive_ready_cores_parallel_with_mmio(
            &mut scheduler,
            &transport,
            &bus,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            |_cpu| |_delivery, _context| panic!("MMIO load must not use memory transport"),
        )
        .unwrap();
    assert_eq!(actions.len(), 1);
    assert!(matches!(
        actions[0].action(),
        RiscvCoreDriveAction::DataAccessIssued { .. }
    ));
}

#[test]
fn failed_leading_store_cancels_the_younger_store_and_forwarded_load() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_store_load_core(fetch_route, data_route);
    issue_data_accesses_without_response(&core, &mut scheduler, &transport, 3);
    let requests = outstanding_data_requests_in_fetch_order(&core);

    core.record_data_failure(requests[0].1, scheduler.now());

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.outstanding_data.is_empty());
    assert_eq!(
        state.o3_runtime.pending_live_data_access_retirement_count(),
        1
    );
    assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
    assert!(state.o3_runtime.snapshot().load_store_queue().is_empty());
    assert!(!state.issued_data_for_fetches.contains(&requests[1].0));
    assert!(!state.issued_data_for_fetches.contains(&requests[2].0));
}

#[test]
fn failed_middle_store_cancels_only_the_forwarded_load_suffix() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_store_load_core(fetch_route, data_route);
    issue_data_accesses_without_response(&core, &mut scheduler, &transport, 3);
    let requests = outstanding_data_requests_in_fetch_order(&core);

    core.record_data_failure(requests[1].1, scheduler.now());

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 1);
    assert!(state.outstanding_data.contains_key(&requests[0].1));
    assert_eq!(
        state.o3_runtime.pending_live_data_access_retirement_count(),
        2
    );
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 1);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 1);
    assert!(state.issued_data_for_fetches.contains(&requests[1].0));
    assert!(!state.issued_data_for_fetches.contains(&requests[2].0));
}

#[test]
fn failed_middle_partial_store_cancels_the_younger_composed_suffix() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_multi_store_byte_composition_core(fetch_route, data_route);
    issue_data_accesses_without_response(&core, &mut scheduler, &transport, 4);
    let requests = outstanding_data_requests_in_fetch_order(&core);
    assert_eq!(requests.len(), 4);

    core.record_data_failure(requests[1].1, scheduler.now());

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 1);
    assert!(state.outstanding_data.contains_key(&requests[0].1));
    assert_eq!(
        state.o3_runtime.pending_live_data_access_retirement_count(),
        2
    );
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 1);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 1);
    assert!(state.issued_data_for_fetches.contains(&requests[1].0));
    assert!(!state.issued_data_for_fetches.contains(&requests[2].0));
    assert!(!state.issued_data_for_fetches.contains(&requests[3].0));
}

#[test]
fn detailed_disjoint_store_prefix_stages_ordered_rows_and_ignores_disjoint_bytes() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_disjoint_store_prefix_core(fetch_route, data_route);

    issue_data_accesses_without_response(&core, &mut scheduler, &transport, 4);

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 4);
    assert_eq!(state.buffered_o3_effects.len(), 2);
    assert_eq!(
        state
            .o3_runtime
            .snapshot()
            .load_store_queue()
            .iter()
            .map(|entry| (entry.kind(), entry.address()))
            .collect::<Vec<_>>(),
        vec![
            (O3LoadStoreQueueKind::Store, Some(Address::new(0x9000))),
            (O3LoadStoreQueueKind::Store, Some(Address::new(0x9040))),
            (O3LoadStoreQueueKind::Store, Some(Address::new(0x9002))),
            (O3LoadStoreQueueKind::Load, Some(Address::new(0x9000))),
        ]
    );
    let mut accesses = state.outstanding_data.values().collect::<Vec<_>>();
    accesses.sort_unstable_by_key(|access| access.fetch_request.sequence());
    let plan = accesses[3]
        .store_load_forwarding_plan
        .expect("the load should retain an overlapping-store composition plan");
    assert_eq!(plan.forwarded_bytes(), 4);
    assert!(plan.is_partial());
}

fn detailed_store_store_load_core(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
) -> RiscvCore {
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(3);
    let mut state = core.state.lock().expect("riscv core lock");
    state.hart.write(reg(2), 0x9000);
    state.events.extend([
        scalar_store_event_with_width_and_value(0x8000, 1, 0x9000, MemoryWidth::Word, 0x2a),
        scalar_store_event_with_width_and_value(0x8004, 2, 0x9000, MemoryWidth::Word, 0x63),
        scalar_load_event_with_base(0x8008, 3, 6, 2, 0x9000),
    ]);
    drop(state);
    core
}

fn detailed_multi_store_byte_composition_core(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
) -> RiscvCore {
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    let mut state = core.state.lock().expect("riscv core lock");
    state.hart.write(reg(2), 0x9000);
    state.events.extend([
        scalar_store_event_with_width_and_value(0x8000, 1, 0x9000, MemoryWidth::Word, 0xaa),
        scalar_store_event_with_width_and_value(0x8004, 2, 0x9002, MemoryWidth::Halfword, 0xccbb),
        scalar_store_event_with_width_and_value(0x8008, 3, 0x9002, MemoryWidth::Byte, 0xdd),
        scalar_load_event_with_base_width(0x800c, 4, 6, 2, 0x9000, MemoryWidth::Doubleword, false),
    ]);
    drop(state);
    core
}

fn detailed_disjoint_store_prefix_core(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
) -> RiscvCore {
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    let mut state = core.state.lock().expect("riscv core lock");
    state.hart.write(reg(2), 0x9000);
    state.events.extend([
        scalar_store_event_with_width_and_value(0x8000, 1, 0x9000, MemoryWidth::Word, 0xaa),
        scalar_store_event_with_width_and_value(0x8004, 2, 0x9040, MemoryWidth::Word, 0x5a),
        scalar_store_event_with_width_and_value(0x8008, 3, 0x9002, MemoryWidth::Halfword, 0x06bb),
        scalar_load_event_with_base_width(0x800c, 4, 6, 2, 0x9000, MemoryWidth::Doubleword, false),
    ]);
    drop(state);
    core
}

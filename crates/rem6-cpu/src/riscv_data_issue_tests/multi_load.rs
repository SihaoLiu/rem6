use super::*;

#[test]
fn three_independent_detailed_scalar_loads_issue_before_first_response() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_three_load_core(fetch_route, data_route, [0x9000, 0x9040, 0x9080]);

    issue_three_without_response(&core, &mut scheduler, &transport);

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 3);
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 3);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 3);
}

#[test]
fn three_load_responses_write_back_in_program_order() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_three_load_core(fetch_route, data_route, [0x9000, 0x9040, 0x9080]);

    for (delay, value) in [(20, 0x2a), (10, 0x63), (0, 0x77)] {
        core.issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            move |delivery, _context| TargetOutcome::RespondAfter {
                delay,
                response: MemoryResponse::completed(delivery.request(), Some(vec![value, 0, 0, 0]))
                    .unwrap(),
            },
        )
        .unwrap()
        .expect("independent scalar load should issue");
    }
    scheduler.run_until_idle_conservative();

    assert_eq!(
        [
            core.read_register(reg(5)),
            core.read_register(reg(6)),
            core.read_register(reg(7))
        ],
        [0, 0, 0]
    );
    for (pc, register, value) in [(0x8000, 5, 0x2a), (0x8004, 6, 0x63), (0x8008, 7, 0x77)] {
        let retired = core
            .record_ready_o3_scalar_memory_event_with_trace(true)
            .expect("completed scalar load should retire in program order");
        assert_eq!(retired.fetch_pc(), Address::new(pc));
        assert_eq!(core.read_register(reg(register)), value);
    }
}

#[test]
fn oldest_load_failure_cancels_two_younger_requests() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_three_load_core(fetch_route, data_route, [0x9000, 0x9040, 0x9080]);
    issue_three_without_response(&core, &mut scheduler, &transport);
    let requests = outstanding_requests_in_fetch_order(&core);

    core.record_data_failure(requests[0].1, scheduler.now());

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.outstanding_data.is_empty());
    assert_eq!(state.o3_runtime.pending_scalar_memory_retirement_count(), 1);
    assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
    assert!(state.o3_runtime.snapshot().load_store_queue().is_empty());
    assert!(!state.issued_data_for_fetches.contains(&requests[1].0));
    assert!(!state.issued_data_for_fetches.contains(&requests[2].0));
}

#[test]
fn middle_load_failure_preserves_older_request_and_cancels_only_third() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_three_load_core(fetch_route, data_route, [0x9000, 0x9040, 0x9080]);
    issue_three_without_response(&core, &mut scheduler, &transport);
    let requests = outstanding_requests_in_fetch_order(&core);

    core.record_data_failure(requests[1].1, scheduler.now());

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 1);
    assert!(state.outstanding_data.contains_key(&requests[0].1));
    assert_eq!(state.o3_runtime.pending_scalar_memory_retirement_count(), 2);
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 1);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 1);
    assert!(state.issued_data_for_fetches.contains(&requests[1].0));
    assert!(!state.issued_data_for_fetches.contains(&requests[2].0));
}

#[test]
fn htm_abort_cancels_three_outstanding_scalar_loads() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_three_load_core(fetch_route, data_route, [0x9000, 0x9040, 0x9080]);
    let begin = core.begin_htm_transaction().unwrap();
    issue_three_without_response(&core, &mut scheduler, &transport);

    core.abort_htm_transaction(begin.uid(), crate::HtmFailureCause::Explicit)
        .unwrap();
    scheduler.run_until_idle_conservative();

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.outstanding_data.is_empty());
    assert_eq!(state.o3_runtime.pending_scalar_memory_retirement_count(), 0);
    assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
    assert!(state.o3_runtime.snapshot().load_store_queue().is_empty());
}

#[test]
fn uncacheable_third_load_stays_serialized_behind_two_resident_loads() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_three_load_core(fetch_route, data_route, [0x9000, 0x9040, 0x9080]);
    core.add_pma_uncacheable_range(RiscvPmaRange::new(0x9080, 0x9084).unwrap())
        .unwrap();

    for _ in 0..2 {
        core.issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("cacheable scalar load should issue");
    }
    assert!(core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| panic!("uncacheable third load must not reach transport"),
        )
        .unwrap()
        .is_none());

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 2);
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 2);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 2);
}

fn issue_three_without_response(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) {
    for _ in 0..3 {
        core.issue_next_data_access(
            scheduler,
            transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("independent scalar load should issue");
    }
}

fn outstanding_requests_in_fetch_order(
    core: &RiscvCore,
) -> Vec<(MemoryRequestId, MemoryRequestId)> {
    let state = core.state.lock().expect("riscv core lock");
    let mut requests = state
        .outstanding_data
        .values()
        .map(|access| (access.fetch_request, access.request))
        .collect::<Vec<_>>();
    requests.sort_unstable_by_key(|(fetch, _)| fetch.sequence());
    requests
}

fn detailed_three_load_core(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
    addresses: [u64; 3],
) -> RiscvCore {
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    let mut state = core.state.lock().expect("riscv core lock");
    for (register, address) in [(2, addresses[0]), (3, addresses[1]), (4, addresses[2])] {
        state.hart.write(reg(register), address);
    }
    state.events.extend([
        scalar_load_event_with_base(0x8000, 1, 5, 2, addresses[0]),
        scalar_load_event_with_base(0x8004, 2, 6, 3, addresses[1]),
        scalar_load_event_with_base(0x8008, 3, 7, 4, addresses[2]),
    ]);
    drop(state);
    core
}

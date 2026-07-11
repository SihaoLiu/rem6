use super::*;

#[test]
fn detailed_store_then_aliasing_load_completes_without_transport() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_load_core(fetch_route, data_route, 0x9000, 0x9000);

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
    .unwrap();

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| panic!("forwarded load must not reach transport"),
    )
    .unwrap()
    .expect("forwarded load should schedule a local completion");

    assert_eq!(
        core.state
            .lock()
            .expect("riscv core lock")
            .outstanding_data
            .len(),
        2
    );
    scheduler.run_next_epoch();

    {
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.outstanding_data.len(), 1);
        let snapshot = state.o3_runtime.snapshot();
        let lsq = snapshot.load_store_queue();
        assert_eq!(lsq.len(), 2);
        assert!(!lsq[0].is_completed());
        assert!(lsq[1].is_completed());
    }
    assert_eq!(core.read_register(reg(6)), 0);
    assert!(core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .is_none());

    scheduler.run_until_idle_conservative();
    core.record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("older store should retire first");
    assert_eq!(core.read_register(reg(6)), 0);
    core.record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("forwarded load should retire second");
    assert_eq!(core.read_register(reg(6)), 0x2a);
}

#[test]
fn htm_abort_cancels_forwarded_load_before_or_after_local_completion() {
    for complete_locally in [false, true] {
        let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
        let core = detailed_store_load_core(fetch_route, data_route, 0x9000, 0x9000);
        let begin = core.begin_htm_transaction().unwrap();

        issue_data_without_response(&core, &mut scheduler, &transport);
        core.issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| panic!("forwarded load must not reach transport"),
        )
        .unwrap()
        .unwrap();
        if complete_locally {
            scheduler.run_next_epoch();
        }

        core.abort_htm_transaction(begin.uid(), crate::HtmFailureCause::Explicit)
            .unwrap();
        scheduler.run_until_idle_conservative();

        let state = core.state.lock().expect("riscv core lock");
        assert!(state.outstanding_data.is_empty());
        assert_eq!(state.o3_runtime.pending_scalar_memory_retirement_count(), 0);
        assert!(state.o3_runtime.snapshot().load_store_queue().is_empty());
        drop(state);
        assert_eq!(core.read_register(reg(6)), 0);
    }
}

#[test]
fn younger_disjoint_load_writeback_waits_for_older_store_retirement() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_load_core(fetch_route, data_route, 0x9000, 0x9040);

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
    .unwrap();
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
    .unwrap();

    while core
        .state
        .lock()
        .expect("riscv core lock")
        .outstanding_data
        .len()
        == 2
    {
        scheduler.run_next_epoch();
    }
    assert_eq!(core.read_register(reg(6)), 0);
    assert!(core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .is_none());

    scheduler.run_until_idle_conservative();
    let store = core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("older store should retire first");
    assert_eq!(store.fetch_pc(), Address::new(0x8000));
    assert_eq!(core.read_register(reg(6)), 0);
    let load = core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("younger load should retire second");
    assert_eq!(load.fetch_pc(), Address::new(0x8004));
    assert_eq!(core.read_register(reg(6)), 0x63);
}

#[test]
fn older_detailed_scalar_store_failure_replays_younger_cancelled_load() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_load_core(fetch_route, data_route, 0x9000, 0x9000);

    issue_data_without_response(&core, &mut scheduler, &transport);
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| panic!("forwarded load must not reach transport"),
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    let older_request = {
        let state = core.state.lock().expect("riscv core lock");
        let younger = state
            .events
            .iter()
            .find(|event| event.fetch_pc() == Address::new(0x8004))
            .expect("forwarded younger load event");
        assert_eq!(
            younger.data_access_event_kind(),
            Some(RiscvDataAccessEventKind::Completed)
        );
        *state
            .outstanding_data
            .keys()
            .next()
            .expect("older store remains outstanding")
    };

    core.record_data_failure(older_request, scheduler.now());
    let state = core.state.lock().expect("riscv core lock");
    let younger = state
        .events
        .iter()
        .find(|event| event.fetch_pc() == Address::new(0x8004))
        .expect("cancelled younger load event");
    assert_eq!(younger.data_access_event_kind(), None);
    assert_eq!(state.o3_runtime.pending_scalar_memory_retirement_count(), 1);
    drop(state);

    let failed = core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("older failed store should drain before replay");
    assert_eq!(
        failed.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::Failed)
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
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(core.read_register(reg(6)), 0);
    let replayed = core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("replayed younger load should complete");
    assert_eq!(replayed.fetch_pc(), Address::new(0x8004));
    assert_eq!(core.read_register(reg(6)), 0x63);
}

#[test]
fn older_store_failure_cancels_issued_disjoint_younger_request() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_load_core(fetch_route, data_route, 0x9000, 0x9040);

    for _ in 0..2 {
        issue_data_without_response(&core, &mut scheduler, &transport);
    }
    let (older_request, younger_request, younger_fetch) = {
        let state = core.state.lock().expect("riscv core lock");
        let older = state
            .outstanding_data
            .values()
            .find(|access| access.fetch_request.sequence() == 1)
            .unwrap();
        let younger = state
            .outstanding_data
            .values()
            .find(|access| access.fetch_request.sequence() == 2)
            .unwrap();
        (older.request, younger.request, younger.fetch_request)
    };

    core.record_data_failure(older_request, scheduler.now());
    let state = core.state.lock().expect("riscv core lock");
    assert!(!state.outstanding_data.contains_key(&younger_request));
    assert!(!state.issued_data_for_fetches.contains(&younger_fetch));
    drop(state);
    core.record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("failed older store should drain");
    assert!(core.has_unissued_data_access());
}

#[test]
fn older_detailed_scalar_store_retry_replays_younger_cancelled_load() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_load_core(fetch_route, data_route, 0x9000, 0x9000);

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
    .unwrap();
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| panic!("forwarded load must not reach transport"),
    )
    .unwrap()
    .unwrap();
    scheduler.run_next_epoch();
    {
        let state = core.state.lock().expect("riscv core lock");
        let younger = state
            .events
            .iter()
            .find(|event| event.fetch_pc() == Address::new(0x8004))
            .expect("forwarded younger load event");
        assert_eq!(
            younger.data_access_event_kind(),
            Some(RiscvDataAccessEventKind::Completed)
        );
    }
    scheduler.run_until_idle_conservative();

    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .outstanding_data
        .is_empty());
    let retry = core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("older store retry should drain before replay");
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
    .unwrap();
    scheduler.run_until_idle_conservative();
    assert_eq!(core.read_register(reg(6)), 0);
    core.record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("replayed younger load should complete");
    assert_eq!(core.read_register(reg(6)), 0x63);
}

use super::*;
use crate::O3LoadStoreQueueKind;

#[test]
fn store_and_two_younger_loads_issue_before_any_response() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = store_led_core(fetch_route, data_route, [0x9000, 0x9040, 0x9080]);

    issue_data_accesses_without_response(&core, &mut scheduler, &transport, 3);

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 3);
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 3);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 3);
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
            O3LoadStoreQueueKind::Load,
            O3LoadStoreQueueKind::Load,
        ]
    );
}

#[test]
fn store_and_three_younger_loads_fill_the_configured_depth_four_window() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = store_led_core(fetch_route, data_route, [0x9000, 0x9040, 0x9080, 0x90c0]);

    issue_data_accesses_without_response(&core, &mut scheduler, &transport, 4);

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 4);
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 4);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 4);
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
            O3LoadStoreQueueKind::Load,
            O3LoadStoreQueueKind::Load,
            O3LoadStoreQueueKind::Load,
        ]
    );
}

#[test]
fn failed_middle_load_cancels_only_the_store_led_younger_request() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = store_led_core(fetch_route, data_route, [0x9000, 0x9040, 0x9080]);
    issue_data_accesses_without_response(&core, &mut scheduler, &transport, 3);
    let requests = outstanding_data_requests_in_fetch_order(&core);

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
fn failed_leading_store_cancels_both_younger_load_requests() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = store_led_core(fetch_route, data_route, [0x9000, 0x9040, 0x9080]);
    issue_data_accesses_without_response(&core, &mut scheduler, &transport, 3);
    let requests = outstanding_data_requests_in_fetch_order(&core);

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
fn uncacheable_second_younger_load_stays_serialized() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = store_led_core(fetch_route, data_route, [0x9000, 0x9040, 0x9080]);
    core.add_pma_uncacheable_range(RiscvPmaRange::new(0x9080, 0x9084).unwrap())
        .unwrap();

    issue_data_accesses_without_response(&core, &mut scheduler, &transport, 2);
    assert!(core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| panic!("uncacheable third row must not reach transport"),
        )
        .unwrap()
        .is_none());

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 2);
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 2);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 2);
}

fn store_led_core<const N: usize>(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
    addresses: [u64; N],
) -> RiscvCore {
    assert!(N >= 2);
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(N);
    let mut state = core.state.lock().expect("riscv core lock");
    state.hart.write(reg(2), addresses[0]);
    state.events.push(scalar_store_event_with_width_and_value(
        0x8000,
        1,
        addresses[0],
        MemoryWidth::Word,
        0x2a,
    ));
    for (index, address) in addresses.into_iter().enumerate().skip(1) {
        let offset = u64::try_from(index).unwrap();
        let register_offset = u8::try_from(index - 1).unwrap();
        let base = 20 + register_offset;
        state.hart.write(reg(base), address);
        state.events.push(scalar_load_event_with_base(
            0x8000 + offset * 4,
            1 + offset,
            6 + register_offset,
            base,
            address,
        ));
    }
    drop(state);
    core
}

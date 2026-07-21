use super::*;
use crate::riscv_cluster_drive::{
    finish_prepared_parallel_actions, push_prepared_data_action, PreparedParallelActions,
};

fn request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn completed_fetch_with_raw(sequence: u64, pc: u64, raw: u32) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            rem6_kernel::PartitionId::new(0),
            rem6_transport::MemoryRouteId::new(0),
            endpoint("cpu0.ifetch"),
            request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        raw.to_le_bytes().to_vec(),
    )
}

fn atomic_type(operation: u32, rs2: u8, rs1: u8, rd: u8) -> u32 {
    (operation << 27)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b011 << 12)
        | (u32::from(rd) << 7)
        | 0x2f
}

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn younger_atomic_core(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
    with_scalar_suffix: bool,
) -> RiscvCore {
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(2), 0x9000);
    core.write_register(reg(3), 7);
    core.write_register(reg(4), 0x9010);
    let float_head = i_type(0, 2, 0b011, 1, 0x07);
    let atomic = atomic_type(0x01, 3, 4, 11);
    let mut events = vec![
        completed_fetch_with_raw(0, 0x8000, float_head),
        completed_fetch_with_raw(1, 0x8004, atomic),
    ];
    if with_scalar_suffix {
        events.extend([
            completed_fetch_with_raw(2, 0x8008, r_type(1, 4, 1, 0b100, 20, 0x33)),
            completed_fetch_with_raw(3, 0x800c, i_type(1, 11, 0, 21, 0x13)),
        ]);
    }
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend(events);
    core
}

fn issue_without_response(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) {
    core.issue_next_data_access(
        scheduler,
        transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .expect("authorized result request issues");
}

#[test]
fn younger_atomic_issue_is_recorded_but_transport_waits_for_the_head() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = younger_atomic_core(fetch_route, data_route, false);
    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );

    core.execute_next_completed_fetch()
        .unwrap()
        .expect("float head executes");
    issue_without_response(&core, &mut scheduler, &transport);
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger atomic executes");
    issue_without_response(&core, &mut scheduler, &transport);

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 2);
    assert_eq!(state.buffered_o3_effects.len(), 1);
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 2);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 3);
}

#[test]
fn younger_atomic_result_stages_the_bounded_scalar_suffix() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = younger_atomic_core(fetch_route, data_route, true);
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("float head executes");
    issue_without_response(&core, &mut scheduler, &transport);
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger atomic executes");
    issue_without_response(&core, &mut scheduler, &transport);

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 4);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 3);
}

#[test]
fn older_retry_cancels_the_buffered_atomic_before_submission() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = younger_atomic_core(fetch_route, data_route, false);
    core.next_fetch_ahead_before_retire();
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("float head executes");
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| TargetOutcome::Respond(MemoryResponse::retry(delivery.request())),
    )
    .unwrap()
    .expect("float head issues");
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger atomic executes");
    issue_without_response(&core, &mut scheduler, &transport);
    assert_eq!(
        core.state
            .lock()
            .expect("riscv core lock")
            .buffered_o3_effects
            .len(),
        1
    );

    scheduler.run_until_idle_conservative();

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.buffered_o3_effects.is_empty());
    assert!(state.outstanding_data.is_empty());
    assert!(!state
        .o3_runtime
        .owns_pending_live_data_access_retirement(request(1)));
}

#[test]
fn missing_younger_authority_blocks_atomic_before_transport() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = younger_atomic_core(fetch_route, data_route, false);
    core.next_fetch_ahead_before_retire();
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("float head executes");
    issue_without_response(&core, &mut scheduler, &transport);
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger atomic executes");
    core.state
        .lock()
        .expect("riscv core lock")
        .memory_result_window_authorizations
        .remove(&request(1));

    let target_calls = Arc::new(AtomicU64::new(0));
    let responder_calls = Arc::clone(&target_calls);
    assert!(core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            move |_delivery, _context| {
                responder_calls.fetch_add(1, Ordering::Relaxed);
                TargetOutcome::NoResponse
            },
        )
        .unwrap()
        .is_none());
    scheduler.run_until_idle_conservative();

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(target_calls.load(Ordering::Relaxed), 0);
    assert_eq!(state.outstanding_data.len(), 1);
    assert!(state.buffered_o3_effects.is_empty());
    assert!(!state.issued_data_for_fetches.contains(&request(1)));
}

#[test]
fn missing_execution_blocks_prepared_atomic_admission() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = younger_atomic_core(fetch_route, data_route, false);
    core.next_fetch_ahead_before_retire();
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("float head executes");
    issue_without_response(&core, &mut scheduler, &transport);
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger atomic executes");
    let prepared = core
        .prepare_data_parallel_access(
            scheduler.now(),
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("younger atomic prepares as a buffered effect");
    let PreparedDataParallelAccess::BufferedEffect { issue, .. } = &prepared else {
        panic!("younger atomic must prepare as a buffered effect");
    };
    core.state
        .lock()
        .expect("riscv core lock")
        .events
        .retain(|event| event.fetch().request_id() != request(1));

    assert!(matches!(
        core.o3_buffered_effect_predecessor(issue),
        BufferedO3EffectAdmission::Blocked
    ));
}

#[test]
fn prepared_parallel_atomic_revalidates_authority_before_buffering() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = younger_atomic_core(fetch_route, data_route, false);
    core.next_fetch_ahead_before_retire();
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("float head executes");
    issue_without_response(&core, &mut scheduler, &transport);
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger atomic executes");
    let target_calls = Arc::new(AtomicU64::new(0));
    let responder_calls = Arc::clone(&target_calls);
    let prepared = core
        .prepare_data_parallel_access(
            scheduler.now(),
            &transport,
            MemoryTrace::new(),
            move |_delivery, _context| {
                responder_calls.fetch_add(1, Ordering::Relaxed);
                TargetOutcome::NoResponse
            },
        )
        .unwrap()
        .expect("younger atomic prepares as a buffered effect");
    assert!(matches!(
        &prepared,
        PreparedDataParallelAccess::BufferedEffect { .. }
    ));
    core.state
        .lock()
        .expect("riscv core lock")
        .memory_result_window_authorizations
        .remove(&request(1));

    let submission = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        core.submit_prepared_data_parallel_access(&mut scheduler, &transport, prepared)
    }));
    assert!(
        submission.is_ok(),
        "stale prepared atomic must fail closed without panicking"
    );
    let submission = submission
        .unwrap()
        .expect("stale prepared atomic cancellation is not a scheduler error");
    assert!(submission.is_none());
    scheduler.run_until_idle_conservative();
    assert!(scheduler.is_idle());

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(target_calls.load(Ordering::Relaxed), 0);
    assert_eq!(state.outstanding_data.len(), 1);
    assert!(state.buffered_o3_effects.is_empty());
    assert!(!state.issued_data_for_fetches.contains(&request(1)));
    assert_eq!(
        state.o3_runtime.pending_live_data_access_retirement_count(),
        1
    );
}

#[test]
fn cluster_discards_stale_prepared_atomic_without_an_issued_action() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = younger_atomic_core(fetch_route, data_route, false);
    core.next_fetch_ahead_before_retire();
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("float head executes");
    issue_without_response(&core, &mut scheduler, &transport);
    scheduler.run_until_idle_conservative();
    assert!(scheduler.is_idle());
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger atomic executes");
    let prepared = core
        .prepare_data_parallel_access(
            scheduler.now(),
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("younger atomic prepares as a buffered effect");
    let mut prepared_actions = PreparedParallelActions::new();
    let mut transaction_cpus = Vec::new();
    let mut transactions = Vec::new();
    assert!(push_prepared_data_action(
        CpuId::new(0),
        &core,
        Some(prepared),
        &mut prepared_actions,
        &mut transaction_cpus,
        &mut transactions,
    ));
    core.state
        .lock()
        .expect("riscv core lock")
        .memory_result_window_authorizations
        .remove(&request(1));

    let actions = finish_prepared_parallel_actions(
        &mut scheduler,
        &transport,
        prepared_actions,
        transaction_cpus,
        transactions,
    )
    .unwrap();

    assert!(actions.is_empty());
    assert!(scheduler.is_idle());
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 1);
    assert!(state.buffered_o3_effects.is_empty());
    assert!(!state.issued_data_for_fetches.contains(&request(1)));
    assert_eq!(
        state.o3_runtime.pending_live_data_access_retirement_count(),
        1
    );
}

#[test]
fn cluster_discards_stale_ready_buffered_effect_before_transport() {
    let (mut scheduler, transport, core) = ready_buffered_atomic_fixture();
    let target_calls = Arc::new(AtomicU64::new(0));
    let responder_calls = Arc::clone(&target_calls);
    let prepared = core
        .prepare_data_parallel_access(
            scheduler.now(),
            &transport,
            MemoryTrace::new(),
            move |_delivery, _context| {
                responder_calls.fetch_add(1, Ordering::Relaxed);
                TargetOutcome::NoResponse
            },
        )
        .unwrap()
        .expect("ready buffered effect prepares");
    assert!(matches!(
        &prepared,
        PreparedDataParallelAccess::BufferedTransaction { .. }
    ));
    let mut prepared_actions = PreparedParallelActions::new();
    let mut transaction_cpus = Vec::new();
    let mut transactions = Vec::new();
    assert!(push_prepared_data_action(
        CpuId::new(0),
        &core,
        Some(prepared),
        &mut prepared_actions,
        &mut transaction_cpus,
        &mut transactions,
    ));
    core.redirect_pc(Address::new(0x9000));

    let submission = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        finish_prepared_parallel_actions(
            &mut scheduler,
            &transport,
            prepared_actions,
            transaction_cpus,
            transactions,
        )
    }));

    assert!(submission.is_ok(), "stale buffered effect must fail closed");
    assert!(submission.unwrap().unwrap().is_empty());
    assert!(scheduler.is_idle());
    assert_eq!(target_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn direct_submit_discards_stale_ready_buffered_effect_before_transport() {
    let (mut scheduler, transport, core) = ready_buffered_atomic_fixture();
    let prepared = core
        .prepare_data_parallel_access(
            scheduler.now(),
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("ready buffered effect prepares");
    core.redirect_pc(Address::new(0x9000));

    assert!(core
        .submit_prepared_data_parallel_access(&mut scheduler, &transport, prepared)
        .unwrap()
        .is_none());
    assert!(scheduler.is_idle());
}

fn ready_buffered_atomic_fixture() -> (PartitionedScheduler, MemoryTransport, RiscvCore) {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = younger_atomic_core(fetch_route, data_route, false);
    core.next_fetch_ahead_before_retire();
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("float head executes");
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(
                    delivery.request(),
                    Some(3.5_f64.to_bits().to_le_bytes().to_vec()),
                )
                .unwrap(),
            )
        },
    )
    .unwrap()
    .expect("float head issues");
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger atomic executes");
    issue_without_response(&core, &mut scheduler, &transport);
    scheduler.run_until_idle_conservative();
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .has_ready_buffered_o3_effect());
    (scheduler, transport, core)
}

#[test]
fn older_failure_cancels_the_buffered_atomic_before_transport() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = younger_atomic_core(fetch_route, data_route, false);
    core.next_fetch_ahead_before_retire();
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("float head executes");
    issue_without_response(&core, &mut scheduler, &transport);
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger atomic executes");
    let target_calls = Arc::new(AtomicU64::new(0));
    let responder_calls = Arc::clone(&target_calls);
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |_delivery, _context| {
            responder_calls.fetch_add(1, Ordering::Relaxed);
            TargetOutcome::NoResponse
        },
    )
    .unwrap()
    .expect("younger atomic is CPU-buffered");
    let requests = outstanding_data_requests_in_fetch_order(&core);

    core.record_data_failure(requests[0].1, scheduler.now());
    scheduler.run_until_idle_conservative();

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(target_calls.load(Ordering::Relaxed), 0);
    assert!(state.buffered_o3_effects.is_empty());
    assert!(state.outstanding_data.is_empty());
    assert!(!state.issued_data_for_fetches.contains(&request(1)));
}

#[test]
fn completed_head_releases_exactly_one_buffered_atomic_serially() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = younger_atomic_core(fetch_route, data_route, false);
    core.next_fetch_ahead_before_retire();
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("float head executes");
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(
                    delivery.request(),
                    Some(3.5_f64.to_bits().to_le_bytes().to_vec()),
                )
                .unwrap(),
            )
        },
    )
    .unwrap()
    .expect("float head issues");
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger atomic executes");
    issue_without_response(&core, &mut scheduler, &transport);
    scheduler.run_until_idle_conservative();

    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .has_ready_buffered_o3_effect());
    let target_calls = Arc::new(AtomicU64::new(0));
    let responder_calls = Arc::clone(&target_calls);
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            responder_calls.fetch_add(1, Ordering::Relaxed);
            TargetOutcome::Respond(
                MemoryResponse::completed(delivery.request(), Some(9_u64.to_le_bytes().to_vec()))
                    .unwrap(),
            )
        },
    )
    .unwrap()
    .expect("ready buffered atomic submits");
    scheduler.run_until_idle_conservative();

    assert_eq!(target_calls.load(Ordering::Relaxed), 1);
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .buffered_o3_effects
        .is_empty());
}

#[test]
fn cluster_batch_buffers_and_releases_exactly_one_younger_atomic() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = younger_atomic_core(fetch_route, data_route, false);
    core.next_fetch_ahead_before_retire();
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("float head executes");
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
                    response: MemoryResponse::completed(
                        delivery.request(),
                        Some(3.5_f64.to_bits().to_le_bytes().to_vec()),
                    )
                    .unwrap(),
                }
            },
        )
        .unwrap();
    assert_eq!(actions.len(), 1);

    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger atomic executes");
    let actions = cluster
        .drive_ready_cores_parallel(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            |_cpu| |_delivery, _context| panic!("younger atomic must remain CPU-buffered"),
        )
        .unwrap();
    assert_eq!(actions.len(), 1);
    scheduler.run_until_idle_parallel().unwrap();
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .has_ready_buffered_o3_effect());

    let target_calls = Arc::new(AtomicU64::new(0));
    let responder_calls = Arc::clone(&target_calls);
    let actions = cluster
        .drive_ready_cores_parallel(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            move |_cpu| {
                let responder_calls = Arc::clone(&responder_calls);
                move |delivery, _context| {
                    responder_calls.fetch_add(1, Ordering::Relaxed);
                    TargetOutcome::Respond(
                        MemoryResponse::completed(
                            delivery.request(),
                            Some(9_u64.to_le_bytes().to_vec()),
                        )
                        .unwrap(),
                    )
                }
            },
        )
        .unwrap();
    assert_eq!(actions.len(), 1);
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(target_calls.load(Ordering::Relaxed), 1);
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .buffered_o3_effects
        .is_empty());
}

#[test]
fn younger_atomic_retry_preserves_the_completed_head_result() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = younger_atomic_core(fetch_route, data_route, false);
    core.next_fetch_ahead_before_retire();
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("float head executes");
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(
                    delivery.request(),
                    Some(3.5_f64.to_bits().to_le_bytes().to_vec()),
                )
                .unwrap(),
            )
        },
    )
    .unwrap()
    .expect("float head issues");
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger atomic executes");
    issue_without_response(&core, &mut scheduler, &transport);
    scheduler.run_until_idle_conservative();

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| TargetOutcome::Respond(MemoryResponse::retry(delivery.request())),
    )
    .unwrap()
    .expect("ready buffered atomic submits");
    scheduler.run_until_idle_conservative();

    let head = core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("completed head remains publishable");
    assert_eq!(head.fetch_pc(), Address::new(0x8000));
    assert_eq!(
        head.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::Completed)
    );
    assert_eq!(
        core.read_float_register(rem6_isa_riscv::FloatRegister::new(1).unwrap()),
        3.5_f64.to_bits()
    );
    let retry = core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("younger retry retires after the head");
    assert_eq!(retry.fetch_pc(), Address::new(0x8004));
    assert_eq!(
        retry.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::Retry)
    );
}

#[test]
fn younger_atomic_failure_preserves_the_completed_head_result() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = younger_atomic_core(fetch_route, data_route, false);
    core.next_fetch_ahead_before_retire();
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("float head executes");
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(
                    delivery.request(),
                    Some(3.5_f64.to_bits().to_le_bytes().to_vec()),
                )
                .unwrap(),
            )
        },
    )
    .unwrap()
    .expect("float head issues");
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger atomic executes");
    issue_without_response(&core, &mut scheduler, &transport);
    scheduler.run_until_idle_conservative();
    let atomic_request = outstanding_data_requests_in_fetch_order(&core)[0].1;

    let target_calls = Arc::new(AtomicU64::new(0));
    let responder_calls = Arc::clone(&target_calls);
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |_delivery, _context| {
            responder_calls.fetch_add(1, Ordering::Relaxed);
            TargetOutcome::NoResponse
        },
    )
    .unwrap()
    .expect("ready buffered atomic submits");
    core.record_data_failure(atomic_request, scheduler.now());
    scheduler.run_until_idle_conservative();

    assert_eq!(target_calls.load(Ordering::Relaxed), 0);
    let head = core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("completed head remains publishable");
    assert_eq!(head.fetch_pc(), Address::new(0x8000));
    assert_eq!(
        head.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::Completed)
    );
    assert_eq!(
        core.read_float_register(rem6_isa_riscv::FloatRegister::new(1).unwrap()),
        3.5_f64.to_bits()
    );
    let failed = core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("younger failure retires after the head");
    assert_eq!(failed.fetch_pc(), Address::new(0x8004));
    assert_eq!(
        failed.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::Failed)
    );
}

#[test]
fn disabling_detailed_mode_discards_unissued_younger_atomic_authority() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = younger_atomic_core(fetch_route, data_route, false);
    core.next_fetch_ahead_before_retire();
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("float head executes");
    issue_without_response(&core, &mut scheduler, &transport);
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .memory_result_window_authorizations
        .contains_key(&request(1)));

    core.set_detailed_live_retire_gate_enabled(false);

    assert!(!core
        .state
        .lock()
        .expect("riscv core lock")
        .memory_result_window_authorizations
        .contains_key(&request(1)));
}

#[test]
fn disabling_detailed_mode_cancels_an_issued_buffered_atomic() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = younger_atomic_core(fetch_route, data_route, true);
    core.next_fetch_ahead_before_retire();
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("float head executes");
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(
                    delivery.request(),
                    Some(3.5_f64.to_bits().to_le_bytes().to_vec()),
                )
                .unwrap(),
            )
        },
    )
    .unwrap()
    .expect("float head issues");
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger atomic executes");
    let canceled_target_calls = Arc::new(AtomicU64::new(0));
    let responder_calls = Arc::clone(&canceled_target_calls);
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |_delivery, _context| {
            responder_calls.fetch_add(1, Ordering::Relaxed);
            TargetOutcome::NoResponse
        },
    )
    .unwrap()
    .expect("younger atomic is CPU-buffered");

    core.set_detailed_live_retire_gate_enabled(false);

    {
        let state = core.state.lock().expect("riscv core lock");
        assert!(state.buffered_o3_effects.is_empty());
        assert_eq!(state.outstanding_data.len(), 1);
        assert_eq!(
            state.o3_runtime.pending_live_data_access_retirement_count(),
            1
        );
        assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 1);
        assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 1);
        assert!(!state.issued_data_for_fetches.contains(&request(1)));
    }
    scheduler.run_until_idle_conservative();
    assert_eq!(canceled_target_calls.load(Ordering::Relaxed), 0);

    core.record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("completed head remains publishable");
    assert!(core.has_unissued_data_access());
    let replay_target_calls = Arc::new(AtomicU64::new(0));
    let responder_calls = Arc::clone(&replay_target_calls);
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            responder_calls.fetch_add(1, Ordering::Relaxed);
            TargetOutcome::Respond(
                MemoryResponse::completed(delivery.request(), Some(9_u64.to_le_bytes().to_vec()))
                    .unwrap(),
            )
        },
    )
    .unwrap()
    .expect("canceled atomic replays after the head drains");
    scheduler.run_until_idle_conservative();

    assert_eq!(replay_target_calls.load(Ordering::Relaxed), 1);
    assert_eq!(core.read_register(reg(11)), 9);
}

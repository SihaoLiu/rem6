use super::dependent_result_address::PendingIssueFixture;
use super::*;
use crate::riscv_cluster_drive::{
    finish_prepared_parallel_actions, push_prepared_data_action, PreparedParallelActions,
};

#[test]
fn retry_response_discards_pending_o3_trace_data_access_outcome() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    let instruction = rem6_isa_riscv::RiscvInstruction::Load {
        rd: reg(5),
        rs1: reg(2),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    let access = MemoryAccessKind::Load {
        rd: reg(5),
        address: 0x9000,
        width: MemoryWidth::Word,
        signed: false,
    };
    let event = RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 1),
        instruction,
        RiscvExecutionRecord::new(instruction, 0x8000, 0x8004, Vec::new(), Some(access)),
    );
    core.record_o3_retired_instruction_with_trace(&event, true);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.events.push(event);
        assert_eq!(state.o3_runtime.pending_trace_data_access_outcomes(), 1);
    }

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| TargetOutcome::Respond(MemoryResponse::retry(delivery.request())),
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.outstanding_data.is_empty());
    assert_eq!(state.o3_runtime.pending_trace_data_access_outcomes(), 0);
    let trace = state.o3_runtime.trace_records();
    assert_eq!(trace.len(), 1);
    assert_eq!(trace[0].lsq_data_response_tick(), 0);
    assert_eq!(trace[0].lsq_data_latency_ticks(), 0);
}

#[test]
fn control_boundary_after_stats_reset_discards_pending_o3_data_access_outcome() {
    let (_scheduler, _transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    let instruction = rem6_isa_riscv::RiscvInstruction::Load {
        rd: reg(5),
        rs1: reg(2),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    let access = MemoryAccessKind::Load {
        rd: reg(5),
        address: 0x9000,
        width: MemoryWidth::Word,
        signed: false,
    };
    let event = RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 1),
        instruction,
        RiscvExecutionRecord::new(instruction, 0x8000, 0x8004, Vec::new(), Some(access)),
    );
    core.record_o3_retired_instruction_with_trace(&event, true);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.events.push(event.clone());
        state.o3_runtime.reset_stats();
    }

    core.redirect_pc(Address::new(0x9000));

    let mut state = core.state.lock().expect("riscv core lock");
    state.o3_runtime.record_data_access_outcome(&event, 41, 7);
    assert_eq!(state.o3_runtime.stats().lsq_data_latency_samples(), 0);
    assert_eq!(state.o3_runtime.stats().lsq_data_latency_ticks(), 0);
}

#[test]
fn cluster_batch_discards_invalidated_prepared_dependent_address_before_transport() {
    let mut fixture = PendingIssueFixture::load(0x9000);
    let target_calls = Arc::new(AtomicU64::new(0));
    let responder_calls = Arc::clone(&target_calls);
    let prepared = fixture
        .core
        .prepare_data_parallel_access(
            fixture.scheduler.now(),
            &fixture.transport,
            MemoryTrace::new(),
            move |_delivery, _context| {
                responder_calls.fetch_add(1, Ordering::Relaxed);
                TargetOutcome::NoResponse
            },
        )
        .unwrap()
        .expect("dependent address prepares");
    let mut prepared_actions = PreparedParallelActions::new();
    let mut transaction_cpus = Vec::new();
    let mut transactions = Vec::new();
    assert!(push_prepared_data_action(
        CpuId::new(0),
        &fixture.core,
        Some(prepared),
        &mut prepared_actions,
        &mut transaction_cpus,
        &mut transactions,
    ));
    fixture.core.redirect_pc(Address::new(0x9000));

    let submission = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        finish_prepared_parallel_actions(
            &mut fixture.scheduler,
            &fixture.transport,
            prepared_actions,
            transaction_cpus,
            transactions,
        )
    }));

    assert!(submission.is_ok(), "stale prepared data must fail closed");
    assert!(submission.unwrap().unwrap().is_empty());
    assert!(fixture.scheduler.is_idle());
    assert_eq!(target_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn direct_submit_discards_invalidated_prepared_dependent_address_before_transport() {
    let mut fixture = PendingIssueFixture::load(0x9000);
    let prepared = fixture
        .core
        .prepare_data_parallel_access(
            fixture.scheduler.now(),
            &fixture.transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("dependent address prepares");
    fixture.core.redirect_pc(Address::new(0x9000));

    assert!(fixture
        .core
        .submit_prepared_data_parallel_access(&mut fixture.scheduler, &fixture.transport, prepared,)
        .unwrap()
        .is_none());
    assert!(fixture.scheduler.is_idle());
}

#[test]
fn detailed_scalar_load_submission_stages_live_o3_rob_and_lsq_rows() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    let instruction = rem6_isa_riscv::RiscvInstruction::Load {
        rd: reg(5),
        rs1: reg(2),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    let access = MemoryAccessKind::Load {
        rd: reg(5),
        address: 0x9000,
        width: MemoryWidth::Word,
        signed: false,
    };
    let event = RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 1),
        instruction,
        RiscvExecutionRecord::new(instruction, 0x8000, 0x8004, Vec::new(), Some(access)),
    );
    core.state
        .lock()
        .expect("riscv core lock")
        .events
        .push(event);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| TargetOutcome::Respond(MemoryResponse::retry(delivery.request())),
    )
    .unwrap()
    .unwrap();

    let snapshot = core.o3_runtime_snapshot();
    assert_eq!(snapshot.reorder_buffer().len(), 1);
    assert_eq!(snapshot.load_store_queue().len(), 1);
    assert!(!snapshot.reorder_buffer()[0].is_ready());
    assert!(!snapshot.load_store_queue()[0].is_completed());

    scheduler.run_until_idle_conservative();

    let mut state = core.state.lock().expect("riscv core lock");
    assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
    assert!(state.o3_runtime.snapshot().load_store_queue().is_empty());
    let retry = state
        .o3_runtime
        .take_ready_live_data_access_event(u64::MAX)
        .expect("retry response should ready one deferred O3 event");
    assert_eq!(
        retry.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::Retry)
    );
}

#[test]
fn detailed_scalar_load_submission_stages_one_completed_younger_fetch() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    complete_scalar_load_and_younger_fetch(&core, &mut scheduler, &transport, 0x9000);

    issue_data_without_response(&core, &mut scheduler, &transport);

    let snapshot = core.o3_runtime_snapshot();
    assert_eq!(snapshot.reorder_buffer().len(), 2);
    assert_eq!(snapshot.reorder_buffer()[0].pc(), Address::new(0x8000));
    assert_eq!(snapshot.reorder_buffer()[1].pc(), Address::new(0x8004));
    assert_eq!(snapshot.load_store_queue().len(), 1);
}

#[test]
fn completed_scalar_load_blocks_younger_execution_until_o3_event_is_consumed() {
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
    assert!(core.execute_next_completed_fetch().unwrap().is_none());
    assert_eq!(core.o3_runtime_snapshot().reorder_buffer().len(), 2);
    assert!(core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .is_some());

    let younger = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(younger.fetch_pc(), Address::new(0x8004));
    core.record_o3_retired_instruction_with_trace(&younger, true);
    assert!(core.o3_runtime_snapshot().reorder_buffer().is_empty());
    assert!(core.o3_live_data_access_lifecycle_is_quiescent());
}

#[test]
fn mode_disable_after_scalar_load_issue_uses_completed_fetch_timing_for_dependent_younger() {
    assert_mode_disable_uses_completed_fetch_timing_for_dependent_scalar_load_younger(false);
}

#[test]
fn mode_disable_before_scalar_load_issue_uses_completed_fetch_timing_for_dependent_younger() {
    assert_mode_disable_uses_completed_fetch_timing_for_dependent_scalar_load_younger(true);
}

fn assert_mode_disable_uses_completed_fetch_timing_for_dependent_scalar_load_younger(
    disable_before_issue: bool,
) {
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
    let dependent = i_type(7, 5, 0b000, 6, 0x13);
    core.issue_next_fetch(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(
                    delivery.request(),
                    Some(dependent.to_le_bytes().to_vec()),
                )
                .unwrap(),
            )
        },
    )
    .unwrap();
    scheduler.run_until_idle_conservative();
    if disable_before_issue {
        core.set_detailed_live_retire_gate_enabled(false);
    }
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| TargetOutcome::RespondAfter {
            delay: 20,
            response: MemoryResponse::completed(delivery.request(), Some(vec![0x2a, 0, 0, 0]))
                .unwrap(),
        },
    )
    .unwrap()
    .unwrap();
    if !disable_before_issue {
        core.set_detailed_live_retire_gate_enabled(false);
    }
    scheduler.run_until_idle_conservative();
    let response_tick = core
        .data_access_events()
        .last()
        .expect("completed load response")
        .tick();
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
    assert!(core.execute_next_completed_fetch().unwrap().is_none());
    assert!(core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .is_some());
    let younger = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(younger.fetch_pc(), Address::new(0x8004));
    assert_eq!(core.read_register(reg(6)), 0x31);
    core.record_o3_retired_instruction_with_trace(&younger, true);
    let trace = core.o3_runtime_trace_records();
    let load = trace
        .iter()
        .find(|event| event.pc() == Address::new(0x8000))
        .expect("scalar load O3 trace event");
    let younger = trace
        .iter()
        .find(|event| event.pc() == Address::new(0x8004))
        .expect("dependent younger O3 trace event");
    let dependent_fetch_tick = core
        .core
        .fetch_events()
        .into_iter()
        .find(|event| {
            event.kind() == crate::CpuFetchEventKind::Completed
                && event.pc() == Address::new(0x8004)
        })
        .expect("completed dependent younger fetch")
        .tick();
    assert_eq!(load.writeback_tick(), response_tick + 1);
    assert_eq!(younger.issue_tick(), dependent_fetch_tick);
    assert!(younger.issue_tick() < load.writeback_tick());
    assert_eq!(younger.writeback_tick(), younger.issue_tick());
}

use std::sync::Mutex;

use rem6_isa_riscv::{Immediate, MemoryWidth, Register, RiscvExecutionRecord};
use rem6_kernel::PartitionedScheduler;
use rem6_memory::{AddressRange, AgentId, MemoryResponse};
use rem6_mmio::{MmioAccess, MmioBus, MmioRegisterBank, MmioRoute};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

use super::*;
use crate::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuFetchEvent, CpuFetchRecord, CpuResetState,
    RiscvCluster, RiscvCpuExecutionEvent,
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
        .take_ready_live_scalar_memory_event()
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
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .unwrap();

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
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .is_some());

    let younger = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(younger.fetch_pc(), Address::new(0x8004));
    core.record_o3_retired_instruction_with_trace(&younger, true);
    assert!(core.o3_runtime_snapshot().reorder_buffer().is_empty());
    assert!(core.o3_scalar_memory_lifecycle_is_quiescent());
}

#[test]
fn detailed_scalar_store_submission_does_not_stage_younger_fetch() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    let instruction = rem6_isa_riscv::RiscvInstruction::Store {
        rs1: reg(2),
        rs2: reg(5),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
    };
    let access = MemoryAccessKind::Store {
        address: 0x9000,
        width: MemoryWidth::Word,
        value: 0x2a,
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
    let independent = i_type(7, 0, 0b000, 6, 0x13);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(CpuFetchEvent::completed(
            CpuFetchRecord::new(
                12,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                endpoint("cpu0.ifetch"),
                MemoryRequestId::new(AgentId::new(7), 2),
                Address::new(0x8004),
                AccessSize::new(4).unwrap(),
            ),
            independent.to_le_bytes().to_vec(),
        ));

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .unwrap();

    let snapshot = core.o3_runtime_snapshot();
    assert_eq!(snapshot.reorder_buffer().len(), 1);
    assert_eq!(snapshot.reorder_buffer()[0].pc(), Address::new(0x8000));
    assert_eq!(snapshot.load_store_queue().len(), 1);
}

#[test]
fn two_independent_detailed_scalar_loads_issue_before_first_response() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_two_load_core(fetch_route, data_route, 0x9000, 0x9040);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .unwrap();

    assert!(core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .is_some());
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 2);
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 2);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 2);
}

#[test]
fn younger_mmio_load_does_not_fall_through_to_memory_while_load_is_outstanding() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_two_load_core(fetch_route, data_route, 0x9000, 0xa000);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .unwrap();

    let bus = test_mmio_bus(0xa000, vec![0x63, 0, 0, 0]);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();

    let actions = cluster
        .drive_ready_cores_parallel_with_mmio(
            &mut scheduler,
            &transport,
            &bus,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap();

    assert!(actions.is_empty());
    assert_eq!(
        core.state
            .lock()
            .expect("riscv core lock")
            .outstanding_data
            .len(),
        1
    );
}

#[test]
fn older_mmio_load_blocks_younger_memory_issue() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_two_load_core(fetch_route, data_route, 0xa000, 0x9000);
    let bus = test_mmio_bus(0xa000, vec![0x2a, 0, 0, 0]);

    core.issue_next_mmio_data_access_parallel(&mut scheduler, &bus)
        .unwrap()
        .unwrap();
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let actions = cluster
        .drive_ready_cores_parallel_with_mmio(
            &mut scheduler,
            &transport,
            &bus,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap();

    assert!(actions.is_empty());
    assert_eq!(
        core.state
            .lock()
            .expect("riscv core lock")
            .outstanding_data
            .len(),
        1
    );
}

#[test]
fn older_detailed_scalar_load_failure_replays_younger_cancelled_request() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_two_load_core(fetch_route, data_route, 0x9000, 0x9040);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .unwrap();
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| TargetOutcome::RespondAfter {
            delay: 40,
            response: MemoryResponse::completed(delivery.request(), Some(vec![0xff, 0, 0, 0]))
                .unwrap(),
        },
    )
    .unwrap()
    .unwrap();

    let (older_request, younger_request) = {
        let state = core.state.lock().expect("riscv core lock");
        let mut requests = state
            .outstanding_data
            .values()
            .map(|access| (access.fetch_request.sequence(), access.request))
            .collect::<Vec<_>>();
        requests.sort_unstable_by_key(|(sequence, _)| *sequence);
        (requests[0].1, requests[1].1)
    };

    core.record_data_failure(older_request, scheduler.now());

    let state = core.state.lock().expect("riscv core lock");
    assert!(!state.outstanding_data.contains_key(&younger_request));
    assert_eq!(state.o3_runtime.pending_scalar_memory_retirement_count(), 1);
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 0);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 0);
    drop(state);

    let failed = core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("older failed load should drain before replay");
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
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .outstanding_data
        .is_empty());
}

#[test]
fn older_detailed_scalar_load_retry_replays_younger_cancelled_request() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_two_load_core(fetch_route, data_route, 0x9000, 0x9040);

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
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .outstanding_data
        .is_empty());
    let retry = core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("older retry should drain before replay");
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

#[test]
fn younger_completed_load_replay_replaces_cancelled_timing_provenance() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_two_load_core(fetch_route, data_route, 0x9000, 0x9040);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .unwrap();
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(delivery.request(), Some(vec![0xff, 0, 0, 0])).unwrap(),
            )
        },
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
            .expect("younger execution event");
        assert_eq!(
            younger.data_access_event_kind(),
            Some(RiscvDataAccessEventKind::Completed)
        );
        *state
            .outstanding_data
            .keys()
            .next()
            .expect("older request remains outstanding")
    };

    core.record_data_failure(older_request, scheduler.now());
    core.record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("older failed load should drain before replay");

    {
        let state = core.state.lock().expect("riscv core lock");
        let younger = state
            .events
            .iter()
            .find(|event| event.fetch_pc() == Address::new(0x8004))
            .expect("younger execution event");
        assert_eq!(younger.data_access_event_kind(), None);
        assert!(younger.in_order_pipeline_cycle().is_none());
        assert_eq!(younger.in_order_pipeline_data_wait_cycles(), 0);
    }

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| TargetOutcome::RespondAfter {
            delay: 10,
            response: MemoryResponse::completed(delivery.request(), Some(vec![0x63, 0, 0, 0]))
                .unwrap(),
        },
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();
    let replayed = core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("replayed younger load should retire");

    assert_eq!(replayed.fetch_pc(), Address::new(0x8004));
    assert_eq!(
        replayed.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::Completed)
    );
    assert!(replayed.in_order_pipeline_cycle().is_some());
    assert_eq!(core.read_register(reg(6)), 0x63);
}

#[test]
fn younger_detailed_scalar_load_response_waits_for_older_retirement() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_two_load_core(fetch_route, data_route, 0x9000, 0x9040);

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
        let before = scheduler.now();
        let summary = scheduler.run_next_epoch();
        assert!(summary.executed_events() > 0 || summary.final_tick() > before);
    }

    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), 0);
    assert!(core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .is_none());

    scheduler.run_until_idle_conservative();
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), 0);

    let older = core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("older completed load should retire first");
    assert_eq!(older.fetch_pc(), Address::new(0x8000));
    assert_eq!(core.read_register(reg(5)), 0x2a);
    assert_eq!(core.read_register(reg(6)), 0);

    let younger = core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("younger completed load should retire second");
    assert_eq!(younger.fetch_pc(), Address::new(0x8004));
    assert_eq!(core.read_register(reg(5)), 0x2a);
    assert_eq!(core.read_register(reg(6)), 0x63);
}

#[test]
fn disabling_detailed_mode_preserves_ordered_pending_load_retirement() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_two_load_core(fetch_route, data_route, 0x9000, 0x9040);

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
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), 0);

    core.set_detailed_live_retire_gate_enabled(false);

    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), 0);
    assert_eq!(core.pending_o3_scalar_memory_retirement_count(), 2);
    assert!(core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .is_none());
    scheduler.run_until_idle_conservative();

    let older = core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("older load should retire first after mode disable");
    assert_eq!(older.fetch_pc(), Address::new(0x8000));
    assert_eq!(core.read_register(reg(5)), 0x2a);
    assert_eq!(core.read_register(reg(6)), 0);
    let younger = core
        .record_ready_o3_scalar_memory_event_with_trace(true)
        .expect("younger completed load should retire second after mode disable");
    assert_eq!(younger.fetch_pc(), Address::new(0x8004));
    assert_eq!(core.read_register(reg(6)), 0x63);
    assert!(core.o3_scalar_memory_lifecycle_is_quiescent());
}

#[test]
fn completed_data_request_blocks_second_issue_until_o3_retirement() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_two_load_core(fetch_route, data_route, 0x9000, 0x9004);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(delivery.request(), Some(vec![1, 0, 0, 0])).unwrap(),
            )
        },
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert!(core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| panic!("completed live slot must block transport submission"),
        )
        .unwrap()
        .is_none());
    assert!(core
        .record_ready_o3_scalar_memory_event_with_trace(false)
        .is_some());
    assert!(core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .is_some());
}

#[test]
fn failed_issue_attempt_clears_deferred_marker_and_allows_retry() {
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
        .push(event.clone());
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .defer_scalar_memory_execution(&event));

    let empty_transport = MemoryTransport::new();
    assert!(core
        .issue_next_data_access(
            &mut scheduler,
            &empty_transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .is_err());

    assert!(core.o3_scalar_memory_lifecycle_is_quiescent());
    assert!(!core.data_access_lifecycle_is_quiescent());
    assert!(core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .is_some());
}

fn memory_routes() -> (
    PartitionedScheduler,
    MemoryTransport,
    MemoryRouteId,
    MemoryRouteId,
) {
    let scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let data_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.dmem"),
                PartitionId::new(0),
                endpoint("l1d0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();

    (scheduler, transport, fetch_route, data_route)
}

fn detailed_two_load_core(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
    older_address: u64,
    younger_address: u64,
) -> RiscvCore {
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.state.lock().expect("riscv core lock").events.extend([
        scalar_load_event(0x8000, 1, 5, older_address),
        scalar_load_event(0x8004, 2, 6, younger_address),
    ]);
    core
}

fn test_mmio_bus(address: u64, value: Vec<u8>) -> MmioBus {
    let mut bank =
        MmioRegisterBank::new(Address::new(address), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        0,
        AccessSize::new(value.len() as u64).unwrap(),
        MmioAccess::ReadOnly,
        value,
    )
    .unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        AddressRange::new(Address::new(address), AccessSize::new(0x100).unwrap()).unwrap(),
        MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 2).unwrap(),
        Mutex::new(bank),
    )
    .unwrap();
    bus
}

fn scalar_load_event(pc: u64, sequence: u64, rd: u8, address: u64) -> RiscvCpuExecutionEvent {
    let instruction = rem6_isa_riscv::RiscvInstruction::Load {
        rd: reg(rd),
        rs1: reg(2),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    let access = MemoryAccessKind::Load {
        rd: reg(rd),
        address,
        width: MemoryWidth::Word,
        signed: false,
    };
    RiscvCpuExecutionEvent::new(
        fetch_event(pc, sequence),
        instruction,
        RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
    )
}

fn cpu_core(route: MemoryRouteId, entry: u64) -> CpuCore {
    CpuCore::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(7),
            Address::new(entry),
        ),
        CpuFetchConfig::new(
            endpoint("cpu0.ifetch"),
            route,
            line_layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap()
}

fn fetch_event(pc: u64, sequence: u64) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            endpoint("cpu0.ifetch"),
            MemoryRequestId::new(AgentId::new(7), sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        0x0000_0013u32.to_le_bytes().to_vec(),
    )
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

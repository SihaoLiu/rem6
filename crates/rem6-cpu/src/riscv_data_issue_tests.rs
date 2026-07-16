use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

use rem6_isa_riscv::{Immediate, MemoryWidth, Register, RiscvExecutionRecord, RiscvPmaRange};
use rem6_kernel::PartitionedScheduler;
use rem6_memory::{
    AccessSize, Address, AddressRange, AgentId, MemoryRequestId, MemoryResponse,
    TranslationPageMap, TranslationPagePermissions, TranslationPageSize, TranslationQueueConfig,
    TranslationTlbConfig,
};
use rem6_mmio::{MmioAccess, MmioBus, MmioRegisterBank, MmioRoute};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

use super::*;
use crate::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuFetchEvent, CpuFetchRecord, CpuResetState,
    CpuTranslationFrontend, RiscvCluster, RiscvClusterError, RiscvCpuExecutionEvent,
};

#[path = "riscv_data_issue_tests/forwarding.rs"]
mod forwarding;
#[path = "riscv_data_issue_tests/multi_load.rs"]
mod multi_load;
#[path = "riscv_data_issue_tests/store_led.rs"]
mod store_led;
#[path = "riscv_data_issue_tests/store_store_load.rs"]
mod store_store_load;
#[path = "riscv_data_issue_tests/translated.rs"]
mod translated;

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
fn mode_disable_after_scalar_load_issue_preserves_dependent_younger_wakeup_timing() {
    assert_mode_disable_preserves_dependent_scalar_load_younger_wakeup_timing(false);
}

#[test]
fn mode_disable_before_scalar_load_issue_preserves_dependent_younger_wakeup_timing() {
    assert_mode_disable_preserves_dependent_scalar_load_younger_wakeup_timing(true);
}

fn assert_mode_disable_preserves_dependent_scalar_load_younger_wakeup_timing(
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
    assert_eq!(younger.issue_tick(), response_tick + 1);
    assert_eq!(younger.issue_tick(), load.writeback_tick());
    assert_eq!(younger.writeback_tick(), load.writeback_tick() + 1);
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
fn detailed_store_then_disjoint_load_issue_before_store_response() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_load_core(fetch_route, data_route, 0x9000, 0x9040);

    issue_data_without_response(&core, &mut scheduler, &transport);

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
fn uncacheable_store_or_younger_load_keeps_store_load_pair_serialized() {
    for uncacheable in [0x9000, 0x9040] {
        let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
        let core = detailed_store_load_core(fetch_route, data_route, 0x9000, 0x9040);
        core.add_pma_uncacheable_range(RiscvPmaRange::new(uncacheable, uncacheable + 4).unwrap())
            .unwrap();

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
                |_delivery, _context| panic!("uncacheable pair must not reach transport"),
            )
            .unwrap()
            .is_none());
        assert_eq!(
            core.state
                .lock()
                .expect("riscv core lock")
                .outstanding_data
                .len(),
            1
        );
    }
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
fn younger_mmio_load_does_not_issue_while_store_is_outstanding() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = detailed_store_load_core(fetch_route, data_route, 0x9000, 0xa000);

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
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 1);
    assert_eq!(
        state.o3_runtime.pending_live_data_access_retirement_count(),
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
    assert_eq!(
        state.o3_runtime.pending_live_data_access_retirement_count(),
        1
    );
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 0);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 0);
    drop(state);

    let failed = core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
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
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
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
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
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
    core.record_ready_o3_data_access_event_with_trace(u64::MAX, true)
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
    core.record_ready_o3_data_access_event_with_trace(u64::MAX, true)
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
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
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
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .is_none());

    scheduler.run_until_idle_conservative();
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), 0);

    let older = core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("older completed load should retire first");
    assert_eq!(older.fetch_pc(), Address::new(0x8000));
    assert_eq!(core.read_register(reg(5)), 0x2a);
    assert_eq!(core.read_register(reg(6)), 0);

    let younger = core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
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
    assert_eq!(core.pending_o3_live_data_access_retirement_count(), 2);
    assert!(core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .is_none());
    scheduler.run_until_idle_conservative();

    let older = core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("older load should retire first after mode disable");
    assert_eq!(older.fetch_pc(), Address::new(0x8000));
    assert_eq!(core.read_register(reg(5)), 0x2a);
    assert_eq!(core.read_register(reg(6)), 0);
    let younger = core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("younger completed load should retire second after mode disable");
    assert_eq!(younger.fetch_pc(), Address::new(0x8004));
    assert_eq!(core.read_register(reg(6)), 0x63);
    assert!(core.o3_live_data_access_lifecycle_is_quiescent());
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
        .record_ready_o3_data_access_event_with_trace(u64::MAX, false)
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
        .defer_live_data_access_execution(&event));

    let empty_transport = MemoryTransport::new();
    assert!(core
        .issue_next_data_access(
            &mut scheduler,
            &empty_transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .is_err());

    assert!(core.o3_live_data_access_lifecycle_is_quiescent());
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

#[test]
fn dropped_prepared_parallel_data_access_clears_deferred_marker_and_allows_retry() {
    let (scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    let fetch_request = defer_scalar_load(&core, 0x9000);

    let prepared = core
        .prepare_data_parallel_access(
            scheduler.now(),
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("direct load should prepare a parallel data access");
    assert!(matches!(
        &prepared,
        PreparedDataParallelAccess::Transaction { issue, .. }
            if issue.fetch_request == fetch_request
    ));

    drop(prepared);

    assert_eq!(core.pending_o3_live_data_access_retirement_count(), 0);
    assert!(core.o3_live_data_access_lifecycle_is_quiescent());
    assert!(core.has_unissued_data_access());
    assert!(!core.has_pending_data_access());

    let retry = core
        .prepare_data_parallel_access(
            scheduler.now(),
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap();
    assert!(matches!(
        retry,
        Some(PreparedDataParallelAccess::Transaction { .. })
    ));
}

#[test]
fn dropped_prepared_translated_parallel_data_access_clears_deferred_marker_and_allows_retry() {
    let (scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data_translation(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
        CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    let fetch_request = defer_scalar_load(&core, 0x4008);
    let mut page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    page_map
        .map(
            Address::new(0x4000),
            Address::new(0x9000),
            1,
            TranslationPagePermissions::read_write_execute(),
        )
        .unwrap();

    let prepared = core
        .prepare_translated_data_parallel_access(
            scheduler.now(),
            &transport,
            MemoryTrace::new(),
            &page_map,
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("translated load should prepare a parallel data access");
    assert!(matches!(
        &prepared,
        PreparedDataParallelAccess::Transaction { issue, .. }
            if issue.fetch_request == fetch_request
                && issue.physical_address == Address::new(0x9008)
    ));

    drop(prepared);

    assert!(!core.owns_pending_o3_live_data_access_retirement(fetch_request));
    assert!(core.o3_live_data_access_lifecycle_is_quiescent());
    assert!(core.has_unissued_data_access());
    assert!(!core.has_pending_data_access());

    let retry = core
        .prepare_translated_data_parallel_access(
            scheduler.now(),
            &transport,
            MemoryTrace::new(),
            &page_map,
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap();
    assert!(matches!(
        retry,
        Some(PreparedDataParallelAccess::Transaction { .. })
    ));
}

#[test]
fn failed_parallel_data_submission_clears_deferred_marker_and_allows_retry() {
    let (mut retry_scheduler, transport, fetch_route, data_route) = memory_routes();
    let mut rejecting_scheduler = PartitionedScheduler::with_min_remote_delay(2, 3).unwrap();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    let fetch_request = defer_scalar_load(&core, 0x9000);

    let error = core
        .issue_next_data_access_parallel(
            &mut rejecting_scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap_err();
    assert!(matches!(error, RiscvCpuError::Transport(_)));
    assert!(rejecting_scheduler.is_idle());
    assert!(!core.owns_pending_o3_live_data_access_retirement(fetch_request));
    assert!(core.has_unissued_data_access());
    assert!(!core.has_pending_data_access());

    assert!(core
        .issue_next_data_access_parallel(
            &mut retry_scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .is_some());
}

#[test]
fn failed_parallel_translated_data_submission_clears_deferred_marker_and_allows_retry() {
    let (mut retry_scheduler, transport, fetch_route, data_route) = memory_routes();
    let mut rejecting_scheduler = PartitionedScheduler::with_min_remote_delay(2, 3).unwrap();
    let core = RiscvCore::with_data_translation(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
        CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    let fetch_request = defer_scalar_load(&core, 0x4008);
    let mut page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    page_map
        .map(
            Address::new(0x4000),
            Address::new(0x9000),
            1,
            TranslationPagePermissions::read_write_execute(),
        )
        .unwrap();

    let error = core
        .issue_next_translated_data_access_parallel(
            &mut rejecting_scheduler,
            &transport,
            MemoryTrace::new(),
            &page_map,
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap_err();
    assert!(matches!(error, RiscvCpuError::Transport(_)));
    assert!(rejecting_scheduler.is_idle());
    assert!(!core.owns_pending_o3_live_data_access_retirement(fetch_request));
    assert!(core.has_unissued_data_access());
    assert!(!core.has_pending_data_access());

    assert!(core
        .issue_next_translated_data_access_parallel(
            &mut retry_scheduler,
            &transport,
            MemoryTrace::new(),
            &page_map,
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .is_some());
}

#[test]
fn dropped_prepared_parallel_data_access_tolerates_poisoned_core_state() {
    let (scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    let fetch_request = defer_scalar_load(&core, 0x9000);
    let prepared = core
        .prepare_data_parallel_access(
            scheduler.now(),
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("direct load should prepare a parallel data access");

    let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe({
        let core = core.clone();
        move || {
            let _state = core.state.lock().expect("riscv core lock");
            panic!("poison riscv core state");
        }
    }));
    assert!(poisoned.is_err());
    assert!(core.state.is_poisoned());

    let dropped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(prepared)));
    assert!(dropped.is_ok());
    assert!(!core
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .o3_runtime
        .owns_pending_live_data_access_retirement(fetch_request));
}

#[test]
fn data_response_writeback_error_is_sticky_and_surfaces_from_drive() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.write_register(reg(2), 0x9000);
    defer_scalar_load(&core, 0x9000);
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
    .expect("detailed scalar load should issue");
    let sequence = core.o3_runtime_snapshot().reorder_buffer()[0].sequence();
    core.reserve_test_fixed_fu_writeback(sequence, 0).unwrap();

    scheduler.run_until_idle_conservative();

    let expected = core
        .pending_callback_error()
        .expect("reservation error is sticky");
    assert!(matches!(
        expected,
        RiscvCpuError::O3Runtime(O3RuntimeError::WritebackReservationMismatch { .. })
    ));
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.events[0].data_access_event_kind(), None);
    assert_eq!(state.outstanding_data.len(), 1);
    assert!(!state.o3_runtime.snapshot().load_store_queue()[0].is_completed());
    assert!(!state.o3_runtime.snapshot().reorder_buffer()[0].is_ready());
    drop(state);

    for _ in 0..2 {
        assert_eq!(
            core.drive_next_action(
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                |_delivery, _context| TargetOutcome::NoResponse,
                |_delivery, _context| TargetOutcome::NoResponse,
            ),
            Err(expected.clone())
        );
    }
}

#[test]
fn mmio_response_writeback_error_is_sticky_without_partial_state() {
    let (mut scheduler, _transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.write_register(reg(2), 0xa000);
    core.write_register(reg(5), 0xfeed_face);
    defer_scalar_load(&core, 0xa000);
    let bus = test_mmio_bus(0xa000, vec![0x2a, 0, 0, 0]);
    core.issue_next_mmio_data_access_parallel(&mut scheduler, &bus)
        .unwrap()
        .expect("detailed scalar MMIO load should issue");
    let sequence = core.o3_runtime_snapshot().reorder_buffer()[0].sequence();
    core.reserve_test_fixed_fu_writeback(sequence, 0).unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    let expected = core
        .pending_callback_error()
        .expect("reservation error is sticky");
    assert!(matches!(
        expected,
        RiscvCpuError::O3Runtime(O3RuntimeError::WritebackReservationMismatch { .. })
    ));
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.events[0].data_access_event_kind(), None);
    assert_eq!(state.outstanding_data.len(), 1);
    assert_eq!(state.hart.read(reg(5)), 0xfeed_face);
    assert!(!state.o3_runtime.snapshot().load_store_queue()[0].is_completed());
    assert!(!state.o3_runtime.snapshot().reorder_buffer()[0].is_ready());
}

fn issue_data_without_response(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) {
    issue_data_accesses_without_response(core, scheduler, transport, 1);
}

fn complete_scalar_load_and_younger_fetch(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    load_base: u64,
) {
    core.write_register(reg(2), load_base);
    let load = i_type(0, 2, 0b010, 5, 0x03);
    core.issue_next_fetch(
        scheduler,
        transport,
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
        scheduler,
        transport,
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
}

fn issue_data_accesses_without_response(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    count: usize,
) {
    for _ in 0..count {
        core.issue_next_data_access(
            scheduler,
            transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("scalar memory row should issue");
    }
}

fn outstanding_data_requests_in_fetch_order(
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

fn detailed_store_load_core(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
    store_address: u64,
    load_address: u64,
) -> RiscvCore {
    detailed_store_load_core_with_accesses(
        fetch_route,
        data_route,
        store_address,
        MemoryWidth::Word,
        0x2a,
        load_address,
        MemoryWidth::Word,
        false,
    )
}

fn detailed_store_load_core_with_accesses(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
    store_address: u64,
    store_width: MemoryWidth,
    store_value: u64,
    load_address: u64,
    load_width: MemoryWidth,
    load_signed: bool,
) -> RiscvCore {
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    let mut state = core.state.lock().expect("riscv core lock");
    state.hart.write(reg(2), store_address);
    state.hart.write(reg(4), load_address);
    state.events.extend([
        scalar_store_event_with_width_and_value(0x8000, 1, store_address, store_width, store_value),
        scalar_load_event_with_base_width(0x8004, 2, 6, 4, load_address, load_width, load_signed),
    ]);
    drop(state);
    core
}

fn defer_scalar_load(core: &RiscvCore, address: u64) -> MemoryRequestId {
    let event = scalar_load_event(0x8000, 1, 5, address);
    let fetch_request = event.fetch().request_id();
    let mut state = core.state.lock().expect("riscv core lock");
    state.events.push(event.clone());
    assert!(state.o3_runtime.defer_live_data_access_execution(&event));
    fetch_request
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
    scalar_load_event_with_base(pc, sequence, rd, 2, address)
}

fn scalar_load_event_with_base(
    pc: u64,
    sequence: u64,
    rd: u8,
    rs1: u8,
    address: u64,
) -> RiscvCpuExecutionEvent {
    scalar_load_event_with_base_width(pc, sequence, rd, rs1, address, MemoryWidth::Word, false)
}

fn scalar_load_event_with_base_width(
    pc: u64,
    sequence: u64,
    rd: u8,
    rs1: u8,
    address: u64,
    width: MemoryWidth,
    signed: bool,
) -> RiscvCpuExecutionEvent {
    let instruction = rem6_isa_riscv::RiscvInstruction::Load {
        rd: reg(rd),
        rs1: reg(rs1),
        offset: Immediate::new(0),
        width,
        signed,
    };
    let access = MemoryAccessKind::Load {
        rd: reg(rd),
        address,
        width,
        signed,
    };
    RiscvCpuExecutionEvent::new(
        fetch_event(pc, sequence),
        instruction,
        RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
    )
}

fn scalar_store_event_with_width_and_value(
    pc: u64,
    sequence: u64,
    address: u64,
    width: MemoryWidth,
    value: u64,
) -> RiscvCpuExecutionEvent {
    let instruction = rem6_isa_riscv::RiscvInstruction::Store {
        rs1: reg(2),
        rs2: reg(3),
        offset: Immediate::new(0),
        width,
    };
    let access = MemoryAccessKind::Store {
        address,
        width,
        value,
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

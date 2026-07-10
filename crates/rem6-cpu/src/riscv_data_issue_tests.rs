use rem6_isa_riscv::{Immediate, MemoryWidth, Register, RiscvExecutionRecord};
use rem6_kernel::PartitionedScheduler;
use rem6_memory::{AgentId, MemoryResponse};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

use super::*;
use crate::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuFetchEvent, CpuFetchRecord, CpuResetState,
    RiscvCpuExecutionEvent,
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
fn pending_data_request_blocks_second_issue_before_transport_submission() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    let load = |pc, sequence, rd, address| {
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
    };
    core.state
        .lock()
        .expect("riscv core lock")
        .events
        .extend([load(0x8000, 1, 5, 0x9000), load(0x8004, 2, 6, 0x9004)]);

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
            |_delivery, _context| panic!("blocked request must not reach the responder"),
        )
        .unwrap()
        .is_none());
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 1);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 1);
}

#[test]
fn completed_data_request_blocks_second_issue_until_o3_retirement() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    let load = |pc, sequence, rd, address| {
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
    };
    core.state
        .lock()
        .expect("riscv core lock")
        .events
        .extend([load(0x8000, 1, 5, 0x9000), load(0x8004, 2, 6, 0x9004)]);

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
    assert!(core.record_ready_o3_scalar_memory_event_with_trace(false));
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
    assert!(core.defer_o3_scalar_memory_execution(&event));

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

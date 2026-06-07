use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCore,
    RiscvDataAccessEventKind,
};
use rem6_isa_riscv::{MemoryAccessKind, Register};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_system::{
    traffic_trace_replay_runtime_data_target_outcome, TrafficTraceReplayTargetRuntime,
};
use rem6_traffic::{
    TrafficControllerEvent, TrafficControllerEventBatch, TrafficTraceErrorKind,
    TrafficTraceMemoryFailure, TrafficTraceReplayAction,
};
use rem6_transport::{MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, TargetOutcome};

mod support;

use support::traffic_trace::endpoint;

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn register(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn cpu_data_core(fetch_route: MemoryRouteId, data_route: MemoryRouteId) -> RiscvCore {
    let fetch = CpuCore::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(0),
            Address::new(0x8000),
        ),
        CpuFetchConfig::new(
            endpoint("cpu0.ifetch"),
            fetch_route,
            layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap();
    RiscvCore::with_data(
        fetch,
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, layout()),
    )
}

fn load_program_store() -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x3000).unwrap(),
        )
        .unwrap();
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(
            Address::new(0x9008),
            0x1122_3344_5566_7788_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

#[test]
fn traffic_trace_replay_runtime_data_target_outcome_records_cpu_data_failure() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
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
    let core = cpu_data_core(fetch_route, data_route);
    core.write_register(register(2), 0x9000);
    let store = load_program_store();

    let fetch_store = Arc::clone(&store);
    core.issue_next_fetch(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            let response = fetch_store
                .lock()
                .unwrap()
                .respond(delivery.request())
                .unwrap()
                .response()
                .cloned()
                .unwrap();
            TargetOutcome::Respond(response)
        },
    )
    .unwrap();
    scheduler.run_until_idle_conservative();
    let executed = core.execute_next_completed_fetch().unwrap().unwrap();
    assert!(matches!(
        executed.execution().memory_access(),
        Some(MemoryAccessKind::Load { .. })
    ));

    let runtime = Arc::new(Mutex::new(TrafficTraceReplayTargetRuntime::default()));
    let replay = Arc::clone(&runtime);
    let data_core = core.clone();
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, context| {
            let failure = TrafficTraceMemoryFailure::new(
                delivery.request().id(),
                TrafficTraceErrorKind::Read,
            );
            replay
                .lock()
                .unwrap()
                .record_batch(&TrafficControllerEventBatch::new(vec![
                    TrafficControllerEvent::TraceReplayAction(
                        TrafficTraceReplayAction::MemoryFailure { tick: 11, failure },
                    ),
                ]))
                .unwrap();
            traffic_trace_replay_runtime_data_target_outcome(
                Arc::clone(&replay),
                data_core,
                &delivery,
                context,
            )
            .unwrap()
        },
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(core.read_register(register(5)), 0);
    let data_events = core.data_access_events();
    assert_eq!(
        data_events
            .iter()
            .map(|event| event.kind())
            .collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Failed,
        ],
    );
    assert_eq!(data_events[1].tick(), 11);
    assert_eq!(data_events[1].physical_address(), Address::new(0x9008));
    assert_eq!(data_events[1].route(), Some(data_route));

    let runtime = runtime.lock().unwrap();
    assert!(runtime.is_empty());
    assert_eq!(runtime.memory_failures().len(), 1);
    assert_eq!(
        runtime.memory_failures()[0].record().failure().error(),
        TrafficTraceErrorKind::Read,
    );
}

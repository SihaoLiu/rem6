use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore,
    RiscvStoreConditionalProgressConfig,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{ParallelSchedulerContext, PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome,
    TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn atomic_type(funct5: u32, aq: bool, rl: bool, rs2: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (funct5 << 27)
        | (u32::from(aq) << 26)
        | (u32::from(rl) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x2f
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn data_routes() -> (
    PartitionedScheduler,
    MemoryTransport,
    MemoryRouteId,
    MemoryRouteId,
) {
    let scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(1),
                2,
                2,
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
                2,
            )
            .unwrap(),
        )
        .unwrap();
    (scheduler, transport, fetch_route, data_route)
}

fn data_core(fetch_route: MemoryRouteId, data_route: MemoryRouteId, entry: u64) -> RiscvCore {
    let core = CpuCore::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(7),
            Address::new(entry),
        ),
        CpuFetchConfig::new(
            endpoint("cpu0.ifetch"),
            fetch_route,
            layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap();
    RiscvCore::with_data_and_store_conditional_progress_config(
        core,
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, layout()),
        RiscvStoreConditionalProgressConfig::new(3).unwrap(),
    )
}

fn program_store(entry: u64, instructions: &[u32]) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap();

    let mut image = BootImage::new(Address::new(entry));
    for (index, instruction) in instructions.iter().enumerate() {
        image = image
            .add_segment(Address::new(entry + (index as u64 * 4)), word(*instruction))
            .unwrap();
    }
    image
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn fetch_responder(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome + Send + 'static
{
    move |delivery, _context| {
        let response = store
            .lock()
            .unwrap()
            .respond(delivery.request())
            .unwrap()
            .response()
            .cloned()
            .unwrap();
        TargetOutcome::Respond(response)
    }
}

#[test]
fn cluster_parallel_run_preserves_store_conditional_failure_diagnostics() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9008);
    core.write_register(reg(6), 0x1122_3344_5566_7788);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = program_store(
        0x8000,
        &[
            atomic_type(0x03, false, false, 6, 2, 0x3, 7),
            atomic_type(0x03, false, false, 6, 2, 0x3, 7),
            atomic_type(0x03, false, false, 6, 2, 0x3, 7),
        ],
    );

    let run = cluster
        .drive_until_parallel(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_| fetch_responder(store.clone()),
            |_| {
                |_delivery, _context| {
                    panic!("failed store conditional should not issue a memory transaction")
                }
            },
            64,
            |_| !core.store_conditional_failure_diagnostics().is_empty(),
        )
        .unwrap();

    let diagnostics = run.store_conditional_failure_diagnostics();
    assert_eq!(run.store_conditional_failure_diagnostic_count(), 1);
    assert_eq!(
        run.store_conditional_failure_diagnostic_count_for_cpu(CpuId::new(0)),
        1,
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].cpu(), CpuId::new(0));
    assert_eq!(diagnostics[0].address(), Address::new(0x9008));
    assert_eq!(diagnostics[0].size(), AccessSize::new(8).unwrap());
    assert_eq!(diagnostics[0].failure_count(), 3);
    assert_eq!(diagnostics[0].diagnostic_threshold(), 3);
}

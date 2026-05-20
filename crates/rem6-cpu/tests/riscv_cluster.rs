use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvClusterError,
    RiscvCore, RiscvCoreDriveAction,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerContext};
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

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

struct CoreSpec<'a> {
    cpu: u32,
    partition: u32,
    agent: u32,
    entry: u64,
    fetch_endpoint: &'a str,
    fetch_route: MemoryRouteId,
    data_endpoint: &'a str,
    data_route: MemoryRouteId,
}

fn riscv_core(spec: CoreSpec<'_>) -> RiscvCore {
    let core = CpuCore::new(
        CpuResetState::new(
            CpuId::new(spec.cpu),
            PartitionId::new(spec.partition),
            AgentId::new(spec.agent),
            Address::new(spec.entry),
        ),
        CpuFetchConfig::new(
            endpoint(spec.fetch_endpoint),
            spec.fetch_route,
            layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap();
    RiscvCore::with_data(
        core,
        CpuDataConfig::new(endpoint(spec.data_endpoint), spec.data_route, layout()),
    )
}

fn core_spec<'a>(
    cpu: u32,
    partition: u32,
    agent: u32,
    entry: u64,
    fetch_endpoint: &'a str,
    data_endpoint: &'a str,
) -> CoreSpec<'a> {
    CoreSpec {
        cpu,
        partition,
        agent,
        entry,
        fetch_endpoint,
        fetch_route: MemoryRouteId::new(u64::from(cpu) * 2),
        data_endpoint,
        data_route: MemoryRouteId::new(u64::from(cpu) * 2 + 1),
    }
}

fn store_with_programs(programs: &[(u64, u32)]) -> Arc<Mutex<PartitionedMemoryStore>> {
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

    let mut image = BootImage::new(Address::new(programs[0].0));
    for (entry, instruction) in programs {
        image = image
            .add_segment(Address::new(*entry), word(*instruction))
            .unwrap();
    }
    image
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn responder(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static {
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
fn riscv_cluster_rejects_duplicate_identities_and_endpoints() {
    assert_eq!(
        RiscvCluster::new([
            riscv_core(core_spec(0, 0, 7, 0x8000, "cpu0.ifetch", "cpu0.dmem")),
            riscv_core(core_spec(0, 1, 8, 0x9000, "cpu1.ifetch", "cpu1.dmem")),
        ])
        .unwrap_err(),
        RiscvClusterError::DuplicateCpu { cpu: CpuId::new(0) }
    );

    assert_eq!(
        RiscvCluster::new([
            riscv_core(core_spec(0, 0, 7, 0x8000, "cpu0.ifetch", "cpu0.dmem")),
            riscv_core(core_spec(1, 1, 7, 0x9000, "cpu1.ifetch", "cpu1.dmem")),
        ])
        .unwrap_err(),
        RiscvClusterError::DuplicateAgent {
            agent: AgentId::new(7),
            existing: CpuId::new(0),
            duplicate: CpuId::new(1),
        }
    );

    assert_eq!(
        RiscvCluster::new([
            riscv_core(core_spec(0, 0, 7, 0x8000, "cpu.ifetch", "cpu0.dmem")),
            riscv_core(core_spec(1, 1, 8, 0x9000, "cpu.ifetch", "cpu1.dmem")),
        ])
        .unwrap_err(),
        RiscvClusterError::DuplicateFetchEndpoint {
            endpoint: endpoint("cpu.ifetch"),
            existing: CpuId::new(0),
            duplicate: CpuId::new(1),
        }
    );

    assert_eq!(
        RiscvCluster::new([
            riscv_core(core_spec(0, 0, 7, 0x8000, "cpu0.ifetch", "cpu.dmem")),
            riscv_core(core_spec(1, 1, 8, 0x9000, "cpu1.ifetch", "cpu.dmem")),
        ])
        .unwrap_err(),
        RiscvClusterError::DuplicateDataEndpoint {
            endpoint: endpoint("cpu.dmem"),
            existing: CpuId::new(0),
            duplicate: CpuId::new(1),
        }
    );
}

#[test]
fn riscv_cluster_drives_distinct_cores_without_hidden_scheduler_runs() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_fetch = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu0_data = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.dmem"),
                PartitionId::new(0),
                endpoint("l1d0"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu1_fetch = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i1"),
                PartitionId::new(3),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu1_data = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.dmem"),
                PartitionId::new(1),
                endpoint("l1d1"),
                PartitionId::new(3),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cluster = RiscvCluster::new([
        riscv_core(CoreSpec {
            cpu: 0,
            partition: 0,
            agent: 7,
            entry: 0x8000,
            fetch_endpoint: "cpu0.ifetch",
            fetch_route: cpu0_fetch,
            data_endpoint: "cpu0.dmem",
            data_route: cpu0_data,
        }),
        riscv_core(CoreSpec {
            cpu: 1,
            partition: 1,
            agent: 8,
            entry: 0x9000,
            fetch_endpoint: "cpu1.ifetch",
            fetch_route: cpu1_fetch,
            data_endpoint: "cpu1.dmem",
            data_route: cpu1_data,
        }),
    ])
    .unwrap();
    let store = store_with_programs(&[
        (0x8000, i_type(11, 0, 0x0, 1, 0x13)),
        (0x9000, i_type(22, 0, 0x0, 1, 0x13)),
    ]);

    assert!(matches!(
        cluster
            .drive_core_next_action(
                CpuId::new(0),
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                responder(store.clone()),
                responder(store.clone()),
            )
            .unwrap(),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert!(matches!(
        cluster
            .drive_core_next_action(
                CpuId::new(1),
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                responder(store.clone()),
                responder(store.clone()),
            )
            .unwrap(),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        cluster
            .drive_core_next_action(
                CpuId::new(0),
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                responder(store.clone()),
                responder(store.clone()),
            )
            .unwrap(),
        None
    );

    scheduler.run_until_idle_conservative();

    assert!(matches!(
        cluster
            .drive_core_next_action(
                CpuId::new(0),
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                responder(store.clone()),
                responder(store.clone()),
            )
            .unwrap(),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        cluster
            .drive_core_next_action(
                CpuId::new(1),
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                responder(store.clone()),
                responder(store),
            )
            .unwrap(),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(1)),
        11
    );
    assert_eq!(
        cluster.core(CpuId::new(1)).unwrap().read_register(reg(1)),
        22
    );
    assert_eq!(
        cluster
            .drive_core_next_action(
                CpuId::new(9),
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                responder(Arc::new(Mutex::new(PartitionedMemoryStore::new()))),
                responder(Arc::new(Mutex::new(PartitionedMemoryStore::new()))),
            )
            .unwrap_err(),
        RiscvClusterError::UnknownCpu { cpu: CpuId::new(9) }
    );
}

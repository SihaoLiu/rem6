use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster,
    RiscvClusterDriveEvent, RiscvClusterError, RiscvClusterRun, RiscvClusterStopReason,
    RiscvClusterTurn, RiscvCore, RiscvCoreDriveAction, RiscvDataAccessEventKind,
    RiscvDataAccessTarget,
};
use rem6_fabric::{FabricLinkId, FabricModel, FabricPath, FabricPathHop};
use rem6_isa_riscv::{Register, RiscvInstruction};
use rem6_kernel::{
    ParallelRunProfile, PartitionId, PartitionedScheduler, ScheduledEventKind, SchedulerContext,
};
use rem6_memory::{
    AccessSize, Address, AddressRange, AgentId, CacheLineLayout, MemoryRequestId, MemoryTargetId,
    PartitionedMemoryStore,
};
use rem6_mmio::{MmioAccess, MmioBus, MmioRegisterBank, MmioRoute};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery,
    TargetOutcome, TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn fabric_path(name: &str, latency: u64, bandwidth_bytes_per_tick: u64) -> FabricPath {
    FabricPath::new([FabricPathHop::new(
        FabricLinkId::new(name).unwrap(),
        latency,
        bandwidth_bytes_per_tick,
    )
    .unwrap()])
    .unwrap()
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

fn b_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = imm as u32;
    (((imm >> 12) & 0x1) << 31)
        | (((imm >> 5) & 0x3f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (((imm >> 1) & 0xf) << 8)
        | (((imm >> 11) & 0x1) << 7)
        | 0x63
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

fn store_with_programs_and_data(
    programs: &[(u64, u32)],
    data_segments: &[(u64, Vec<u8>)],
) -> Arc<Mutex<PartitionedMemoryStore>> {
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
    for (address, data) in data_segments {
        image = image
            .add_segment(Address::new(*address), data.clone())
            .unwrap();
    }
    image
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn store_with_programs(programs: &[(u64, u32)]) -> Arc<Mutex<PartitionedMemoryStore>> {
    store_with_programs_and_data(programs, &[])
}

fn store_with_raw_program(entry: u64, bytes: Vec<u8>) -> Arc<Mutex<PartitionedMemoryStore>> {
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
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), bytes)
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn memory_response(
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
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

fn responder(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static {
    move |delivery, _context| memory_response(&store, &delivery)
}

fn drive_parallel_fetch_until_non_pipeline_action(
    cluster: &RiscvCluster,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> Vec<RiscvClusterDriveEvent> {
    for _ in 0..16 {
        let actions = cluster
            .drive_ready_cores_parallel_fetch(scheduler, transport, MemoryTrace::new(), |_cpu| {
                let store = store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            })
            .unwrap();
        if actions.iter().any(|event| {
            !matches!(
                event.action(),
                RiscvCoreDriveAction::PipelineCycleScheduled { .. }
            )
        }) {
            return actions;
        }
        assert!(
            !scheduler.is_idle(),
            "pipeline action should schedule a wake"
        );
        scheduler.run_until_idle_parallel().unwrap();
    }
    panic!("expected a non-pipeline cluster action");
}

fn drive_core_until_non_pipeline_action(
    cluster: &RiscvCluster,
    cpu: CpuId,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> Option<RiscvCoreDriveAction> {
    for _ in 0..16 {
        let action = cluster
            .drive_core_next_action(
                cpu,
                scheduler,
                transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                responder(store.clone()),
                responder(store.clone()),
            )
            .unwrap();
        if !matches!(
            action,
            Some(RiscvCoreDriveAction::PipelineCycleScheduled { .. })
        ) {
            return action;
        }
        scheduler.run_until_idle_conservative();
    }
    panic!("expected a non-pipeline core action");
}

fn drive_ready_cores_until_non_pipeline_action(
    cluster: &RiscvCluster,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> Vec<RiscvClusterDriveEvent> {
    for _ in 0..16 {
        let actions = cluster
            .drive_ready_cores(
                scheduler,
                transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                |_cpu| responder(store.clone()),
                |_cpu| responder(store.clone()),
            )
            .unwrap();
        if actions.iter().any(|event| {
            !matches!(
                event.action(),
                RiscvCoreDriveAction::PipelineCycleScheduled { .. }
            )
        }) {
            return actions;
        }
        assert!(
            !scheduler.is_idle(),
            "pipeline action should schedule a wake"
        );
        scheduler.run_until_idle_conservative();
    }
    panic!("expected a non-pipeline ready-core action");
}

fn drive_parallel_mmio_until_non_pipeline_action(
    cluster: &RiscvCluster,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    bus: &MmioBus,
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> RiscvClusterTurn {
    for _ in 0..24 {
        let turn = cluster
            .drive_turn_parallel_with_mmio(
                scheduler,
                transport,
                bus,
                MemoryTrace::new(),
                MemoryTrace::new(),
                |_cpu| {
                    let store = store.clone();
                    move |delivery, _context| memory_response(&store, &delivery)
                },
                |_cpu| {
                    let store = store.clone();
                    move |delivery, _context| memory_response(&store, &delivery)
                },
            )
            .unwrap();
        if turn.core_events().iter().any(|event| {
            !matches!(
                event.action(),
                RiscvCoreDriveAction::PipelineCycleScheduled { .. }
            )
        }) {
            return turn;
        }
    }
    panic!("expected a non-pipeline MMIO cluster action");
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
        drive_core_until_non_pipeline_action(
            &cluster,
            CpuId::new(0),
            &mut scheduler,
            &transport,
            store.clone(),
        ),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert!(matches!(
        drive_core_until_non_pipeline_action(
            &cluster,
            CpuId::new(1),
            &mut scheduler,
            &transport,
            store.clone(),
        ),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(1)),
        0
    );
    assert_eq!(
        cluster.core(CpuId::new(1)).unwrap().read_register(reg(1)),
        0
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

#[test]
fn riscv_cluster_drives_ready_cores_in_cpu_order() {
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
            cpu: 1,
            partition: 1,
            agent: 8,
            entry: 0x9000,
            fetch_endpoint: "cpu1.ifetch",
            fetch_route: cpu1_fetch,
            data_endpoint: "cpu1.dmem",
            data_route: cpu1_data,
        }),
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
    ])
    .unwrap();
    let store = store_with_programs(&[
        (0x8000, i_type(11, 0, 0x0, 1, 0x13)),
        (0x9000, i_type(22, 0, 0x0, 1, 0x13)),
    ]);

    let issued = cluster
        .drive_ready_cores(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(store.clone()),
            |_cpu| responder(store.clone()),
        )
        .unwrap();
    assert_eq!(
        issued
            .iter()
            .map(RiscvClusterDriveEvent::cpu)
            .collect::<Vec<_>>(),
        vec![CpuId::new(0), CpuId::new(1)]
    );
    assert!(issued
        .iter()
        .all(|event| matches!(event.action(), RiscvCoreDriveAction::FetchIssued { .. })));
    assert!(cluster
        .drive_ready_cores(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(store.clone()),
            |_cpu| responder(store.clone()),
        )
        .unwrap()
        .is_empty());

    scheduler.run_until_idle_conservative();

    let executed = drive_ready_cores_until_non_pipeline_action(
        &cluster,
        &mut scheduler,
        &transport,
        store.clone(),
    );
    assert_eq!(
        executed
            .iter()
            .map(RiscvClusterDriveEvent::cpu)
            .collect::<Vec<_>>(),
        vec![CpuId::new(0), CpuId::new(1)]
    );
    assert!(executed
        .iter()
        .all(|event| matches!(event.action(), RiscvCoreDriveAction::FetchIssued { .. })));
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(1)),
        0
    );
    assert_eq!(
        cluster.core(CpuId::new(1)).unwrap().read_register(reg(1)),
        0
    );

    scheduler.run_until_idle_conservative();

    let executed = cluster
        .drive_ready_cores(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(store.clone()),
            |_cpu| responder(store.clone()),
        )
        .unwrap();
    assert_eq!(
        executed
            .iter()
            .map(RiscvClusterDriveEvent::cpu)
            .collect::<Vec<_>>(),
        vec![CpuId::new(0), CpuId::new(1)]
    );
    assert!(executed
        .iter()
        .all(|event| matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_))));
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(1)),
        11
    );
    assert_eq!(
        cluster.core(CpuId::new(1)).unwrap().read_register(reg(1)),
        22
    );
}

#[test]
fn riscv_cluster_turns_drive_cores_before_scheduler_epochs() {
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
        (0x8000, i_type(31, 0, 0x0, 1, 0x13)),
        (0x9000, i_type(41, 0, 0x0, 1, 0x13)),
    ]);

    let first = cluster
        .drive_turn(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(store.clone()),
            |_cpu| responder(store.clone()),
        )
        .unwrap();
    assert_eq!(first.scheduler_summary(), None);
    assert_eq!(first.idle_tick(), None);
    assert_eq!(
        first
            .core_events()
            .iter()
            .map(RiscvClusterDriveEvent::cpu)
            .collect::<Vec<_>>(),
        vec![CpuId::new(0), CpuId::new(1)]
    );
    assert!(first
        .core_events()
        .iter()
        .all(|event| matches!(event.action(), RiscvCoreDriveAction::FetchIssued { .. })));
    assert_eq!(scheduler.now(), 0);

    let mut turns = vec![first];
    let executed = loop {
        assert!(turns.len() < 24);
        let turn = cluster
            .drive_turn(
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                |_cpu| responder(store.clone()),
                |_cpu| responder(store.clone()),
            )
            .unwrap();
        if turn
            .core_events()
            .iter()
            .any(|event| matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_)))
        {
            break turn;
        }
        turns.push(turn);
    };

    let scheduler_summaries = turns
        .iter()
        .filter_map(RiscvClusterTurn::scheduler_summary)
        .collect::<Vec<_>>();
    assert!(scheduler_summaries
        .iter()
        .any(|summary| summary.executed_events() > 0));
    assert_eq!(executed.scheduler_summary(), None);
    assert_eq!(
        executed
            .core_events()
            .iter()
            .map(RiscvClusterDriveEvent::cpu)
            .collect::<Vec<_>>(),
        vec![CpuId::new(0), CpuId::new(1)]
    );
    assert!(executed
        .core_events()
        .iter()
        .all(|event| matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_))));
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(1)),
        31
    );
    assert_eq!(
        cluster.core(CpuId::new(1)).unwrap().read_register(reg(1)),
        41
    );
}

#[test]
fn riscv_cluster_parallel_fetch_turns_drive_scheduler_epochs() {
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
        (0x8000, i_type(71, 0, 0x0, 1, 0x13)),
        (0x9000, i_type(81, 0, 0x0, 1, 0x13)),
    ]);

    let first = cluster
        .drive_turn_parallel_fetch(&mut scheduler, &transport, MemoryTrace::new(), |_cpu| {
            let store = store.clone();
            move |delivery, _context| memory_response(&store, &delivery)
        })
        .unwrap();
    assert_eq!(first.scheduler_summary(), None);
    assert_eq!(
        first
            .core_events()
            .iter()
            .map(RiscvClusterDriveEvent::cpu)
            .collect::<Vec<_>>(),
        vec![CpuId::new(0), CpuId::new(1)]
    );
    assert!(first
        .core_events()
        .iter()
        .all(|event| matches!(event.action(), RiscvCoreDriveAction::FetchIssued { .. })));
    assert_eq!(scheduler.now(), 0);

    let mut turns = vec![first];
    let executed = loop {
        assert!(turns.len() < 24);
        let turn = cluster
            .drive_turn_parallel_fetch(&mut scheduler, &transport, MemoryTrace::new(), |_cpu| {
                let store = store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            })
            .unwrap();
        if turn
            .core_events()
            .iter()
            .any(|event| matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_)))
        {
            break turn;
        }
        turns.push(turn);
    };

    let scheduler_summaries = turns
        .iter()
        .filter_map(RiscvClusterTurn::scheduler_summary)
        .collect::<Vec<_>>();
    assert!(scheduler_summaries
        .iter()
        .any(|summary| summary.executed_events() > 0));
    let parallel_epochs = turns
        .iter()
        .filter_map(RiscvClusterTurn::parallel_scheduler_epoch)
        .collect::<Vec<_>>();
    assert!(!parallel_epochs.is_empty());
    assert!(parallel_epochs
        .iter()
        .all(|epoch| epoch.plan().is_parallel_safe()));
    assert!(parallel_epochs
        .iter()
        .all(|epoch| epoch.plan().frontier_count() == scheduler.partition_count() as usize));
    assert!(parallel_epochs.iter().any(|epoch| epoch
        .dispatches()
        .iter()
        .any(|record| record.kind() == ScheduledEventKind::Parallel)));
    for epoch in &parallel_epochs {
        assert_eq!(epoch.horizon(), epoch.plan().horizon());
        assert_eq!(epoch.batch_count(), epoch.batches().len());
        assert_eq!(
            epoch.total_parallel_workers(),
            epoch
                .batches()
                .iter()
                .map(|batch| batch.worker_count())
                .sum::<usize>()
        );
        assert_eq!(
            epoch.dispatches().len(),
            epoch
                .batches()
                .iter()
                .map(|batch| batch.dispatches().len())
                .sum::<usize>()
        );
        if epoch.dispatches().is_empty() {
            assert_eq!(epoch.max_parallel_workers(), 0);
            assert_eq!(epoch.batch_count(), 0);
            assert!(!epoch.has_parallel_work());
            assert!(epoch.parallel_worker_partitions().is_empty());
        } else {
            assert!(epoch.max_parallel_workers() >= 1);
            assert!(epoch.has_parallel_work());
            assert!(!epoch.parallel_worker_partitions().is_empty());
        }
        assert_eq!(
            epoch.parallel_worker_partitions().len(),
            epoch.total_parallel_workers()
        );
        assert_eq!(
            epoch.ready_partition_count(),
            epoch.plan().ready_partition_count()
        );
        assert_eq!(epoch.ready_partitions(), epoch.plan().ready_partitions());
        assert_eq!(epoch.frontiers(), epoch.plan().frontiers());
        assert_eq!(epoch.initial_frontiers(), epoch.plan().frontiers());
        assert_eq!(epoch.final_frontier_count(), epoch.plan().frontier_count());
        assert!(epoch
            .final_frontiers()
            .iter()
            .all(|frontier| frontier.now() == epoch.summary().final_tick()));
        assert_eq!(epoch.serial_blockers(), epoch.plan().serial_blockers());
        assert!(epoch.serial_blockers().is_empty());
        assert_eq!(
            epoch.frontier(PartitionId::new(0)),
            epoch.plan().frontier(PartitionId::new(0))
        );
        assert_eq!(
            epoch.initial_frontier(PartitionId::new(0)),
            epoch.plan().frontier(PartitionId::new(0))
        );
        assert!(epoch.final_frontier(PartitionId::new(0)).is_some());
        assert!(epoch
            .parallel_dispatches()
            .iter()
            .all(|record| record.kind() == ScheduledEventKind::Parallel));
        assert!(epoch
            .dispatches_for_partition(PartitionId::new(0))
            .iter()
            .all(|record| record.partition() == PartitionId::new(0)));
        assert_eq!(epoch.summary().final_tick(), epoch.plan().horizon());
        assert_eq!(Some(epoch.summary()), epoch.turn_summary());
        assert!(epoch
            .dispatches()
            .iter()
            .all(|record| record.tick() <= epoch.plan().horizon()));
    }
    assert!(parallel_epochs
        .iter()
        .any(|epoch| epoch.max_parallel_workers() >= 1));
    assert_eq!(
        executed
            .core_events()
            .iter()
            .map(RiscvClusterDriveEvent::cpu)
            .collect::<Vec<_>>(),
        vec![CpuId::new(0), CpuId::new(1)]
    );
    assert!(executed
        .core_events()
        .iter()
        .all(|event| matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_))));
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(1)),
        71
    );
    assert_eq!(
        cluster.core(CpuId::new(1)).unwrap().read_register(reg(1)),
        81
    );
}

#[test]
fn riscv_cluster_run_collects_parallel_epoch_records() {
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
    let cluster = RiscvCluster::new([riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route: cpu0_fetch,
        data_endpoint: "cpu0.dmem",
        data_route: cpu0_data,
    })])
    .unwrap();
    let store = store_with_programs(&[(0x8000, i_type(91, 0, 0x0, 1, 0x13))]);

    let run = cluster
        .drive_until(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(store.clone()),
            |_cpu| responder(store.clone()),
            24,
            |turn| {
                turn.core_events().iter().any(|event| {
                    matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_))
                })
            },
        )
        .unwrap();

    assert!(run.parallel_scheduler_epochs().is_empty());
    assert!(run.parallel_scheduler_dispatches().is_empty());

    let parallel_cluster = RiscvCluster::new([riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route: cpu0_fetch,
        data_endpoint: "cpu0.dmem",
        data_route: cpu0_data,
    })])
    .unwrap();
    let parallel_store = store_with_programs(&[(0x8000, i_type(91, 0, 0x0, 1, 0x13))]);
    let mut parallel_scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let run = parallel_cluster
        .drive_until_parallel(
            &mut parallel_scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| {
                let store = parallel_store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                let store = parallel_store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            },
            24,
            |turn| {
                turn.core_events().iter().any(|event| {
                    matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_))
                })
            },
        )
        .unwrap();

    let epochs = run.parallel_scheduler_epochs();
    assert!(!epochs.is_empty());
    assert_eq!(
        run.parallel_scheduler_dispatches().len(),
        epochs
            .iter()
            .map(|epoch| epoch.dispatches().len())
            .sum::<usize>()
    );
    assert_eq!(
        run.parallel_scheduler_batches().len(),
        epochs
            .iter()
            .map(|epoch| epoch.batches().len())
            .sum::<usize>()
    );
    assert_eq!(
        run.parallel_scheduler_workers().len(),
        epochs
            .iter()
            .map(|epoch| epoch.total_parallel_workers())
            .sum::<usize>()
    );
    assert_eq!(
        run.parallel_scheduler_profile(),
        ParallelRunProfile::new(
            epochs.len(),
            epochs
                .iter()
                .filter(|epoch| epoch.dispatches().is_empty())
                .count(),
            run.parallel_scheduler_batches().len(),
            run.parallel_scheduler_dispatches().len(),
            run.parallel_scheduler_workers().len(),
            run.max_parallel_scheduler_workers(),
        )
    );
    assert_eq!(
        epochs
            .iter()
            .map(|epoch| epoch.dispatch_count())
            .sum::<usize>(),
        run.parallel_scheduler_profile().dispatch_count()
    );
    assert_eq!(
        epochs
            .iter()
            .map(|epoch| epoch.empty_epoch_count())
            .sum::<usize>(),
        run.parallel_scheduler_profile().empty_epoch_count()
    );
    assert_eq!(
        epochs.iter().filter(|epoch| epoch.is_empty_epoch()).count(),
        run.parallel_scheduler_profile().empty_epoch_count()
    );
    assert!(epochs
        .iter()
        .all(|epoch| epoch.profile().epoch_count() == 1));
    assert_eq!(
        run.parallel_scheduler_worker_partitions().len(),
        run.parallel_scheduler_workers().len()
    );
    assert!(run.max_parallel_scheduler_workers() >= 1);
    assert_eq!(
        run.parallel_scheduler_frontiers().len(),
        epochs
            .iter()
            .map(|epoch| epoch.frontiers().len())
            .sum::<usize>()
    );
    assert_eq!(
        run.parallel_scheduler_final_frontiers().len(),
        epochs
            .iter()
            .map(|epoch| epoch.final_frontiers().len())
            .sum::<usize>()
    );
    assert_eq!(
        run.parallel_scheduler_ready_partitions().len(),
        epochs
            .iter()
            .map(|epoch| epoch.ready_partitions().len())
            .sum::<usize>()
    );
    assert!(run
        .parallel_scheduler_dispatches_for_partition(PartitionId::new(0))
        .iter()
        .all(|record| record.partition() == PartitionId::new(0)));
    assert!(epochs
        .iter()
        .all(|epoch| epoch.plan().frontier(PartitionId::new(0)).is_some()));
}

#[test]
fn riscv_cluster_parallel_run_respects_scheduler_worker_limit() {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 2, 1).unwrap();
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
        (0x8000, i_type(61, 0, 0x0, 1, 0x13)),
        (0x9000, i_type(62, 0, 0x0, 1, 0x13)),
    ]);

    let run = cluster
        .drive_until_parallel(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| {
                let store = store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                let store = store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            },
            20,
            |turn| {
                turn.core_events().iter().any(|event| {
                    matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_))
                })
            },
        )
        .unwrap();

    assert_eq!(run.max_parallel_scheduler_workers(), 1);
    assert_eq!(run.parallel_scheduler_profile().max_parallel_workers(), 1);
    assert_eq!(
        run.parallel_scheduler_profile().batch_count(),
        run.parallel_scheduler_batches().len()
    );
    assert!(run
        .parallel_scheduler_batches()
        .iter()
        .all(|batch| batch.worker_count() <= 1));
    assert!(run.parallel_scheduler_batches().len() >= 2);
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(1)),
        61
    );
    assert_eq!(
        cluster.core(CpuId::new(1)).unwrap().read_register(reg(1)),
        62
    );
}

#[test]
fn riscv_cluster_parallel_fetch_batches_shared_fabric_by_packet_order() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let mut transport = MemoryTransport::with_fabric(FabricModel::new());
    let shared_path = fabric_path("fetch_mesh", 2, 4);
    let cpu1_fetch = transport
        .add_route(
            MemoryRoute::new_path(
                endpoint("cpu1.ifetch"),
                PartitionId::new(0),
                [
                    MemoryRouteHop::new(endpoint("memory0"), PartitionId::new(1), 2, 2)
                        .unwrap()
                        .with_request_fabric_path(shared_path.clone()),
                ],
            )
            .unwrap(),
        )
        .unwrap();
    let cpu0_fetch = transport
        .add_route(
            MemoryRoute::new_path(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                [
                    MemoryRouteHop::new(endpoint("memory0"), PartitionId::new(1), 2, 2)
                        .unwrap()
                        .with_request_fabric_path(shared_path),
                ],
            )
            .unwrap(),
        )
        .unwrap();
    let cpu0_data = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.dmem"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu1_data = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.dmem"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                2,
                2,
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
            partition: 0,
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
    let deliveries = Arc::new(Mutex::new(Vec::new()));

    let issued = cluster
        .drive_ready_cores_parallel_fetch(&mut scheduler, &transport, MemoryTrace::new(), |_cpu| {
            let store = store.clone();
            let deliveries = Arc::clone(&deliveries);
            move |delivery, _context| {
                deliveries.lock().unwrap().push((
                    delivery.route(),
                    delivery.tick(),
                    delivery.request().id(),
                ));
                memory_response(&store, &delivery)
            }
        })
        .unwrap();

    assert_eq!(
        issued
            .iter()
            .map(RiscvClusterDriveEvent::cpu)
            .collect::<Vec<_>>(),
        vec![CpuId::new(0), CpuId::new(1)],
    );
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        *deliveries.lock().unwrap(),
        vec![
            (cpu1_fetch, 3, MemoryRequestId::new(AgentId::new(8), 0),),
            (cpu0_fetch, 4, MemoryRequestId::new(AgentId::new(7), 0),),
        ],
    );
}

#[test]
fn riscv_cluster_parallel_turns_issue_mmio_and_memory_data_accesses() {
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
            entry: 0x8100,
            fetch_endpoint: "cpu1.ifetch",
            fetch_route: cpu1_fetch,
            data_endpoint: "cpu1.dmem",
            data_route: cpu1_data,
        }),
    ])
    .unwrap();
    cluster
        .core(CpuId::new(0))
        .unwrap()
        .write_register(reg(2), 0x1000);
    cluster
        .core(CpuId::new(1))
        .unwrap()
        .write_register(reg(2), 0x9000);
    let store = store_with_programs_and_data(
        &[
            (0x8000, i_type(8, 2, 0x3, 5, 0x03)),
            (0x8100, i_type(8, 2, 0x3, 5, 0x03)),
        ],
        &[(0x9008, vec![0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f])],
    );
    let mut bank =
        MmioRegisterBank::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        8,
        AccessSize::new(8).unwrap(),
        MmioAccess::ReadOnly,
        vec![0x21, 0x43, 0x65, 0x87, 0xa9, 0xcb, 0xed, 0x0f],
    )
    .unwrap();
    let mut bus = MmioBus::new();
    let mmio_route = MmioRoute::new(PartitionId::new(0), PartitionId::new(2), 2, 2).unwrap();
    bus.insert_device(
        AddressRange::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap(),
        mmio_route,
        Mutex::new(bank),
    )
    .unwrap();

    let mut turns = Vec::new();
    for _ in 0..32 {
        let turn = cluster
            .drive_turn_parallel_with_mmio(
                &mut scheduler,
                &transport,
                &bus,
                MemoryTrace::new(),
                MemoryTrace::new(),
                |_cpu| {
                    let store = store.clone();
                    move |delivery, _context| memory_response(&store, &delivery)
                },
                |_cpu| {
                    let store = store.clone();
                    move |delivery, _context| memory_response(&store, &delivery)
                },
            )
            .unwrap();
        let loaded = cluster.core(CpuId::new(0)).unwrap().read_register(reg(5))
            == 0x0fed_cba9_8765_4321
            && cluster.core(CpuId::new(1)).unwrap().read_register(reg(5)) == 0x0f1e_2d3c_4b5a_6978;
        turns.push(turn);
        if loaded {
            break;
        }
    }

    let data_turn = turns
        .iter()
        .find(|turn| {
            turn.core_events().iter().all(|event| {
                matches!(
                    event.action(),
                    RiscvCoreDriveAction::DataAccessIssued { .. }
                )
            }) && !turn.core_events().is_empty()
        })
        .expect("parallel data access issue turn");
    assert_eq!(
        data_turn
            .core_events()
            .iter()
            .map(RiscvClusterDriveEvent::cpu)
            .collect::<Vec<_>>(),
        vec![CpuId::new(0), CpuId::new(1)]
    );
    assert!(turns
        .iter()
        .filter_map(RiscvClusterTurn::scheduler_summary)
        .any(|summary| summary.executed_events() > 0));

    let cpu0_events = cluster.core(CpuId::new(0)).unwrap().data_access_events();
    let cpu1_events = cluster.core(CpuId::new(1)).unwrap().data_access_events();
    assert_eq!(
        cpu0_events
            .iter()
            .map(|event| event.kind())
            .collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
        ]
    );
    assert_eq!(
        cpu1_events
            .iter()
            .map(|event| event.kind())
            .collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
        ]
    );
    assert_eq!(
        cpu0_events[0].target(),
        RiscvDataAccessTarget::Mmio { route: mmio_route }
    );
    assert!(matches!(
        cpu1_events[0].target(),
        RiscvDataAccessTarget::Memory { route, .. } if route == cpu1_data
    ));
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(5)),
        0x0fed_cba9_8765_4321
    );
    assert_eq!(
        cluster.core(CpuId::new(1)).unwrap().read_register(reg(5)),
        0x0f1e_2d3c_4b5a_6978
    );
}

#[test]
fn zero_instruction_budget_drains_existing_mmio_work_without_retiring_next_instruction() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
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
    let data_route = transport
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
    let cluster = RiscvCluster::new([riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route,
        data_endpoint: "cpu0.dmem",
        data_route,
    })])
    .unwrap();
    let cpu = cluster.core(CpuId::new(0)).unwrap();
    cpu.write_register(reg(2), 0x1000);
    let store = store_with_programs(&[
        (0x8000, i_type(8, 2, 0x3, 5, 0x03)),
        (0x8004, i_type(1, 0, 0x0, 7, 0x13)),
    ]);
    let mut bank =
        MmioRegisterBank::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        8,
        AccessSize::new(8).unwrap(),
        MmioAccess::ReadOnly,
        vec![0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    )
    .unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        AddressRange::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap(),
        MmioRoute::new(PartitionId::new(0), PartitionId::new(2), 2, 2).unwrap(),
        Mutex::new(bank),
    )
    .unwrap();

    let mut retired_load = false;
    for _ in 0..16 {
        let turn = cluster
            .drive_turn_parallel_with_mmio_and_instruction_budget_until_tick(
                &mut scheduler,
                &transport,
                &bus,
                MemoryTrace::new(),
                MemoryTrace::new(),
                |_cpu| {
                    let store = store.clone();
                    move |delivery, _context| memory_response(&store, &delivery)
                },
                |_cpu| {
                    let store = store.clone();
                    move |delivery, _context| memory_response(&store, &delivery)
                },
                1,
                100,
            )
            .unwrap()
            .expect("load setup turn");
        if turn
            .core_events()
            .iter()
            .any(|event| matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_)))
        {
            retired_load = true;
            break;
        }
    }
    assert!(retired_load);
    assert_eq!(cpu.read_register(reg(5)), 0);
    assert_eq!(cpu.read_register(reg(7)), 0);
    assert!(cpu.has_unissued_data_access() || cpu.has_pending_data_access());

    let mut drain_turns = Vec::new();
    for _ in 0..8 {
        if !(cpu.has_unissued_data_access() || cpu.has_pending_data_access()) {
            break;
        }
        let turn = cluster
            .drive_turn_parallel_with_mmio_and_instruction_budget_until_tick(
                &mut scheduler,
                &transport,
                &bus,
                MemoryTrace::new(),
                MemoryTrace::new(),
                |_cpu| {
                    let store = store.clone();
                    move |delivery, _context| memory_response(&store, &delivery)
                },
                |_cpu| {
                    let store = store.clone();
                    move |delivery, _context| memory_response(&store, &delivery)
                },
                0,
                100,
            )
            .unwrap()
            .expect("MMIO data drain turn");
        drain_turns.push(turn);
    }

    assert!(!drain_turns.is_empty());
    assert!(drain_turns.iter().all(|turn| {
        turn.core_events()
            .iter()
            .all(|event| !matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_)))
    }));
    assert_eq!(cpu.read_register(reg(5)), 0x0123_4567_89ab_cdef);
    assert_eq!(cpu.read_register(reg(7)), 0);

    let mut retired_addi = false;
    for _ in 0..16 {
        let turn = cluster
            .drive_turn_parallel_with_mmio_and_instruction_budget_until_tick(
                &mut scheduler,
                &transport,
                &bus,
                MemoryTrace::new(),
                MemoryTrace::new(),
                |_cpu| {
                    let store = store.clone();
                    move |delivery, _context| memory_response(&store, &delivery)
                },
                |_cpu| {
                    let store = store.clone();
                    move |delivery, _context| memory_response(&store, &delivery)
                },
                1,
                100,
            )
            .unwrap()
            .expect("addi turn");
        if turn
            .core_events()
            .iter()
            .any(|event| matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_)))
        {
            retired_addi = true;
            break;
        }
    }
    assert!(retired_addi);
    assert_eq!(cpu.read_register(reg(7)), 1);
}

#[test]
fn riscv_cluster_parallel_fetch_commits_branch_fetch_ahead_speculation() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("memory0"),
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
                endpoint("memory0"),
                PartitionId::new(1),
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let cluster = RiscvCluster::new([riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route,
        data_endpoint: "cpu0.dmem",
        data_route,
    })])
    .unwrap();
    let branch = b_type(8, 0, 0, 0x0);
    let store = store_with_programs(&[
        (0x8000, branch),
        (0x8004, i_type(1, 0, 0x0, 1, 0x13)),
        (0x8008, i_type(2, 0, 0x0, 2, 0x13)),
    ]);

    let issued = cluster
        .drive_ready_cores_parallel_fetch(&mut scheduler, &transport, MemoryTrace::new(), |_cpu| {
            let store = store.clone();
            move |delivery, _context| memory_response(&store, &delivery)
        })
        .unwrap();
    assert!(matches!(
        issued.first().map(RiscvClusterDriveEvent::action),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_parallel().unwrap();

    let fetch_ahead = drive_parallel_fetch_until_non_pipeline_action(
        &cluster,
        &mut scheduler,
        &transport,
        store.clone(),
    );
    assert!(matches!(
        fetch_ahead.first().map(RiscvClusterDriveEvent::action),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        cluster
            .core(CpuId::new(0))
            .unwrap()
            .branch_predictor_snapshot()
            .pending_speculations()
            .len(),
        1
    );
    scheduler.run_until_idle_parallel().unwrap();

    let retired = cluster
        .drive_ready_cores_parallel_fetch(&mut scheduler, &transport, MemoryTrace::new(), |_cpu| {
            let store = store.clone();
            move |delivery, _context| memory_response(&store, &delivery)
        })
        .unwrap();
    let Some(RiscvCoreDriveAction::InstructionExecuted(event)) =
        retired.first().map(RiscvClusterDriveEvent::action)
    else {
        panic!("expected parallel fetch path to retire the branch");
    };
    assert!(event.branch_update().unwrap().actual_taken());
    let resolved = cluster
        .core(CpuId::new(0))
        .unwrap()
        .branch_predictor_snapshot();
    assert_eq!(resolved.pending_speculations(), &[]);
    assert_eq!(resolved.committed_history(), 1);
    assert_eq!(resolved.speculative_history(), 1);
}

#[test]
fn riscv_cluster_parallel_fetch_retires_branch_before_wrong_path_fetch_ahead_completes() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("memory0"),
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
                endpoint("memory0"),
                PartitionId::new(1),
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let cluster = RiscvCluster::new([riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route,
        data_endpoint: "cpu0.dmem",
        data_route,
    })])
    .unwrap();
    let branch = b_type(8, 0, 0, 0x0);
    let store = store_with_programs(&[
        (0x8000, branch),
        (0x8004, i_type(1, 0, 0x0, 1, 0x13)),
        (0x8008, i_type(2, 0, 0x0, 2, 0x13)),
    ]);

    let issued = cluster
        .drive_ready_cores_parallel_fetch(&mut scheduler, &transport, MemoryTrace::new(), |_cpu| {
            let store = store.clone();
            move |delivery, _context| memory_response(&store, &delivery)
        })
        .unwrap();
    assert!(matches!(
        issued.first().map(RiscvClusterDriveEvent::action),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_parallel().unwrap();

    let fetch_ahead = drive_parallel_fetch_until_non_pipeline_action(
        &cluster,
        &mut scheduler,
        &transport,
        store.clone(),
    );
    assert!(matches!(
        fetch_ahead.first().map(RiscvClusterDriveEvent::action),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        cluster
            .core(CpuId::new(0))
            .unwrap()
            .branch_predictor_snapshot()
            .pending_speculations()
            .len(),
        1
    );

    let retired = cluster
        .drive_ready_cores_parallel_fetch(&mut scheduler, &transport, MemoryTrace::new(), |_cpu| {
            let store = store.clone();
            move |delivery, _context| memory_response(&store, &delivery)
        })
        .unwrap();
    let Some(RiscvCoreDriveAction::InstructionExecuted(event)) =
        retired.first().map(RiscvClusterDriveEvent::action)
    else {
        panic!("expected parallel fetch path to retire branch before wrong-path fetch completes");
    };
    assert!(event.branch_update().unwrap().actual_taken());
    let resolved = cluster
        .core(CpuId::new(0))
        .unwrap()
        .branch_predictor_snapshot();
    assert_eq!(resolved.pending_speculations(), &[]);
    assert_eq!(resolved.committed_history(), 1);
    assert_eq!(resolved.speculative_history(), 1);

    scheduler.run_until_idle_parallel().unwrap();
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().pc(),
        Address::new(0x8008)
    );
}

#[test]
fn riscv_cluster_parallel_fetch_retires_fallthrough_branch_after_predicted_target_completes() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("memory0"),
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
                endpoint("memory0"),
                PartitionId::new(1),
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let cluster = RiscvCluster::new([riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route,
        data_endpoint: "cpu0.dmem",
        data_route,
    })])
    .unwrap();
    let taken_branch = b_type(12, 0, 0, 0x0);
    let training_store = store_with_programs(&[(0x8000, taken_branch)]);
    drive_parallel_fetch_until_non_pipeline_action(
        &cluster,
        &mut scheduler,
        &transport,
        training_store.clone(),
    );
    scheduler.run_until_idle_parallel().unwrap();
    drive_parallel_fetch_until_non_pipeline_action(
        &cluster,
        &mut scheduler,
        &transport,
        training_store.clone(),
    );
    let trained = cluster
        .drive_ready_cores_parallel_fetch(&mut scheduler, &transport, MemoryTrace::new(), |_cpu| {
            let store = training_store.clone();
            move |delivery, _context| memory_response(&store, &delivery)
        })
        .unwrap();
    let Some(RiscvCoreDriveAction::InstructionExecuted(event)) =
        trained.first().map(RiscvClusterDriveEvent::action)
    else {
        panic!("expected training branch to retire");
    };
    assert!(event.branch_update().unwrap().actual_taken());
    cluster
        .core(CpuId::new(0))
        .unwrap()
        .redirect_pc(Address::new(0x8000));

    let fallthrough_branch = b_type(12, 0, 0, 0x1);
    let store = store_with_programs(&[
        (0x8000, fallthrough_branch),
        (0x8004, i_type(1, 0, 0x0, 1, 0x13)),
        (0x800c, i_type(2, 0, 0x0, 2, 0x13)),
    ]);
    drive_parallel_fetch_until_non_pipeline_action(
        &cluster,
        &mut scheduler,
        &transport,
        store.clone(),
    );
    scheduler.run_until_idle_parallel().unwrap();
    drive_parallel_fetch_until_non_pipeline_action(
        &cluster,
        &mut scheduler,
        &transport,
        store.clone(),
    );
    scheduler.run_until_idle_parallel().unwrap();

    let retired = cluster
        .drive_ready_cores_parallel_fetch(&mut scheduler, &transport, MemoryTrace::new(), |_cpu| {
            let store = store.clone();
            move |delivery, _context| memory_response(&store, &delivery)
        })
        .unwrap();
    let Some(RiscvCoreDriveAction::InstructionExecuted(event)) =
        retired.first().map(RiscvClusterDriveEvent::action)
    else {
        panic!("expected fallthrough branch to retire after predicted target completes");
    };
    let update = event.branch_update().unwrap();
    assert!(update.predicted_taken());
    assert!(!update.actual_taken());
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().pc(),
        Address::new(0x8004)
    );

    let mut fallthrough = None;
    for _ in 0..16 {
        let actions = cluster
            .drive_ready_cores_parallel_fetch(
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                |_cpu| {
                    let store = store.clone();
                    move |delivery, _context| memory_response(&store, &delivery)
                },
            )
            .unwrap();
        if let Some(RiscvCoreDriveAction::InstructionExecuted(event)) =
            actions.first().map(RiscvClusterDriveEvent::action)
        {
            fallthrough = Some(event.clone());
            break;
        }
        assert!(
            actions.iter().all(|event| matches!(
                event.action(),
                RiscvCoreDriveAction::FetchIssued { .. }
                    | RiscvCoreDriveAction::PipelineCycleScheduled { .. }
            )),
            "unexpected cluster action before fallthrough instruction retired"
        );
        scheduler.run_until_idle_parallel().unwrap();
    }
    let Some(event) = fallthrough else {
        panic!("expected fallthrough instruction to retire after predicted target squash");
    };
    assert_eq!(event.fetch().pc(), Address::new(0x8004));
    assert_eq!(
        event.instruction(),
        RiscvInstruction::decode(i_type(1, 0, 0x0, 1, 0x13)).unwrap()
    );
    assert!(cluster
        .core(CpuId::new(0))
        .unwrap()
        .execution_events()
        .iter()
        .all(|event| event.fetch().pc() != Address::new(0x800c)));
}

#[test]
fn riscv_cluster_parallel_fetch_retires_halfword_aligned_branch_pair() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("memory0"),
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
                endpoint("memory0"),
                PartitionId::new(1),
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let cluster = RiscvCluster::new([riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route,
        data_endpoint: "cpu0.dmem",
        data_route,
    })])
    .unwrap();
    let core = cluster.core(CpuId::new(0)).unwrap();
    core.write_register(reg(10), 1);
    core.write_register(reg(15), 0);
    core.write_register(reg(19), 1);
    core.write_register(reg(22), 1007);

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0x894e_u16.to_le_bytes());
    bytes.extend_from_slice(&0x00a7_f463_u32.to_le_bytes());
    bytes.extend_from_slice(&0x073b_4263_u32.to_le_bytes());
    bytes.extend_from_slice(&0x0010_0073_u32.to_le_bytes());
    let store = store_with_raw_program(0x8000, bytes);

    let mut retired = 0;
    for _ in 0..32 {
        let actions = cluster
            .drive_ready_cores_parallel_fetch(
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                |_cpu| {
                    let store = store.clone();
                    move |delivery, _context| memory_response(&store, &delivery)
                },
            )
            .unwrap();
        retired += actions
            .iter()
            .filter(|event| matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_)))
            .count();
        if retired >= 4 {
            break;
        }
        scheduler.run_until_idle_parallel().unwrap();
    }

    assert_eq!(retired, 4);
    assert_eq!(
        core.execution_events()
            .iter()
            .map(|event| event.fetch().pc())
            .collect::<Vec<_>>(),
        vec![
            Address::new(0x8000),
            Address::new(0x8002),
            Address::new(0x8006),
            Address::new(0x800a),
        ]
    );
    assert!(core.has_pending_trap());
    assert_eq!(core.branch_predictor_snapshot().pending_speculations(), &[]);
}

#[test]
fn riscv_cluster_parallel_fetch_retires_completed_fetch_while_fetch_ahead_is_pending() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
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
    let cluster = RiscvCluster::new([riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route,
        data_endpoint: "cpu0.dmem",
        data_route,
    })])
    .unwrap();
    let store = store_with_programs(&[
        (0x8000, i_type(3, 0, 0x0, 5, 0x13)),
        (0x8004, i_type(4, 0, 0x0, 6, 0x13)),
    ]);

    let issued = cluster
        .drive_ready_cores_parallel_fetch(&mut scheduler, &transport, MemoryTrace::new(), |_cpu| {
            let store = store.clone();
            move |delivery, _context| memory_response(&store, &delivery)
        })
        .unwrap();
    assert!(matches!(
        issued.first().map(RiscvClusterDriveEvent::action),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_parallel().unwrap();

    let fetch_ahead = drive_parallel_fetch_until_non_pipeline_action(
        &cluster,
        &mut scheduler,
        &transport,
        store.clone(),
    );
    assert!(matches!(
        fetch_ahead.first().map(RiscvClusterDriveEvent::action),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));

    let retired = cluster
        .drive_ready_cores_parallel_fetch(&mut scheduler, &transport, MemoryTrace::new(), |_cpu| {
            let store = store.clone();
            move |delivery, _context| memory_response(&store, &delivery)
        })
        .unwrap();
    assert!(matches!(
        retired.first().map(RiscvClusterDriveEvent::action),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(5)),
        3
    );
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().pc(),
        Address::new(0x8004)
    );
}

#[test]
fn riscv_cluster_parallel_fetch_ahead_accepts_compressed_straight_line_instruction() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
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
    let cluster = RiscvCluster::new([riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route,
        data_endpoint: "cpu0.dmem",
        data_route,
    })])
    .unwrap();
    let mut program = Vec::new();
    program.extend_from_slice(&0x0001_u16.to_le_bytes());
    program.extend_from_slice(&0x0000_0073_u32.to_le_bytes());
    let store = store_with_raw_program(0x8000, program);

    let issued = cluster
        .drive_ready_cores_parallel_fetch(&mut scheduler, &transport, MemoryTrace::new(), |_cpu| {
            let store = store.clone();
            move |delivery, _context| memory_response(&store, &delivery)
        })
        .unwrap();
    assert!(matches!(
        issued.first().map(RiscvClusterDriveEvent::action),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_parallel().unwrap();

    let fetch_ahead = drive_parallel_fetch_until_non_pipeline_action(
        &cluster,
        &mut scheduler,
        &transport,
        store.clone(),
    );
    assert!(matches!(
        fetch_ahead.first().map(RiscvClusterDriveEvent::action),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().inner().pc(),
        Address::new(0x8002)
    );
}

#[test]
fn riscv_cluster_parallel_mmio_path_commits_branch_fetch_ahead_speculation() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("memory0"),
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
                endpoint("memory0"),
                PartitionId::new(1),
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let cluster = RiscvCluster::new([riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route,
        data_endpoint: "cpu0.dmem",
        data_route,
    })])
    .unwrap();
    let branch = b_type(8, 0, 0, 0x0);
    let store = store_with_programs(&[
        (0x8000, branch),
        (0x8004, i_type(1, 0, 0x0, 1, 0x13)),
        (0x8008, i_type(2, 0, 0x0, 2, 0x13)),
    ]);
    let bus = MmioBus::new();

    let issued = cluster
        .drive_turn_parallel_with_mmio(
            &mut scheduler,
            &transport,
            &bus,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| {
                let store = store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                let store = store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            },
        )
        .unwrap();
    assert!(matches!(
        issued
            .core_events()
            .first()
            .map(RiscvClusterDriveEvent::action),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_parallel().unwrap();

    let fetch_ahead = drive_parallel_mmio_until_non_pipeline_action(
        &cluster,
        &mut scheduler,
        &transport,
        &bus,
        store.clone(),
    );
    assert!(matches!(
        fetch_ahead
            .core_events()
            .first()
            .map(RiscvClusterDriveEvent::action),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        cluster
            .core(CpuId::new(0))
            .unwrap()
            .branch_predictor_snapshot()
            .pending_speculations()
            .len(),
        1
    );
    scheduler.run_until_idle_parallel().unwrap();

    let retired = cluster
        .drive_turn_parallel_with_mmio(
            &mut scheduler,
            &transport,
            &bus,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| {
                let store = store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                let store = store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            },
        )
        .unwrap();
    let Some(RiscvCoreDriveAction::InstructionExecuted(event)) = retired
        .core_events()
        .first()
        .map(RiscvClusterDriveEvent::action)
    else {
        panic!("expected MMIO-parallel path to retire the branch");
    };
    assert!(event.branch_update().unwrap().actual_taken());
    let resolved = cluster
        .core(CpuId::new(0))
        .unwrap()
        .branch_predictor_snapshot();
    assert_eq!(resolved.pending_speculations(), &[]);
    assert_eq!(resolved.committed_history(), 1);
    assert_eq!(resolved.speculative_history(), 1);
}

#[test]
fn riscv_cluster_run_records_bounded_turn_trace_until_stop_condition() {
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
        (0x8000, i_type(51, 0, 0x0, 1, 0x13)),
        (0x9000, i_type(61, 0, 0x0, 1, 0x13)),
    ]);

    let run = cluster
        .drive_until(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(store.clone()),
            |_cpu| responder(store.clone()),
            24,
            |turn| {
                turn.core_events().iter().any(|event| {
                    matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_))
                })
            },
        )
        .unwrap();

    assert_eq!(run.stop_reason(), RiscvClusterStopReason::StopCondition);
    assert_eq!(run.idle_tick(), None);
    assert!(matches!(
        run.turns().first().unwrap().core_events()[0].action(),
        RiscvCoreDriveAction::FetchIssued { .. }
    ));
    assert!(run
        .scheduler_summaries()
        .iter()
        .any(|summary| summary.executed_events() > 0));
    assert_eq!(
        run.turns()
            .last()
            .unwrap()
            .core_events()
            .iter()
            .map(RiscvClusterDriveEvent::cpu)
            .collect::<Vec<_>>(),
        vec![CpuId::new(0), CpuId::new(1)]
    );
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(1)),
        51
    );
    assert_eq!(
        cluster.core(CpuId::new(1)).unwrap().read_register(reg(1)),
        61
    );

    let summary = RiscvClusterRun::new(
        run.turns().to_vec(),
        RiscvClusterStopReason::Idle {
            tick: scheduler.now(),
        },
    );
    assert_eq!(summary.idle_tick(), Some(scheduler.now()));
}

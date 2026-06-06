use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster,
    RiscvClusterDriveEvent, RiscvClusterTurn, RiscvCore, RiscvCoreDriveAction,
    RiscvDataAccessEventKind,
};
use rem6_fabric::{FabricLinkId, FabricModel, FabricPath, FabricPathHop};
use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerContext};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryTargetId,
    PartitionedMemoryStore,
};
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

fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = (imm as u32) & 0x0fff;
    (((imm >> 5) & 0x7f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | 0x23
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

fn data_read(address: u64, size: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(99), sequence),
        Address::new(address),
        AccessSize::new(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn read_store_bytes(
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    address: u64,
    size: u64,
    sequence: u64,
) -> Vec<u8> {
    store
        .lock()
        .unwrap()
        .respond(&data_read(address, size, sequence))
        .unwrap()
        .response()
        .unwrap()
        .data()
        .unwrap()
        .to_vec()
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

#[test]
fn riscv_cluster_parallel_turns_issue_completed_data_accesses() {
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
        .write_register(reg(2), 0x9000);
    cluster
        .core(CpuId::new(1))
        .unwrap()
        .write_register(reg(2), 0x9010);
    let store = store_with_programs_and_data(
        &[
            (0x8000, i_type(8, 2, 0x3, 5, 0x03)),
            (0x8100, i_type(8, 2, 0x3, 5, 0x03)),
        ],
        &[
            (0x9008, vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]),
            (0x9018, vec![0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f]),
        ],
    );

    let mut turns = Vec::new();
    for _ in 0..10 {
        let turn = cluster
            .drive_turn_parallel(
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
            )
            .unwrap();
        let loaded = cluster.core(CpuId::new(0)).unwrap().read_register(reg(5))
            == 0x1122_3344_5566_7788
            && cluster.core(CpuId::new(1)).unwrap().read_register(reg(5)) == 0x0f1e_2d3c_4b5a_6978;
        turns.push(turn);
        if loaded {
            break;
        }
    }

    assert_eq!(
        turns[0]
            .core_events()
            .iter()
            .map(RiscvClusterDriveEvent::cpu)
            .collect::<Vec<_>>(),
        vec![CpuId::new(0), CpuId::new(1)]
    );
    assert!(turns[0]
        .core_events()
        .iter()
        .all(|event| matches!(event.action(), RiscvCoreDriveAction::FetchIssued { .. })));
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

    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(5)),
        0x1122_3344_5566_7788
    );
    assert_eq!(
        cluster.core(CpuId::new(1)).unwrap().read_register(reg(5)),
        0x0f1e_2d3c_4b5a_6978
    );
    for cpu in [CpuId::new(0), CpuId::new(1)] {
        let core = cluster.core(cpu).unwrap();
        let kinds = core
            .data_access_events()
            .iter()
            .map(|event| event.kind())
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                RiscvDataAccessEventKind::Issued,
                RiscvDataAccessEventKind::Completed,
            ]
        );
    }
}

#[test]
fn riscv_cluster_invalidates_peer_reservation_after_completed_store() {
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
    let cpu0 = cluster.core(CpuId::new(0)).unwrap();
    let cpu1 = cluster.core(CpuId::new(1)).unwrap();
    cpu0.write_register(reg(2), 0x9008);
    cpu0.write_register(reg(6), 0x0102_0304_0506_0708);
    cpu1.write_register(reg(2), 0x9008);
    cpu1.write_register(reg(6), 0x1112_1314_1516_1718);
    let store = store_with_programs_and_data(
        &[
            (0x8000, atomic_type(0x02, false, false, 0, 2, 0x3, 5)),
            (0x8004, atomic_type(0x03, false, true, 6, 2, 0x3, 7)),
            (0x8100, s_type(0, 6, 2, 0x3)),
        ],
        &[(0x9008, vec![0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11])],
    );

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
                CpuId::new(0),
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                responder(store.clone()),
                responder(store.clone()),
            )
            .unwrap(),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(cpu0.load_reservation().is_some());

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
    scheduler.run_until_idle_conservative();
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
                responder(store.clone()),
            )
            .unwrap(),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
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
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(cpu0.load_reservation(), None);
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
                CpuId::new(0),
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                responder(store.clone()),
                responder(store.clone()),
            )
            .unwrap(),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert_eq!(cpu0.read_register(reg(7)), 1);
    assert_eq!(
        read_store_bytes(&store, 0x9008, 8, 90),
        vec![0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11]
    );
    assert!(cpu0.data_access_events().iter().any(|event| {
        event.kind() == RiscvDataAccessEventKind::ConditionalFailed
            && event.request_id().agent() == AgentId::new(7)
    }));
}

#[test]
fn riscv_cluster_invalidates_peer_reservation_after_completed_store_conditional() {
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
    let cpu0 = cluster.core(CpuId::new(0)).unwrap();
    let cpu1 = cluster.core(CpuId::new(1)).unwrap();
    cpu0.write_register(reg(2), 0x9008);
    cpu0.write_register(reg(6), 0x0102_0304_0506_0708);
    cpu1.write_register(reg(2), 0x9008);
    cpu1.write_register(reg(6), 0x1112_1314_1516_1718);
    let store = store_with_programs_and_data(
        &[
            (0x8000, atomic_type(0x02, false, false, 0, 2, 0x3, 5)),
            (0x8004, atomic_type(0x03, false, true, 6, 2, 0x3, 7)),
            (0x8100, atomic_type(0x02, false, false, 0, 2, 0x3, 5)),
            (0x8104, atomic_type(0x03, false, true, 6, 2, 0x3, 7)),
        ],
        &[(0x9008, vec![0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11])],
    );

    let mut drive = |cpu| {
        let action = cluster
            .drive_core_next_action(
                cpu,
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                responder(store.clone()),
                responder(store.clone()),
            )
            .unwrap();
        scheduler.run_until_idle_conservative();
        action
    };

    assert!(matches!(
        drive(CpuId::new(0)),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert!(matches!(
        drive(CpuId::new(0)),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive(CpuId::new(0)),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    assert!(cpu0.load_reservation().is_some());

    assert!(matches!(
        drive(CpuId::new(1)),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert!(matches!(
        drive(CpuId::new(1)),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive(CpuId::new(1)),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    assert!(cpu1.load_reservation().is_some());

    assert!(matches!(
        drive(CpuId::new(0)),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert!(matches!(
        drive(CpuId::new(0)),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive(CpuId::new(0)),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    assert_eq!(cpu0.read_register(reg(7)), 0);
    assert_eq!(cpu0.load_reservation(), None);

    assert!(matches!(
        drive(CpuId::new(1)),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(cpu1.load_reservation(), None);
    assert!(matches!(
        drive(CpuId::new(1)),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive(CpuId::new(1)),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));

    assert_eq!(cpu1.read_register(reg(7)), 1);
    assert_eq!(
        read_store_bytes(&store, 0x9008, 8, 91),
        vec![0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
    );
    assert!(cpu1.data_access_events().iter().any(|event| {
        event.kind() == RiscvDataAccessEventKind::ConditionalFailed
            && event.request_id().agent() == AgentId::new(8)
    }));
}

#[test]
fn riscv_cluster_parallel_data_batches_shared_fabric_by_packet_order() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let mut transport = MemoryTransport::with_fabric(FabricModel::new());
    let cpu0_fetch = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(2),
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu1_fetch = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("memory0"),
                PartitionId::new(2),
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let shared_path = fabric_path("data_mesh", 2, 4);
    let cpu1_data = transport
        .add_route(
            MemoryRoute::new_path(
                endpoint("cpu1.dmem"),
                PartitionId::new(1),
                [
                    MemoryRouteHop::new(endpoint("memory0"), PartitionId::new(2), 2, 2)
                        .unwrap()
                        .with_request_fabric_path(shared_path.clone()),
                ],
            )
            .unwrap(),
        )
        .unwrap();
    let cpu0_data = transport
        .add_route(
            MemoryRoute::new_path(
                endpoint("cpu0.dmem"),
                PartitionId::new(0),
                [
                    MemoryRouteHop::new(endpoint("memory0"), PartitionId::new(2), 2, 2)
                        .unwrap()
                        .with_request_fabric_path(shared_path),
                ],
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
        .write_register(reg(2), 0x9000);
    cluster
        .core(CpuId::new(1))
        .unwrap()
        .write_register(reg(2), 0x9010);
    let store = store_with_programs_and_data(
        &[
            (0x8000, i_type(8, 2, 0x3, 5, 0x03)),
            (0x8100, i_type(8, 2, 0x3, 5, 0x03)),
        ],
        &[
            (0x9008, vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]),
            (0x9018, vec![0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f]),
        ],
    );
    let deliveries = Arc::new(Mutex::new(Vec::new()));

    for _ in 0..10 {
        let turn = cluster
            .drive_turn_parallel(
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
            )
            .unwrap();
        if turn
            .core_events()
            .iter()
            .all(|event| matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_)))
            && turn.core_events().len() == 2
        {
            break;
        }
    }

    let data_issued = cluster
        .drive_ready_cores_parallel(
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
                let deliveries = Arc::clone(&deliveries);
                move |delivery, _context| {
                    deliveries.lock().unwrap().push((
                        delivery.route(),
                        delivery.tick(),
                        delivery.request().id(),
                    ));
                    memory_response(&store, &delivery)
                }
            },
        )
        .unwrap();

    assert_eq!(
        data_issued
            .iter()
            .map(RiscvClusterDriveEvent::cpu)
            .collect::<Vec<_>>(),
        vec![CpuId::new(0), CpuId::new(1)],
    );
    assert!(data_issued.iter().all(|event| matches!(
        event.action(),
        RiscvCoreDriveAction::DataAccessIssued { .. }
    )));
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        *deliveries.lock().unwrap(),
        vec![
            (cpu1_data, 8, MemoryRequestId::new(AgentId::new(8), 1),),
            (cpu0_data, 10, MemoryRequestId::new(AgentId::new(7), 1),),
        ],
    );
}

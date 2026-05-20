use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster,
    RiscvClusterDriveEvent, RiscvClusterError, RiscvClusterRun, RiscvClusterStopReason,
    RiscvClusterTurn, RiscvCore, RiscvCoreDriveAction, RiscvDataAccessEventKind,
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
        assert!(turns.len() < 10);
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
        assert!(turns.len() < 10);
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
            10,
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

use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, CpuTranslationFrontend,
    RiscvCluster, RiscvClusterDriveEvent, RiscvCore, RiscvCoreDriveAction,
};
use rem6_isa_riscv::{Register, RiscvPrivilegeMode, RiscvTrapKind};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequestId, MemoryTargetId,
    PartitionedMemoryStore, TranslationPageMap, TranslationPagePermissions, TranslationPageSize,
    TranslationQueueConfig, TranslationTlbConfig,
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

fn translated_riscv_core(spec: CoreSpec<'_>) -> RiscvCore {
    translated_riscv_core_with_latency(spec, 0)
}

fn translated_riscv_core_with_latency(spec: CoreSpec<'_>, latency: u64) -> RiscvCore {
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
    RiscvCore::with_data_translation(
        core,
        CpuDataConfig::new(endpoint(spec.data_endpoint), spec.data_route, layout()),
        CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, latency).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ),
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

fn single_page_map(virtual_base: u64, physical_base: u64) -> TranslationPageMap {
    let mut map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    map.map(
        Address::new(virtual_base),
        Address::new(physical_base),
        1,
        TranslationPagePermissions::read_write_execute(),
    )
    .unwrap();
    map
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

#[test]
fn riscv_cluster_parallel_turns_issue_translated_data_accesses() {
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
        translated_riscv_core_with_latency(
            CoreSpec {
                cpu: 0,
                partition: 0,
                agent: 7,
                entry: 0x8000,
                fetch_endpoint: "cpu0.ifetch",
                fetch_route: cpu0_fetch,
                data_endpoint: "cpu0.dmem",
                data_route: cpu0_data,
            },
            2,
        ),
        translated_riscv_core_with_latency(
            CoreSpec {
                cpu: 1,
                partition: 1,
                agent: 8,
                entry: 0x8100,
                fetch_endpoint: "cpu1.ifetch",
                fetch_route: cpu1_fetch,
                data_endpoint: "cpu1.dmem",
                data_route: cpu1_data,
            },
            2,
        ),
    ])
    .unwrap();
    cluster
        .core(CpuId::new(0))
        .unwrap()
        .write_register(reg(2), 0x4000);
    cluster
        .core(CpuId::new(1))
        .unwrap()
        .write_register(reg(2), 0x4010);
    let page_map = single_page_map(0x4000, 0x9000);
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

    let mut turns = Vec::new();
    for _ in 0..10 {
        let turn = cluster
            .drive_turn_parallel_with_data_translation(
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                &page_map,
                |_cpu| {
                    let store = store.clone();
                    move |delivery, _context| memory_response(&store, &delivery)
                },
                |_cpu| {
                    let store = store.clone();
                    let deliveries = Arc::clone(&deliveries);
                    move |delivery, _context| {
                        deliveries
                            .lock()
                            .unwrap()
                            .push((delivery.request().id(), delivery.request().range().start()));
                        memory_response(&store, &delivery)
                    }
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

    let data_turn = turns
        .iter()
        .find(|turn| {
            turn.core_events().iter().all(|event| {
                matches!(
                    event.action(),
                    RiscvCoreDriveAction::DataAccessIssued { .. }
                )
            }) && turn.core_events().len() == 2
        })
        .expect("parallel translated data access issue turn");
    assert_eq!(
        data_turn
            .core_events()
            .iter()
            .map(RiscvClusterDriveEvent::cpu)
            .collect::<Vec<_>>(),
        vec![CpuId::new(0), CpuId::new(1)]
    );
    let mut deliveries = deliveries.lock().unwrap().clone();
    deliveries.sort();
    assert_eq!(
        deliveries,
        vec![
            (
                MemoryRequestId::new(AgentId::new(7), 1),
                Address::new(0x9008),
            ),
            (
                MemoryRequestId::new(AgentId::new(8), 1),
                Address::new(0x9018),
            ),
        ]
    );
}

#[test]
fn riscv_cluster_parallel_data_translation_commits_branch_fetch_ahead_speculation() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
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
    let cluster = RiscvCluster::new([translated_riscv_core(CoreSpec {
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
    let page_map = single_page_map(0x4000, 0x9000);
    let store = store_with_programs_and_data(
        &[
            (0x8000, branch),
            (0x8004, i_type(1, 0, 0x0, 1, 0x13)),
            (0x8008, i_type(2, 0, 0x0, 2, 0x13)),
        ],
        &[],
    );

    let issued = cluster
        .drive_ready_cores_parallel_with_data_translation(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            &page_map,
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
        issued.first().map(RiscvClusterDriveEvent::action),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_parallel().unwrap();

    let fetch_ahead = cluster
        .drive_ready_cores_parallel_with_data_translation(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            &page_map,
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
        .drive_ready_cores_parallel_with_data_translation(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            &page_map,
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
    let Some(RiscvCoreDriveAction::InstructionExecuted(event)) =
        retired.first().map(RiscvClusterDriveEvent::action)
    else {
        panic!("expected data-translation parallel path to retire the branch");
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
fn riscv_cluster_parallel_data_translation_fault_emits_guest_page_fault_trap() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
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
    let core = translated_riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route,
        data_endpoint: "cpu0.dmem",
        data_route,
    });
    core.write_register(reg(2), 0x4000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    core.set_machine_exception_delegation(1 << 13);
    core.set_supervisor_trap_vector(0xa000);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    let store = store_with_programs_and_data(&[(0x8000, i_type(8, 2, 0x3, 5, 0x03))], &[]);

    let mut trap = None;
    for _ in 0..8 {
        let turn = cluster
            .drive_turn_parallel_with_data_translation(
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                &page_map,
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
        trap = turn
            .core_events()
            .iter()
            .find_map(|event| match event.action() {
                RiscvCoreDriveAction::InstructionExecuted(executed) => {
                    let trap = executed.execution().trap().copied()?;
                    assert!(!executed.counts_as_retired_instruction());
                    Some(trap)
                }
                _ => None,
            });
        if trap.is_some() {
            break;
        }
    }

    assert_eq!(
        trap.map(|trap| trap.kind()),
        Some(RiscvTrapKind::LoadPageFault { address: 0x4008 })
    );
    assert_eq!(core.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(core.supervisor_exception_pc(), 0x8000);
    assert_eq!(core.supervisor_trap_cause(), 13);
    assert_eq!(core.supervisor_trap_value(), 0x4008);
    assert_eq!(core.pc(), Address::new(0xa000));
    let execution_events = core.execution_events();
    assert_eq!(execution_events.len(), 1);
    assert_eq!(
        execution_events[0]
            .execution()
            .trap()
            .map(|trap| trap.kind()),
        Some(RiscvTrapKind::LoadPageFault { address: 0x4008 })
    );
}

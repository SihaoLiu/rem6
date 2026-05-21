use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore,
    RiscvCoreDriveAction, RiscvDataAccessEventKind, RiscvDataAccessTarget,
};
use rem6_kernel::{PartitionId, PartitionedScheduler, ScheduledEventKind, SchedulerContext};
use rem6_memory::{
    AccessSize, Address, AddressRange, AgentId, CacheLineLayout, MemoryTargetId,
    PartitionedMemoryStore,
};
use rem6_mmio::{MmioAccess, MmioBus, MmioRegisterBank, MmioRoute};
use rem6_stats::{StatSample, StatSnapshot, StatsRegistry};
use rem6_system::{
    GuestEventId, GuestSourceId, GuestTrap, GuestTrapKind, HostEventPolicy, RiscvInstructionStats,
    RiscvSystemRunDriver, RiscvSystemRunStopReason, RiscvTrapEventPort, StopRequest,
    SystemActionOutcome, SystemHostController, SystemHostEventPort,
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

fn loaded_program_store_with_data(
    instructions: &[(u64, u32)],
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

    let mut image = BootImage::new(Address::new(instructions[0].0));
    for (address, instruction) in instructions {
        image = image
            .add_segment(Address::new(*address), word(*instruction))
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

fn loaded_program_store(instructions: &[(u64, u32)]) -> Arc<Mutex<PartitionedMemoryStore>> {
    loaded_program_store_with_data(instructions, &[])
}

fn riscv_core(
    cpu: u32,
    partition: u32,
    agent: u32,
    fetch_endpoint: &str,
    fetch_route: MemoryRouteId,
    entry: u64,
) -> RiscvCore {
    RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(cpu),
                PartitionId::new(partition),
                AgentId::new(agent),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint(fetch_endpoint),
                fetch_route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    )
}

#[allow(clippy::too_many_arguments)]
fn riscv_data_core(
    cpu: u32,
    partition: u32,
    agent: u32,
    fetch_endpoint: &str,
    fetch_route: MemoryRouteId,
    data_endpoint: &str,
    data_route: MemoryRouteId,
    entry: u64,
) -> RiscvCore {
    RiscvCore::with_data(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(cpu),
                PartitionId::new(partition),
                AgentId::new(agent),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint(fetch_endpoint),
                fetch_route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
        CpuDataConfig::new(endpoint(data_endpoint), data_route, layout()),
    )
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
fn riscv_system_run_driver_stops_after_cluster_traps_reach_host() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(31);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu1_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cluster = RiscvCluster::new([
        riscv_core(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000),
        riscv_core(1, 1, 8, "cpu1.ifetch", cpu1_route, 0x9000),
    ])
    .unwrap();
    let store = loaded_program_store(&[(0x8000, 0x0000_0073), (0x9000, 0x0010_0073)]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port);

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            20,
            |cpu| GuestEventId::new(80 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(80), source, 0);
    assert_eq!(run.stop_reason(), RiscvSystemRunStopReason::HostStop(stop));
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run
        .turns()
        .iter()
        .flat_map(|turn| turn.core_events())
        .any(|event| matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_))));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| (
                record.cpu(),
                record.event(),
                record.source_partition(),
                record.trap()
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                CpuId::new(0),
                GuestEventId::new(80),
                PartitionId::new(0),
                GuestTrap::new(GuestTrapKind::EnvironmentCall, 0x8000),
            ),
            (
                CpuId::new(1),
                GuestEventId::new(81),
                PartitionId::new(1),
                GuestTrap::new(GuestTrapKind::Breakpoint, 0x9000),
            ),
        ]
    );

    let controller = controller.lock().unwrap();
    assert_eq!(controller.run().stop_request(), Some(&stop));
    assert_eq!(
        controller.run().action_outcomes(),
        &[
            SystemActionOutcome::Stop(stop),
            SystemActionOutcome::Stop(StopRequest::new(
                stop.tick(),
                GuestEventId::new(81),
                source,
                1,
            )),
        ]
    );
    assert_eq!(scheduler.now(), stop.tick());
}

#[test]
fn riscv_system_run_driver_records_committed_instruction_stats() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(32);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu1_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cluster = RiscvCluster::new([
        riscv_core(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000),
        riscv_core(1, 1, 8, "cpu1.ifetch", cpu1_route, 0x9000),
    ])
    .unwrap();
    let store = loaded_program_store(&[(0x8000, 0x0000_0073), (0x9000, 0x0010_0073)]);
    let mut stats = StatsRegistry::new();
    let cpu0_committed = stats
        .register_counter("cpu0.committed_insts", "count")
        .unwrap();
    let cpu1_committed = stats
        .register_counter("cpu1.committed_insts", "count")
        .unwrap();
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        stats,
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::with_instruction_stats(
        trap_port,
        RiscvInstructionStats::new([
            (CpuId::new(0), cpu0_committed),
            (CpuId::new(1), cpu1_committed),
        ]),
    );

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            20,
            |cpu| GuestEventId::new(90 + u64::from(cpu.get())),
        )
        .unwrap();
    let tick = run.final_tick().unwrap();

    assert_eq!(
        controller.lock().unwrap().executor().stats().snapshot(tick),
        StatSnapshot::new(
            tick,
            0,
            0,
            vec![
                StatSample::new(cpu0_committed, "cpu0.committed_insts", "count", 1),
                StatSample::new(cpu1_committed, "cpu1.committed_insts", "count", 1),
            ],
        )
    );
}

#[test]
fn riscv_system_run_driver_parallel_path_drives_data_accesses_to_host_stop() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(33);
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
                PartitionId::new(2),
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
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core0 = riscv_data_core(
        0,
        0,
        7,
        "cpu0.ifetch",
        cpu0_fetch,
        "cpu0.dmem",
        cpu0_data,
        0x8000,
    );
    let core1 = riscv_data_core(
        1,
        1,
        8,
        "cpu1.ifetch",
        cpu1_fetch,
        "cpu1.dmem",
        cpu1_data,
        0x9000,
    );
    core0.write_register(rem6_isa_riscv::Register::new(2).unwrap(), 0x9800);
    core1.write_register(rem6_isa_riscv::Register::new(2).unwrap(), 0x9810);
    let cluster = RiscvCluster::new([core0, core1]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, i_type(8, 2, 0x3, 5, 0x03)),
            (0x8004, 0x0000_0073),
            (0x9000, i_type(8, 2, 0x3, 5, 0x03)),
            (0x9004, 0x0010_0073),
        ],
        &[
            (0x9808, vec![0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]),
            (0x9818, vec![0x89, 0x67, 0x45, 0x23, 0x01, 0xef, 0xcd, 0xab]),
        ],
    );
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port);

    let run = driver
        .drive_until_host_stop_parallel(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context| memory_response(&store, &delivery)
            },
            30,
            |cpu| GuestEventId::new(100 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(100), source, 0);
    assert_eq!(run.stop_reason(), RiscvSystemRunStopReason::HostStop(stop));
    assert!(run.turns().iter().any(|turn| {
        !turn.core_events().is_empty()
            && turn.core_events().iter().all(|event| {
                matches!(
                    event.action(),
                    RiscvCoreDriveAction::DataAccessIssued { .. }
                )
            })
    }));
    assert!(run
        .turns()
        .iter()
        .filter_map(|turn| turn.scheduler_summary())
        .any(|summary| summary.executed_events() > 0));
    let parallel_epochs = run.parallel_scheduler_epochs();
    assert!(!parallel_epochs.is_empty());
    assert!(parallel_epochs
        .iter()
        .all(|epoch| epoch.plan().is_parallel_safe()));
    assert!(parallel_epochs
        .iter()
        .all(|epoch| epoch.summary().final_tick() == epoch.plan().horizon()));
    assert_eq!(
        run.parallel_scheduler_dispatches().len(),
        parallel_epochs
            .iter()
            .map(|epoch| epoch.dispatches().len())
            .sum::<usize>()
    );
    assert_eq!(
        run.parallel_scheduler_frontiers().len(),
        parallel_epochs
            .iter()
            .map(|epoch| epoch.frontiers().len())
            .sum::<usize>()
    );
    assert_eq!(
        run.parallel_scheduler_ready_partitions().len(),
        parallel_epochs
            .iter()
            .map(|epoch| epoch.ready_partitions().len())
            .sum::<usize>()
    );
    assert!(run
        .parallel_scheduler_dispatches_for_partition(PartitionId::new(0))
        .iter()
        .all(|record| {
            record.partition() == PartitionId::new(0)
                && record.kind() == ScheduledEventKind::Parallel
        }));
    assert_eq!(
        cluster
            .core(CpuId::new(0))
            .unwrap()
            .read_register(rem6_isa_riscv::Register::new(5).unwrap()),
        0xfedc_ba98_7654_3210
    );
    assert_eq!(
        cluster
            .core(CpuId::new(1))
            .unwrap()
            .read_register(rem6_isa_riscv::Register::new(5).unwrap()),
        0xabcd_ef01_2345_6789
    );
    for cpu in [CpuId::new(0), CpuId::new(1)] {
        let kinds = cluster
            .core(cpu)
            .unwrap()
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
fn riscv_system_run_driver_parallel_mmio_path_drives_data_accesses_to_host_stop() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(34);
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
                PartitionId::new(2),
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
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core0 = riscv_data_core(
        0,
        0,
        7,
        "cpu0.ifetch",
        cpu0_fetch,
        "cpu0.dmem",
        cpu0_data,
        0x8000,
    );
    let core1 = riscv_data_core(
        1,
        1,
        8,
        "cpu1.ifetch",
        cpu1_fetch,
        "cpu1.dmem",
        cpu1_data,
        0x9000,
    );
    core0.write_register(rem6_isa_riscv::Register::new(2).unwrap(), 0x1000);
    core1.write_register(rem6_isa_riscv::Register::new(2).unwrap(), 0x9810);
    let cluster = RiscvCluster::new([core0, core1]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, i_type(8, 2, 0x3, 5, 0x03)),
            (0x8004, 0x0000_0073),
            (0x9000, i_type(8, 2, 0x3, 5, 0x03)),
            (0x9004, 0x0010_0073),
        ],
        &[(0x9818, vec![0x89, 0x67, 0x45, 0x23, 0x01, 0xef, 0xcd, 0xab])],
    );
    let mut bank =
        MmioRegisterBank::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        8,
        AccessSize::new(8).unwrap(),
        MmioAccess::ReadOnly,
        vec![0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe],
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
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port);

    let run = driver
        .drive_until_host_stop_parallel_with_mmio(
            &cluster,
            &mut scheduler,
            &transport,
            &bus,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context| memory_response(&store, &delivery)
            },
            30,
            |cpu| GuestEventId::new(110 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(110), source, 0);
    assert_eq!(run.stop_reason(), RiscvSystemRunStopReason::HostStop(stop));
    assert!(run.turns().iter().any(|turn| {
        !turn.core_events().is_empty()
            && turn.core_events().iter().all(|event| {
                matches!(
                    event.action(),
                    RiscvCoreDriveAction::DataAccessIssued { .. }
                )
            })
    }));
    assert_eq!(
        cluster
            .core(CpuId::new(0))
            .unwrap()
            .read_register(rem6_isa_riscv::Register::new(5).unwrap()),
        0xfedc_ba98_7654_3210
    );
    assert_eq!(
        cluster
            .core(CpuId::new(1))
            .unwrap()
            .read_register(rem6_isa_riscv::Register::new(5).unwrap()),
        0xabcd_ef01_2345_6789
    );

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
    let parallel_epochs = run.parallel_scheduler_epochs();
    assert!(!parallel_epochs.is_empty());
    assert_eq!(
        run.parallel_scheduler_frontiers().len(),
        parallel_epochs
            .iter()
            .map(|epoch| epoch.frontiers().len())
            .sum::<usize>()
    );
    assert!(run
        .parallel_scheduler_dispatches()
        .iter()
        .all(|record| record.kind() == ScheduledEventKind::Parallel));
}

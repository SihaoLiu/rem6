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
use rem6_stats::{
    GlobalInstTrackerSnapshot, ProbePayload, StatId, StatSample, StatSnapshot, StatsRegistry,
};
use rem6_system::{
    GuestEventId, GuestHostCallResponse, GuestSourceId, GuestTrap, GuestTrapKind, HostEventPolicy,
    RiscvInstructionStats, RiscvSystemRunDriver, RiscvSystemRunStopReason, RiscvTrapEventPort,
    StopRequest, SystemActionOutcome, SystemHostController, SystemHostEventPort,
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

fn gem5_m5op_type(function: u32) -> u32 {
    0x0000_007b | (function << 25)
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
        ])
        .with_retired_inst_thresholds(vec![2]),
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

    let retired = run
        .retired_instruction_probes()
        .expect("run should carry retired instruction probe evidence");
    assert_eq!(
        retired.tracker(),
        &GlobalInstTrackerSnapshot::new(2, Vec::new())
    );
    assert!(retired.point_for_cpu(CpuId::new(0)).is_some());
    assert!(retired.point_for_cpu(CpuId::new(1)).is_some());
    assert_eq!(retired.probes().events().len(), 2);
    let probe_ticks = retired
        .probes()
        .events()
        .iter()
        .map(|event| event.tick())
        .collect::<Vec<_>>();
    assert!(probe_ticks.iter().all(|probe_tick| *probe_tick <= tick));
    assert!(probe_ticks.windows(2).all(|window| window[0] <= window[1]));
    assert_eq!(
        retired
            .probes()
            .events()
            .iter()
            .map(|event| event.payload())
            .collect::<Vec<_>>(),
        vec![
            &ProbePayload::Counter { amount: 1 },
            &ProbePayload::Counter { amount: 1 },
        ]
    );
}

#[test]
fn riscv_instruction_stats_clone_uses_independent_retired_probe_recorders() {
    let stats = RiscvInstructionStats::new([(CpuId::new(0), StatId::new(0))])
        .with_retired_inst_thresholds(vec![1]);
    let cloned = stats.clone();

    stats
        .record_retired_instruction_probe(CpuId::new(0), 10)
        .unwrap();

    assert_eq!(
        stats
            .retired_instruction_probe_snapshot()
            .probes()
            .events()
            .len(),
        1
    );
    assert!(cloned
        .retired_instruction_probe_snapshot()
        .probes()
        .events()
        .is_empty());

    cloned
        .record_retired_instruction_probe(CpuId::new(0), 12)
        .unwrap();

    assert_eq!(
        stats
            .retired_instruction_probe_snapshot()
            .probes()
            .events()
            .len(),
        1
    );
    assert_eq!(
        cloned
            .retired_instruction_probe_snapshot()
            .probes()
            .events()
            .len(),
        1
    );
}

#[test]
fn riscv_system_run_driver_routes_gem5_work_marker_pseudo_ops_to_host() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(37);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
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
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.write_register(rem6_isa_riscv::Register::new(10).unwrap(), 0x51);
    core.write_register(rem6_isa_riscv::Register::new(11).unwrap(), 0x9);
    let cluster = RiscvCluster::new([core]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, gem5_m5op_type(0x5a)),
        (0x8004, i_type(0x51, 0, 0x0, 10, 0x13)),
        (0x8008, gem5_m5op_type(0x5b)),
        (0x800c, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port);
    let mut next_event_id = 110_u64;

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            30,
            |_cpu| {
                let event = GuestEventId::new(next_event_id);
                next_event_id += 1;
                event
            },
        )
        .unwrap();

    assert_eq!(
        run.host_stop(),
        Some(StopRequest::new(
            run.final_tick().unwrap(),
            GuestEventId::new(112),
            source,
            0,
        ))
    );

    let controller = controller.lock().unwrap();
    let outcomes = controller.run().action_outcomes();
    let roi_outcomes = outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            SystemActionOutcome::RoiBegin {
                event,
                source: actual_source,
                work_id,
                thread_id,
                ..
            } => Some(("begin", *event, *actual_source, *work_id, *thread_id)),
            SystemActionOutcome::RoiEnd {
                event,
                source: actual_source,
                work_id,
                thread_id,
                ..
            } => Some(("end", *event, *actual_source, *work_id, *thread_id)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        roi_outcomes,
        vec![
            ("begin", GuestEventId::new(110), source, 0x51, 0x9),
            ("end", GuestEventId::new(111), source, 0x51, 0x9),
        ]
    );
}

#[test]
fn riscv_system_run_driver_routes_gem5_fail_pseudo_op_to_host_stop() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(38);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
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
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.write_register(rem6_isa_riscv::Register::new(10).unwrap(), 2);
    core.write_register(rem6_isa_riscv::Register::new(11).unwrap(), 7);
    let cluster = RiscvCluster::new([core]).unwrap();
    let store = loaded_program_store(&[(0x8000, gem5_m5op_type(0x22)), (0x8004, 0x0000_0073)]);
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
            |_cpu| GuestEventId::new(120),
        )
        .unwrap();

    assert_eq!(
        run.host_stop(),
        Some(StopRequest::new(
            run.final_tick().unwrap(),
            GuestEventId::new(120),
            source,
            7,
        ))
    );
    assert!(run.final_tick().unwrap() >= 2);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(StopRequest::new(
            run.final_tick().unwrap(),
            GuestEventId::new(120),
            source,
            7,
        ))]
    );
}

#[test]
fn riscv_system_run_driver_routes_gem5_stats_pseudo_ops_to_host() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(39);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
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
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.write_register(rem6_isa_riscv::Register::new(10).unwrap(), 0);
    core.write_register(rem6_isa_riscv::Register::new(11).unwrap(), 0);
    let cluster = RiscvCluster::new([core]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, i_type(2, 0, 0x0, 10, 0x13)),
        (0x8004, gem5_m5op_type(0x40)),
        (0x8008, i_type(3, 0, 0x0, 10, 0x13)),
        (0x800c, gem5_m5op_type(0x41)),
        (0x8010, i_type(4, 0, 0x0, 10, 0x13)),
        (0x8014, gem5_m5op_type(0x42)),
        (0x8018, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port);
    let mut next_event_id = 130_u64;

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            50,
            |_cpu| {
                let event = GuestEventId::new(next_event_id);
                next_event_id += 1;
                event
            },
        )
        .unwrap();

    assert_eq!(
        run.host_stop(),
        Some(StopRequest::new(
            run.final_tick().unwrap(),
            GuestEventId::new(133),
            source,
            0,
        ))
    );

    let controller = controller.lock().unwrap();
    let stats_actions = controller
        .run()
        .action_outcomes()
        .iter()
        .filter_map(|outcome| match outcome {
            SystemActionOutcome::StatsReset(record) => Some(("reset", record.tick())),
            SystemActionOutcome::StatsDump(record) => Some(("dump", record.tick())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        stats_actions,
        vec![("reset", 16), ("dump", 29), ("dump", 42), ("reset", 42),]
    );
}

#[test]
fn riscv_system_run_driver_routes_gem5_checkpoint_pseudo_op_to_host() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(40);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
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
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.write_register(rem6_isa_riscv::Register::new(10).unwrap(), 0);
    core.write_register(rem6_isa_riscv::Register::new(11).unwrap(), 0);
    let cluster = RiscvCluster::new([core]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, i_type(2, 0, 0x0, 10, 0x13)),
        (0x8004, i_type(0x30, 0, 0x0, 11, 0x13)),
        (0x8008, gem5_m5op_type(0x43)),
        (0x800c, i_type(0, 0, 0x0, 5, 0x13)),
        (0x8010, i_type(0, 0, 0x0, 5, 0x13)),
        (0x8014, i_type(0, 0, 0x0, 5, 0x13)),
        (0x8018, i_type(0, 0, 0x0, 5, 0x13)),
        (0x801c, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port);
    let mut next_event_id = 140_u64;

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            60,
            |_cpu| {
                let event = GuestEventId::new(next_event_id);
                next_event_id += 1;
                event
            },
        )
        .unwrap();

    assert_eq!(
        run.host_stop(),
        Some(StopRequest::new(
            run.final_tick().unwrap(),
            GuestEventId::new(141),
            source,
            0,
        ))
    );

    let controller = controller.lock().unwrap();
    let checkpoints = controller
        .run()
        .action_outcomes()
        .iter()
        .filter_map(|outcome| match outcome {
            SystemActionOutcome::Checkpoint {
                tick,
                event,
                source: actual_source,
                manifest,
            } => Some((*tick, *event, *actual_source, manifest.label().to_string())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        checkpoints,
        vec![(
            22,
            GuestEventId::new(140),
            source,
            "gem5-m5-checkpoint".to_string(),
        )]
    );
}

#[test]
fn riscv_system_run_driver_routes_gem5_hypercall_pseudo_op_to_host() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(41);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
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
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    let cluster = RiscvCluster::new([core]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, i_type(0x321, 0, 0x0, 10, 0x13)),
        (0x8004, gem5_m5op_type(0x71)),
        (0x8008, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port);
    let mut next_event_id = 150_u64;

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            30,
            |_cpu| {
                let event = GuestEventId::new(next_event_id);
                next_event_id += 1;
                event
            },
        )
        .unwrap();

    assert_eq!(
        run.host_stop(),
        Some(StopRequest::new(
            run.final_tick().unwrap(),
            GuestEventId::new(151),
            source,
            0,
        ))
    );

    let controller = controller.lock().unwrap();
    assert_eq!(
        controller.run().action_outcomes(),
        &[
            SystemActionOutcome::GuestHostCall {
                tick: 14,
                event: GuestEventId::new(150),
                source,
                selector: 0x321,
                arguments: Vec::new(),
                payload: Vec::new(),
                response: GuestHostCallResponse::unhandled(),
            },
            SystemActionOutcome::Stop(StopRequest::new(
                run.final_tick().unwrap(),
                GuestEventId::new(151),
                source,
                0,
            )),
        ]
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
        run.parallel_scheduler_batches().len(),
        parallel_epochs
            .iter()
            .map(|epoch| epoch.batches().len())
            .sum::<usize>()
    );
    assert_eq!(
        run.parallel_scheduler_workers().len(),
        parallel_epochs
            .iter()
            .map(|epoch| epoch.total_parallel_workers())
            .sum::<usize>()
    );
    assert_eq!(
        run.parallel_scheduler_worker_partitions().len(),
        run.parallel_scheduler_workers().len()
    );
    let profile = run.parallel_scheduler_profile();
    assert_eq!(profile.epoch_count(), parallel_epochs.len());
    assert_eq!(
        profile.empty_epoch_count(),
        parallel_epochs
            .iter()
            .filter(|epoch| epoch.dispatches().is_empty())
            .count()
    );
    assert_eq!(
        profile.batch_count(),
        run.parallel_scheduler_batches().len()
    );
    assert_eq!(
        profile.dispatch_count(),
        run.parallel_scheduler_dispatches().len()
    );
    assert_eq!(
        profile.total_parallel_workers(),
        run.parallel_scheduler_workers().len()
    );
    assert_eq!(
        profile.max_parallel_workers(),
        run.max_parallel_scheduler_workers()
    );
    assert!(run.max_parallel_scheduler_workers() >= 1);
    assert_eq!(
        run.parallel_scheduler_frontiers().len(),
        parallel_epochs
            .iter()
            .map(|epoch| epoch.frontiers().len())
            .sum::<usize>()
    );
    assert_eq!(
        run.parallel_scheduler_final_frontiers().len(),
        parallel_epochs
            .iter()
            .map(|epoch| epoch.final_frontiers().len())
            .sum::<usize>()
    );
    assert!(run
        .parallel_scheduler_final_frontiers()
        .iter()
        .all(|frontier| Some(frontier.now()) <= run.final_tick()));
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
    assert_eq!(
        run.parallel_scheduler_final_frontiers().len(),
        parallel_epochs
            .iter()
            .map(|epoch| epoch.final_frontiers().len())
            .sum::<usize>()
    );
    assert_eq!(
        run.parallel_scheduler_batches().len(),
        parallel_epochs
            .iter()
            .map(|epoch| epoch.batches().len())
            .sum::<usize>()
    );
    assert_eq!(
        run.parallel_scheduler_worker_partitions().len(),
        run.parallel_scheduler_workers().len()
    );
    let profile = run.parallel_scheduler_profile();
    assert_eq!(profile.epoch_count(), parallel_epochs.len());
    assert_eq!(
        profile.batch_count(),
        run.parallel_scheduler_batches().len()
    );
    assert_eq!(
        profile.total_parallel_workers(),
        run.parallel_scheduler_workers().len()
    );
    assert_eq!(
        profile.max_parallel_workers(),
        run.max_parallel_scheduler_workers()
    );
    assert!(run.max_parallel_scheduler_workers() >= 1);
    assert!(run
        .parallel_scheduler_dispatches()
        .iter()
        .all(|record| record.kind() == ScheduledEventKind::Parallel));
    let cpu0_activity = run.cpu_activity(CpuId::new(0)).unwrap();
    assert_eq!(cpu0_activity.fetch_issue_count(), 2);
    assert_eq!(cpu0_activity.instruction_execution_count(), 2);
    assert_eq!(cpu0_activity.data_access_issue_count(), 1);
    assert_eq!(cpu0_activity.scheduled_trap_count(), 1);
    assert_eq!(cpu0_activity.total_activity_count(), 6);
    assert!(cpu0_activity.has_core_activity());
    assert!(cpu0_activity.has_trap_activity());
    assert_eq!(run.cpu_activities().len(), 2);
    assert_eq!(run.active_cpu_count(), 2);
    assert!(run.has_cpu_activity(CpuId::new(0)));
    assert!(!run.has_cpu_activity(CpuId::new(2)));
    let partition0_activity = run.partition_activity(PartitionId::new(0)).unwrap();
    assert_eq!(partition0_activity.fetch_issue_count(), 2);
    assert_eq!(partition0_activity.instruction_execution_count(), 2);
    assert_eq!(partition0_activity.data_access_issue_count(), 1);
    assert_eq!(partition0_activity.scheduled_trap_count(), 1);
    assert_eq!(partition0_activity.total_activity_count(), 6);
    assert_eq!(run.partition_activities().len(), 2);
    assert_eq!(run.active_partition_count(), 2);
    assert!(run.has_partition_activity(PartitionId::new(0)));
    assert!(!run.has_partition_activity(PartitionId::new(2)));
    let scheduler_partition0 = run
        .parallel_scheduler_partition_activity(PartitionId::new(0))
        .unwrap();
    assert!(scheduler_partition0.worker_count() >= 1);
    assert!(scheduler_partition0.dispatch_count() >= 1);
    assert!(scheduler_partition0.max_pending_events() >= 1);
    assert!(run.active_parallel_scheduler_partition_count() >= 2);
    assert!(run.has_parallel_scheduler_partition_activity(PartitionId::new(0)));
    assert!(!run.has_parallel_scheduler_partition_activity(PartitionId::new(4)));
}

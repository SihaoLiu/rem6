use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore, RiscvCoreDriveAction,
};
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerContext};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, GuestTrap, GuestTrapKind, HostEventPolicy, RiscvSystemRunDriver,
    RiscvSystemRunStopReason, RiscvTrapEventPort, StopRequest, SystemActionOutcome,
    SystemHostController, SystemHostEventPort,
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

fn loaded_program_store(instructions: &[(u64, u32)]) -> Arc<Mutex<PartitionedMemoryStore>> {
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
    image
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
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

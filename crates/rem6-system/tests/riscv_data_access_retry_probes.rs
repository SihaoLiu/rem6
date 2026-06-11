use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore,
    RiscvDataAccessEventKind,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{ParallelSchedulerContext, PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryResponse, MemoryTargetId,
    PartitionedMemoryStore,
};
use rem6_stats::{MemProbePacketAccess, MemProbePacketKind, ProbePayload, StackDistProbeConfig};
use rem6_system::{
    GuestEventId, GuestSourceId, HostEventPolicy, RiscvDataAccessStats, RiscvSystemRunDriver,
    RiscvTrapEventPort, SystemHostController, SystemHostEventPort,
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

fn responder(
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

fn retry_responder(
) -> impl FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome + Send + 'static
{
    move |delivery, _context| TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
}

fn stack_distance_config() -> StackDistProbeConfig {
    StackDistProbeConfig::builder(16, 16).build().unwrap()
}

#[test]
fn system_run_data_access_stats_emit_retry_response_probe_and_keep_checker_pending() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(51);
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
    let data_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.dmem"),
                PartitionId::new(0),
                endpoint("l1d"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_data_core(
        0,
        0,
        7,
        "cpu0.ifetch",
        fetch_route,
        "cpu0.dmem",
        data_route,
        0x8000,
    );
    core.write_register(reg(2), 0x9800);
    let cluster = RiscvCluster::new([core]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, i_type(0x008, 2, 0x3, 5, 0x03)),
        (0x8004, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        rem6_stats::StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_data_access_stats(
        RiscvDataAccessStats::with_stack_distance(stack_distance_config())
            .with_mem_checker_monitor(),
    );

    let run = driver
        .drive_until_host_stop_parallel(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| retry_responder(),
            40,
            |cpu| GuestEventId::new(210 + u64::from(cpu.get())),
        )
        .unwrap();

    let core_events = cluster.core(CpuId::new(0)).unwrap().data_access_events();
    assert_eq!(
        core_events
            .iter()
            .map(|event| event.kind())
            .collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Retry,
        ]
    );

    let data = run
        .data_access_probes()
        .expect("run should carry data access probe evidence");
    assert_eq!(data.probes().events().len(), 2);
    let packets = data
        .probes()
        .events()
        .iter()
        .map(|event| match event.payload() {
            ProbePayload::MemoryPacket(packet) => *packet,
            _ => panic!("data access probe should emit memory packets"),
        })
        .collect::<Vec<_>>();
    assert_eq!(packets[0].kind(), MemProbePacketKind::Request);
    assert_eq!(packets[1].kind(), MemProbePacketKind::Response);
    assert_eq!(packets[0].access(), MemProbePacketAccess::Read);
    assert_eq!(packets[1].access(), MemProbePacketAccess::Read);
    assert_eq!(packets[0].address(), 0x9808);
    assert_eq!(packets[1].address(), 0x9808);
    assert_eq!(packets[0].size(), 8);
    assert_eq!(packets[1].size(), 8);
    assert_eq!(packets[1].flags(), 1);

    let checker = data
        .mem_checker_monitor()
        .expect("run should carry memory checker monitor evidence");
    assert_eq!(checker.pending().len(), 1);
    assert_eq!(checker.pending()[0].packet_id(), packets[0].packet_id());
    assert_eq!(checker.checker().next_serial(), 2);
    let byte = checker
        .checker()
        .bytes()
        .iter()
        .find(|byte| byte.address() == 0x9808)
        .expect("retrying read should keep byte-level pending state");
    assert_eq!(byte.outstanding_reads().len(), 1);
    assert_eq!(byte.outstanding_reads()[0].serial(), 1);
    assert_eq!(byte.read_observations().len(), 1);
}

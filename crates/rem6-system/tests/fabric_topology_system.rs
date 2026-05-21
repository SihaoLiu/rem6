use std::sync::{Arc, Mutex};

use rem6_accelerator::{
    AcceleratorCommandId, AcceleratorDmaCompletion, AcceleratorDmaCopy, AcceleratorEngineConfig,
    AcceleratorEngineId, AcceleratorPendingDmaWrite, AcceleratorTopologyConfig,
};
use rem6_boot::BootImage;
use rem6_checkpoint::CheckpointComponentId;
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_dram::{DramGeometry, DramTiming};
use rem6_fabric::{
    FabricLinkId, FabricPacket, FabricPacketId, FabricPath, FabricPathHop, VirtualNetworkId,
};
use rem6_gpu::{
    GpuComputeConfig, GpuDeviceId, GpuDmaCompletion, GpuDmaCopy, GpuDmaId, GpuPendingDmaWrite,
    GpuTopologyConfig,
};
use rem6_kernel::{ClockDomain, ParallelRunProfile, PartitionId};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryTargetId,
    PartitionedMemoryStore, ResponseStatus,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, RiscvSystemRunStopReason,
    RiscvTopologyDmaCopy, RiscvTopologyDramConfig, RiscvTopologyHostConfig,
    RiscvTopologyMemoryConfig, RiscvTopologySystem, SystemActionOutcome,
};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, FabricConnectionConfig, PortDirection,
    PortName, Topology, TopologyBuilder,
};
use rem6_transport::{MemoryTrace, MemoryTraceEvent, MemoryTraceKind, TransportEndpointId};

fn component(name: &str) -> ComponentId {
    ComponentId::new(name).unwrap()
}

fn kind(name: &str) -> ComponentKind {
    ComponentKind::new(name).unwrap()
}

fn port(name: &str) -> PortName {
    PortName::new(name).unwrap()
}

fn endpoint(component_name: &str, port_name: &str) -> Endpoint {
    Endpoint::new(component(component_name), port(port_name))
}

fn transport_endpoint(component_name: &str, port_name: &str) -> TransportEndpointId {
    TransportEndpointId::from_topology_endpoint(&endpoint(component_name, port_name)).unwrap()
}

fn clock(period: u64) -> ClockDomain {
    ClockDomain::new(period).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn ecall_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
}

fn fabric(link: &str, bandwidth: u64) -> FabricConnectionConfig {
    FabricConnectionConfig::new(FabricLinkId::new(link).unwrap(), bandwidth)
        .with_virtual_networks(VirtualNetworkId::new(1), VirtualNetworkId::new(2))
}

fn fabric_packet(id: u64, bytes: u64, virtual_network: u16) -> FabricPacket {
    FabricPacket::new(
        FabricPacketId::new(id),
        bytes,
        VirtualNetworkId::new(virtual_network),
    )
    .unwrap()
}

fn fabric_path(link: &str) -> FabricPath {
    FabricPath::new([FabricPathHop::new(FabricLinkId::new(link).unwrap(), 10, 8)
        .unwrap()
        .with_credit_depth(2)
        .unwrap()])
    .unwrap()
}

fn memory_config() -> RiscvTopologyMemoryConfig {
    RiscvTopologyMemoryConfig::new(MemoryTargetId::new(0), layout())
        .add_region(Address::new(0x8000), AccessSize::new(0x1000).unwrap())
}

fn dram_config() -> RiscvTopologyDramConfig {
    RiscvTopologyDramConfig::new(
        MemoryTargetId::new(0),
        layout(),
        DramGeometry::new(2, 64, 16).unwrap(),
        DramTiming::new(5, 7, 11, 3, 2).unwrap(),
    )
    .add_region(Address::new(0x8000), AccessSize::new(0x1000).unwrap())
}

fn core_config(cpu: u32, partition: u32, agent: u32, entry: u64) -> RiscvCoreTopologyConfig {
    let cpu_name = format!("cpu{cpu}");
    RiscvCoreTopologyConfig::new(
        CpuResetState::new(
            CpuId::new(cpu),
            PartitionId::new(partition),
            AgentId::new(agent),
            Address::new(entry),
        ),
        endpoint(&cpu_name, "ifetch"),
        endpoint("mem0", "requests"),
        layout(),
        AccessSize::new(4).unwrap(),
    )
    .with_data(
        endpoint(&cpu_name, "dmem"),
        endpoint("mem0", "requests"),
        layout(),
    )
}

fn cpu_fabric_topology() -> Topology {
    TopologyBuilder::new(3)
        .add_component(
            ComponentSpec::new(
                component("cpu0"),
                kind("cpu"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("ifetch"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("dmem"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mem0"),
                kind("dram"),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_fabric_config(
            endpoint("cpu0", "ifetch"),
            endpoint("mem0", "requests"),
            2,
            3,
            fabric("cpu_mem", 4),
        )
        .unwrap()
        .connect_with_fabric_config(
            endpoint("cpu0", "dmem"),
            endpoint("mem0", "requests"),
            2,
            3,
            fabric("cpu_mem", 4),
        )
        .unwrap()
        .build()
        .unwrap()
}

fn accelerator_fabric_topology() -> Topology {
    TopologyBuilder::new(3)
        .add_component(
            ComponentSpec::new(
                component("cpu0"),
                kind("cpu"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("ifetch"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("dmem"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("accelerator0"),
                kind("accelerator"),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(port("dma"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mem0"),
                kind("dram"),
                PartitionId::new(2),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(
            endpoint("cpu0", "ifetch"),
            endpoint("mem0", "requests"),
            2,
            2,
        )
        .unwrap()
        .connect_with_latencies(endpoint("cpu0", "dmem"), endpoint("mem0", "requests"), 2, 2)
        .unwrap()
        .connect_with_fabric_config(
            endpoint("accelerator0", "dma"),
            endpoint("mem0", "requests"),
            3,
            5,
            fabric("accel_mem", 4),
        )
        .unwrap()
        .build()
        .unwrap()
}

fn heterogeneous_fabric_topology() -> Topology {
    TopologyBuilder::new(4)
        .add_component(
            ComponentSpec::new(
                component("cpu0"),
                kind("cpu"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("ifetch"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("dmem"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("gpu"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("accelerator0"),
                kind("accelerator"),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(port("dma"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("gpu0"),
                kind("gpu"),
                PartitionId::new(2),
                clock(1),
            )
            .add_port(port("control"), PortDirection::Target)
            .unwrap()
            .add_port(port("dma"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mem0"),
                kind("dram"),
                PartitionId::new(3),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(
            endpoint("cpu0", "ifetch"),
            endpoint("mem0", "requests"),
            2,
            2,
        )
        .unwrap()
        .connect_with_latencies(endpoint("cpu0", "dmem"), endpoint("mem0", "requests"), 2, 2)
        .unwrap()
        .connect_with_latencies(endpoint("cpu0", "gpu"), endpoint("gpu0", "control"), 4, 1)
        .unwrap()
        .connect_with_fabric_config(
            endpoint("accelerator0", "dma"),
            endpoint("mem0", "requests"),
            3,
            5,
            fabric("hetero_mem", 4),
        )
        .unwrap()
        .connect_with_fabric_config(
            endpoint("gpu0", "dma"),
            endpoint("mem0", "requests"),
            3,
            5,
            fabric("hetero_mem", 4),
        )
        .unwrap()
        .build()
        .unwrap()
}

fn memory_request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(44), sequence)
}

fn memory_read(sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        memory_request(sequence),
        Address::new(address),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap()
}

fn memory_store() -> PartitionedMemoryStore {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x1000),
            AccessSize::new(0x3000).unwrap(),
        )
        .unwrap();

    let mut source_line = vec![0; 16];
    source_line[4..8].copy_from_slice(&[0x21, 0x32, 0x43, 0x54]);
    source_line[12..16].copy_from_slice(&[0x65, 0x76, 0x87, 0x98]);
    store
        .insert_line(target, Address::new(0x1000), source_line)
        .unwrap();
    store
        .insert_line(target, Address::new(0x3000), vec![0; 16])
        .unwrap();
    store
}

fn accelerator_config(engine: AcceleratorEngineId) -> AcceleratorTopologyConfig {
    AcceleratorTopologyConfig::new(
        AcceleratorEngineConfig::new(engine, PartitionId::new(1), 2).unwrap(),
        endpoint("accelerator0", "dma"),
        endpoint("mem0", "requests"),
    )
}

fn gpu_config(device: GpuDeviceId) -> GpuTopologyConfig {
    GpuTopologyConfig::new(
        GpuComputeConfig::new(device, PartitionId::new(2), 2, 1).unwrap(),
        endpoint("cpu0", "gpu"),
        endpoint("gpu0", "control"),
    )
    .with_memory(endpoint("gpu0", "dma"), endpoint("mem0", "requests"))
}

fn dma_copy(route: rem6_transport::MemoryRouteId, command: u64, source: u64) -> AcceleratorDmaCopy {
    AcceleratorDmaCopy::new(
        AcceleratorCommandId::new(command),
        route,
        memory_read(command * 2, source),
        route,
        memory_request(command * 2 + 1),
        Address::new(0x3008),
    )
    .unwrap()
}

fn gpu_dma_copy(route: rem6_transport::MemoryRouteId, transfer: u64, source: u64) -> GpuDmaCopy {
    GpuDmaCopy::new(
        GpuDmaId::new(transfer),
        route,
        memory_read(transfer * 2, source),
        route,
        memory_request(transfer * 2 + 1),
        Address::new(0x300c),
    )
    .unwrap()
}

fn memory_response(
    memory: &Arc<Mutex<PartitionedMemoryStore>>,
    delivery: &rem6_transport::RequestDelivery,
) -> rem6_transport::TargetOutcome {
    let response = memory
        .lock()
        .unwrap()
        .respond(delivery.request())
        .unwrap()
        .response()
        .cloned()
        .unwrap();
    rem6_transport::TargetOutcome::Respond(response)
}

#[test]
fn topology_system_drives_cpu_fetch_over_declared_fabric_path() {
    let source = GuestSourceId::new(70);
    let system = RiscvTopologySystem::with_min_remote_delay(
        cpu_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 70, 0x8000)]),
        2,
    )
    .unwrap()
    .with_boot_image_memory(memory_config(), &ecall_image())
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(2), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    let fetch_trace = MemoryTrace::new();
    let run = system
        .drive_attached_until_host_stop_parallel(
            fetch_trace.clone(),
            MemoryTrace::new(),
            20,
            |cpu| GuestEventId::new(700 + u64::from(cpu.get())),
        )
        .unwrap();

    let fetch_request = system
        .cluster()
        .core(CpuId::new(0))
        .unwrap()
        .execution_events()[0]
        .fetch()
        .request_id();
    assert_eq!(
        run.stop_reason(),
        RiscvSystemRunStopReason::HostStop(rem6_system::StopRequest::new(
            10,
            GuestEventId::new(700),
            source,
            0,
        )),
    );
    assert_eq!(
        fetch_trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                rem6_transport::MemoryRouteId::new(0),
                transport_endpoint("cpu0", "ifetch"),
                MemoryTraceKind::RequestSent,
                fetch_request,
            ),
            MemoryTraceEvent::request(
                3,
                rem6_transport::MemoryRouteId::new(0),
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                fetch_request,
            ),
            MemoryTraceEvent::response(
                7,
                rem6_transport::MemoryRouteId::new(0),
                transport_endpoint("cpu0", "ifetch"),
                fetch_request,
                ResponseStatus::Completed,
            ),
        ],
    );
}

#[test]
fn topology_host_controller_checkpoints_attached_fabric() {
    let source = GuestSourceId::new(82);
    let system = RiscvTopologySystem::with_min_remote_delay(
        cpu_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 82, 0x8000)]),
        2,
    )
    .unwrap()
    .with_boot_image_memory(memory_config(), &ecall_image())
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(2), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    let host = system.host_controller().unwrap();
    let fabric_component = CheckpointComponentId::new("fabric0").unwrap();
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .fabric_checkpoint_bank()
        .is_some());

    let fetch_trace = MemoryTrace::new();
    system
        .drive_attached_until_host_stop_parallel(fetch_trace, Default::default(), 20, |_| {
            GuestEventId::new(182)
        })
        .unwrap();

    let checkpoint = HostActionRecord::new(
        26,
        PartitionId::new(2),
        PartitionId::new(2),
        GuestEventId::new(183),
        source,
        HostAction::Checkpoint {
            label: "attached-fabric".to_string(),
        },
    );
    let manifest = match host
        .lock()
        .unwrap()
        .executor_mut()
        .apply(&checkpoint)
        .unwrap()
    {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };

    assert!(manifest
        .states()
        .iter()
        .any(|state| state.component() == &fabric_component));
    assert!(
        host.lock()
            .unwrap()
            .executor()
            .checkpoints()
            .chunk(&fabric_component, "fabric")
            .unwrap()
            .len()
            > 16
    );
}

#[test]
fn topology_host_controller_restores_custom_fabric_checkpoint_component() {
    let source = GuestSourceId::new(83);
    let fabric_component = CheckpointComponentId::new("mesh-fabric").unwrap();
    let system = RiscvTopologySystem::with_min_remote_delay(
        cpu_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 83, 0x8000)]),
        2,
    )
    .unwrap()
    .with_boot_image_memory(memory_config(), &ecall_image())
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(2), 2, source)
            .with_fabric_checkpoint_component(fabric_component.clone()),
        StatsRegistry::new(),
    )
    .unwrap();
    let host = system.host_controller().unwrap();
    let fabric = system.transport().fabric().unwrap();
    let path = fabric_path("manual_mesh");
    {
        let mut fabric = fabric.lock().unwrap();
        fabric
            .transmit_batch(
                0,
                [
                    (fabric_packet(1, 8, 1), path.clone()),
                    (fabric_packet(2, 8, 1), path.clone()),
                ],
            )
            .unwrap();
    }
    let fabric_snapshot = fabric.lock().unwrap().lane_snapshots();
    let mut expected = fabric.lock().unwrap().clone();
    let expected_transfer = expected
        .transmit(1, fabric_packet(3, 8, 1), path.clone())
        .unwrap();
    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .fabric_checkpoint_bank()
            .unwrap()
            .components(),
        vec![fabric_component.clone()]
    );

    let checkpoint = HostActionRecord::new(
        28,
        PartitionId::new(2),
        PartitionId::new(2),
        GuestEventId::new(184),
        source,
        HostAction::Checkpoint {
            label: "custom-fabric".to_string(),
        },
    );
    let checkpoint = host
        .lock()
        .unwrap()
        .executor_mut()
        .apply(&checkpoint)
        .unwrap();
    let SystemActionOutcome::Checkpoint { manifest, .. } = &checkpoint else {
        panic!("checkpoint outcome expected");
    };
    assert!(manifest
        .states()
        .iter()
        .any(|state| state.component() == &fabric_component));

    fabric
        .lock()
        .unwrap()
        .transmit(20, fabric_packet(9, 8, 1), path.clone())
        .unwrap();
    assert_ne!(fabric.lock().unwrap().lane_snapshots(), fabric_snapshot);

    let restore = HostActionRecord::new(
        36,
        PartitionId::new(2),
        PartitionId::new(2),
        GuestEventId::new(185),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(fabric.lock().unwrap().lane_snapshots(), fabric_snapshot);
    let replayed = fabric
        .lock()
        .unwrap()
        .transmit(1, fabric_packet(3, 8, 1), path)
        .unwrap();
    assert_eq!(replayed, expected_transfer);
    assert_eq!(
        fabric.lock().unwrap().lane_snapshots(),
        expected.lane_snapshots()
    );
}

#[test]
fn topology_system_composes_declared_fabric_path_with_dram_timing() {
    let source = GuestSourceId::new(71);
    let system = RiscvTopologySystem::with_min_remote_delay(
        cpu_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 71, 0x8000)]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(dram_config(), &ecall_image())
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(2), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    let fetch_trace = MemoryTrace::new();
    let run = system
        .drive_attached_until_host_stop_parallel(
            fetch_trace.clone(),
            MemoryTrace::new(),
            30,
            |cpu| GuestEventId::new(710 + u64::from(cpu.get())),
        )
        .unwrap();

    let fetch_request = system
        .cluster()
        .core(CpuId::new(0))
        .unwrap()
        .execution_events()[0]
        .fetch()
        .request_id();
    assert_eq!(
        run.stop_reason(),
        RiscvSystemRunStopReason::HostStop(rem6_system::StopRequest::new(
            22,
            GuestEventId::new(710),
            source,
            0,
        )),
    );
    assert_eq!(
        fetch_trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                rem6_transport::MemoryRouteId::new(0),
                transport_endpoint("cpu0", "ifetch"),
                MemoryTraceKind::RequestSent,
                fetch_request,
            ),
            MemoryTraceEvent::request(
                3,
                rem6_transport::MemoryRouteId::new(0),
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                fetch_request,
            ),
            MemoryTraceEvent::response(
                19,
                rem6_transport::MemoryRouteId::new(0),
                transport_endpoint("cpu0", "ifetch"),
                fetch_request,
                ResponseStatus::Completed,
            ),
        ],
    );
}

#[test]
fn topology_system_runs_accelerator_dma_copy_over_declared_fabric_path() {
    let accelerator_id = AcceleratorEngineId::new(72);
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        accelerator_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 72, 0x8000)]),
        2,
    )
    .unwrap()
    .with_memory_store(memory_store())
    .unwrap()
    .with_accelerator(accelerator_config(accelerator_id))
    .unwrap();
    let route = system.accelerator(accelerator_id).unwrap().dma_route();
    let trace = MemoryTrace::new();

    system
        .run_accelerator_dma_copy_parallel(
            accelerator_id,
            dma_copy(route, 300, 0x1004),
            trace.clone(),
        )
        .unwrap();

    let destination = system
        .memory_store()
        .unwrap()
        .lock()
        .unwrap()
        .line_data(MemoryTargetId::new(0), Address::new(0x3000))
        .unwrap();
    assert_eq!(&destination[8..12], &[0x21, 0x32, 0x43, 0x54]);
    assert_eq!(
        system
            .accelerator(accelerator_id)
            .unwrap()
            .engine()
            .dma_completions(),
        vec![AcceleratorDmaCompletion::new(
            AcceleratorCommandId::new(300),
            memory_request(600),
            memory_request(601),
            10,
            20,
        )],
    );
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                route,
                transport_endpoint("accelerator0", "dma"),
                MemoryTraceKind::RequestSent,
                memory_request(600),
            ),
            MemoryTraceEvent::request(
                4,
                route,
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                memory_request(600),
            ),
            MemoryTraceEvent::response(
                10,
                route,
                transport_endpoint("accelerator0", "dma"),
                memory_request(600),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::request(
                10,
                route,
                transport_endpoint("accelerator0", "dma"),
                MemoryTraceKind::RequestSent,
                memory_request(601),
            ),
            MemoryTraceEvent::request(
                14,
                route,
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                memory_request(601),
            ),
            MemoryTraceEvent::response(
                20,
                route,
                transport_endpoint("accelerator0", "dma"),
                memory_request(601),
                ResponseStatus::Completed,
            ),
        ],
    );
}

#[test]
fn topology_system_reserves_shared_fabric_for_concurrent_accelerator_dma_reads() {
    let accelerator_id = AcceleratorEngineId::new(73);
    let system = RiscvTopologySystem::with_min_remote_delay(
        accelerator_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 73, 0x8000)]),
        2,
    )
    .unwrap()
    .with_memory_store(memory_store())
    .unwrap()
    .with_accelerator(accelerator_config(accelerator_id))
    .unwrap();
    let route = system.accelerator(accelerator_id).unwrap().dma_route();
    let accelerator = system.accelerator(accelerator_id).unwrap().engine().clone();
    let memory = Arc::clone(system.memory_store().unwrap());
    let trace = MemoryTrace::new();

    {
        let (_cluster, mut scheduler, transport) = system.execution_parts_mut();
        let first_memory = Arc::clone(&memory);
        accelerator
            .submit_dma_copy_read(
                &mut scheduler,
                transport,
                dma_copy(route, 310, 0x1004),
                trace.clone(),
                move |delivery, _context| memory_response(&first_memory, &delivery),
            )
            .unwrap();
        let second_memory = Arc::clone(&memory);
        accelerator
            .submit_dma_copy_read(
                &mut scheduler,
                transport,
                dma_copy(route, 311, 0x100c),
                trace.clone(),
                move |delivery, _context| memory_response(&second_memory, &delivery),
            )
            .unwrap();
        scheduler.run_until_idle_parallel().unwrap();
    }

    assert_eq!(
        accelerator.pending_dma_writes(),
        vec![
            AcceleratorPendingDmaWrite::new(
                dma_copy(route, 310, 0x1004),
                vec![0x21, 0x32, 0x43, 0x54],
                10,
            ),
            AcceleratorPendingDmaWrite::new(
                dma_copy(route, 311, 0x100c),
                vec![0x65, 0x76, 0x87, 0x98],
                11,
            ),
        ],
    );
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                route,
                transport_endpoint("accelerator0", "dma"),
                MemoryTraceKind::RequestSent,
                memory_request(620),
            ),
            MemoryTraceEvent::request(
                0,
                route,
                transport_endpoint("accelerator0", "dma"),
                MemoryTraceKind::RequestSent,
                memory_request(622),
            ),
            MemoryTraceEvent::request(
                4,
                route,
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                memory_request(620),
            ),
            MemoryTraceEvent::request(
                5,
                route,
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                memory_request(622),
            ),
            MemoryTraceEvent::response(
                10,
                route,
                transport_endpoint("accelerator0", "dma"),
                memory_request(620),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                11,
                route,
                transport_endpoint("accelerator0", "dma"),
                memory_request(622),
                ResponseStatus::Completed,
            ),
        ],
    );
}

#[test]
fn topology_system_batches_gpu_and_accelerator_dma_reads_on_shared_fabric() {
    let accelerator_id = AcceleratorEngineId::new(75);
    let gpu_id = GpuDeviceId::new(76);
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        heterogeneous_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 75, 0x8000)]),
        2,
    )
    .unwrap()
    .with_memory_store(memory_store())
    .unwrap()
    .with_accelerator(accelerator_config(accelerator_id))
    .unwrap()
    .with_gpu(gpu_config(gpu_id))
    .unwrap();
    let accelerator_route = system.accelerator(accelerator_id).unwrap().dma_route();
    let gpu_route = system.gpu(gpu_id).unwrap().memory_route().unwrap();
    let trace = MemoryTrace::new();

    system
        .run_dma_copy_reads_parallel(
            [
                RiscvTopologyDmaCopy::gpu(gpu_id, gpu_dma_copy(gpu_route, 400, 0x100c)),
                RiscvTopologyDmaCopy::accelerator(
                    accelerator_id,
                    dma_copy(accelerator_route, 330, 0x1004),
                ),
            ],
            trace.clone(),
        )
        .unwrap();

    assert_eq!(
        system
            .accelerator(accelerator_id)
            .unwrap()
            .engine()
            .pending_dma_writes(),
        vec![AcceleratorPendingDmaWrite::new(
            dma_copy(accelerator_route, 330, 0x1004),
            vec![0x21, 0x32, 0x43, 0x54],
            10,
        )],
    );
    assert_eq!(
        system.gpu(gpu_id).unwrap().gpu().pending_dma_writes(),
        vec![GpuPendingDmaWrite::new(
            gpu_dma_copy(gpu_route, 400, 0x100c),
            vec![0x65, 0x76, 0x87, 0x98],
            11,
        )],
    );
    let events = trace.snapshot();
    assert_eq!(events.len(), 6);
    let mut source_events = events[..2].to_vec();
    source_events.sort_by_key(|event| (event.route().get(), event.request_id().sequence()));
    assert_eq!(
        source_events,
        vec![
            MemoryTraceEvent::request(
                0,
                accelerator_route,
                transport_endpoint("accelerator0", "dma"),
                MemoryTraceKind::RequestSent,
                memory_request(660),
            ),
            MemoryTraceEvent::request(
                0,
                gpu_route,
                transport_endpoint("gpu0", "dma"),
                MemoryTraceKind::RequestSent,
                memory_request(800),
            ),
        ],
    );
    assert_eq!(
        &events[2..],
        &[
            MemoryTraceEvent::request(
                4,
                accelerator_route,
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                memory_request(660),
            ),
            MemoryTraceEvent::request(
                5,
                gpu_route,
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                memory_request(800),
            ),
            MemoryTraceEvent::response(
                10,
                accelerator_route,
                transport_endpoint("accelerator0", "dma"),
                memory_request(660),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                11,
                gpu_route,
                transport_endpoint("gpu0", "dma"),
                memory_request(800),
                ResponseStatus::Completed,
            ),
        ],
    );
}

#[test]
fn topology_system_batches_gpu_and_accelerator_dma_copies_on_shared_fabric() {
    let accelerator_id = AcceleratorEngineId::new(79);
    let gpu_id = GpuDeviceId::new(80);
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        heterogeneous_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 79, 0x8000)]),
        2,
    )
    .unwrap()
    .with_memory_store(memory_store())
    .unwrap()
    .with_accelerator(accelerator_config(accelerator_id))
    .unwrap()
    .with_gpu(gpu_config(gpu_id))
    .unwrap();
    let accelerator_route = system.accelerator(accelerator_id).unwrap().dma_route();
    let gpu_route = system.gpu(gpu_id).unwrap().memory_route().unwrap();
    let trace = MemoryTrace::new();

    let summary = system
        .run_dma_copies_parallel_recorded(
            [
                RiscvTopologyDmaCopy::gpu(gpu_id, gpu_dma_copy(gpu_route, 410, 0x100c)),
                RiscvTopologyDmaCopy::accelerator(
                    accelerator_id,
                    dma_copy(accelerator_route, 340, 0x1004),
                ),
            ],
            trace.clone(),
        )
        .unwrap();

    assert_eq!(summary.read().event_count(), 2);
    assert_eq!(summary.read().trace_event_count(), 4);
    assert_eq!(summary.read().pending_dma_write_count(), 2);
    assert_eq!(summary.read().dma_completion_count(), 0);
    assert_eq!(summary.read().final_tick(), 12);
    assert_eq!(
        summary.read().profile(),
        ParallelRunProfile::new(
            summary.read().epoch_count(),
            summary.read().empty_epoch_count(),
            summary.read().batch_count(),
            summary.read().dispatch_count(),
            summary.read().total_parallel_workers(),
            summary.read().max_parallel_workers(),
        )
    );
    assert_eq!(summary.write().event_count(), 2);
    assert_eq!(summary.write().trace_event_count(), 4);
    assert_eq!(summary.write().pending_dma_write_count(), 0);
    assert_eq!(summary.write().dma_completion_count(), 2);
    assert_eq!(summary.write().final_tick(), 24);
    assert_eq!(
        summary.profile(),
        summary.read().profile().merge(summary.write().profile()),
    );
    assert_eq!(summary.event_count(), 4);
    assert_eq!(summary.trace_event_count(), 8);
    assert_eq!(summary.dma_completion_count(), 2);
    assert_eq!(summary.pending_dma_write_count(), 0);
    assert_eq!(summary.final_tick(), 24);
    assert!(summary.has_parallel_work());
    assert!(summary.has_dma_activity());

    let destination = system
        .memory_store()
        .unwrap()
        .lock()
        .unwrap()
        .line_data(MemoryTargetId::new(0), Address::new(0x3000))
        .unwrap();
    assert_eq!(&destination[8..12], &[0x21, 0x32, 0x43, 0x54]);
    assert_eq!(&destination[12..16], &[0x65, 0x76, 0x87, 0x98]);
    assert_eq!(
        system
            .accelerator(accelerator_id)
            .unwrap()
            .engine()
            .dma_completions(),
        vec![AcceleratorDmaCompletion::new(
            AcceleratorCommandId::new(340),
            memory_request(680),
            memory_request(681),
            10,
            22,
        )],
    );
    assert_eq!(
        system.gpu(gpu_id).unwrap().gpu().dma_completions(),
        vec![GpuDmaCompletion::new(
            GpuDmaId::new(410),
            memory_request(820),
            memory_request(821),
            11,
            23,
        )],
    );

    let events = trace.snapshot();
    assert_eq!(events.len(), 12);
    let mut read_sources = events[..2].to_vec();
    read_sources.sort_by_key(|event| (event.route().get(), event.request_id().sequence()));
    assert_eq!(
        read_sources,
        vec![
            MemoryTraceEvent::request(
                0,
                accelerator_route,
                transport_endpoint("accelerator0", "dma"),
                MemoryTraceKind::RequestSent,
                memory_request(680),
            ),
            MemoryTraceEvent::request(
                0,
                gpu_route,
                transport_endpoint("gpu0", "dma"),
                MemoryTraceKind::RequestSent,
                memory_request(820),
            ),
        ],
    );
    assert_eq!(
        &events[2..6],
        &[
            MemoryTraceEvent::request(
                4,
                accelerator_route,
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                memory_request(680),
            ),
            MemoryTraceEvent::request(
                5,
                gpu_route,
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                memory_request(820),
            ),
            MemoryTraceEvent::response(
                10,
                accelerator_route,
                transport_endpoint("accelerator0", "dma"),
                memory_request(680),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                11,
                gpu_route,
                transport_endpoint("gpu0", "dma"),
                memory_request(820),
                ResponseStatus::Completed,
            ),
        ],
    );
    let mut write_sources = events[6..8].to_vec();
    write_sources.sort_by_key(|event| (event.route().get(), event.request_id().sequence()));
    assert_eq!(
        write_sources,
        vec![
            MemoryTraceEvent::request(
                12,
                accelerator_route,
                transport_endpoint("accelerator0", "dma"),
                MemoryTraceKind::RequestSent,
                memory_request(681),
            ),
            MemoryTraceEvent::request(
                12,
                gpu_route,
                transport_endpoint("gpu0", "dma"),
                MemoryTraceKind::RequestSent,
                memory_request(821),
            ),
        ],
    );
    assert_eq!(
        &events[8..],
        &[
            MemoryTraceEvent::request(
                16,
                accelerator_route,
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                memory_request(681),
            ),
            MemoryTraceEvent::request(
                17,
                gpu_route,
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                memory_request(821),
            ),
            MemoryTraceEvent::response(
                22,
                accelerator_route,
                transport_endpoint("accelerator0", "dma"),
                memory_request(681),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                23,
                gpu_route,
                transport_endpoint("gpu0", "dma"),
                memory_request(821),
                ResponseStatus::Completed,
            ),
        ],
    );
}

#[test]
fn topology_system_treats_empty_dma_read_batch_as_noop_without_memory_backend() {
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        heterogeneous_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 77, 0x8000)]),
        2,
    )
    .unwrap()
    .with_accelerator(accelerator_config(AcceleratorEngineId::new(77)))
    .unwrap()
    .with_gpu(gpu_config(GpuDeviceId::new(78)))
    .unwrap();
    let trace = MemoryTrace::new();

    let events = system
        .run_dma_copy_reads_parallel(std::iter::empty::<RiscvTopologyDmaCopy>(), trace.clone())
        .unwrap();

    assert!(events.is_empty());
    assert!(trace.is_empty());
    assert_eq!(system.scheduler().now(), 0);
    assert!(system.scheduler().is_idle());
    assert!(system
        .accelerator(AcceleratorEngineId::new(77))
        .unwrap()
        .engine()
        .trace()
        .is_empty());
    assert!(system
        .gpu(GpuDeviceId::new(78))
        .unwrap()
        .gpu()
        .trace()
        .is_empty());
}

#[test]
fn topology_system_empty_dma_read_batch_does_not_drive_pending_scheduler_work() {
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        heterogeneous_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 81, 0x8000)]),
        2,
    )
    .unwrap()
    .with_accelerator(accelerator_config(AcceleratorEngineId::new(81)))
    .unwrap()
    .with_gpu(gpu_config(GpuDeviceId::new(82)))
    .unwrap();
    system
        .scheduler_mut()
        .schedule_parallel_at(PartitionId::new(0), 5, |_| {})
        .unwrap();
    let trace = MemoryTrace::new();

    let events = system
        .run_dma_copy_reads_parallel(std::iter::empty::<RiscvTopologyDmaCopy>(), trace.clone())
        .unwrap();

    assert!(events.is_empty());
    assert!(trace.is_empty());
    assert_eq!(system.scheduler().now(), 0);
    assert!(!system.scheduler().is_idle());
}

#[test]
fn topology_system_empty_recorded_dma_copy_batch_does_not_drive_pending_scheduler_work() {
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        heterogeneous_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 83, 0x8000)]),
        2,
    )
    .unwrap()
    .with_accelerator(accelerator_config(AcceleratorEngineId::new(83)))
    .unwrap()
    .with_gpu(gpu_config(GpuDeviceId::new(84)))
    .unwrap();
    system
        .scheduler_mut()
        .schedule_parallel_at(PartitionId::new(0), 5, |_| {})
        .unwrap();
    let trace = MemoryTrace::new();

    let summary = system
        .run_dma_copies_parallel_recorded(std::iter::empty::<RiscvTopologyDmaCopy>(), trace.clone())
        .unwrap();

    assert_eq!(summary.event_count(), 0);
    assert_eq!(summary.trace_event_count(), 0);
    assert_eq!(summary.dma_completion_count(), 0);
    assert_eq!(summary.pending_dma_write_count(), 0);
    assert_eq!(summary.final_tick(), 0);
    assert!(!summary.has_parallel_work());
    assert!(!summary.has_dma_activity());
    assert!(trace.is_empty());
    assert_eq!(system.scheduler().now(), 0);
    assert!(!system.scheduler().is_idle());
}

#[test]
fn topology_system_keeps_fabric_route_data_mutation_after_write_completion() {
    let accelerator_id = AcceleratorEngineId::new(74);
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        accelerator_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 74, 0x8000)]),
        2,
    )
    .unwrap()
    .with_memory_store(memory_store())
    .unwrap()
    .with_accelerator(accelerator_config(accelerator_id))
    .unwrap();
    let route = system.accelerator(accelerator_id).unwrap().dma_route();

    system
        .run_accelerator_dma_copy_parallel(
            accelerator_id,
            AcceleratorDmaCopy::new(
                AcceleratorCommandId::new(320),
                route,
                memory_read(640, 0x1004),
                route,
                memory_request(641),
                Address::new(0x300c),
            )
            .unwrap(),
            MemoryTrace::new(),
        )
        .unwrap();

    assert_eq!(
        &system
            .memory_store()
            .unwrap()
            .lock()
            .unwrap()
            .line_data(MemoryTargetId::new(0), Address::new(0x3000))
            .unwrap()[12..16],
        &[0x21, 0x32, 0x43, 0x54],
    );
}

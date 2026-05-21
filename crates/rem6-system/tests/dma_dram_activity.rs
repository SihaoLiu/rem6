use rem6_accelerator::{
    AcceleratorCommandId, AcceleratorDmaCopy, AcceleratorEngineConfig, AcceleratorEngineId,
    AcceleratorTopologyConfig,
};
use rem6_boot::BootImage;
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_dram::{DramGeometry, DramTiming};
use rem6_fabric::{FabricLinkId, VirtualNetworkId};
use rem6_gpu::{GpuComputeConfig, GpuDeviceId, GpuDmaCopy, GpuDmaId, GpuTopologyConfig};
use rem6_kernel::{PartitionId, WaitForEdgeKind};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryTargetId,
};
use rem6_system::{RiscvTopologyDmaCopy, RiscvTopologyDramConfig, RiscvTopologySystem};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, FabricConnectionConfig, PortDirection,
    PortName, Topology, TopologyBuilder,
};
use rem6_transport::MemoryTrace;

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

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
}

fn core_config(cpu: u32, partition: u32, agent: u32) -> RiscvCoreTopologyConfig {
    let cpu_name = format!("cpu{cpu}");
    RiscvCoreTopologyConfig::new(
        CpuResetState::new(
            CpuId::new(cpu),
            PartitionId::new(partition),
            AgentId::new(agent),
            Address::new(0x8000),
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

fn fabric(link: &str, bandwidth: u64) -> FabricConnectionConfig {
    FabricConnectionConfig::new(FabricLinkId::new(link).unwrap(), bandwidth)
        .with_virtual_networks(VirtualNetworkId::new(1), VirtualNetworkId::new(2))
}

fn accelerator_fabric_topology() -> Topology {
    TopologyBuilder::new(3)
        .add_component(
            ComponentSpec::new(
                component("cpu0"),
                kind("cpu"),
                PartitionId::new(0),
                rem6_kernel::ClockDomain::new(1).unwrap(),
            )
            .add_port(port("ifetch"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("dmem"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("accel"), PortDirection::Initiator)
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
                rem6_kernel::ClockDomain::new(1).unwrap(),
            )
            .add_port(port("dma"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("control"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mem0"),
                kind("dram"),
                PartitionId::new(2),
                rem6_kernel::ClockDomain::new(1).unwrap(),
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
                rem6_kernel::ClockDomain::new(1).unwrap(),
            )
            .add_port(port("ifetch"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("dmem"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("accel"), PortDirection::Initiator)
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
                rem6_kernel::ClockDomain::new(1).unwrap(),
            )
            .add_port(port("dma"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("control"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("gpu0"),
                kind("gpu"),
                PartitionId::new(2),
                rem6_kernel::ClockDomain::new(1).unwrap(),
            )
            .add_port(port("dma"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("control"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mem0"),
                kind("dram"),
                PartitionId::new(3),
                rem6_kernel::ClockDomain::new(1).unwrap(),
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
        .connect_with_latencies(
            endpoint("cpu0", "accel"),
            endpoint("accelerator0", "control"),
            4,
            1,
        )
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

fn dram_config() -> RiscvTopologyDramConfig {
    RiscvTopologyDramConfig::new(
        MemoryTargetId::new(0),
        layout(),
        DramGeometry::new(2, 64, 16).unwrap(),
        DramTiming::new(5, 7, 11, 3, 2).unwrap(),
    )
    .add_region(Address::new(0x8000), AccessSize::new(0x1000).unwrap())
    .add_region(Address::new(0x1000), AccessSize::new(0x3000).unwrap())
}

fn split_dram_config() -> RiscvTopologyDramConfig {
    RiscvTopologyDramConfig::new(
        MemoryTargetId::new(0),
        layout(),
        DramGeometry::new(2, 64, 16).unwrap(),
        DramTiming::new(5, 7, 11, 3, 2).unwrap(),
    )
    .add_region(Address::new(0x8000), AccessSize::new(0x1000).unwrap())
    .add_region(Address::new(0x1000), AccessSize::new(0x1000).unwrap())
    .add_target(
        MemoryTargetId::new(1),
        layout(),
        DramGeometry::new(4, 64, 16).unwrap(),
        DramTiming::new(3, 5, 9, 2, 2).unwrap(),
    )
    .unwrap()
    .add_region_for_target(
        MemoryTargetId::new(1),
        Address::new(0x3000),
        AccessSize::new(0x1000).unwrap(),
    )
    .unwrap()
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

fn accelerator_dma_copy(
    route: rem6_transport::MemoryRouteId,
    command: u64,
    source: u64,
    destination: u64,
) -> AcceleratorDmaCopy {
    AcceleratorDmaCopy::new(
        AcceleratorCommandId::new(command),
        route,
        memory_read(command * 2, source),
        route,
        memory_request(command * 2 + 1),
        Address::new(destination),
    )
    .unwrap()
}

fn gpu_dma_copy(
    route: rem6_transport::MemoryRouteId,
    transfer: u64,
    source: u64,
    destination: u64,
) -> GpuDmaCopy {
    GpuDmaCopy::new(
        GpuDmaId::new(transfer),
        route,
        memory_read(transfer * 2, source),
        route,
        memory_request(transfer * 2 + 1),
        Address::new(destination),
    )
    .unwrap()
}

fn seed_dram(system: &RiscvTopologySystem, destination_target: MemoryTargetId) {
    let mut source_line = vec![0; 16];
    source_line[4..8].copy_from_slice(&[0x21, 0x32, 0x43, 0x54]);
    source_line[12..16].copy_from_slice(&[0x65, 0x76, 0x87, 0x98]);

    let dram = system.dram_memory_controller().unwrap();
    let mut dram = dram.lock().unwrap();
    dram.insert_line(MemoryTargetId::new(0), Address::new(0x1000), source_line)
        .unwrap();
    dram.insert_line(destination_target, Address::new(0x3000), vec![0; 16])
        .unwrap();
}

#[test]
fn dma_summary_merges_shared_target_dram_activity_for_multiple_devices() {
    let accelerator_id = AcceleratorEngineId::new(31);
    let gpu_id = GpuDeviceId::new(32);
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        heterogeneous_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 31)]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(dram_config(), &boot_image())
    .unwrap()
    .with_accelerator(accelerator_config(accelerator_id))
    .unwrap()
    .with_gpu(gpu_config(gpu_id))
    .unwrap();
    seed_dram(&system, MemoryTargetId::new(0));
    let accelerator_route = system.accelerator(accelerator_id).unwrap().dma_route();
    let gpu_route = system.gpu(gpu_id).unwrap().memory_route().unwrap();

    let summary = system
        .run_dma_copies_parallel_recorded(
            [
                RiscvTopologyDmaCopy::accelerator(
                    accelerator_id,
                    accelerator_dma_copy(accelerator_route, 70, 0x1004, 0x3008),
                ),
                RiscvTopologyDmaCopy::gpu(gpu_id, gpu_dma_copy(gpu_route, 71, 0x100c, 0x300c)),
            ],
            MemoryTrace::new(),
        )
        .unwrap();

    assert_eq!(summary.read().active_dram_target_count(), 1);
    assert_eq!(summary.read().dram_profile().access_count(), 2);
    assert_eq!(summary.read().dram_profile().read_count(), 2);
    assert_eq!(summary.read().dram_profile().write_count(), 0);
    assert!(summary.read().has_dram_wait_for_edges());
    assert_eq!(
        summary
            .read()
            .dram_wait_for_edge_count_by_kind(WaitForEdgeKind::Queue),
        1,
    );
    assert_eq!(summary.write().active_dram_target_count(), 1);
    assert_eq!(summary.write().dram_profile().access_count(), 2);
    assert_eq!(summary.write().dram_profile().read_count(), 0);
    assert_eq!(summary.write().dram_profile().write_count(), 2);
    assert!(summary.write().has_dram_wait_for_edges());
    assert_eq!(
        summary
            .write()
            .dram_wait_for_edge_count_by_kind(WaitForEdgeKind::Queue),
        1,
    );
    assert_eq!(summary.active_dram_target_count(), 1);
    assert_eq!(summary.dram_profile().access_count(), 4);
    assert_eq!(summary.dram_profile().read_count(), 2);
    assert_eq!(summary.dram_profile().write_count(), 2);
    assert!(summary.has_dram_wait_for_edges());
    assert_eq!(
        summary.dram_wait_for_edge_count(),
        summary.read().dram_wait_for_edge_count() + summary.write().dram_wait_for_edge_count(),
    );
    assert_eq!(
        summary.dram_wait_for_edge_count_by_kind(WaitForEdgeKind::Queue),
        summary
            .read()
            .dram_wait_for_edge_count_by_kind(WaitForEdgeKind::Queue)
            + summary
                .write()
                .dram_wait_for_edge_count_by_kind(WaitForEdgeKind::Queue),
    );
    assert!(!summary.dram_wait_for_blocked_nodes().is_empty());
    assert_eq!(
        summary
            .dram_target_activity(MemoryTargetId::new(0))
            .unwrap()
            .profile()
            .access_count(),
        4,
    );
    assert_eq!(summary.read().fabric_profile().transfer_count(), 4);
    assert_eq!(summary.fabric_profile().transfer_count(), 8);
    assert_eq!(
        system.dram_activity_profile().unwrap().access_count(),
        summary.dram_profile().access_count(),
    );
    assert_eq!(
        system
            .accelerator(accelerator_id)
            .unwrap()
            .engine()
            .dma_completions()
            .len(),
        1,
    );
    assert_eq!(system.gpu(gpu_id).unwrap().gpu().dma_completions().len(), 1);

    let destination = system
        .dram_memory_controller()
        .unwrap()
        .lock()
        .unwrap()
        .line_data(MemoryTargetId::new(0), Address::new(0x3000))
        .unwrap();
    assert_eq!(&destination[8..12], &[0x21, 0x32, 0x43, 0x54]);
    assert_eq!(&destination[12..16], &[0x65, 0x76, 0x87, 0x98]);
}

#[test]
fn dma_summary_keeps_split_target_dram_activity_separate() {
    let accelerator_id = AcceleratorEngineId::new(41);
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        accelerator_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 41)]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(split_dram_config(), &boot_image())
    .unwrap()
    .with_accelerator(accelerator_config(accelerator_id))
    .unwrap();
    seed_dram(&system, MemoryTargetId::new(1));
    let route = system.accelerator(accelerator_id).unwrap().dma_route();

    let summary = system
        .run_accelerator_dma_copy_parallel_recorded(
            accelerator_id,
            accelerator_dma_copy(route, 80, 0x1004, 0x3008),
            MemoryTrace::new(),
        )
        .unwrap();

    assert_eq!(summary.read().active_dram_target_count(), 1);
    assert_eq!(summary.write().active_dram_target_count(), 1);
    assert_eq!(
        summary
            .read()
            .dram_target_activity(MemoryTargetId::new(0))
            .unwrap()
            .profile()
            .read_count(),
        1,
    );
    assert!(summary
        .read()
        .dram_target_activity(MemoryTargetId::new(1))
        .is_none());
    assert_eq!(
        summary
            .write()
            .dram_target_activity(MemoryTargetId::new(1))
            .unwrap()
            .profile()
            .write_count(),
        1,
    );
    assert!(summary
        .write()
        .dram_target_activity(MemoryTargetId::new(0))
        .is_none());
    assert_eq!(summary.active_dram_target_count(), 2);
    assert_eq!(summary.dram_target_activities().len(), 2);
    assert_eq!(summary.dram_profile().access_count(), 2);
    assert_eq!(
        summary
            .dram_target_activity(MemoryTargetId::new(0))
            .unwrap()
            .profile()
            .read_count(),
        1,
    );
    assert_eq!(
        summary
            .dram_target_activity(MemoryTargetId::new(1))
            .unwrap()
            .profile()
            .write_count(),
        1,
    );

    let destination = system
        .dram_memory_controller()
        .unwrap()
        .lock()
        .unwrap()
        .line_data(MemoryTargetId::new(1), Address::new(0x3000))
        .unwrap();
    assert_eq!(&destination[8..12], &[0x21, 0x32, 0x43, 0x54]);
}

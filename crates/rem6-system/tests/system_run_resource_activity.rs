use rem6_boot::BootImage;
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_dram::{DramGeometry, DramTiming};
use rem6_isa_riscv::Register;
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, RiscvSystemRun, RiscvSystemRunStopReason, RiscvTopologyDramConfig,
    RiscvTopologyHostConfig, RiscvTopologyMemoryConfig, RiscvTopologySystem, StopRequest,
};
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

fn clock(period: u64) -> ClockDomain {
    ClockDomain::new(period).unwrap()
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

fn fabric(link: &str, bandwidth: u64) -> FabricConnectionConfig {
    FabricConnectionConfig::new(rem6_fabric::FabricLinkId::new(link).unwrap(), bandwidth)
        .with_virtual_networks(
            rem6_fabric::VirtualNetworkId::new(1),
            rem6_fabric::VirtualNetworkId::new(2),
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

fn core_config(agent: u32) -> RiscvCoreTopologyConfig {
    RiscvCoreTopologyConfig::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(agent),
            Address::new(0x8000),
        ),
        endpoint("cpu0", "ifetch"),
        endpoint("mem0", "requests"),
        layout(),
        AccessSize::new(4).unwrap(),
    )
    .with_data(
        endpoint("cpu0", "dmem"),
        endpoint("mem0", "requests"),
        layout(),
    )
}

fn ecall_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
}

fn load_then_ecall_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x8004), word(0x0000_0073))
        .unwrap()
        .add_segment(
            Address::new(0x3008),
            vec![0x89, 0x67, 0x45, 0x23, 0x01, 0xef, 0xcd, 0xab],
        )
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
}

fn memory_config() -> RiscvTopologyMemoryConfig {
    RiscvTopologyMemoryConfig::new(MemoryTargetId::new(0), layout())
        .add_region(Address::new(0x8000), AccessSize::new(0x1000).unwrap())
}

fn split_dram_config() -> RiscvTopologyDramConfig {
    dram_config()
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

#[test]
fn system_run_starts_without_resource_activity() {
    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 0 },
    );

    assert!(!run.has_resource_activity());
    assert!(!run.has_fabric_activity());
    assert!(!run.has_dram_activity());
    assert_eq!(run.resource_activity_count(), 0);
    assert_eq!(run.fabric_transfer_count(), 0);
    assert_eq!(run.dram_access_count(), 0);
    assert_eq!(run.fabric_activities().len(), 0);
    assert_eq!(run.dram_target_activities().len(), 0);
}

#[test]
fn topology_run_reports_fabric_and_dram_activity_for_fetch_window() {
    let source = GuestSourceId::new(91);
    let system = RiscvTopologySystem::with_min_remote_delay(
        cpu_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(91)]),
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

    let run = system
        .drive_attached_until_host_stop_parallel(
            MemoryTrace::new(),
            MemoryTrace::new(),
            30,
            |cpu| GuestEventId::new(910 + u64::from(cpu.get())),
        )
        .unwrap();

    assert_eq!(
        run.stop_reason(),
        RiscvSystemRunStopReason::HostStop(StopRequest::new(
            run.final_tick().unwrap(),
            GuestEventId::new(910),
            source,
            0,
        )),
    );
    assert!(run.has_fabric_activity());
    assert!(run.active_fabric_lane_count() >= 1);
    assert_eq!(
        run.fabric_transfer_count(),
        run.fabric_profile().transfer_count(),
    );
    assert!(run
        .fabric_activity(
            &rem6_fabric::FabricLinkId::new("cpu_mem").unwrap(),
            rem6_fabric::VirtualNetworkId::new(1),
        )
        .is_some());
    assert!(run.has_dram_activity());
    assert_eq!(run.active_dram_target_count(), 1);
    assert_eq!(run.dram_profile().access_count(), 1);
    assert_eq!(run.dram_profile().read_count(), 1);
    assert_eq!(
        run.dram_target_activity(MemoryTargetId::new(0))
            .unwrap()
            .profile()
            .read_count(),
        1,
    );
    assert!(run.has_resource_activity());
    assert_eq!(
        run.resource_activity_count(),
        run.fabric_transfer_count() + run.dram_access_count(),
    );
}

#[test]
fn topology_run_keeps_code_and_data_dram_targets_separate() {
    let source = GuestSourceId::new(92);
    let system = RiscvTopologySystem::with_min_remote_delay(
        cpu_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(92)]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(split_dram_config(), &load_then_ecall_image())
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(2), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    system
        .cluster()
        .core(CpuId::new(0))
        .unwrap()
        .write_register(Register::new(2).unwrap(), 0x3000);

    let run = system
        .drive_attached_until_host_stop_parallel(
            MemoryTrace::new(),
            MemoryTrace::new(),
            40,
            |cpu| GuestEventId::new(920 + u64::from(cpu.get())),
        )
        .unwrap();

    assert_eq!(run.active_dram_target_count(), 2);
    assert_eq!(run.dram_profile().access_count(), 3);
    assert_eq!(run.dram_profile().read_count(), 3);
    assert_eq!(
        run.dram_target_activity(MemoryTargetId::new(0))
            .unwrap()
            .profile()
            .read_count(),
        2,
    );
    assert_eq!(
        run.dram_target_activity(MemoryTargetId::new(1))
            .unwrap()
            .profile()
            .read_count(),
        1,
    );
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(0))
            .unwrap()
            .read_register(Register::new(5).unwrap()),
        0xabcd_ef01_2345_6789,
    );
    assert_eq!(
        system.dram_activity_profile().unwrap().access_count(),
        run.dram_profile().access_count(),
    );
}

#[test]
fn topology_run_reports_fabric_without_dram_for_store_memory() {
    let source = GuestSourceId::new(93);
    let system = RiscvTopologySystem::with_min_remote_delay(
        cpu_fabric_topology(),
        RiscvClusterTopologyConfig::new([core_config(93)]),
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

    let run = system
        .drive_attached_until_host_stop_parallel(
            MemoryTrace::new(),
            MemoryTrace::new(),
            30,
            |cpu| GuestEventId::new(930 + u64::from(cpu.get())),
        )
        .unwrap();

    assert!(run.has_resource_activity());
    assert!(run.has_fabric_activity());
    assert!(!run.has_dram_activity());
    assert!(run.fabric_transfer_count() > 0);
    assert_eq!(run.dram_access_count(), 0);
    assert_eq!(run.dram_target_activities().len(), 0);
    assert_eq!(run.resource_activity_count(), run.fabric_transfer_count());
    assert!(system.memory_store().is_some());
    assert!(system.dram_memory_controller().is_none());
}

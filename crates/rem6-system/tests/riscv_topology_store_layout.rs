use rem6_boot::BootImage;
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_isa_riscv::Register;
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_stats::StatsRegistry;
use rem6_system::{GuestEventId, GuestSourceId, RiscvTopologyHostConfig, RiscvTopologySystem};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, PortDirection, PortName, Topology,
    TopologyBuilder,
};

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

fn layout(bytes: u64) -> CacheLineLayout {
    CacheLineLayout::new(bytes).unwrap()
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

fn j_type(imm: i32, rd: u8) -> u32 {
    let imm = imm as u32;
    (((imm >> 20) & 0x1) << 31)
        | (((imm >> 1) & 0x3ff) << 21)
        | (((imm >> 11) & 0x1) << 20)
        | (((imm >> 12) & 0xff) << 12)
        | (u32::from(rd) << 7)
        | 0x6f
}

fn topology() -> Topology {
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
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mem0"),
                kind("memory"),
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
            3,
        )
        .unwrap()
        .connect_with_latencies(endpoint("cpu0", "dmem"), endpoint("mem0", "requests"), 2, 3)
        .unwrap()
        .build()
        .unwrap()
}

fn core_config(entry: u64) -> RiscvCoreTopologyConfig {
    RiscvCoreTopologyConfig::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(7),
            Address::new(entry),
        ),
        endpoint("cpu0", "ifetch"),
        endpoint("mem0", "requests"),
        layout(16),
        AccessSize::new(4).unwrap(),
    )
    .with_data(
        endpoint("cpu0", "dmem"),
        endpoint("mem0", "requests"),
        layout(16),
    )
}

fn two_target_store(image: &BootImage, second_region: Address) -> PartitionedMemoryStore {
    let mut store = PartitionedMemoryStore::new();
    store
        .add_partition(MemoryTargetId::new(0), layout(16))
        .unwrap();
    store
        .map_region(
            MemoryTargetId::new(0),
            Address::new(0x8000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    store
        .add_partition(MemoryTargetId::new(1), layout(32))
        .unwrap();
    store
        .map_region(
            MemoryTargetId::new(1),
            second_region,
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    image
        .load_into_partitioned_store_by_address(&mut store)
        .unwrap();
    store
}

fn system_with_store(image: &BootImage, second_region: Address) -> RiscvTopologySystem {
    RiscvTopologySystem::with_min_remote_delay(
        topology(),
        RiscvClusterTopologyConfig::new([core_config(image.entry().get())]),
        2,
    )
    .unwrap()
    .with_memory_store(two_target_store(image, second_region))
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(3), 2, GuestSourceId::new(61)),
        StatsRegistry::new(),
    )
    .unwrap()
}

#[test]
fn topology_store_fetch_uses_addressed_target_layout_after_pc_redirect() {
    let image = BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(j_type(0x1000, 0)))
        .unwrap()
        .add_segment(Address::new(0x9000), word(0x0000_0073))
        .unwrap();
    let system = system_with_store(&image, Address::new(0x9000));

    let run = system
        .drive_attached_until_host_stop_parallel(
            Default::default(),
            Default::default(),
            50,
            |cpu| GuestEventId::new(210 + u64::from(cpu.get())),
        )
        .unwrap();

    assert_eq!(
        run.host_stop().map(|stop| stop.event()),
        Some(GuestEventId::new(210))
    );
    assert_eq!(
        system.cluster().core(CpuId::new(0)).unwrap().pc(),
        Address::new(0x9000)
    );
}

#[test]
fn topology_store_data_access_uses_addressed_target_layout() {
    let image = BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x8004), word(0x0000_0073))
        .unwrap()
        .add_segment(
            Address::new(0xa008),
            0x8877_6655_4433_2211_u64.to_le_bytes().to_vec(),
        )
        .unwrap();
    let system = system_with_store(&image, Address::new(0xa000));
    system
        .cluster()
        .core(CpuId::new(0))
        .unwrap()
        .write_register(Register::new(2).unwrap(), 0xa000);

    let run = system
        .drive_attached_until_host_stop_parallel(
            Default::default(),
            Default::default(),
            50,
            |cpu| GuestEventId::new(220 + u64::from(cpu.get())),
        )
        .unwrap();

    assert_eq!(
        run.host_stop().map(|stop| stop.event()),
        Some(GuestEventId::new(220))
    );
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(0))
            .unwrap()
            .read_register(Register::new(5).unwrap()),
        0x8877_6655_4433_2211
    );
}

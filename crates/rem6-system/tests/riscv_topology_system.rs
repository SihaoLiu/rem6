use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_platform::{PlatformBuilder, PlatformTopologyRoute, PlatformUartConfig};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostEventPolicy, RiscvSystemRunDriver, RiscvSystemRunStopReason,
    RiscvTopologyHostConfig, RiscvTopologyMemoryConfig, RiscvTopologySystem, RiscvTrapEventPort,
    StopRequest, SystemHostController, SystemHostEventPort,
};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, PortDirection, PortName, Topology,
    TopologyBuilder,
};
use rem6_transport::{RequestDelivery, TargetOutcome};
use rem6_uart::{UartId, UART_MMIO_DATA_OFFSET};

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

fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32, opcode: u32) -> u32 {
    let imm = imm as u32;
    ((imm & 0xfe0) << 20)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | opcode
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
                component("cpu1"),
                kind("cpu"),
                PartitionId::new(1),
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
        .connect_with_latencies(
            endpoint("cpu1", "ifetch"),
            endpoint("mem0", "requests"),
            2,
            3,
        )
        .unwrap()
        .connect_with_latencies(endpoint("cpu1", "dmem"), endpoint("mem0", "requests"), 2, 3)
        .unwrap()
        .build()
        .unwrap()
}

fn topology_with_uart() -> Topology {
    TopologyBuilder::new(5)
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
            .add_port(port("mmio"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("cpu1"),
                kind("cpu"),
                PartitionId::new(1),
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
                PartitionId::new(2),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("uart0"),
                kind("uart"),
                PartitionId::new(3),
                clock(1),
            )
            .add_port(port("mmio"), PortDirection::Target)
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
        .connect_with_latencies(
            endpoint("cpu1", "ifetch"),
            endpoint("mem0", "requests"),
            2,
            3,
        )
        .unwrap()
        .connect_with_latencies(endpoint("cpu1", "dmem"), endpoint("mem0", "requests"), 2, 3)
        .unwrap()
        .connect_with_latencies(endpoint("cpu0", "mmio"), endpoint("uart0", "mmio"), 2, 2)
        .unwrap()
        .build()
        .unwrap()
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

fn loaded_program_store(
    instructions: &[(u64, u32)],
    data_segments: &[(u64, Vec<u8>)],
) -> Arc<Mutex<PartitionedMemoryStore>> {
    Arc::new(Mutex::new(program_store(instructions, data_segments)))
}

fn program_store(
    instructions: &[(u64, u32)],
    data_segments: &[(u64, Vec<u8>)],
) -> PartitionedMemoryStore {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x3000).unwrap(),
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
    store
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
fn topology_system_builds_cluster_and_drives_parallel_host_stop() {
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        topology(),
        RiscvClusterTopologyConfig::new([
            core_config(0, 0, 7, 0x8000),
            core_config(1, 1, 8, 0x9000),
        ]),
        2,
    )
    .unwrap();
    assert_eq!(system.scheduler().partition_count(), 4);
    assert_eq!(system.scheduler().min_remote_delay(), 2);
    assert_eq!(system.transport().route_count(), 4);
    assert_eq!(system.cluster().core_count(), 2);

    let store = loaded_program_store(
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
    system
        .cluster()
        .core(CpuId::new(0))
        .unwrap()
        .write_register(rem6_isa_riscv::Register::new(2).unwrap(), 0x9800);
    system
        .cluster()
        .core(CpuId::new(1))
        .unwrap()
        .write_register(rem6_isa_riscv::Register::new(2).unwrap(), 0x9810);

    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let source = GuestSourceId::new(41);
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(PartitionId::new(3), 2, Arc::clone(&controller))
            .unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port);
    let (cluster, scheduler, transport) = system.execution_parts_mut();

    let run = driver
        .drive_until_host_stop_parallel(
            cluster,
            scheduler,
            transport,
            Default::default(),
            Default::default(),
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context: &mut rem6_kernel::ParallelSchedulerContext<'_>| {
                    memory_response(&store, &delivery)
                }
            },
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context: &mut rem6_kernel::ParallelSchedulerContext<'_>| {
                    memory_response(&store, &delivery)
                }
            },
            30,
            |cpu| GuestEventId::new(120 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(120), source, 0);
    assert_eq!(run.stop_reason(), RiscvSystemRunStopReason::HostStop(stop));
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(0))
            .unwrap()
            .read_register(rem6_isa_riscv::Register::new(5).unwrap()),
        0xfedc_ba98_7654_3210
    );
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(1))
            .unwrap()
            .read_register(rem6_isa_riscv::Register::new(5).unwrap()),
        0xabcd_ef01_2345_6789
    );
}

#[test]
fn topology_system_with_platform_drives_parallel_mmio_and_memory_accesses() {
    let topology = topology_with_uart();
    let uart_id = UartId::new(1);
    let uart_route =
        PlatformTopologyRoute::new(endpoint("cpu0", "mmio"), endpoint("uart0", "mmio"))
            .resolve(&topology)
            .unwrap();
    let platform = PlatformBuilder::from_topology(&topology)
        .add_uart(PlatformUartConfig {
            id: uart_id,
            base: Address::new(0xa000),
            size: AccessSize::new(0x100).unwrap(),
            route: uart_route,
            interrupt_line: rem6_interrupt::InterruptLineId::new(40),
            interrupt_target: rem6_interrupt::InterruptTargetId::new(0),
            interrupt_source: rem6_interrupt::InterruptSourceId::new(50),
            interrupt_latency: 2,
        })
        .build()
        .unwrap();

    let image = BootImage::new(Address::new(0x8000))
        .add_segment(
            Address::new(0x8000),
            word(i_type(b'R'.into(), 0, 0x0, 3, 0x13)),
        )
        .unwrap()
        .add_segment(
            Address::new(0x8004),
            word(s_type(UART_MMIO_DATA_OFFSET as i32, 3, 2, 0x0, 0x23)),
        )
        .unwrap()
        .add_segment(Address::new(0x8008), word(0x0000_0073))
        .unwrap()
        .add_segment(Address::new(0x9000), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x9004), word(0x0010_0073))
        .unwrap()
        .add_segment(
            Address::new(0x9818),
            vec![0x89, 0x67, 0x45, 0x23, 0x01, 0xef, 0xcd, 0xab],
        )
        .unwrap();
    let memory = RiscvTopologyMemoryConfig::new(MemoryTargetId::new(0), layout())
        .add_region(Address::new(0x8000), AccessSize::new(0x3000).unwrap());
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        topology,
        RiscvClusterTopologyConfig::new([
            core_config(0, 0, 7, 0x8000),
            core_config(1, 1, 8, 0x9000),
        ]),
        2,
    )
    .unwrap()
    .with_boot_image_memory(memory, &image)
    .unwrap()
    .with_platform(platform)
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(4), 2, GuestSourceId::new(42)),
        StatsRegistry::new(),
    )
    .unwrap();
    assert!(system.platform().is_some());
    assert!(system.platform_bus().is_some());
    assert!(system.host_controller().is_some());
    assert!(system.memory_store().is_some());
    system
        .cluster()
        .core(CpuId::new(0))
        .unwrap()
        .write_register(rem6_isa_riscv::Register::new(2).unwrap(), 0xa000);
    system
        .cluster()
        .core(CpuId::new(1))
        .unwrap()
        .write_register(rem6_isa_riscv::Register::new(2).unwrap(), 0x9810);

    let run = system
        .drive_attached_until_host_stop_parallel(
            Default::default(),
            Default::default(),
            40,
            |cpu| GuestEventId::new(140 + u64::from(cpu.get())),
        )
        .unwrap();

    let source = GuestSourceId::new(42);
    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(141), source, 1);
    assert_eq!(run.stop_reason(), RiscvSystemRunStopReason::HostStop(stop));
    assert_eq!(
        system
            .host_controller()
            .unwrap()
            .lock()
            .unwrap()
            .run()
            .stop_request(),
        Some(&stop)
    );
    assert_eq!(
        system
            .platform()
            .unwrap()
            .uart(uart_id)
            .unwrap()
            .snapshot()
            .tx_bytes()
            .iter()
            .map(|byte| byte.byte())
            .collect::<Vec<_>>(),
        vec![b'R']
    );
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(1))
            .unwrap()
            .read_register(rem6_isa_riscv::Register::new(5).unwrap()),
        0xabcd_ef01_2345_6789
    );
}

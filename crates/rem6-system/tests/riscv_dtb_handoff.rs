use rem6_boot::BootImage;
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_dram::{DramGeometry, DramTiming};
use rem6_isa_riscv::Register;
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId};
use rem6_platform::{
    Platform, PlatformBuilder, PlatformClintConfig, PlatformClintHartConfig,
    PlatformRiscvDeviceTreeConfig,
};
use rem6_system::{
    RiscvLinuxBootHandoffConfig, RiscvLinuxInitrdImage, RiscvTopologyDramConfig,
    RiscvTopologyMemoryConfig, RiscvTopologySystem, RiscvTopologySystemError,
};
use rem6_timer::{ClintId, ClintResetPolicy};
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

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
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

fn platform_with_two_hart_clint(topology: &Topology, clint_id: ClintId) -> Platform {
    PlatformBuilder::from_topology(topology)
        .add_clint(PlatformClintConfig {
            id: clint_id,
            base: Address::new(0x200_0000),
            size: AccessSize::new(0x1_0000).unwrap(),
            route: rem6_mmio::MmioRoute::new(PartitionId::new(0), PartitionId::new(3), 2, 2)
                .unwrap(),
            reset_policy: ClintResetPolicy::preserve_mtimecmp(),
            harts: vec![
                PlatformClintHartConfig {
                    hart: 0,
                    target_partition: PartitionId::new(0),
                    interrupt_target: rem6_interrupt::InterruptTargetId::new(0),
                    software_interrupt_line: rem6_interrupt::InterruptLineId::new(42),
                    software_interrupt_source: rem6_interrupt::InterruptSourceId::new(52),
                    timer_interrupt_line: rem6_interrupt::InterruptLineId::new(43),
                    timer_interrupt_source: rem6_interrupt::InterruptSourceId::new(53),
                    interrupt_latency: 2,
                },
                PlatformClintHartConfig {
                    hart: 1,
                    target_partition: PartitionId::new(1),
                    interrupt_target: rem6_interrupt::InterruptTargetId::new(1),
                    software_interrupt_line: rem6_interrupt::InterruptLineId::new(44),
                    software_interrupt_source: rem6_interrupt::InterruptSourceId::new(54),
                    timer_interrupt_line: rem6_interrupt::InterruptLineId::new(45),
                    timer_interrupt_source: rem6_interrupt::InterruptSourceId::new(55),
                    interrupt_latency: 2,
                },
            ],
        })
        .build()
        .unwrap()
}

fn device_tree_config() -> PlatformRiscvDeviceTreeConfig {
    PlatformRiscvDeviceTreeConfig::new(10_000_000, "rv64imafdc", "riscv,sv48", 0x384000).unwrap()
}

fn cluster_config() -> RiscvClusterTopologyConfig {
    RiscvClusterTopologyConfig::new([
        core_config(0, 0, 7, 0x8000_0000),
        core_config(1, 1, 8, 0x8000_0000),
    ])
}

fn assert_a1_handoff(system: &RiscvTopologySystem, dtb_addr: Address) {
    let a1 = Register::new(11).unwrap();
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(0))
            .unwrap()
            .read_register(a1),
        dtb_addr.get()
    );
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(1))
            .unwrap()
            .read_register(a1),
        dtb_addr.get()
    );
}

fn read_store_blob(system: &RiscvTopologySystem, start: Address, len: usize) -> Vec<u8> {
    let mut cursor = start.get();
    let mut bytes = Vec::with_capacity(len);
    let memory = system.memory_store().unwrap().lock().unwrap();
    while bytes.len() < len {
        let address = Address::new(cursor);
        let line = layout().line_address(address);
        let offset = layout().line_offset(address) as usize;
        let data = memory.line_data(MemoryTargetId::new(0), line).unwrap();
        let take = (data.len() - offset).min(len - bytes.len());
        bytes.extend_from_slice(&data[offset..offset + take]);
        cursor += take as u64;
    }
    bytes
}

fn read_dram_blob(system: &RiscvTopologySystem, start: Address, len: usize) -> Vec<u8> {
    let mut cursor = start.get();
    let mut bytes = Vec::with_capacity(len);
    let controller = system.dram_memory_controller().unwrap().lock().unwrap();
    while bytes.len() < len {
        let address = Address::new(cursor);
        let line = layout().line_address(address);
        let offset = layout().line_offset(address) as usize;
        let data = controller.line_data(MemoryTargetId::new(0), line).unwrap();
        let take = (data.len() - offset).min(len - bytes.len());
        bytes.extend_from_slice(&data[offset..offset + take]);
        cursor += take as u64;
    }
    bytes
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[test]
fn riscv_device_tree_handoff_sets_a0_hart_id_and_a1_dtb_pointer() {
    let topology = topology();
    let platform = platform_with_two_hart_clint(&topology, ClintId::new(0));
    let memory = RiscvTopologyMemoryConfig::new(MemoryTargetId::new(0), layout()).add_region(
        Address::new(0x8000_0000),
        AccessSize::new(0x1000_0000).unwrap(),
    );
    let image = BootImage::new(Address::new(0x8000_0000));
    let system = RiscvTopologySystem::with_min_remote_delay(topology, cluster_config(), 2)
        .unwrap()
        .with_boot_image_memory(memory, &image)
        .unwrap()
        .with_platform(platform)
        .unwrap();
    let dtb_addr = Address::new(0x87e0_0000);

    let report = system
        .install_riscv_device_tree_handoff(&device_tree_config(), dtb_addr)
        .unwrap();

    assert_eq!(report.dtb_addr(), dtb_addr);
    assert!(report.dtb_len() > 40);
    assert_eq!(report.load_report().writes()[0].line(), dtb_addr);

    let line = system
        .memory_store()
        .unwrap()
        .lock()
        .unwrap()
        .line_data(MemoryTargetId::new(0), dtb_addr)
        .unwrap();
    assert_eq!(&line[..4], &[0xd0, 0x0d, 0xfe, 0xed]);

    let a0 = Register::new(10).unwrap();
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(0))
            .unwrap()
            .read_register(a0),
        0
    );
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(1))
            .unwrap()
            .read_register(a0),
        1
    );
    assert_a1_handoff(&system, dtb_addr);
}

#[test]
fn topology_system_installs_linux_boot_data_in_riscv_device_tree_handoff() {
    let topology = topology();
    let platform = platform_with_two_hart_clint(&topology, ClintId::new(2));
    let memory = RiscvTopologyMemoryConfig::new(MemoryTargetId::new(0), layout()).add_region(
        Address::new(0x8000_0000),
        AccessSize::new(0x1000_0000).unwrap(),
    );
    let image = BootImage::new(Address::new(0x8000_0000));
    let system = RiscvTopologySystem::with_min_remote_delay(topology, cluster_config(), 2)
        .unwrap()
        .with_boot_image_memory(memory, &image)
        .unwrap()
        .with_platform(platform)
        .unwrap();
    let bootargs = "console=ttyS0 root=/dev/vda";
    let config = device_tree_config()
        .with_bootargs(bootargs)
        .with_initrd(Address::new(0x8800_0000), AccessSize::new(0x2000).unwrap())
        .unwrap();
    let dtb_addr = Address::new(0x87e0_0000);

    let report = system
        .install_riscv_device_tree_handoff(&config, dtb_addr)
        .unwrap();

    let dtb = read_store_blob(&system, dtb_addr, report.dtb_len());
    assert!(contains_bytes(&dtb, b"chosen\0"));
    assert!(contains_bytes(&dtb, format!("{bootargs}\0").as_bytes()));
    assert!(contains_bytes(&dtb, b"bootargs\0"));
    assert!(contains_bytes(&dtb, b"linux,initrd-start\0"));
    assert!(contains_bytes(&dtb, b"linux,initrd-end\0"));
    assert!(contains_bytes(&dtb, &0x8800_0000_u64.to_be_bytes()));
    assert!(contains_bytes(&dtb, &0x8800_2000_u64.to_be_bytes()));
    assert_a1_handoff(&system, dtb_addr);
}

#[test]
fn topology_system_installs_riscv_linux_boot_handoff_with_initrd_blob() {
    let topology = topology();
    let platform = platform_with_two_hart_clint(&topology, ClintId::new(3));
    let memory = RiscvTopologyMemoryConfig::new(MemoryTargetId::new(0), layout()).add_region(
        Address::new(0x8000_0000),
        AccessSize::new(0x1000_0000).unwrap(),
    );
    let image = BootImage::new(Address::new(0x8000_0000));
    let system = RiscvTopologySystem::with_min_remote_delay(topology, cluster_config(), 2)
        .unwrap()
        .with_boot_image_memory(memory, &image)
        .unwrap()
        .with_platform(platform)
        .unwrap();
    let bootargs = "console=ttyS0 root=/dev/vda";
    let initrd_addr = Address::new(0x8800_0008);
    let initrd_data = (0..24).map(|byte| byte as u8 + 0xa0).collect::<Vec<_>>();
    let dtb_addr = Address::new(0x87e0_0000);
    let handoff =
        RiscvLinuxBootHandoffConfig::new(device_tree_config().with_bootargs(bootargs), dtb_addr)
            .with_initrd(RiscvLinuxInitrdImage::new(initrd_addr, initrd_data.clone()).unwrap());

    let report = system.install_riscv_linux_boot_handoff(&handoff).unwrap();

    assert_eq!(report.dtb().dtb_addr(), dtb_addr);
    assert_eq!(
        report.initrd_load_report().unwrap().writes()[0].line(),
        Address::new(0x8800_0000)
    );
    assert_eq!(
        read_store_blob(&system, initrd_addr, initrd_data.len()),
        initrd_data
    );

    let dtb = read_store_blob(&system, dtb_addr, report.dtb().dtb_len());
    assert!(contains_bytes(&dtb, format!("{bootargs}\0").as_bytes()));
    assert!(contains_bytes(&dtb, &initrd_addr.get().to_be_bytes()));
    assert!(contains_bytes(
        &dtb,
        &(initrd_addr.get() + initrd_data.len() as u64).to_be_bytes()
    ));
    assert_a1_handoff(&system, dtb_addr);
}

#[test]
fn riscv_linux_boot_handoff_does_not_write_initrd_when_dtb_validation_fails() {
    let topology = topology();
    let memory = RiscvTopologyMemoryConfig::new(MemoryTargetId::new(0), layout()).add_region(
        Address::new(0x8000_0000),
        AccessSize::new(0x1000_0000).unwrap(),
    );
    let initrd_addr = Address::new(0x8800_0008);
    let sentinel = vec![0x11; 24];
    let image = BootImage::new(Address::new(0x8000_0000))
        .add_segment(initrd_addr, sentinel.clone())
        .unwrap();
    let system = RiscvTopologySystem::with_min_remote_delay(topology, cluster_config(), 2)
        .unwrap()
        .with_boot_image_memory(memory, &image)
        .unwrap();
    let handoff = RiscvLinuxBootHandoffConfig::new(device_tree_config(), Address::new(0x87e0_0000))
        .with_initrd(
            RiscvLinuxInitrdImage::new(
                initrd_addr,
                (0..24).map(|byte| byte as u8 + 0xa0).collect(),
            )
            .unwrap(),
        );

    let error = system
        .install_riscv_linux_boot_handoff(&handoff)
        .unwrap_err();

    assert!(matches!(error, RiscvTopologySystemError::MissingPlatform));
    assert_eq!(
        read_store_blob(&system, initrd_addr, sentinel.len()),
        sentinel
    );
}

#[test]
fn topology_system_installs_riscv_linux_boot_handoff_with_initrd_blob_into_dram() {
    let topology = topology();
    let platform = platform_with_two_hart_clint(&topology, ClintId::new(4));
    let dram = RiscvTopologyDramConfig::new(
        MemoryTargetId::new(0),
        layout(),
        DramGeometry::new(2, 64, 16).unwrap(),
        DramTiming::new(5, 7, 11, 3, 2).unwrap(),
    )
    .add_region(
        Address::new(0x8000_0000),
        AccessSize::new(0x1000_0000).unwrap(),
    );
    let image = BootImage::new(Address::new(0x8000_0000));
    let system = RiscvTopologySystem::with_min_remote_delay(topology, cluster_config(), 2)
        .unwrap()
        .with_boot_image_dram_memory(dram, &image)
        .unwrap()
        .with_platform(platform)
        .unwrap();
    let bootargs = "console=ttyS0 root=/dev/vda rw";
    let initrd_addr = Address::new(0x8800_0010);
    let initrd_data = (0..32).map(|byte| byte as u8 + 0x40).collect::<Vec<_>>();
    let dtb_addr = Address::new(0x87e0_0000);
    let handoff =
        RiscvLinuxBootHandoffConfig::new(device_tree_config().with_bootargs(bootargs), dtb_addr)
            .with_initrd(RiscvLinuxInitrdImage::new(initrd_addr, initrd_data.clone()).unwrap());

    let report = system.install_riscv_linux_boot_handoff(&handoff).unwrap();

    assert_eq!(
        report.initrd_load_report().unwrap().writes()[0].line(),
        initrd_addr
    );
    assert_eq!(
        read_dram_blob(&system, initrd_addr, initrd_data.len()),
        initrd_data
    );

    let dtb = read_dram_blob(&system, dtb_addr, report.dtb().dtb_len());
    assert!(contains_bytes(&dtb, format!("{bootargs}\0").as_bytes()));
    assert!(contains_bytes(&dtb, &initrd_addr.get().to_be_bytes()));
    assert!(contains_bytes(
        &dtb,
        &(initrd_addr.get() + initrd_data.len() as u64).to_be_bytes()
    ));
    assert_a1_handoff(&system, dtb_addr);
}

#[test]
fn topology_system_installs_riscv_device_tree_handoff_into_dram() {
    let topology = topology();
    let platform = platform_with_two_hart_clint(&topology, ClintId::new(1));
    let dram = RiscvTopologyDramConfig::new(
        MemoryTargetId::new(0),
        layout(),
        DramGeometry::new(2, 64, 16).unwrap(),
        DramTiming::new(5, 7, 11, 3, 2).unwrap(),
    )
    .add_region(
        Address::new(0x8000_0000),
        AccessSize::new(0x1000_0000).unwrap(),
    );
    let image = BootImage::new(Address::new(0x8000_0000));
    let system = RiscvTopologySystem::with_min_remote_delay(topology, cluster_config(), 2)
        .unwrap()
        .with_boot_image_dram_memory(dram, &image)
        .unwrap()
        .with_platform(platform)
        .unwrap();
    let dtb_addr = Address::new(0x87e0_0000);

    let report = system
        .install_riscv_device_tree_handoff(&device_tree_config(), dtb_addr)
        .unwrap();

    assert_eq!(report.dtb_addr(), dtb_addr);
    assert!(report.dtb_len() > 40);
    assert_eq!(report.load_report().writes()[0].line(), dtb_addr);

    let line = system
        .dram_memory_controller()
        .unwrap()
        .lock()
        .unwrap()
        .line_data(MemoryTargetId::new(0), dtb_addr)
        .unwrap();
    assert_eq!(&line[..4], &[0xd0, 0x0d, 0xfe, 0xed]);
    assert_a1_handoff(&system, dtb_addr);
}

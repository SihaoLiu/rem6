use rem6_interrupt::{
    InterruptError, InterruptEvent, InterruptEventKind, InterruptLineId, InterruptSourceId,
    InterruptTargetId, PLIC_MMIO_CLAIM_COMPLETE_OFFSET, PLIC_MMIO_CONTEXT_BASE_OFFSET,
    PLIC_MMIO_ENABLE_BASE_OFFSET, PLIC_MMIO_PENDING_BASE_OFFSET, PLIC_MMIO_REGISTER_BYTES,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{MmioCompletion, MmioError, MmioRequest, MmioRequestId, MmioResponse, MmioRoute};
use rem6_platform::{
    PlatformBuilder, PlatformClintConfig, PlatformClintHartConfig, PlatformError,
    PlatformInterruptControllerConfig, PlatformRiscvDeviceTreeConfig, PlatformTimerConfig,
    PlatformTopologyError, PlatformTopologyRoute, PlatformUartConfig,
};
use rem6_timer::{
    ClintId, ClintResetPolicy, TimerArm, TimerExpiry, TimerId, CLINT_MSIP_BASE_OFFSET,
    CLINT_MSIP_REGISTER_BYTES, CLINT_MSIP_STRIDE, CLINT_MTIMECMP_BASE_OFFSET,
    CLINT_MTIMECMP_REGISTER_BYTES, CLINT_MTIMECMP_STRIDE, TIMER_MMIO_DEADLINE_OFFSET,
};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, PortDirection, PortName, Topology,
    TopologyBuilder, TopologyError,
};
use rem6_uart::{UartId, UartRxByte, UartTxByte, UART_MMIO_DATA_OFFSET};

fn full_mask(bytes: u64) -> ByteMask {
    ByteMask::full(AccessSize::new(bytes).unwrap()).unwrap()
}

fn le64(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn le32(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

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

fn topology_clock(period: u64) -> rem6_kernel::ClockDomain {
    rem6_kernel::ClockDomain::new(period).unwrap()
}

fn platform_topology() -> Topology {
    TopologyBuilder::new(3)
        .add_component(
            ComponentSpec::new(
                component("cpu"),
                kind("cpu"),
                PartitionId::new(0),
                topology_clock(1),
            )
            .add_port(port("mmio"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("bus"),
                kind("bus"),
                PartitionId::new(0),
                topology_clock(1),
            )
            .add_port(port("cpu_in"), PortDirection::Target)
            .unwrap()
            .add_port(port("timer_out"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("uart_out"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("timer"),
                kind("timer"),
                PartitionId::new(1),
                topology_clock(1),
            )
            .add_port(port("mmio"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("uart"),
                kind("uart"),
                PartitionId::new(2),
                topology_clock(1),
            )
            .add_port(port("mmio"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(endpoint("cpu", "mmio"), endpoint("bus", "cpu_in"), 2, 1)
        .unwrap()
        .connect_with_latencies(
            endpoint("bus", "timer_out"),
            endpoint("timer", "mmio"),
            3,
            2,
        )
        .unwrap()
        .connect_with_latencies(endpoint("bus", "uart_out"), endpoint("uart", "mmio"), 5, 4)
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn platform_builder_wires_timer_uart_interrupts_and_mmio_bus() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let uart_partition = PartitionId::new(2);
    let timer_id = TimerId::new(1);
    let uart_id = UartId::new(2);
    let timer_line = InterruptLineId::new(20);
    let uart_line = InterruptLineId::new(21);
    let timer_source = InterruptSourceId::new(30);
    let uart_source = InterruptSourceId::new(31);

    let platform = PlatformBuilder::new(3)
        .add_timer(PlatformTimerConfig {
            id: timer_id,
            base: Address::new(0x5000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu, timer_partition, 2, 1).unwrap(),
            interrupt_line: timer_line,
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: timer_source,
            interrupt_latency: 2,
        })
        .add_uart(PlatformUartConfig {
            id: uart_id,
            base: Address::new(0x6000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu, uart_partition, 2, 1).unwrap(),
            interrupt_line: uart_line,
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: uart_source,
            interrupt_latency: 2,
        })
        .build()
        .unwrap();

    let timer = platform.timer(timer_id).unwrap().clone();
    let uart = platform.uart(uart_id).unwrap().clone();
    let controller = platform.interrupt_controller();
    let bus = platform.mmio_bus().clone();
    let completions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(platform.partition_count()).unwrap();

    let uart_input = uart.clone();
    scheduler
        .schedule_at(uart_partition, 2, move |context| {
            uart_input.inject_rx_after(context, 2, [b'R']).unwrap();
        })
        .unwrap();

    let completed = std::sync::Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 1, move |context| {
            let timer_completed = std::sync::Arc::clone(&completed);
            bus.submit(
                context,
                MmioRequest::write(
                    MmioRequestId::new(1),
                    Address::new(0x5000 + TIMER_MMIO_DEADLINE_OFFSET),
                    le64(7),
                    full_mask(8),
                )
                .unwrap(),
                move |completion| timer_completed.lock().unwrap().push(completion),
            )
            .unwrap();

            bus.submit(
                context,
                MmioRequest::write(
                    MmioRequestId::new(2),
                    Address::new(0x6000 + UART_MMIO_DATA_OFFSET),
                    vec![b'B'],
                    full_mask(1),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 10);
    assert_eq!(summary.final_tick(), 9);
    assert_eq!(timer.snapshot().arms(), &[TimerArm::new(1, 3, 7)]);
    assert_eq!(timer.snapshot().expiries(), &[TimerExpiry::new(1, 7)]);
    assert_eq!(uart.snapshot().rx_injected(), &[UartRxByte::new(4, b'R')]);
    assert_eq!(uart.snapshot().tx_bytes(), &[UartTxByte::new(3, b'B')]);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[
            MmioCompletion::new(
                4,
                MmioRoute::new(cpu, timer_partition, 2, 1).unwrap(),
                Ok(MmioResponse::completed(MmioRequestId::new(1), None)),
            ),
            MmioCompletion::new(
                4,
                MmioRoute::new(cpu, uart_partition, 2, 1).unwrap(),
                Ok(MmioResponse::completed(MmioRequestId::new(2), None)),
            ),
        ]
    );
    assert_eq!(
        controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(
                6,
                uart_line,
                InterruptTargetId::new(0),
                cpu,
                uart_source,
                InterruptEventKind::Assert,
            ),
            InterruptEvent::routed(
                9,
                timer_line,
                InterruptTargetId::new(0),
                cpu,
                timer_source,
                InterruptEventKind::Assert,
            ),
        ]
    );
}

#[test]
fn platform_builder_wires_clint_hart_interrupts_and_mmio_bus() {
    let cpu0 = PartitionId::new(0);
    let cpu1 = PartitionId::new(1);
    let clint_partition = PartitionId::new(2);
    let clint_id = ClintId::new(0);
    let software_line = InterruptLineId::new(30);
    let timer_line = InterruptLineId::new(31);
    let software_source = InterruptSourceId::new(40);
    let timer_source = InterruptSourceId::new(41);

    let platform = PlatformBuilder::new(3)
        .add_clint(PlatformClintConfig {
            id: clint_id,
            base: Address::new(0x200_0000),
            size: AccessSize::new(0x1_0000).unwrap(),
            route: MmioRoute::new(cpu0, clint_partition, 2, 1).unwrap(),
            reset_policy: ClintResetPolicy::reset_mtimecmp_to(99),
            harts: vec![PlatformClintHartConfig {
                hart: 1,
                target_partition: cpu1,
                interrupt_target: InterruptTargetId::new(0),
                software_interrupt_line: software_line,
                software_interrupt_source: software_source,
                timer_interrupt_line: timer_line,
                timer_interrupt_source: timer_source,
                interrupt_latency: 2,
            }],
        })
        .build()
        .unwrap();

    let clint = platform.clint(clint_id).unwrap().clone();
    let controller = platform.interrupt_controller();
    let bus = platform.mmio_bus().clone();
    let completions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(platform.partition_count()).unwrap();

    let completed = std::sync::Arc::clone(&completions);
    scheduler
        .schedule_at(cpu0, 1, move |context| {
            let msip_completed = std::sync::Arc::clone(&completed);
            bus.submit(
                context,
                MmioRequest::write(
                    MmioRequestId::new(20),
                    Address::new(0x200_0000 + CLINT_MSIP_BASE_OFFSET + CLINT_MSIP_STRIDE),
                    le32(1),
                    full_mask(CLINT_MSIP_REGISTER_BYTES),
                )
                .unwrap(),
                move |completion| msip_completed.lock().unwrap().push(completion),
            )
            .unwrap();

            bus.submit(
                context,
                MmioRequest::write(
                    MmioRequestId::new(21),
                    Address::new(0x200_0000 + CLINT_MTIMECMP_BASE_OFFSET + CLINT_MTIMECMP_STRIDE),
                    le64(7),
                    full_mask(CLINT_MTIMECMP_REGISTER_BYTES),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();
    let snapshot = clint.snapshot();

    assert_eq!(summary.final_tick(), 9);
    assert_eq!(snapshot.harts()[0].hart(), 1);
    assert_eq!(snapshot.harts()[0].msip(), 1);
    assert_eq!(snapshot.harts()[0].mtimecmp(), 7);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[
            MmioCompletion::new(
                4,
                MmioRoute::new(cpu0, clint_partition, 2, 1).unwrap(),
                Ok(MmioResponse::completed(MmioRequestId::new(20), None)),
            ),
            MmioCompletion::new(
                4,
                MmioRoute::new(cpu0, clint_partition, 2, 1).unwrap(),
                Ok(MmioResponse::completed(MmioRequestId::new(21), None)),
            ),
        ]
    );
    assert_eq!(
        controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(
                5,
                software_line,
                InterruptTargetId::new(0),
                cpu1,
                software_source,
                InterruptEventKind::Assert,
            ),
            InterruptEvent::routed(
                9,
                timer_line,
                InterruptTargetId::new(0),
                cpu1,
                timer_source,
                InterruptEventKind::Assert,
            ),
        ]
    );
}

#[test]
fn platform_builder_emits_typed_riscv_device_tree_for_clint_and_uart() {
    let cpu0 = PartitionId::new(0);
    let cpu1 = PartitionId::new(1);
    let clint_partition = PartitionId::new(2);
    let uart_partition = PartitionId::new(3);
    let controller_target = InterruptTargetId::new(0);

    let platform = PlatformBuilder::new(4)
        .add_interrupt_controller(PlatformInterruptControllerConfig {
            base: Address::new(0x0c00_0000),
            size: AccessSize::new(0x400_0000).unwrap(),
            route: MmioRoute::new(cpu0, cpu0, 1, 1).unwrap(),
            target: controller_target,
        })
        .add_clint(PlatformClintConfig {
            id: ClintId::new(0),
            base: Address::new(0x0200_0000),
            size: AccessSize::new(0x1_0000).unwrap(),
            route: MmioRoute::new(cpu0, clint_partition, 2, 1).unwrap(),
            reset_policy: ClintResetPolicy::preserve_mtimecmp(),
            harts: vec![
                PlatformClintHartConfig {
                    hart: 0,
                    target_partition: cpu0,
                    interrupt_target: InterruptTargetId::new(0),
                    software_interrupt_line: InterruptLineId::new(60),
                    software_interrupt_source: InterruptSourceId::new(70),
                    timer_interrupt_line: InterruptLineId::new(61),
                    timer_interrupt_source: InterruptSourceId::new(71),
                    interrupt_latency: 2,
                },
                PlatformClintHartConfig {
                    hart: 1,
                    target_partition: cpu1,
                    interrupt_target: InterruptTargetId::new(1),
                    software_interrupt_line: InterruptLineId::new(62),
                    software_interrupt_source: InterruptSourceId::new(72),
                    timer_interrupt_line: InterruptLineId::new(63),
                    timer_interrupt_source: InterruptSourceId::new(73),
                    interrupt_latency: 2,
                },
            ],
        })
        .add_uart(PlatformUartConfig {
            id: UartId::new(1),
            base: Address::new(0x1000_0000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu0, uart_partition, 2, 1).unwrap(),
            interrupt_line: InterruptLineId::new(64),
            interrupt_target: controller_target,
            interrupt_source: InterruptSourceId::new(10),
            interrupt_latency: 2,
        })
        .build()
        .unwrap();

    let config =
        PlatformRiscvDeviceTreeConfig::new(10_000_000, "rv64imafdc", "riscv,sv48", 0x384000)
            .unwrap();
    let tree = platform.riscv_device_tree(&config).unwrap();
    let cpus = tree.root().child("cpus").unwrap();
    let cpu0_node = cpus.child("cpu@0").unwrap();
    let cpu1_node = cpus.child("cpu@1").unwrap();
    let cpu0_intc = cpu0_node.child("interrupt-controller").unwrap();
    let cpu1_intc = cpu1_node.child("interrupt-controller").unwrap();
    let cpu0_phandle = cpu0_intc.property("phandle").unwrap().words().unwrap()[0];
    let cpu1_phandle = cpu1_intc.property("phandle").unwrap().words().unwrap()[0];
    let soc = tree.root().child("soc").unwrap();
    let plic = soc.child("interrupt-controller@c000000").unwrap();
    let plic_phandle = plic.property("phandle").unwrap().words().unwrap()[0];
    let clint = soc.child("clint@2000000").unwrap();
    let uart = soc.child("uart@10000000").unwrap();

    assert_eq!(
        cpus.property("timebase-frequency").unwrap().words(),
        Some(&[10_000_000][..])
    );
    assert_eq!(
        cpu0_node.property("riscv,isa").unwrap().strings(),
        Some(&["rv64imafdc".to_string()][..])
    );
    assert_eq!(
        cpu1_node.property("mmu-type").unwrap().strings(),
        Some(&["riscv,sv48".to_string()][..])
    );
    assert_eq!(
        clint.property("reg").unwrap().words(),
        Some(&[0, 0x0200_0000, 0, 0x1_0000][..])
    );
    assert_eq!(
        clint.property("interrupts-extended").unwrap().words(),
        Some(
            &[
                cpu0_phandle,
                0x3,
                cpu0_phandle,
                0x7,
                cpu1_phandle,
                0x3,
                cpu1_phandle,
                0x7,
            ][..]
        )
    );
    assert_eq!(
        plic.property("riscv,ndev").unwrap().words(),
        Some(&[10][..])
    );
    assert_eq!(
        uart.property("interrupt-parent").unwrap().words(),
        Some(&[plic_phandle][..])
    );
    assert_eq!(
        uart.property("interrupts").unwrap().words(),
        Some(&[10][..])
    );

    let dts = tree.to_dts();
    assert!(dts.contains("compatible = \"riscv,clint0\";"));
    assert!(dts.contains("compatible = \"ns8250\", \"ns16550a\";"));
    assert!(dts.contains("interrupt-controller@c000000"));
}

#[test]
fn platform_builder_emits_binary_riscv_device_tree_blob() {
    let cpu0 = PartitionId::new(0);
    let cpu1 = PartitionId::new(1);
    let clint_partition = PartitionId::new(2);
    let uart_partition = PartitionId::new(3);
    let controller_target = InterruptTargetId::new(0);

    let platform = PlatformBuilder::new(4)
        .add_interrupt_controller(PlatformInterruptControllerConfig {
            base: Address::new(0x0c00_0000),
            size: AccessSize::new(0x400_0000).unwrap(),
            route: MmioRoute::new(cpu0, cpu0, 1, 1).unwrap(),
            target: controller_target,
        })
        .add_clint(PlatformClintConfig {
            id: ClintId::new(0),
            base: Address::new(0x0200_0000),
            size: AccessSize::new(0x1_0000).unwrap(),
            route: MmioRoute::new(cpu0, clint_partition, 2, 1).unwrap(),
            reset_policy: ClintResetPolicy::preserve_mtimecmp(),
            harts: vec![
                PlatformClintHartConfig {
                    hart: 0,
                    target_partition: cpu0,
                    interrupt_target: InterruptTargetId::new(0),
                    software_interrupt_line: InterruptLineId::new(60),
                    software_interrupt_source: InterruptSourceId::new(70),
                    timer_interrupt_line: InterruptLineId::new(61),
                    timer_interrupt_source: InterruptSourceId::new(71),
                    interrupt_latency: 2,
                },
                PlatformClintHartConfig {
                    hart: 1,
                    target_partition: cpu1,
                    interrupt_target: InterruptTargetId::new(1),
                    software_interrupt_line: InterruptLineId::new(62),
                    software_interrupt_source: InterruptSourceId::new(72),
                    timer_interrupt_line: InterruptLineId::new(63),
                    timer_interrupt_source: InterruptSourceId::new(73),
                    interrupt_latency: 2,
                },
            ],
        })
        .add_uart(PlatformUartConfig {
            id: UartId::new(1),
            base: Address::new(0x1000_0000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu0, uart_partition, 2, 1).unwrap(),
            interrupt_line: InterruptLineId::new(64),
            interrupt_target: controller_target,
            interrupt_source: InterruptSourceId::new(10),
            interrupt_latency: 2,
        })
        .build()
        .unwrap();
    let config =
        PlatformRiscvDeviceTreeConfig::new(10_000_000, "rv64imafdc", "riscv,sv48", 0x384000)
            .unwrap();

    let dtb = platform.riscv_device_tree(&config).unwrap().to_dtb();

    assert_eq!(be32_at(&dtb, 0), 0xd00d_feed);
    assert_eq!(be32_at(&dtb, 4), dtb.len() as u32);
    assert_eq!(be32_at(&dtb, 20), 17);
    assert_eq!(be32_at(&dtb, 24), 16);
    assert_eq!(be32_at(&dtb, 28), 0);

    let struct_offset = be32_at(&dtb, 8) as usize;
    let strings_offset = be32_at(&dtb, 12) as usize;
    let reserve_offset = be32_at(&dtb, 16) as usize;
    let strings_size = be32_at(&dtb, 32) as usize;
    let struct_size = be32_at(&dtb, 36) as usize;
    assert_eq!(reserve_offset, 40);
    assert_eq!(struct_offset, 56);
    assert_eq!(strings_offset, struct_offset + struct_size);
    assert_eq!(strings_offset + strings_size, dtb.len());
    assert_eq!(&dtb[reserve_offset..struct_offset], &[0; 16]);

    let strings = std::str::from_utf8(&dtb[strings_offset..]).unwrap();
    assert!(strings.contains("timebase-frequency\0"));
    assert!(strings.contains("interrupts-extended\0"));
    assert!(strings.contains("interrupt-parent\0"));
    assert_eq!(strings.matches("compatible\0").count(), 1);

    let struct_block = &dtb[struct_offset..strings_offset];
    assert_eq!(be32_at(struct_block, 0), 1);
    assert_eq!(be32_at(struct_block, struct_block.len() - 4), 9);
    assert!(find_padded_ascii(struct_block, b"cpu@0\0"));
    assert!(find_padded_ascii(struct_block, b"clint@2000000\0"));
    assert!(find_padded_ascii(struct_block, b"uart@10000000\0"));
    assert!(find_ascii(struct_block, b"riscv,clint0\0"));
    assert!(find_ascii(struct_block, b"ns8250\0ns16550a\0"));
}

#[test]
fn platform_builder_emits_riscv_chosen_boot_data() {
    let platform = PlatformBuilder::new(3)
        .add_clint(PlatformClintConfig {
            id: ClintId::new(0),
            base: Address::new(0x0200_0000),
            size: AccessSize::new(0x1_0000).unwrap(),
            route: MmioRoute::new(PartitionId::new(0), PartitionId::new(2), 2, 1).unwrap(),
            reset_policy: ClintResetPolicy::preserve_mtimecmp(),
            harts: vec![PlatformClintHartConfig {
                hart: 0,
                target_partition: PartitionId::new(0),
                interrupt_target: InterruptTargetId::new(0),
                software_interrupt_line: InterruptLineId::new(60),
                software_interrupt_source: InterruptSourceId::new(70),
                timer_interrupt_line: InterruptLineId::new(61),
                timer_interrupt_source: InterruptSourceId::new(71),
                interrupt_latency: 2,
            }],
        })
        .build()
        .unwrap();
    let bootargs = "console=ttyS0 root=/dev/vda";
    let config =
        PlatformRiscvDeviceTreeConfig::new(10_000_000, "rv64imafdc", "riscv,sv48", 0x384000)
            .unwrap()
            .with_bootargs(bootargs)
            .with_initrd(Address::new(0x8800_0000), AccessSize::new(0x2000).unwrap())
            .unwrap();

    let tree = platform.riscv_device_tree(&config).unwrap();

    let chosen = tree.root().child("chosen").unwrap();
    assert_eq!(
        chosen.property("bootargs").unwrap().strings().unwrap(),
        &[bootargs.to_string()]
    );
    assert_eq!(
        chosen
            .property("linux,initrd-start")
            .unwrap()
            .double_words()
            .unwrap(),
        &[0x8800_0000]
    );
    assert_eq!(
        chosen
            .property("linux,initrd-end")
            .unwrap()
            .double_words()
            .unwrap(),
        &[0x8800_2000]
    );

    let dts = tree.to_dts();
    assert!(dts.contains("linux,initrd-start = <0x0 0x88000000>;"));
    assert!(dts.contains("linux,initrd-end = <0x0 0x88002000>;"));

    let dtb = tree.to_dtb();
    let strings_offset = be32_at(&dtb, 12) as usize;
    let strings = std::str::from_utf8(&dtb[strings_offset..]).unwrap();
    assert!(strings.contains("bootargs\0"));
    assert!(strings.contains("linux,initrd-start\0"));
    assert!(strings.contains("linux,initrd-end\0"));

    assert!(find_ascii(&dtb, b"chosen\0"));
    assert!(find_ascii(&dtb, format!("{bootargs}\0").as_bytes()));
    assert!(find_ascii(&dtb, &0x8800_0000_u64.to_be_bytes()));
    assert!(find_ascii(&dtb, &0x8800_2000_u64.to_be_bytes()));
}

#[test]
fn platform_builder_rejects_riscv_uart_device_tree_without_interrupt_controller() {
    let cpu = PartitionId::new(0);
    let uart_partition = PartitionId::new(1);
    let platform = PlatformBuilder::new(2)
        .add_uart(PlatformUartConfig {
            id: UartId::new(2),
            base: Address::new(0x1000_0000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu, uart_partition, 2, 1).unwrap(),
            interrupt_line: InterruptLineId::new(65),
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: InterruptSourceId::new(11),
            interrupt_latency: 2,
        })
        .build()
        .unwrap();
    let config =
        PlatformRiscvDeviceTreeConfig::new(10_000_000, "rv64imafdc", "riscv,sv48", 0x384000)
            .unwrap();

    assert_eq!(
        platform.riscv_device_tree(&config),
        Err(PlatformError::DeviceTreeMissingInterruptController {
            device: "uart@10000000".to_string(),
        })
    );
}

fn be32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn find_padded_ascii(bytes: &[u8], needle: &[u8]) -> bool {
    bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .is_some_and(|offset| offset % 4 == 0)
}

fn find_ascii(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

#[test]
fn platform_builder_resolves_mmio_routes_from_topology_paths() {
    let topology = platform_topology();
    let timer_route =
        PlatformTopologyRoute::new(endpoint("cpu", "mmio"), endpoint("timer", "mmio"))
            .resolve(&topology)
            .unwrap();
    let uart_route = PlatformTopologyRoute::new(endpoint("cpu", "mmio"), endpoint("uart", "mmio"))
        .resolve(&topology)
        .unwrap();

    assert_eq!(
        timer_route,
        MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 5, 3).unwrap()
    );
    assert_eq!(
        uart_route,
        MmioRoute::new(PartitionId::new(0), PartitionId::new(2), 7, 5).unwrap()
    );

    let platform = PlatformBuilder::from_topology(&topology)
        .add_timer(PlatformTimerConfig {
            id: TimerId::new(20),
            base: Address::new(0x5000),
            size: AccessSize::new(0x100).unwrap(),
            route: timer_route,
            interrupt_line: InterruptLineId::new(80),
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: InterruptSourceId::new(90),
            interrupt_latency: 2,
        })
        .add_uart(PlatformUartConfig {
            id: UartId::new(21),
            base: Address::new(0x6000),
            size: AccessSize::new(0x100).unwrap(),
            route: uart_route,
            interrupt_line: InterruptLineId::new(81),
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: InterruptSourceId::new(91),
            interrupt_latency: 2,
        })
        .build()
        .unwrap();

    assert_eq!(platform.partition_count(), 3);
    assert!(platform.timer(TimerId::new(20)).is_some());
    assert!(platform.uart(UartId::new(21)).is_some());
}

#[test]
fn platform_topology_route_reports_missing_endpoint_and_path() {
    let topology = platform_topology();
    let unknown_port =
        PlatformTopologyRoute::new(endpoint("cpu", "missing"), endpoint("timer", "mmio"))
            .resolve(&topology);

    match unknown_port {
        Err(error) => assert_eq!(
            error,
            PlatformTopologyError::Topology(TopologyError::UnknownPort {
                component: component("cpu"),
                port: port("missing"),
            }),
        ),
        Ok(_) => panic!("unknown topology endpoint was accepted"),
    }

    let missing_path =
        PlatformTopologyRoute::new(endpoint("timer", "mmio"), endpoint("cpu", "mmio"))
            .resolve(&topology);

    match missing_path {
        Err(error) => assert_eq!(
            error,
            PlatformTopologyError::MissingPath {
                source: endpoint("timer", "mmio"),
                target: endpoint("cpu", "mmio"),
            },
        ),
        Ok(_) => panic!("missing topology path was accepted"),
    }
}

#[test]
fn platform_builder_rejects_device_map_and_interrupt_conflicts() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let uart_partition = PartitionId::new(2);
    let shared_line = InterruptLineId::new(40);

    let overlap = PlatformBuilder::new(3)
        .add_timer(PlatformTimerConfig {
            id: TimerId::new(3),
            base: Address::new(0x8000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu, timer_partition, 2, 1).unwrap(),
            interrupt_line: InterruptLineId::new(41),
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: InterruptSourceId::new(50),
            interrupt_latency: 2,
        })
        .add_uart(PlatformUartConfig {
            id: UartId::new(4),
            base: Address::new(0x8080),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu, uart_partition, 2, 1).unwrap(),
            interrupt_line: InterruptLineId::new(42),
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: InterruptSourceId::new(51),
            interrupt_latency: 2,
        })
        .build();

    match overlap {
        Err(error) => assert_eq!(
            error,
            PlatformError::Mmio(MmioError::OverlappingDeviceRegion {
                existing_start: Address::new(0x8000),
                existing_end: Address::new(0x8100),
                requested_start: Address::new(0x8080),
                requested_end: Address::new(0x8180),
            }),
        ),
        Ok(_) => panic!("overlapping MMIO regions were accepted"),
    }

    let duplicate_line = PlatformBuilder::new(3)
        .add_timer(PlatformTimerConfig {
            id: TimerId::new(5),
            base: Address::new(0x9000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu, timer_partition, 2, 1).unwrap(),
            interrupt_line: shared_line,
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: InterruptSourceId::new(52),
            interrupt_latency: 2,
        })
        .add_uart(PlatformUartConfig {
            id: UartId::new(6),
            base: Address::new(0xa000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu, uart_partition, 2, 1).unwrap(),
            interrupt_line: shared_line,
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: InterruptSourceId::new(53),
            interrupt_latency: 2,
        })
        .build();

    match duplicate_line {
        Err(error) => assert_eq!(
            error,
            PlatformError::Interrupt(InterruptError::DuplicateLine { line: shared_line }),
        ),
        Ok(_) => panic!("duplicate interrupt lines were accepted"),
    }
}

#[test]
fn platform_builder_rejects_empty_and_unknown_partitions() {
    let empty = PlatformBuilder::new(0).build();

    match empty {
        Err(error) => assert_eq!(error, PlatformError::NoPartitions),
        Ok(_) => panic!("empty platform was accepted"),
    }

    let target = PartitionId::new(3);
    let unknown_target = PlatformBuilder::new(3)
        .add_timer(PlatformTimerConfig {
            id: TimerId::new(7),
            base: Address::new(0xb000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(PartitionId::new(0), target, 2, 1).unwrap(),
            interrupt_line: InterruptLineId::new(60),
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: InterruptSourceId::new(70),
            interrupt_latency: 2,
        })
        .build();

    match unknown_target {
        Err(error) => assert_eq!(
            error,
            PlatformError::UnknownPartition {
                partition: target,
                partitions: 3,
            },
        ),
        Ok(_) => panic!("unknown target partition was accepted"),
    }

    let source = PartitionId::new(4);
    let unknown_source = PlatformBuilder::new(3)
        .add_uart(PlatformUartConfig {
            id: UartId::new(8),
            base: Address::new(0xc000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(source, PartitionId::new(2), 2, 1).unwrap(),
            interrupt_line: InterruptLineId::new(61),
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: InterruptSourceId::new(71),
            interrupt_latency: 2,
        })
        .build();

    match unknown_source {
        Err(error) => assert_eq!(
            error,
            PlatformError::UnknownPartition {
                partition: source,
                partitions: 3,
            },
        ),
        Ok(_) => panic!("unknown source partition was accepted"),
    }
}

#[test]
fn platform_builder_maps_interrupt_controller_mmio() {
    let cpu = PartitionId::new(0);
    let interrupt_partition = PartitionId::new(1);
    let timer_partition = PartitionId::new(2);
    let target = InterruptTargetId::new(0);
    let timer_id = TimerId::new(9);
    let timer_line = InterruptLineId::new(80);
    let timer_source = InterruptSourceId::new(90);
    let plic_base = Address::new(0x0c00_0000);

    let platform = PlatformBuilder::new(3)
        .add_interrupt_controller(PlatformInterruptControllerConfig {
            base: plic_base,
            size: AccessSize::new(0x210000).unwrap(),
            route: MmioRoute::new(cpu, interrupt_partition, 2, 1).unwrap(),
            target,
        })
        .add_timer(PlatformTimerConfig {
            id: timer_id,
            base: Address::new(0x5000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu, timer_partition, 2, 1).unwrap(),
            interrupt_line: timer_line,
            interrupt_target: target,
            interrupt_source: timer_source,
            interrupt_latency: 2,
        })
        .build()
        .unwrap();

    let timer = platform.timer(timer_id).unwrap().clone();
    let controller = platform.interrupt_controller();
    let bus = platform.mmio_bus().clone();
    let timer_bus = bus.clone();
    let interrupt_bus = bus;
    let completions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(platform.partition_count()).unwrap();

    let completed = std::sync::Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 1, move |context| {
            let first_completed = std::sync::Arc::clone(&completed);
            timer_bus
                .submit(
                    context,
                    MmioRequest::write(
                        MmioRequestId::new(10),
                        Address::new(0x5000 + TIMER_MMIO_DEADLINE_OFFSET),
                        le64(7),
                        full_mask(8),
                    )
                    .unwrap(),
                    move |completion| first_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    let completed = std::sync::Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 10, move |context| {
            let first_completed = std::sync::Arc::clone(&completed);
            let pending_word = timer_line.get() / 32;
            let pending_bit = 1u32 << (timer_line.get() % 32);
            interrupt_bus
                .submit(
                    context,
                    MmioRequest::read(
                        MmioRequestId::new(11),
                        Address::new(
                            plic_base.get()
                                + PLIC_MMIO_PENDING_BASE_OFFSET
                                + pending_word * PLIC_MMIO_REGISTER_BYTES,
                        ),
                        AccessSize::new(PLIC_MMIO_REGISTER_BYTES).unwrap(),
                    )
                    .unwrap(),
                    move |completion| first_completed.lock().unwrap().push(completion),
                )
                .unwrap();

            let second_completed = std::sync::Arc::clone(&completed);
            interrupt_bus
                .submit(
                    context,
                    MmioRequest::write(
                        MmioRequestId::new(12),
                        Address::new(
                            plic_base.get()
                                + PLIC_MMIO_ENABLE_BASE_OFFSET
                                + pending_word * PLIC_MMIO_REGISTER_BYTES,
                        ),
                        le32(pending_bit),
                        full_mask(PLIC_MMIO_REGISTER_BYTES),
                    )
                    .unwrap(),
                    move |completion| second_completed.lock().unwrap().push(completion),
                )
                .unwrap();

            let third_completed = std::sync::Arc::clone(&completed);
            interrupt_bus
                .submit(
                    context,
                    MmioRequest::read(
                        MmioRequestId::new(13),
                        Address::new(
                            plic_base.get()
                                + PLIC_MMIO_CONTEXT_BASE_OFFSET
                                + PLIC_MMIO_CLAIM_COMPLETE_OFFSET,
                        ),
                        AccessSize::new(PLIC_MMIO_REGISTER_BYTES).unwrap(),
                    )
                    .unwrap(),
                    move |completion| third_completed.lock().unwrap().push(completion),
                )
                .unwrap();

            interrupt_bus
                .submit(
                    context,
                    MmioRequest::write(
                        MmioRequestId::new(14),
                        Address::new(
                            plic_base.get()
                                + PLIC_MMIO_CONTEXT_BASE_OFFSET
                                + PLIC_MMIO_CLAIM_COMPLETE_OFFSET,
                        ),
                        le32(timer_line.get() as u32),
                        full_mask(PLIC_MMIO_REGISTER_BYTES),
                    )
                    .unwrap(),
                    move |completion| completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 14);
    assert_eq!(summary.final_tick(), 13);
    assert_eq!(timer.snapshot().expiries(), &[TimerExpiry::new(1, 7)]);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[
            MmioCompletion::new(
                4,
                MmioRoute::new(cpu, timer_partition, 2, 1).unwrap(),
                Ok(MmioResponse::completed(MmioRequestId::new(10), None)),
            ),
            MmioCompletion::new(
                13,
                MmioRoute::new(cpu, interrupt_partition, 2, 1).unwrap(),
                Ok(MmioResponse::completed(
                    MmioRequestId::new(11),
                    Some(le32(1u32 << (timer_line.get() % 32))),
                )),
            ),
            MmioCompletion::new(
                13,
                MmioRoute::new(cpu, interrupt_partition, 2, 1).unwrap(),
                Ok(MmioResponse::completed(MmioRequestId::new(12), None)),
            ),
            MmioCompletion::new(
                13,
                MmioRoute::new(cpu, interrupt_partition, 2, 1).unwrap(),
                Ok(MmioResponse::completed(
                    MmioRequestId::new(13),
                    Some(le32(timer_line.get() as u32)),
                )),
            ),
            MmioCompletion::new(
                13,
                MmioRoute::new(cpu, interrupt_partition, 2, 1).unwrap(),
                Ok(MmioResponse::completed(MmioRequestId::new(14), None)),
            ),
        ]
    );

    let controller = controller.lock().unwrap();
    assert!(controller.pending().is_empty());
    assert!(controller.claimed().is_empty());
    assert_eq!(
        controller.history(),
        &[
            InterruptEvent::routed(
                9,
                timer_line,
                target,
                cpu,
                timer_source,
                InterruptEventKind::Assert,
            ),
            InterruptEvent::routed(
                12,
                timer_line,
                target,
                cpu,
                timer_source,
                InterruptEventKind::Claim,
            ),
            InterruptEvent::routed(
                12,
                timer_line,
                target,
                cpu,
                timer_source,
                InterruptEventKind::Complete,
            ),
        ]
    );
}

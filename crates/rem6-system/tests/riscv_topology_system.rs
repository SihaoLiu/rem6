use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_checkpoint::CheckpointComponentId;
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_dram::{DramGeometry, DramMemoryTechnology, DramTiming, ExternalMemoryProfile};
use rem6_interrupt::{
    InterruptLineId, InterruptPriority, InterruptSourceId, InterruptTargetId, PlicContextSnapshot,
    PlicSnapshot,
};
use rem6_kernel::{ClockDomain, PartitionId, PartitionedScheduler, SchedulerError};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId, PartitionedMemoryStore, ResponseStatus,
};
use rem6_mmio::{MmioRequest, MmioRequestId};
use rem6_platform::{
    Platform, PlatformBuilder, PlatformClintConfig, PlatformClintHartConfig,
    PlatformInterruptControllerConfig, PlatformInterruptControllerContextConfig,
    PlatformTimerConfig, PlatformTopologyRoute, PlatformUartConfig,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, HostEventPolicy,
    RiscvSystemRunDriver, RiscvSystemRunStopReason, RiscvTopologyDramConfig,
    RiscvTopologyHostConfig, RiscvTopologyMemoryConfig, RiscvTopologySystem,
    RiscvTopologySystemError, RiscvTrapEventPort, StopRequest, SystemActionOutcome,
    SystemHostController, SystemHostEventPort,
};
use rem6_timer::{
    ClintHartSnapshot, ClintId, ClintResetPolicy, ClintSnapshot, TimerArm, TimerId, TimerSnapshot,
};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, PortDirection, PortName, Topology,
    TopologyBuilder,
};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, RequestDelivery, TargetOutcome,
    TransportEndpointId,
};
use rem6_uart::{UartId, UartMmioDevice, UartTxByte, UART_MMIO_DATA_OFFSET};

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

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(19), sequence)
}

fn memory_read(address: u64, size: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn memory_write(address: u64, bytes: &[u8], sequence: u64) -> MemoryRequest {
    MemoryRequest::write(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(bytes.len() as u64).unwrap(),
        bytes.to_vec(),
        ByteMask::full(AccessSize::new(bytes.len() as u64).unwrap()).unwrap(),
        layout(),
    )
    .unwrap()
}

fn platform_with_uart(topology: &Topology, uart_id: UartId) -> Platform {
    let uart_route =
        PlatformTopologyRoute::new(endpoint("cpu0", "mmio"), endpoint("uart0", "mmio"))
            .resolve(topology)
            .unwrap();
    PlatformBuilder::from_topology(topology)
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
        .unwrap()
}

fn platform_with_timer(topology: &Topology, timer_id: TimerId) -> Platform {
    let timer_route =
        PlatformTopologyRoute::new(endpoint("cpu0", "mmio"), endpoint("timer0", "mmio"))
            .resolve(topology)
            .unwrap();
    PlatformBuilder::from_topology(topology)
        .add_timer(PlatformTimerConfig {
            id: timer_id,
            base: Address::new(0xb000),
            size: AccessSize::new(0x100).unwrap(),
            route: timer_route,
            interrupt_line: rem6_interrupt::InterruptLineId::new(41),
            interrupt_target: rem6_interrupt::InterruptTargetId::new(0),
            interrupt_source: rem6_interrupt::InterruptSourceId::new(51),
            interrupt_latency: 2,
        })
        .build()
        .unwrap()
}

fn platform_with_clint(topology: &Topology, clint_id: ClintId) -> Platform {
    PlatformBuilder::from_topology(topology)
        .add_clint(PlatformClintConfig {
            id: clint_id,
            base: Address::new(0x200_0000),
            size: AccessSize::new(0x1_0000).unwrap(),
            route: rem6_mmio::MmioRoute::new(PartitionId::new(0), PartitionId::new(3), 2, 2)
                .unwrap(),
            reset_policy: ClintResetPolicy::preserve_mtimecmp(),
            harts: vec![PlatformClintHartConfig {
                hart: 0,
                target_partition: PartitionId::new(0),
                interrupt_target: rem6_interrupt::InterruptTargetId::new(0),
                software_interrupt_line: rem6_interrupt::InterruptLineId::new(42),
                software_interrupt_source: rem6_interrupt::InterruptSourceId::new(52),
                timer_interrupt_line: rem6_interrupt::InterruptLineId::new(43),
                timer_interrupt_source: rem6_interrupt::InterruptSourceId::new(53),
                interrupt_latency: 2,
            }],
        })
        .build()
        .unwrap()
}

fn platform_with_plic(topology: &Topology, base: Address) -> Platform {
    PlatformBuilder::from_topology(topology)
        .add_interrupt_controller(PlatformInterruptControllerConfig {
            base,
            size: AccessSize::new(0x400_0000).unwrap(),
            route: rem6_mmio::MmioRoute::new(PartitionId::new(0), PartitionId::new(3), 2, 2)
                .unwrap(),
            target: InterruptTargetId::new(0),
            contexts: vec![PlatformInterruptControllerContextConfig {
                context: 0,
                hart: 0,
                interrupt: 0xB,
                target: InterruptTargetId::new(0),
                target_partition: PartitionId::new(0),
            }],
        })
        .build()
        .unwrap()
}

fn uart_byte_mask() -> ByteMask {
    ByteMask::full(AccessSize::new(rem6_uart::UART_MMIO_REGISTER_BYTES).unwrap()).unwrap()
}

fn write_uart_byte(uart: &UartMmioDevice, tick: u64, byte: u8) {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let base = uart.base();
    let uart = uart.clone();
    scheduler
        .schedule_at(PartitionId::new(0), tick, move |context| {
            uart.respond(
                context,
                &MmioRequest::write(
                    MmioRequestId::new(300 + tick),
                    Address::new(base.get() + UART_MMIO_DATA_OFFSET),
                    vec![byte],
                    uart_byte_mask(),
                )
                .unwrap(),
            )
            .unwrap();
        })
        .unwrap();
    scheduler.run_until_idle();
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

fn topology_with_timer() -> Topology {
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
                component("timer0"),
                kind("timer"),
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
        .connect_with_latencies(endpoint("cpu0", "mmio"), endpoint("timer0", "mmio"), 2, 2)
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
    let system = RiscvTopologySystem::with_min_remote_delay(
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
    let (cluster, mut scheduler, transport) = system.execution_parts_mut();

    let run = driver
        .drive_until_host_stop_parallel(
            cluster,
            &mut scheduler,
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
fn topology_system_drives_parallel_host_stop_with_worker_limit() {
    let system = RiscvTopologySystem::with_parallel_worker_limit(
        topology(),
        RiscvClusterTopologyConfig::new([
            core_config(0, 0, 7, 0x8000),
            core_config(1, 1, 8, 0x9000),
        ]),
        2,
        1,
    )
    .unwrap();
    assert_eq!(system.scheduler().partition_count(), 4);
    assert_eq!(system.scheduler().min_remote_delay(), 2);
    assert_eq!(system.scheduler().max_parallel_workers(), 1);

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
    let source = GuestSourceId::new(47);
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(PartitionId::new(3), 2, Arc::clone(&controller))
            .unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port);
    let (cluster, mut scheduler, transport) = system.execution_parts_mut();

    let run = driver
        .drive_until_host_stop_parallel(
            cluster,
            &mut scheduler,
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
            40,
            |cpu| GuestEventId::new(180 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(180), source, 0);
    assert_eq!(run.stop_reason(), RiscvSystemRunStopReason::HostStop(stop));
    assert_eq!(run.max_parallel_scheduler_workers(), 1);
    assert_eq!(run.parallel_scheduler_profile().max_parallel_workers(), 1);
    assert_eq!(
        run.parallel_scheduler_profile().batch_count(),
        run.parallel_scheduler_batches().len()
    );
    assert!(run
        .parallel_scheduler_batches()
        .iter()
        .all(|batch| batch.worker_count() <= 1));
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
fn topology_system_rejects_zero_parallel_worker_limit() {
    let error = match RiscvTopologySystem::with_parallel_worker_limit(
        topology(),
        RiscvClusterTopologyConfig::new([
            core_config(0, 0, 7, 0x8000),
            core_config(1, 1, 8, 0x9000),
        ]),
        2,
        0,
    ) {
        Ok(_) => panic!("zero parallel worker limit was accepted"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        RiscvTopologySystemError::Scheduler(SchedulerError::ZeroParallelWorkers)
    );
}

#[test]
fn topology_system_with_platform_drives_parallel_mmio_and_memory_accesses() {
    let topology = topology_with_uart();
    let uart_id = UartId::new(1);
    let platform = platform_with_uart(&topology, uart_id);

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
    let system = RiscvTopologySystem::with_min_remote_delay(
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

#[test]
fn topology_host_controller_checkpoints_attached_uart() {
    let topology = topology_with_uart();
    let uart_id = UartId::new(0);
    let platform = platform_with_uart(&topology, uart_id);
    let source = GuestSourceId::new(46);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology,
        RiscvClusterTopologyConfig::new([
            core_config(0, 0, 7, 0x8000),
            core_config(1, 1, 8, 0x9000),
        ]),
        2,
    )
    .unwrap()
    .with_platform(platform)
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(4), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    let host = system.host_controller().unwrap();
    let uart_component = CheckpointComponentId::new("uart0").unwrap();
    let uart = system.platform().unwrap().uart(uart_id).unwrap().clone();
    uart.inject_rx([b'A', b'B']).unwrap();
    write_uart_byte(&uart, 10, b'O');
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .uart_checkpoint_bank()
        .is_some());

    let checkpoint = HostActionRecord::new(
        24,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(190),
        source,
        HostAction::Checkpoint {
            label: "attached-uart".to_string(),
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
        .any(|state| state.component() == &uart_component));
    assert!(
        host.lock()
            .unwrap()
            .executor()
            .checkpoints()
            .chunk(&uart_component, "uart")
            .unwrap()
            .len()
            > 48
    );

    write_uart_byte(&uart, 11, b'X');
    uart.inject_rx([b'C']).unwrap();
    assert_ne!(uart.snapshot().tx_bytes(), &[UartTxByte::new(10, b'O')]);

    let restore = HostActionRecord::new(
        36,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(191),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    let restored = host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(
        restored,
        SystemActionOutcome::CheckpointRestored {
            tick: 36,
            event: GuestEventId::new(191),
            source,
            manifest,
        }
    );
    assert_eq!(uart.snapshot().tx_bytes(), &[UartTxByte::new(10, b'O')]);
    assert_eq!(uart.snapshot().rx_pending(), b"AB");
}

#[test]
fn topology_host_controller_checkpoints_attached_scheduler() {
    let source = GuestSourceId::new(47);
    let scheduler_component = CheckpointComponentId::new("sched-custom").unwrap();
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology(),
        RiscvClusterTopologyConfig::new([
            core_config(0, 0, 7, 0x8000),
            core_config(1, 1, 8, 0x9000),
        ]),
        2,
    )
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(3), 2, source)
            .with_scheduler_checkpoint_component(scheduler_component.clone()),
        StatsRegistry::new(),
    )
    .unwrap();
    let host = system.host_controller().unwrap();

    {
        let mut scheduler = system.scheduler_mut();
        let first_id = scheduler
            .schedule_parallel_at(PartitionId::new(0), 5, |context| {
                context.schedule_local_after(2, |_| {}).unwrap();
            })
            .unwrap();
        assert_eq!(
            first_id,
            rem6_kernel::PartitionEventId::new(PartitionId::new(0), 0)
        );
        assert_eq!(scheduler.run_until_idle_parallel().unwrap().final_tick(), 8);
    }
    let scheduler_snapshot = system.scheduler().quiescent_snapshot().unwrap();
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .scheduler_checkpoint_bank()
        .is_some());
    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .scheduler_checkpoint_bank()
            .unwrap()
            .components(),
        vec![scheduler_component.clone()]
    );

    let checkpoint = HostActionRecord::new(
        30,
        PartitionId::new(3),
        PartitionId::new(3),
        GuestEventId::new(194),
        source,
        HostAction::Checkpoint {
            label: "attached-scheduler".to_string(),
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
        .any(|state| state.component() == &scheduler_component));
    assert!(
        host.lock()
            .unwrap()
            .executor()
            .checkpoints()
            .chunk(&scheduler_component, "scheduler")
            .unwrap()
            .len()
            > 64
    );

    {
        let mut scheduler = system.scheduler_mut();
        scheduler
            .schedule_parallel_at(PartitionId::new(1), 12, |_| {})
            .unwrap();
        assert_eq!(
            scheduler.run_until_idle_parallel().unwrap().final_tick(),
            12
        );
    }
    assert_ne!(system.scheduler().snapshot(), scheduler_snapshot);

    let restore = HostActionRecord::new(
        38,
        PartitionId::new(3),
        PartitionId::new(3),
        GuestEventId::new(195),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    let restored = host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(
        restored,
        SystemActionOutcome::CheckpointRestored {
            tick: 38,
            event: GuestEventId::new(195),
            source,
            manifest,
        }
    );
    assert_eq!(system.scheduler().snapshot(), scheduler_snapshot);
    let restored_id = system
        .scheduler_mut()
        .schedule_parallel_at(PartitionId::new(0), 8, |_| {})
        .unwrap();
    assert_eq!(
        restored_id,
        rem6_kernel::PartitionEventId::new(PartitionId::new(0), 2)
    );
}

#[test]
fn topology_host_controller_checkpoints_attached_timer() {
    let topology = topology_with_timer();
    let timer_id = TimerId::new(0);
    let platform = platform_with_timer(&topology, timer_id);
    let source = GuestSourceId::new(47);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology,
        RiscvClusterTopologyConfig::new([
            core_config(0, 0, 7, 0x8000),
            core_config(1, 1, 8, 0x9000),
        ]),
        2,
    )
    .unwrap()
    .with_platform(platform)
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(4), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    let host = system.host_controller().unwrap();
    let timer_component = CheckpointComponentId::new("timer0").unwrap();
    let timer = system.platform().unwrap().timer(timer_id).unwrap().clone();
    let captured = TimerSnapshot::new(
        timer_id,
        timer.partition(),
        timer.source(),
        Some(64),
        vec![TimerArm::new(1, 12, 64)],
        Vec::new(),
        Vec::new(),
    );
    let empty = TimerSnapshot::new(
        timer_id,
        timer.partition(),
        timer.source(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    timer.restore(&captured).unwrap();
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .timer_checkpoint_bank()
        .is_some());

    let checkpoint = HostActionRecord::new(
        25,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(192),
        source,
        HostAction::Checkpoint {
            label: "attached-timer".to_string(),
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
        .any(|state| state.component() == &timer_component));
    assert!(
        host.lock()
            .unwrap()
            .executor()
            .checkpoints()
            .chunk(&timer_component, "timer")
            .unwrap()
            .len()
            > 48
    );

    timer.restore(&empty).unwrap();
    assert_ne!(timer.snapshot(), captured);

    let restore = HostActionRecord::new(
        37,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(193),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    let restored = host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(
        restored,
        SystemActionOutcome::CheckpointRestored {
            tick: 37,
            event: GuestEventId::new(193),
            source,
            manifest,
        }
    );
    assert_eq!(timer.snapshot(), captured);
}

#[test]
fn topology_host_controller_checkpoints_attached_clint() {
    let topology = topology_with_timer();
    let clint_id = ClintId::new(0);
    let platform = platform_with_clint(&topology, clint_id);
    let source = GuestSourceId::new(50);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology,
        RiscvClusterTopologyConfig::new([
            core_config(0, 0, 7, 0x8000),
            core_config(1, 1, 8, 0x9000),
        ]),
        2,
    )
    .unwrap()
    .with_platform(platform)
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(4), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    let host = system.host_controller().unwrap();
    let component = CheckpointComponentId::new("clint0").unwrap();
    let clint = system.platform().unwrap().clint(clint_id).unwrap().clone();
    let captured = ClintSnapshot::new(
        Address::new(0x200_0000),
        vec![ClintHartSnapshot::new(0, 1, 64, 2, true)],
    );
    let empty = ClintSnapshot::new(
        Address::new(0x200_0000),
        vec![ClintHartSnapshot::new(0, 0, u64::MAX, 0, false)],
    );
    clint.restore(&captured).unwrap();
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .clint_checkpoint_bank()
        .is_some());

    let checkpoint = HostActionRecord::new(
        26,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(194),
        source,
        HostAction::Checkpoint {
            label: "attached-clint".to_string(),
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
        .any(|state| state.component() == &component));
    assert!(
        host.lock()
            .unwrap()
            .executor()
            .checkpoints()
            .chunk(&component, "clint")
            .unwrap()
            .len()
            >= 56
    );

    clint.restore(&empty).unwrap();
    assert_ne!(clint.snapshot(), captured);

    let restore = HostActionRecord::new(
        39,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(195),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    let restored = host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(
        restored,
        SystemActionOutcome::CheckpointRestored {
            tick: 39,
            event: GuestEventId::new(195),
            source,
            manifest,
        }
    );
    assert_eq!(clint.snapshot(), captured);
}

#[test]
fn topology_host_controller_checkpoints_attached_interrupt_controller() {
    let topology = topology_with_timer();
    let timer_id = TimerId::new(0);
    let platform = platform_with_timer(&topology, timer_id);
    let source = GuestSourceId::new(48);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology,
        RiscvClusterTopologyConfig::new([
            core_config(0, 0, 7, 0x8000),
            core_config(1, 1, 8, 0x9000),
        ]),
        2,
    )
    .unwrap()
    .with_platform(platform)
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(4), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    let host = system.host_controller().unwrap();
    let interrupt_component = CheckpointComponentId::new("interrupt0").unwrap();
    let controller = system.platform().unwrap().interrupt_controller();
    let line = InterruptLineId::new(41);
    let target = InterruptTargetId::new(0);
    let target_partition = PartitionId::new(0);
    let interrupt_source = InterruptSourceId::new(51);
    {
        let mut controller = controller.lock().unwrap();
        controller
            .set_priority(line, InterruptPriority::new(7))
            .unwrap();
        controller.assert(line, interrupt_source, 12).unwrap();
        assert!(controller.claim(target, target_partition, 13).is_some());
    }
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .interrupt_controller_checkpoint_bank()
        .is_some());

    let checkpoint = HostActionRecord::new(
        29,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(194),
        source,
        HostAction::Checkpoint {
            label: "attached-interrupt".to_string(),
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
    let captured = controller.lock().unwrap().snapshot(29);

    assert!(manifest
        .states()
        .iter()
        .any(|state| state.component() == &interrupt_component));
    assert!(
        host.lock()
            .unwrap()
            .executor()
            .checkpoints()
            .chunk(&interrupt_component, "interrupt")
            .unwrap()
            .len()
            > 96
    );

    {
        let mut controller = controller.lock().unwrap();
        controller
            .complete(target, target_partition, line, 30)
            .unwrap();
        controller
            .set_priority(line, InterruptPriority::ZERO)
            .unwrap();
        controller.assert(line, interrupt_source, 31).unwrap();
        assert_ne!(controller.snapshot(29), captured);
    }

    let restore = HostActionRecord::new(
        38,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(195),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    let restored = host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(
        restored,
        SystemActionOutcome::CheckpointRestored {
            tick: 38,
            event: GuestEventId::new(195),
            source,
            manifest,
        }
    );
    assert_eq!(controller.lock().unwrap().snapshot(29), captured);
}

#[test]
fn topology_host_controller_checkpoints_attached_plic() {
    let topology = topology_with_timer();
    let plic_base = Address::new(0x0c00_0000);
    let platform = platform_with_plic(&topology, plic_base);
    let source = GuestSourceId::new(49);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology,
        RiscvClusterTopologyConfig::new([
            core_config(0, 0, 7, 0x8000),
            core_config(1, 1, 8, 0x9000),
        ]),
        2,
    )
    .unwrap()
    .with_platform(platform)
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(4), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    let host = system.host_controller().unwrap();
    let component = CheckpointComponentId::new("plic.c000000").unwrap();
    let target = InterruptTargetId::new(0);
    let target_partition = PartitionId::new(0);
    let line = InterruptLineId::new(41);
    let captured = PlicSnapshot::new(
        plic_base,
        vec![PlicContextSnapshot::new(
            0,
            target,
            target_partition,
            vec![line],
            InterruptPriority::new(5),
        )],
    );
    let empty = PlicSnapshot::new(
        plic_base,
        vec![PlicContextSnapshot::new(
            0,
            target,
            target_partition,
            Vec::new(),
            InterruptPriority::ZERO,
        )],
    );
    let plic = system.platform().unwrap().plic(plic_base).unwrap().clone();
    plic.restore(&captured).unwrap();
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .plic_checkpoint_bank()
        .is_some());

    let checkpoint = HostActionRecord::new(
        31,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(196),
        source,
        HostAction::Checkpoint {
            label: "attached-plic".to_string(),
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
        .any(|state| state.component() == &component));
    assert!(
        host.lock()
            .unwrap()
            .executor()
            .checkpoints()
            .chunk(&component, "plic")
            .unwrap()
            .len()
            >= 52
    );

    plic.restore(&empty).unwrap();
    assert_ne!(plic.snapshot(), captured);

    let restore = HostActionRecord::new(
        45,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(197),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    let restored = host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(
        restored,
        SystemActionOutcome::CheckpointRestored {
            tick: 45,
            event: GuestEventId::new(197),
            source,
            manifest,
        }
    );
    assert_eq!(plic.snapshot(), captured);
}

#[test]
fn topology_system_with_dram_memory_delays_fetch_response_by_dram_timing() {
    let image = BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap();
    let dram = RiscvTopologyDramConfig::new(
        MemoryTargetId::new(0),
        layout(),
        DramGeometry::new(2, 64, 16).unwrap(),
        DramTiming::new(5, 7, 11, 3, 2).unwrap(),
    )
    .add_region(Address::new(0x8000), AccessSize::new(0x1000).unwrap());
    let source = GuestSourceId::new(43);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 7, 0x8000)]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(dram, &image)
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(3), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    assert!(system.memory_store().is_none());
    assert!(system.dram_memory_controller().is_some());

    let fetch_trace = MemoryTrace::new();
    let data_trace = MemoryTrace::new();
    let run = system
        .drive_attached_until_host_stop_parallel(
            fetch_trace.clone(),
            data_trace.clone(),
            30,
            |cpu| GuestEventId::new(150 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(20, GuestEventId::new(150), source, 0);
    let fetch_request = system
        .cluster()
        .core(CpuId::new(0))
        .unwrap()
        .execution_events()[0]
        .fetch()
        .request_id();
    assert_eq!(run.stop_reason(), RiscvSystemRunStopReason::HostStop(stop));
    assert_eq!(
        fetch_trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                MemoryRouteId::new(0),
                TransportEndpointId::from_topology_endpoint(&endpoint("cpu0", "ifetch")).unwrap(),
                MemoryTraceKind::RequestSent,
                fetch_request,
            ),
            MemoryTraceEvent::request(
                2,
                MemoryRouteId::new(0),
                TransportEndpointId::from_topology_endpoint(&endpoint("mem0", "requests")).unwrap(),
                MemoryTraceKind::RequestArrived,
                fetch_request,
            ),
            MemoryTraceEvent::response(
                17,
                MemoryRouteId::new(0),
                TransportEndpointId::from_topology_endpoint(&endpoint("cpu0", "ifetch")).unwrap(),
                fetch_request,
                ResponseStatus::Completed,
            ),
        ]
    );
    assert!(data_trace.snapshot().is_empty());
    let dram_profile = system.dram_activity_profile().unwrap();
    assert_eq!(dram_profile.active_target_count(), 1);
    assert_eq!(dram_profile.access_count(), 1);
    assert_eq!(dram_profile.read_count(), 1);
    assert_eq!(dram_profile.row_miss_count(), 1);
    assert_eq!(dram_profile.command_count(), 2);
    assert_eq!(dram_profile.total_ready_latency_cycles(), 12);
    assert_eq!(dram_profile.max_ready_latency_cycles(), 12);
    let target_activity = system.dram_target_activity(MemoryTargetId::new(0)).unwrap();
    assert_eq!(target_activity.profile().access_count(), 1);
    assert_eq!(target_activity.profile().active_port_count(), 1);
    assert_eq!(target_activity.profile().active_bank_count(), 1);
}

#[test]
fn topology_host_controller_checkpoints_attached_dram_memory() {
    let target = MemoryTargetId::new(0);
    let image = BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap();
    let dram = RiscvTopologyDramConfig::new(
        target,
        layout(),
        DramGeometry::new(2, 64, 16).unwrap(),
        DramTiming::new(5, 7, 11, 3, 2).unwrap(),
    )
    .add_region(Address::new(0x8000), AccessSize::new(0x1000).unwrap());
    let source = GuestSourceId::new(45);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 7, 0x8000)]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(dram, &image)
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(3), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    let cpu_component = CheckpointComponentId::new("cpu0").unwrap();
    let dram_component = CheckpointComponentId::new("dram0").unwrap();
    let core = system.cluster().core(CpuId::new(0)).unwrap();
    let x1 = rem6_isa_riscv::Register::new(1).unwrap();
    core.redirect_pc(Address::new(0x8004));
    core.write_register(x1, 0x1122);
    let controller = Arc::clone(system.dram_memory_controller().unwrap());
    let first = controller
        .lock()
        .unwrap()
        .accept(0, &memory_read(0x8000, 4, 201))
        .unwrap();
    assert_eq!(first.ready_cycle(), 12);
    let host = system.host_controller().unwrap();
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .dram_memory_checkpoint_bank()
        .is_some());
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .riscv_checkpoint_bank()
        .is_some());

    let checkpoint = HostActionRecord::new(
        24,
        PartitionId::new(3),
        PartitionId::new(3),
        GuestEventId::new(180),
        source,
        HostAction::Checkpoint {
            label: "attached-dram".to_string(),
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
    assert_eq!(
        manifest
            .states()
            .iter()
            .map(|state| state.component().clone())
            .collect::<Vec<_>>(),
        vec![
            cpu_component.clone(),
            dram_component.clone(),
            CheckpointComponentId::new("fabric0").unwrap(),
            CheckpointComponentId::new("scheduler0").unwrap(),
        ]
    );
    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .checkpoints()
            .chunk(&cpu_component, "pc"),
        Some(&0x8004_u64.to_le_bytes()[..])
    );
    assert!(
        host.lock()
            .unwrap()
            .executor()
            .checkpoints()
            .chunk(&dram_component, "dram")
            .unwrap()
            .len()
            > 192
    );

    core.redirect_pc(Address::new(0x9000));
    core.write_register(x1, 0);
    controller
        .lock()
        .unwrap()
        .accept(12, &memory_write(0x8000, &[0xaa, 0xbb, 0xcc, 0xdd], 202))
        .unwrap();
    assert_eq!(
        &controller
            .lock()
            .unwrap()
            .line_data(target, Address::new(0x8000))
            .unwrap()[..4],
        &[0xaa, 0xbb, 0xcc, 0xdd]
    );

    let restore = HostActionRecord::new(
        36,
        PartitionId::new(3),
        PartitionId::new(3),
        GuestEventId::new(181),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    let restored = host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(
        restored,
        SystemActionOutcome::CheckpointRestored {
            tick: 36,
            event: GuestEventId::new(181),
            source,
            manifest,
        }
    );
    assert_eq!(core.pc(), Address::new(0x8004));
    assert_eq!(core.read_register(x1), 0x1122);
    let mut controller = controller.lock().unwrap();
    assert_eq!(
        &controller.line_data(target, Address::new(0x8000)).unwrap()[..4],
        &[0x73, 0x00, 0x00, 0x00]
    );
    let bank = controller
        .dram_controller(target)
        .unwrap()
        .bank_state(0)
        .unwrap();
    assert_eq!(bank.open_row(), Some(256));
    assert_eq!(bank.available_cycle(), 12);
    let row_hit = controller.accept(12, &memory_read(0x8000, 4, 203)).unwrap();
    assert!(row_hit.dram_access().row_hit());
    assert_eq!(row_hit.ready_cycle(), 19);
}

#[test]
fn topology_system_dram_boot_image_loads_segments_into_addressed_targets() {
    let code_target = MemoryTargetId::new(0);
    let data_target = MemoryTargetId::new(1);
    let image = BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x8004), word(0x0000_0073))
        .unwrap()
        .add_segment(
            Address::new(0xa008),
            vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
        )
        .unwrap();
    let dram = RiscvTopologyDramConfig::new(
        code_target,
        layout(),
        DramGeometry::new(2, 64, 16).unwrap(),
        DramTiming::new(5, 7, 11, 3, 2).unwrap(),
    )
    .add_region(Address::new(0x8000), AccessSize::new(0x1000).unwrap())
    .add_target(
        data_target,
        layout(),
        DramGeometry::new(4, 128, 16).unwrap(),
        DramTiming::new(2, 4, 6, 2, 1).unwrap(),
    )
    .unwrap()
    .add_region_for_target(
        data_target,
        Address::new(0xa000),
        AccessSize::new(0x1000).unwrap(),
    )
    .unwrap();
    let source = GuestSourceId::new(44);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 7, 0x8000)]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(dram, &image)
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(3), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    system
        .cluster()
        .core(CpuId::new(0))
        .unwrap()
        .write_register(rem6_isa_riscv::Register::new(2).unwrap(), 0xa000);

    let run = system
        .drive_attached_until_host_stop_parallel(
            Default::default(),
            Default::default(),
            50,
            |cpu| GuestEventId::new(160 + u64::from(cpu.get())),
        )
        .unwrap();

    assert_eq!(
        run.host_stop().map(|stop| stop.event()),
        Some(GuestEventId::new(160))
    );
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(0))
            .unwrap()
            .read_register(rem6_isa_riscv::Register::new(5).unwrap()),
        0x1122_3344_5566_7788
    );
    let controller = system.dram_memory_controller().unwrap().lock().unwrap();
    assert_eq!(controller.line_count(code_target).unwrap(), 1);
    assert_eq!(controller.line_count(data_target).unwrap(), 1);
    assert_eq!(
        &controller
            .line_data(data_target, Address::new(0xa000))
            .unwrap()[8..16],
        &[0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
    );
}

#[test]
fn topology_system_dram_profiles_preserve_external_memory_metadata() {
    let code_target = MemoryTargetId::new(0);
    let data_target = MemoryTargetId::new(1);
    let code_profile = ExternalMemoryProfile::ddr(
        code_target,
        layout(),
        2,
        1,
        DramGeometry::new(2, 64, 16).unwrap(),
        DramTiming::new(5, 7, 11, 3, 2).unwrap(),
    )
    .unwrap();
    let data_profile = ExternalMemoryProfile::hbm(
        data_target,
        layout(),
        1,
        4,
        DramGeometry::new(4, 128, 16).unwrap(),
        DramTiming::new(2, 4, 6, 2, 1).unwrap(),
    )
    .unwrap();
    let image = BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
        .add_segment(Address::new(0xa000), vec![0x5a; 8])
        .unwrap();
    let dram = RiscvTopologyDramConfig::from_profile(code_profile)
        .add_region(Address::new(0x8000), AccessSize::new(0x1000).unwrap())
        .add_profile_target(data_profile)
        .unwrap()
        .add_region_for_target(
            data_target,
            Address::new(0xa000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();

    let system = RiscvTopologySystem::with_min_remote_delay(
        topology(),
        RiscvClusterTopologyConfig::new([core_config(0, 0, 7, 0x8000)]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(dram, &image)
    .unwrap();
    let controller = system.dram_memory_controller().unwrap().lock().unwrap();

    assert_eq!(
        controller.memory_profile(code_target).unwrap().technology(),
        DramMemoryTechnology::Ddr,
    );
    assert_eq!(
        controller.memory_profile(data_target).unwrap().technology(),
        DramMemoryTechnology::Hbm,
    );
    assert_eq!(controller.line_count(code_target).unwrap(), 1);
    assert_eq!(controller.line_count(data_target).unwrap(), 1);
}

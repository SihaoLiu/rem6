use rem6_boot::BootImage;
use rem6_cache::MshrQueueConfig;
use rem6_coherence::{
    MsiBankDirectoryHarness, ParallelCoherenceRunHistory, PartitionedDirectoryLineHarness,
    TopologyCacheAgentConfig, TopologyDirectoryConfig, TopologyDirectoryHarnessConfig,
    TopologyDramMemoryConfig,
};
use rem6_cpu::{
    CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig,
    RiscvDataAccessEventKind,
};
use rem6_dram::{DramControllerConfig, DramGeometry, DramMemoryController, DramTiming};
use rem6_isa_riscv::Register;
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryTargetId};
use rem6_protocol_msi::MsiState;
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, RiscvDataCacheProtocol, RiscvDataCacheRunHistoryRecord,
    RiscvSystemRunStopReason, RiscvTopologyDramConfig, RiscvTopologyHostConfig,
    RiscvTopologySystem, StopRequest,
};
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

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
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

fn atomic_type(funct5: u32, aq: bool, rl: bool, rs2: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (funct5 << 27)
        | (u32::from(aq) << 26)
        | (u32::from(rl) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x2f
}

fn nop() -> u32 {
    i_type(0, 0, 0x0, 0, 0x13)
}

fn msi_topology() -> Topology {
    TopologyBuilder::new(7)
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
                component("l1d0"),
                kind("l1_cache"),
                PartitionId::new(2),
                clock(1),
            )
            .add_port(port("cpu_side"), PortDirection::Target)
            .unwrap()
            .add_port(port("mem_side"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("l1d1"),
                kind("l1_cache"),
                PartitionId::new(3),
                clock(1),
            )
            .add_port(port("cpu_side"), PortDirection::Target)
            .unwrap()
            .add_port(port("mem_side"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("dir0"),
                kind("directory"),
                PartitionId::new(4),
                clock(1),
            )
            .add_port(port("cache_side"), PortDirection::Target)
            .unwrap()
            .add_port(port("mem_side"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mem0"),
                kind("dram"),
                PartitionId::new(5),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mem1"),
                kind("dram"),
                PartitionId::new(5),
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
        .connect_with_latencies(
            endpoint("cpu1", "ifetch"),
            endpoint("mem0", "requests"),
            2,
            3,
        )
        .unwrap()
        .connect_with_latencies(endpoint("cpu0", "dmem"), endpoint("l1d0", "cpu_side"), 2, 2)
        .unwrap()
        .connect_with_latencies(endpoint("cpu1", "dmem"), endpoint("l1d1", "cpu_side"), 2, 2)
        .unwrap()
        .connect_with_latencies(
            endpoint("l1d0", "mem_side"),
            endpoint("dir0", "cache_side"),
            2,
            3,
        )
        .unwrap()
        .connect_with_latencies(
            endpoint("l1d1", "mem_side"),
            endpoint("dir0", "cache_side"),
            2,
            3,
        )
        .unwrap()
        .connect_with_latencies(
            endpoint("dir0", "mem_side"),
            endpoint("mem1", "requests"),
            4,
            5,
        )
        .unwrap()
        .build()
        .unwrap()
}

fn core_config(cpu: u32, partition: u32, agent_id: u32, entry: u64) -> RiscvCoreTopologyConfig {
    let cpu_name = format!("cpu{cpu}");
    RiscvCoreTopologyConfig::new(
        CpuResetState::new(
            CpuId::new(cpu),
            PartitionId::new(partition),
            agent(agent_id),
            Address::new(entry),
        ),
        endpoint(&cpu_name, "ifetch"),
        endpoint("mem0", "requests"),
        layout(),
        AccessSize::new(4).unwrap(),
    )
    .with_data(
        endpoint(&cpu_name, "dmem"),
        endpoint(&format!("l1d{cpu}"), "cpu_side"),
        layout(),
    )
}

fn code_image() -> BootImage {
    let mut image = BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(s_type(8, 3, 2, 0x3, 0x23)))
        .unwrap();
    for index in 0..20 {
        image = image
            .add_segment(Address::new(0x8004 + index * 4), word(nop()))
            .unwrap();
    }
    image = image
        .add_segment(Address::new(0x8054), word(0x0000_0073))
        .unwrap();
    for index in 0..8 {
        image = image
            .add_segment(Address::new(0x9000 + index * 4), word(nop()))
            .unwrap();
    }
    image
        .add_segment(Address::new(0x9020), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x9024), word(0x0010_0073))
        .unwrap()
}

fn lr_sc_snoop_image() -> BootImage {
    let mut image = BootImage::new(Address::new(0x8000))
        .add_segment(
            Address::new(0x8000),
            word(atomic_type(0x02, false, false, 0, 2, 0x3, 5)),
        )
        .unwrap();
    image = image
        .add_segment(Address::new(0x8004), word(nop()))
        .unwrap()
        .add_segment(
            Address::new(0x8008),
            word(atomic_type(0x03, false, true, 6, 2, 0x3, 7)),
        )
        .unwrap()
        .add_segment(Address::new(0x800c), word(0x0010_0073))
        .unwrap();

    for index in 0..2 {
        image = image
            .add_segment(Address::new(0x9000 + index * 4), word(nop()))
            .unwrap();
    }
    image = image
        .add_segment(Address::new(0x9008), word(s_type(0, 6, 2, 0x3, 0x23)))
        .unwrap();
    for index in 0..40 {
        image = image
            .add_segment(Address::new(0x900c + index * 4), word(nop()))
            .unwrap();
    }
    image
}

fn lr_sc_bank_snoop_image() -> BootImage {
    let mut image = BootImage::new(Address::new(0x8000))
        .add_segment(
            Address::new(0x8000),
            word(atomic_type(0x02, false, false, 0, 2, 0x3, 5)),
        )
        .unwrap();
    image = image
        .add_segment(Address::new(0x8004), word(nop()))
        .unwrap()
        .add_segment(Address::new(0x8008), word(nop()))
        .unwrap()
        .add_segment(
            Address::new(0x800c),
            word(atomic_type(0x03, false, true, 6, 2, 0x3, 7)),
        )
        .unwrap()
        .add_segment(Address::new(0x8010), word(0x0010_0073))
        .unwrap();

    for index in 0..2 {
        image = image
            .add_segment(Address::new(0x9000 + index * 4), word(nop()))
            .unwrap();
    }
    image = image
        .add_segment(Address::new(0x9008), word(s_type(0, 6, 2, 0x3, 0x23)))
        .unwrap();
    for index in 0..40 {
        image = image
            .add_segment(Address::new(0x900c + index * 4), word(nop()))
            .unwrap();
    }
    image
}

fn code_dram_config() -> RiscvTopologyDramConfig {
    RiscvTopologyDramConfig::new(
        MemoryTargetId::new(0),
        layout(),
        DramGeometry::new(2, 128, 16).unwrap(),
        DramTiming::new(5, 7, 11, 3, 2).unwrap(),
    )
    .add_region(Address::new(0x8000), AccessSize::new(0x2000).unwrap())
}

fn data_dram_memory() -> DramMemoryController {
    let target = MemoryTargetId::new(7);
    let mut memory = DramMemoryController::new();
    memory
        .add_target(DramControllerConfig::new(
            target,
            layout(),
            DramGeometry::new(4, 64, 16).unwrap(),
            DramTiming::new(3, 5, 9, 2, 2).unwrap(),
        ))
        .unwrap();
    memory
        .map_region(
            target,
            Address::new(0x3000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    memory
        .insert_line(target, Address::new(0x3000), (0..16).collect())
        .unwrap();
    memory
}

fn msi_data_harness(topology: &Topology) -> PartitionedDirectoryLineHarness {
    PartitionedDirectoryLineHarness::new_with_topology(
        topology,
        TopologyDirectoryHarnessConfig::new(
            layout(),
            Address::new(0x3000),
            TopologyDirectoryConfig::new(component("dir0"), port("cache_side"), port("mem_side")),
            TopologyDramMemoryConfig::new(component("mem1"), port("requests"), data_dram_memory()),
            [
                TopologyCacheAgentConfig::new(agent(7), component("l1d0"), port("mem_side")),
                TopologyCacheAgentConfig::new(agent(8), component("l1d1"), port("mem_side")),
            ],
        ),
    )
    .unwrap()
}

fn msi_bank_data_harness() -> MsiBankDirectoryHarness {
    let mut harness = MsiBankDirectoryHarness::new_with_mshr(
        layout(),
        [agent(7), agent(8)],
        MshrQueueConfig::new(2, 3, 0).unwrap(),
    )
    .unwrap();
    harness
        .insert_backing_line(Address::new(0x3000), (0..16).collect())
        .unwrap();
    harness
}

#[test]
fn topology_system_msi_snoop_invalidates_peer_lr_reservation_before_store_response() {
    let topology = msi_topology();
    let source = GuestSourceId::new(127);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology.clone(),
        RiscvClusterTopologyConfig::new([
            core_config(0, 0, 7, 0x8000),
            core_config(1, 1, 8, 0x9000),
        ]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(code_dram_config(), &lr_sc_snoop_image())
    .unwrap()
    .with_msi_data_cache(msi_data_harness(&topology))
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(6), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    let cpu0 = system.cluster().core(CpuId::new(0)).unwrap();
    let cpu1 = system.cluster().core(CpuId::new(1)).unwrap();
    cpu0.write_register(Register::new(2).unwrap(), 0x3000);
    cpu0.write_register(Register::new(6).unwrap(), 0x0102_0304_0506_0708);
    cpu1.write_register(Register::new(2).unwrap(), 0x3000);
    cpu1.write_register(Register::new(6).unwrap(), 0x1112_1314_1516_1718);

    let run = system
        .drive_attached_until_host_stop_parallel(
            Default::default(),
            Default::default(),
            260,
            |cpu: CpuId| GuestEventId::new(1270 + u64::from(cpu.get())),
        )
        .unwrap();

    assert_eq!(
        run.stop_reason(),
        RiscvSystemRunStopReason::HostStop(StopRequest::new(
            run.final_tick().unwrap(),
            GuestEventId::new(1270),
            source,
            1,
        )),
    );
    assert_eq!(cpu0.read_register(Register::new(7).unwrap()), 1);
    assert_eq!(cpu0.load_reservation(), None);
    let cpu0_sc_failure_tick = cpu0
        .data_access_events()
        .into_iter()
        .find(|event| {
            event.kind() == RiscvDataAccessEventKind::ConditionalFailed
                && event.operation() == MemoryOperation::StoreConditional
        })
        .expect("core0 store conditional failure")
        .tick();
    let cpu1_store_completion_tick = cpu1
        .data_access_events()
        .into_iter()
        .find(|event| {
            event.kind() == RiscvDataAccessEventKind::Completed
                && event.operation() == MemoryOperation::Write
        })
        .expect("core1 store completion")
        .tick();
    assert!(
        cpu0_sc_failure_tick < cpu1_store_completion_tick,
        "SC failure tick {cpu0_sc_failure_tick}, store completion tick {cpu1_store_completion_tick}"
    );
    let cache = system.msi_data_cache().unwrap();
    let harness = cache.lock().unwrap();
    let decisions = harness.directory_decisions();
    assert!(decisions.iter().any(|record| {
        record.decision().request().agent() == agent(8)
            && record
                .decision()
                .snoops()
                .iter()
                .any(|snoop| snoop.target() == agent(7))
    }));
}

#[test]
fn topology_system_msi_bank_snoop_invalidates_peer_lr_reservation_before_later_sc() {
    let topology = msi_topology();
    let source = GuestSourceId::new(128);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology,
        RiscvClusterTopologyConfig::new([
            core_config(0, 0, 7, 0x8000),
            core_config(1, 1, 8, 0x9000),
        ]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(code_dram_config(), &lr_sc_bank_snoop_image())
    .unwrap()
    .with_msi_bank_data_cache(msi_bank_data_harness())
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(6), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    let cpu0 = system.cluster().core(CpuId::new(0)).unwrap();
    let cpu1 = system.cluster().core(CpuId::new(1)).unwrap();
    cpu0.write_register(Register::new(2).unwrap(), 0x3000);
    cpu0.write_register(Register::new(6).unwrap(), 0x0102_0304_0506_0708);
    cpu1.write_register(Register::new(2).unwrap(), 0x3000);
    cpu1.write_register(Register::new(6).unwrap(), 0x1112_1314_1516_1718);

    let run = system
        .drive_attached_until_host_stop_parallel(
            Default::default(),
            Default::default(),
            260,
            |cpu: CpuId| GuestEventId::new(1280 + u64::from(cpu.get())),
        )
        .unwrap();

    assert_eq!(
        run.stop_reason(),
        RiscvSystemRunStopReason::HostStop(StopRequest::new(
            run.final_tick().unwrap(),
            GuestEventId::new(1280),
            source,
            1,
        )),
    );
    assert_eq!(cpu0.read_register(Register::new(7).unwrap()), 1);
    assert_eq!(cpu0.load_reservation(), None);
    let cpu0_sc_failure_tick = cpu0
        .data_access_events()
        .into_iter()
        .find(|event| {
            event.kind() == RiscvDataAccessEventKind::ConditionalFailed
                && event.operation() == MemoryOperation::StoreConditional
        })
        .expect("core0 store conditional failure")
        .tick();
    let cpu1_store_completion_tick = cpu1
        .data_access_events()
        .into_iter()
        .find(|event| {
            event.kind() == RiscvDataAccessEventKind::Completed
                && event.operation() == MemoryOperation::Write
        })
        .expect("core1 store completion")
        .tick();
    assert!(
        cpu1_store_completion_tick < cpu0_sc_failure_tick,
        "store completion tick {cpu1_store_completion_tick}, SC failure tick {cpu0_sc_failure_tick}"
    );
    let cache = system.msi_bank_data_cache().unwrap();
    let harness = cache.lock().unwrap();
    let history = harness.parallel_cycle_history();
    assert_eq!(history.total_accepted(), 2);
    assert_eq!(history.accepted_by_agent(agent(7)), 1);
    assert_eq!(history.accepted_by_agent(agent(8)), 1);
    assert!(harness.directory_decisions().iter().any(|decision| {
        decision.request().agent() == agent(8)
            && decision
                .snoops()
                .iter()
                .any(|snoop| snoop.target() == agent(7))
    }));
}

#[test]
fn topology_system_routes_riscv_data_accesses_through_msi_cache_bank() {
    let topology = msi_topology();
    let source = GuestSourceId::new(122);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology,
        RiscvClusterTopologyConfig::new([
            core_config(0, 0, 7, 0x8000),
            core_config(1, 1, 8, 0x9000),
        ]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(code_dram_config(), &code_image())
    .unwrap()
    .with_msi_bank_data_cache(msi_bank_data_harness())
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(6), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    system
        .cluster()
        .core(CpuId::new(0))
        .unwrap()
        .write_register(Register::new(2).unwrap(), 0x3000);
    system
        .cluster()
        .core(CpuId::new(0))
        .unwrap()
        .write_register(Register::new(3).unwrap(), 0x1122_3344_5566_7788);
    system
        .cluster()
        .core(CpuId::new(1))
        .unwrap()
        .write_register(Register::new(2).unwrap(), 0x3000);

    let run = system
        .drive_attached_until_host_stop_parallel(
            Default::default(),
            Default::default(),
            240,
            |cpu: CpuId| GuestEventId::new(1220 + u64::from(cpu.get())),
        )
        .unwrap();

    assert_eq!(
        run.stop_reason(),
        RiscvSystemRunStopReason::HostStop(StopRequest::new(
            run.final_tick().unwrap(),
            GuestEventId::new(1221),
            source,
            1,
        )),
    );
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(1))
            .unwrap()
            .read_register(Register::new(5).unwrap()),
        0x1122_3344_5566_7788,
    );

    let cache = system.msi_bank_data_cache().unwrap();
    let harness = cache.lock().unwrap();
    let history = harness.parallel_cycle_history();
    assert_eq!(history.cycle_count(), 2);
    assert_eq!(history.total_accepted(), 2);
    assert_eq!(history.accepted_by_agent(agent(7)), 1);
    assert_eq!(history.accepted_by_agent(agent(8)), 1);
    assert_eq!(history.accepted_by_line(Address::new(0x3000)), 2);
    assert_eq!(history.total_scheduled_misses(), 2);
    assert!(history.total_responses() >= 2);
    drop(harness);
    let cache_runs = system.msi_bank_data_cache_runs();
    let cache_history = ParallelCoherenceRunHistory::from_runs(&cache_runs);
    assert_eq!(cache_runs.len(), history.cycle_count());
    assert_eq!(run.data_cache_runs(), cache_runs.as_slice());
    assert_eq!(run.data_cache_run_count(), history.cycle_count());
    assert_eq!(
        run.data_cache_run_count_for_protocol(RiscvDataCacheProtocol::Msi),
        history.cycle_count(),
    );
    assert_eq!(run.unattributed_data_cache_run_count(), 0);
    assert_eq!(run.data_cache_parallel_run_history(), cache_history);
    assert_eq!(system.msi_data_cache_run_history(), cache_history);
    assert_eq!(
        system.data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Msi),
        cache_history,
    );
}

#[test]
fn topology_system_routes_dirty_peer_read_through_msi_data_cache() {
    let topology = msi_topology();
    let source = GuestSourceId::new(121);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology.clone(),
        RiscvClusterTopologyConfig::new([
            core_config(0, 0, 7, 0x8000),
            core_config(1, 1, 8, 0x9000),
        ]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(code_dram_config(), &code_image())
    .unwrap()
    .with_msi_data_cache(msi_data_harness(&topology))
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(6), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    system
        .cluster()
        .core(CpuId::new(0))
        .unwrap()
        .write_register(Register::new(2).unwrap(), 0x3000);
    system
        .cluster()
        .core(CpuId::new(0))
        .unwrap()
        .write_register(Register::new(3).unwrap(), 0x1122_3344_5566_7788);
    system
        .cluster()
        .core(CpuId::new(1))
        .unwrap()
        .write_register(Register::new(2).unwrap(), 0x3000);

    let run = system
        .drive_attached_until_host_stop_parallel(
            Default::default(),
            Default::default(),
            240,
            |cpu: CpuId| GuestEventId::new(1210 + u64::from(cpu.get())),
        )
        .unwrap();

    assert_eq!(
        run.stop_reason(),
        RiscvSystemRunStopReason::HostStop(StopRequest::new(
            run.final_tick().unwrap(),
            GuestEventId::new(1211),
            source,
            1,
        )),
    );
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(1))
            .unwrap()
            .read_register(Register::new(5).unwrap()),
        0x1122_3344_5566_7788,
    );
    assert_eq!(run.active_dram_target_count(), 2);
    assert_eq!(
        run.dram_target_activity(MemoryTargetId::new(7))
            .unwrap()
            .profile()
            .read_count(),
        1,
    );

    let cache = system.msi_data_cache().unwrap();
    let harness = cache.lock().unwrap();
    assert_eq!(harness.cache_state(agent(7)).unwrap(), MsiState::Shared);
    assert_eq!(harness.cache_state(agent(8)).unwrap(), MsiState::Shared);
    assert_eq!(harness.dram_memory_accesses().len(), 1);
    drop(harness);

    let cache_runs = system.msi_data_cache_runs();
    assert_eq!(cache_runs.len(), 2);
    assert_eq!(run.data_cache_runs(), cache_runs.as_slice());
    let cache_history = ParallelCoherenceRunHistory::from_runs(&cache_runs);
    assert_eq!(run.data_cache_parallel_run_history(), cache_history);
    assert_eq!(system.msi_data_cache_run_history(), cache_history);
    assert_eq!(system.data_cache_parallel_run_history(), cache_history);
    assert_eq!(
        system.data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Msi),
        cache_history,
    );
    assert_eq!(
        system.data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Mesi),
        ParallelCoherenceRunHistory::default(),
    );
    assert_eq!(
        system
            .data_cache_parallel_run_histories_by_protocol()
            .get(&RiscvDataCacheProtocol::Msi),
        Some(&cache_history),
    );
    let cache_history_record =
        RiscvDataCacheRunHistoryRecord::new(RiscvDataCacheProtocol::Msi, cache_history);
    assert_eq!(
        system.data_cache_parallel_run_history_records(),
        vec![cache_history_record.clone()],
    );
    assert_eq!(
        system.data_cache_parallel_run_history_record(RiscvDataCacheProtocol::Msi),
        Some(cache_history_record),
    );
    assert_eq!(
        system.data_cache_parallel_run_count_for_protocol(RiscvDataCacheProtocol::Msi),
        cache_runs.len(),
    );
    assert!(system.has_data_cache_parallel_run_history_for_protocol(RiscvDataCacheProtocol::Msi));
    assert_eq!(
        system.attributed_data_cache_parallel_run_count(),
        cache_runs.len(),
    );
    assert_eq!(system.unattributed_data_cache_parallel_run_count(), 0);
    assert_eq!(run.data_cache_run_count(), 2);
    assert_eq!(
        run.data_cache_protocols(),
        vec![Some(RiscvDataCacheProtocol::Msi); cache_runs.len()],
    );
    assert_eq!(run.unattributed_data_cache_run_count(), 0);
    assert_eq!(
        run.data_cache_run_count_for_protocol(RiscvDataCacheProtocol::Msi),
        cache_runs.len(),
    );
    assert_eq!(
        run.data_cache_run_count_for_protocol(RiscvDataCacheProtocol::Mesi),
        0,
    );
    assert!(run.has_data_cache_protocol(RiscvDataCacheProtocol::Msi));
    assert!(!run.has_data_cache_protocol(RiscvDataCacheProtocol::Moesi));
    assert_eq!(
        run.data_cache_runs_for_protocol(RiscvDataCacheProtocol::Msi),
        cache_runs.clone(),
    );
    assert!(run
        .data_cache_runs_for_protocol(RiscvDataCacheProtocol::Mesi)
        .is_empty());
    assert_eq!(
        run.data_cache_parallel_scheduler_epoch_count(),
        cache_runs
            .iter()
            .map(rem6_coherence::ParallelCoherenceRunSummary::epoch_count)
            .sum::<usize>(),
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_dispatch_count(),
        cache_runs
            .iter()
            .map(rem6_coherence::ParallelCoherenceRunSummary::dispatch_count)
            .sum::<usize>(),
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_batch_count(),
        cache_runs
            .iter()
            .map(rem6_coherence::ParallelCoherenceRunSummary::batch_count)
            .sum::<usize>(),
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_profile().dispatch_count(),
        run.data_cache_parallel_scheduler_dispatch_count(),
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_max_workers(),
        cache_runs
            .iter()
            .map(rem6_coherence::ParallelCoherenceRunSummary::max_parallel_workers)
            .max()
            .unwrap_or(0),
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_dispatches().len(),
        run.data_cache_parallel_scheduler_dispatch_count(),
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_batches().len(),
        run.data_cache_parallel_scheduler_batch_count(),
    );
    assert!(!run
        .data_cache_parallel_scheduler_worker_partitions()
        .is_empty());
    assert!(run
        .data_cache_parallel_scheduler_profile()
        .has_parallel_work());
    let partition_activities = run.data_cache_parallel_scheduler_partition_activities();
    assert_eq!(
        run.active_data_cache_parallel_scheduler_partition_count(),
        partition_activities.len(),
    );
    let partition = *partition_activities.keys().next().unwrap();
    assert_eq!(
        run.data_cache_parallel_scheduler_partition_activity(partition),
        Some(partition_activities[&partition]),
    );
    assert!(run.has_data_cache_parallel_scheduler_partition_activity(partition));
    assert_eq!(
        run.data_cache_parallel_scheduler_dispatches_for_partition(partition)
            .len(),
        partition_activities[&partition].dispatch_count(),
    );
    assert_eq!(
        run.full_system_parallel_scheduler_profile()
            .dispatch_count(),
        run.parallel_scheduler_profile().dispatch_count()
            + run.data_cache_parallel_scheduler_dispatch_count(),
    );
    assert_eq!(
        run.full_system_parallel_scheduler_max_workers(),
        run.parallel_scheduler_profile()
            .max_parallel_workers()
            .max(run.data_cache_parallel_scheduler_max_workers()),
    );
    assert_eq!(
        run.full_system_parallel_scheduler_dispatches().len(),
        run.parallel_scheduler_dispatches().len()
            + run.data_cache_parallel_scheduler_dispatches().len(),
    );
    assert_eq!(
        run.full_system_parallel_scheduler_batches().len(),
        run.parallel_scheduler_batches().len() + run.data_cache_parallel_scheduler_batches().len(),
    );
    assert!(
        run.full_system_parallel_scheduler_worker_partitions().len()
            >= run.data_cache_parallel_scheduler_worker_partitions().len()
    );
    let full_partition_activities = run.full_system_parallel_scheduler_partition_activities();
    assert!(full_partition_activities.len() >= partition_activities.len());
    assert_eq!(
        run.full_system_parallel_scheduler_partition_activity(partition),
        Some(full_partition_activities[&partition]),
    );
    assert!(run.has_full_system_parallel_scheduler_partition_activity(partition));
    assert_eq!(
        run.full_system_parallel_scheduler_dispatches_for_partition(partition)
            .len(),
        run.parallel_scheduler_dispatches_for_partition(partition)
            .len()
            + run
                .data_cache_parallel_scheduler_dispatches_for_partition(partition)
                .len(),
    );
    assert!(run.has_full_system_parallel_scheduler_work());
    assert_eq!(run.data_cache_wait_for_edge_count(), 0);
    assert!(run
        .remaining_data_cache_wait_for_edge_kind_counts()
        .is_empty());
    assert_eq!(run.data_cache_deadlock_diagnostic_count(), 0);
    assert!(!run.has_data_cache_wait_for_edges());
    assert_eq!(cache_runs[0].dram_access_count(), 1);
    assert_eq!(cache_runs[1].dram_access_count(), 0);
    assert!(cache_runs[1].has_directory_activity());
}

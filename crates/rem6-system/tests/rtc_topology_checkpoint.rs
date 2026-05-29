use rem6_checkpoint::CheckpointComponentId;
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_platform::{
    Platform, PlatformBuilder, PlatformPl031RtcConfig, PlatformRtcConfig, PlatformSp804TimerConfig,
    PlatformSp804TimerInterruptConfig, PlatformTopologyRoute,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, RiscvTopologyHostConfig,
    RiscvTopologySystem, SystemActionOutcome,
};
use rem6_timer::{
    Mc146818RtcMmioSnapshot, Pl031RtcMmioSnapshot, Pl031Snapshot, Pl031SnapshotFields, RtcDateTime,
    RtcEncoding, RtcSnapshot, Sp804DualTimer, Sp804DualTimerMmioDevice, Sp804DualTimerMmioSnapshot,
    Sp804TimerControl, RTC_CMOS_REGISTER_COUNT, SP804_BGLOAD_OFFSET, SP804_CONTROL_OFFSET,
    SP804_LOAD_OFFSET,
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

fn clock(period: u64) -> rem6_kernel::ClockDomain {
    rem6_kernel::ClockDomain::new(period).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn topology_with_rtc() -> Topology {
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
                component("rtc0"),
                kind("rtc"),
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
        .connect_with_latencies(endpoint("cpu0", "mmio"), endpoint("rtc0", "mmio"), 2, 2)
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

fn platform_with_rtc(topology: &Topology, base: Address) -> Platform {
    let route = PlatformTopologyRoute::new(endpoint("cpu0", "mmio"), endpoint("rtc0", "mmio"))
        .resolve(topology)
        .unwrap();
    PlatformBuilder::from_topology(topology)
        .add_rtc(PlatformRtcConfig {
            base,
            size: AccessSize::new(2).unwrap(),
            route,
            time: RtcDateTime::new(2026, 5, 29, 1, 2, 3, 6).unwrap(),
            encoding: RtcEncoding::Bcd,
            periodic_interrupt: None,
        })
        .build()
        .unwrap()
}

fn platform_with_pl031(topology: &Topology, base: Address) -> Platform {
    let route = PlatformTopologyRoute::new(endpoint("cpu0", "mmio"), endpoint("rtc0", "mmio"))
        .resolve(topology)
        .unwrap();
    PlatformBuilder::from_topology(topology)
        .add_pl031_rtc(PlatformPl031RtcConfig {
            base,
            size: AccessSize::new(0x1000).unwrap(),
            route,
            initial_time: 10,
            ticks_per_second: 5,
            interrupt: None,
        })
        .build()
        .unwrap()
}

fn platform_with_sp804(topology: &Topology, base: Address) -> Platform {
    let route = PlatformTopologyRoute::new(endpoint("cpu0", "mmio"), endpoint("rtc0", "mmio"))
        .resolve(topology)
        .unwrap();
    PlatformBuilder::from_topology(topology)
        .add_sp804_timer(PlatformSp804TimerConfig {
            base,
            size: AccessSize::new(0x1000).unwrap(),
            route,
            clock0: 2,
            clock1: 4,
            interrupts: Some([
                PlatformSp804TimerInterruptConfig {
                    line: rem6_interrupt::InterruptLineId::new(57),
                    target: rem6_interrupt::InterruptTargetId::new(0),
                    source: rem6_interrupt::InterruptSourceId::new(77),
                    latency: 2,
                },
                PlatformSp804TimerInterruptConfig {
                    line: rem6_interrupt::InterruptLineId::new(58),
                    target: rem6_interrupt::InterruptTargetId::new(0),
                    source: rem6_interrupt::InterruptSourceId::new(78),
                    latency: 2,
                },
            ]),
        })
        .build()
        .unwrap()
}

fn configured_sp804_device(base: Address) -> Sp804DualTimerMmioDevice {
    let mut timers = Sp804DualTimer::new(2, 4).unwrap();
    let timer0_control = Sp804TimerControl::default()
        .with_interrupt_enabled(true)
        .with_enabled(true)
        .with_one_shot(true);
    timers
        .timer_mut(0)
        .unwrap()
        .write_register(SP804_LOAD_OFFSET, 3, 10)
        .unwrap();
    timers
        .timer_mut(0)
        .unwrap()
        .write_register(SP804_CONTROL_OFFSET, timer0_control.bits(), 10)
        .unwrap();
    timers.timer_mut(0).unwrap().record_zero(16).unwrap();

    let timer1_control = Sp804TimerControl::default()
        .with_interrupt_enabled(true)
        .with_periodic(true)
        .with_enabled(true);
    timers
        .timer_mut(1)
        .unwrap()
        .write_register(SP804_LOAD_OFFSET, 5, 20)
        .unwrap();
    timers
        .timer_mut(1)
        .unwrap()
        .write_register(SP804_BGLOAD_OFFSET, 2, 20)
        .unwrap();
    timers
        .timer_mut(1)
        .unwrap()
        .write_register(SP804_CONTROL_OFFSET, timer1_control.bits(), 20)
        .unwrap();

    Sp804DualTimerMmioDevice::new(base, timers)
}

fn rtc_snapshot(
    selected_address: u8,
    cmos_index: usize,
    cmos_value: u8,
) -> Mc146818RtcMmioSnapshot {
    let mut cmos = [0; RTC_CMOS_REGISTER_COUNT];
    cmos[cmos_index] = cmos_value;
    Mc146818RtcMmioSnapshot::new(
        selected_address,
        cmos,
        RtcSnapshot::new(
            [0x03, 0, 0x02, 0, 0x01, 0, 0x06, 0x29, 0x05, 0x26],
            0x26,
            0x42,
        ),
    )
}

fn pl031_snapshot(
    time_value: u32,
    last_written_tick: u64,
    match_value: u32,
) -> Pl031RtcMmioSnapshot {
    Pl031RtcMmioSnapshot::new(Pl031Snapshot::from_fields(Pl031SnapshotFields {
        time_value,
        last_written_tick,
        load_value: time_value,
        match_value,
        raw_interrupt: true,
        interrupt_mask: true,
        pending_interrupt: true,
        ticks_per_second: 5,
        generation: 3,
    }))
}

fn sp804_snapshot(base: Address) -> Sp804DualTimerMmioSnapshot {
    configured_sp804_device(base).snapshot()
}

#[test]
fn topology_host_controller_checkpoints_attached_rtc() {
    let topology = topology_with_rtc();
    let rtc_base = Address::new(0x70);
    let platform = platform_with_rtc(&topology, rtc_base);
    let source = GuestSourceId::new(52);
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
    let component = CheckpointComponentId::new("rtc.70").unwrap();
    let captured = rtc_snapshot(0xa0, 0x20, 0x5a);
    let empty = rtc_snapshot(0x00, 0x20, 0x00);
    let rtc = system.platform().unwrap().rtc(rtc_base).unwrap().clone();
    rtc.restore(&captured).unwrap();
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .rtc_checkpoint_bank()
        .is_some());

    let checkpoint = HostActionRecord::new(
        31,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(198),
        source,
        HostAction::Checkpoint {
            label: "attached-rtc".to_string(),
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
            .chunk(&component, "rtc")
            .unwrap()
            .len()
            > RTC_CMOS_REGISTER_COUNT
    );

    rtc.restore(&empty).unwrap();
    assert_ne!(rtc.snapshot(), captured);

    let restore = HostActionRecord::new(
        45,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(199),
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
            event: GuestEventId::new(199),
            source,
            manifest,
        }
    );
    assert_eq!(rtc.snapshot(), captured);
}

#[test]
fn topology_host_controller_checkpoints_attached_pl031() {
    let topology = topology_with_rtc();
    let rtc_base = Address::new(0x1c17_0000);
    let platform = platform_with_pl031(&topology, rtc_base);
    let source = GuestSourceId::new(53);
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
    let component = CheckpointComponentId::new("pl031.1c170000").unwrap();
    let captured = pl031_snapshot(40, 15, 45);
    let empty = pl031_snapshot(0, 0, 0);
    let rtc = system
        .platform()
        .unwrap()
        .pl031_rtc(rtc_base)
        .unwrap()
        .clone();
    rtc.restore(&captured).unwrap();
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .pl031_checkpoint_bank()
        .is_some());

    let checkpoint = HostActionRecord::new(
        32,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(200),
        source,
        HostAction::Checkpoint {
            label: "attached-pl031".to_string(),
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
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .checkpoints()
        .chunk(&component, "pl031")
        .is_some());

    rtc.restore(&empty).unwrap();
    assert_ne!(rtc.snapshot(), captured);

    let restore = HostActionRecord::new(
        46,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(201),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    let restored = host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(
        restored,
        SystemActionOutcome::CheckpointRestored {
            tick: 46,
            event: GuestEventId::new(201),
            source,
            manifest,
        }
    );
    assert_eq!(rtc.snapshot(), captured);
}

#[test]
fn topology_host_controller_checkpoints_attached_sp804() {
    let topology = topology_with_rtc();
    let timer_base = Address::new(0x1c11_0000);
    let platform = platform_with_sp804(&topology, timer_base);
    let source = GuestSourceId::new(54);
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
    let component = CheckpointComponentId::new("sp804.1c110000").unwrap();
    let captured = sp804_snapshot(timer_base);
    let empty =
        Sp804DualTimerMmioDevice::new(timer_base, Sp804DualTimer::new(2, 4).unwrap()).snapshot();
    let timer = system
        .platform()
        .unwrap()
        .sp804_timer(timer_base)
        .unwrap()
        .clone();
    timer.restore(&captured).unwrap();
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .sp804_checkpoint_bank()
        .is_some());

    let checkpoint = HostActionRecord::new(
        33,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(202),
        source,
        HostAction::Checkpoint {
            label: "attached-sp804".to_string(),
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
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .checkpoints()
        .chunk(&component, "sp804")
        .is_some());

    timer.restore(&empty).unwrap();
    assert_ne!(timer.snapshot(), captured);

    let restore = HostActionRecord::new(
        47,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(203),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    let restored = host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(
        restored,
        SystemActionOutcome::CheckpointRestored {
            tick: 47,
            event: GuestEventId::new(203),
            source,
            manifest,
        }
    );
    assert_eq!(timer.snapshot(), captured);
}

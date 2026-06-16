use rem6_checkpoint::CheckpointComponentId;
use rem6_cpu::{CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_platform::{Platform, PlatformBuilder, PlatformReadfileConfig, PlatformTopologyRoute};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, RiscvTopologyHostConfig,
    RiscvTopologySystem, SystemActionOutcome,
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

fn topology_with_readfile() -> Topology {
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
                kind("memory"),
                PartitionId::new(2),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("readfile0"),
                kind("readfile"),
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
        .connect_with_latencies(
            endpoint("cpu0", "mmio"),
            endpoint("readfile0", "mmio"),
            2,
            2,
        )
        .unwrap()
        .build()
        .unwrap()
}

fn core_config(cpu: u32, partition: u32, agent: u32, reset_pc: u64) -> RiscvCoreTopologyConfig {
    RiscvCoreTopologyConfig::new(
        CpuResetState::new(
            rem6_cpu::CpuId::new(cpu),
            PartitionId::new(partition),
            AgentId::new(agent),
            Address::new(reset_pc),
        ),
        endpoint(&format!("cpu{cpu}"), "ifetch"),
        endpoint("mem0", "requests"),
        layout(),
        AccessSize::new(4).unwrap(),
    )
}

fn readfile_platform(topology: &Topology) -> Platform {
    let base = Address::new(0xa000);
    let route = PlatformTopologyRoute::new(endpoint("cpu0", "mmio"), endpoint("readfile0", "mmio"))
        .resolve(topology)
        .unwrap();
    PlatformBuilder::from_topology(topology)
        .add_readfile(PlatformReadfileConfig {
            base,
            size: AccessSize::new(0x100).unwrap(),
            route,
            payload: b"bootargs\n".to_vec(),
        })
        .build()
        .unwrap()
}

fn topology_system(topology: Topology) -> RiscvTopologySystem {
    RiscvTopologySystem::with_min_remote_delay(
        topology,
        RiscvClusterTopologyConfig::new([
            core_config(0, 0, 7, 0x8000),
            core_config(1, 1, 8, 0x9000),
        ]),
        2,
    )
    .unwrap()
}

fn system_with_readfile(host_first: bool) -> (RiscvTopologySystem, GuestSourceId) {
    let topology = topology_with_readfile();
    let platform = readfile_platform(&topology);
    let source = GuestSourceId::new(46);
    let host = RiscvTopologyHostConfig::new(PartitionId::new(4), 2, source);
    let system = topology_system(topology);

    let system = if host_first {
        system
            .with_host_controller(host, StatsRegistry::new())
            .unwrap()
            .with_platform(platform)
            .unwrap()
    } else {
        system
            .with_platform(platform)
            .unwrap()
            .with_host_controller(host, StatsRegistry::new())
            .unwrap()
    };

    (system, source)
}

fn assert_readfile_checkpoint_roundtrip(system: RiscvTopologySystem, source: GuestSourceId) {
    let controller = system.host_controller().unwrap();
    let readfile_component = CheckpointComponentId::new("readfile.a000").unwrap();
    {
        let host = controller.lock().unwrap();
        let bank = host
            .executor()
            .readfile_checkpoint_bank()
            .expect("host executor should have attached readfile checkpoint bank");
        assert_eq!(bank.components(), vec![readfile_component.clone()]);
    }

    let checkpoint = HostActionRecord::new(
        24,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(190),
        source,
        HostAction::Checkpoint {
            label: "attached-readfile".to_string(),
        },
    );
    let manifest = match system
        .host_controller()
        .unwrap()
        .lock()
        .unwrap()
        .executor_mut()
        .apply(&checkpoint)
        .unwrap()
    {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };

    let readfile = manifest
        .states()
        .iter()
        .find(|state| state.component() == &readfile_component)
        .expect("checkpoint manifest should include readfile platform state");
    assert!(readfile
        .chunks()
        .iter()
        .any(|chunk| { chunk.name() == "readfile" && chunk.payload().ends_with(b"bootargs\n") }));

    let restore = HostActionRecord::new(
        25,
        PartitionId::new(4),
        PartitionId::new(4),
        GuestEventId::new(191),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    match system
        .host_controller()
        .unwrap()
        .lock()
        .unwrap()
        .executor_mut()
        .apply(&restore)
        .unwrap()
    {
        SystemActionOutcome::CheckpointRestored {
            manifest: restored, ..
        } => assert_eq!(restored.label(), "attached-readfile"),
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn topology_host_controller_checkpoints_attached_readfile_payload() {
    let (system, source) = system_with_readfile(false);

    assert_readfile_checkpoint_roundtrip(system, source);
}

#[test]
fn topology_platform_attach_checkpoints_readfile_after_host_controller() {
    let (system, source) = system_with_readfile(true);

    assert_readfile_checkpoint_roundtrip(system, source);
}

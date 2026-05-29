use rem6_checkpoint::CheckpointComponentId;
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_stats::StatsRegistry;
use rem6_storage::{RawStorageImage, StorageSectorId};
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, RiscvTopologyHostConfig,
    RiscvTopologySystem, StorageImageCheckpointPort, SystemActionOutcome,
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

fn sector(byte: u8) -> [u8; 512] {
    [byte; 512]
}

fn image_bytes(bytes: &[u8]) -> Vec<u8> {
    bytes.iter().flat_map(|byte| sector(*byte)).collect()
}

fn topology() -> Topology {
    TopologyBuilder::new(2)
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
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(
            endpoint("cpu0", "ifetch"),
            endpoint("mem0", "requests"),
            1,
            1,
        )
        .unwrap()
        .connect_with_latencies(endpoint("cpu0", "dmem"), endpoint("mem0", "requests"), 1, 1)
        .unwrap()
        .build()
        .unwrap()
}

fn core_config() -> RiscvCoreTopologyConfig {
    RiscvCoreTopologyConfig::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(7),
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

fn base_system() -> RiscvTopologySystem {
    RiscvTopologySystem::with_min_remote_delay(
        topology(),
        RiscvClusterTopologyConfig::new([core_config()]),
        1,
    )
    .unwrap()
}

fn host_config(source: GuestSourceId) -> RiscvTopologyHostConfig {
    RiscvTopologyHostConfig::new(PartitionId::new(1), 1, source)
}

#[test]
fn topology_host_controller_attaches_existing_storage_image_checkpoint_port() {
    let source = GuestSourceId::new(91);
    let component = CheckpointComponentId::new("storage.disk0").unwrap();
    let image = RawStorageImage::from_bytes(image_bytes(&[0x11, 0x22])).unwrap();
    image
        .write_sector(StorageSectorId::new(1), sector(0xaa))
        .unwrap();
    let expected = image.snapshot();
    let system = base_system()
        .with_storage_image_checkpoint_port(StorageImageCheckpointPort::raw(
            component.clone(),
            image.clone(),
        ))
        .unwrap()
        .with_host_controller(host_config(source), StatsRegistry::new())
        .unwrap();
    let host = system.host_controller().unwrap();

    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .storage_image_checkpoint_bank()
            .unwrap()
            .components(),
        vec![component.clone()]
    );

    let checkpoint = HostActionRecord::new(
        20,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(201),
        source,
        HostAction::Checkpoint {
            label: "topology-storage".to_string(),
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
        .any(|state| state.component() == &component
            && state
                .chunks()
                .iter()
                .any(|chunk| chunk.name() == "storage-image")));

    image
        .write_sector(StorageSectorId::new(1), sector(0xbb))
        .unwrap();
    let restore = HostActionRecord::new(
        30,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(202),
        source,
        HostAction::RestoreCheckpoint { manifest },
    );
    host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(image.snapshot(), expected);
}

#[test]
fn topology_host_controller_attaches_late_storage_image_checkpoint_port() {
    let source = GuestSourceId::new(92);
    let component = CheckpointComponentId::new("storage.disk1").unwrap();
    let image = RawStorageImage::from_bytes(image_bytes(&[0x31])).unwrap();
    let system = base_system()
        .with_host_controller(host_config(source), StatsRegistry::new())
        .unwrap()
        .with_storage_image_checkpoint_port(StorageImageCheckpointPort::raw(
            component.clone(),
            image,
        ))
        .unwrap();
    let host = system.host_controller().unwrap();

    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .storage_image_checkpoint_bank()
            .unwrap()
            .components(),
        vec![component]
    );
}

use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError};
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_interrupt::{InterruptController, InterruptLineId, InterruptTargetId};
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarSpec, PciBridgeBusRange, PciBridgeConfig, PciClassCode,
    PciConfigAperture, PciDeviceIdentity, PciEndpointConfig, PciFunctionAddress,
    PciHostAddressBases, PciHostBridge, PciInterruptPin, PciLegacyInterruptMapper,
    PciLegacyInterruptPolicy, PciLegacyInterruptRouter, PciLegacyInterruptRoutingEntry,
    PciLegacyInterruptRoutingTable,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, PciHostCheckpointPort,
    PciLegacyInterruptRouterCheckpointPort, RiscvTopologyHostConfig, RiscvTopologySystem,
    RiscvTopologySystemError, SystemActionOutcome, SystemError,
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

fn pci_bridge(function: PciFunctionAddress, secondary: u8) -> PciBridgeConfig {
    PciBridgeConfig::new(
        function,
        PciDeviceIdentity::new(0x1011, 0x0026),
        PciClassCode::new(0x06, 0x04, 0x00, 0x00),
        PciBridgeBusRange::new(function.bus(), secondary, secondary).unwrap(),
    )
}

fn pci_endpoint(function: PciFunctionAddress) -> PciEndpointConfig {
    let mut endpoint = PciEndpointConfig::new(
        function,
        PciDeviceIdentity::new(0x1291, 0x1293),
        PciClassCode::new(0x02, 0x00, 0x00, 0x00),
    );
    endpoint
        .install_bar(
            PciBarSpec::new(
                PciBarIndex::new(0).unwrap(),
                PciBarKind::Memory32 {
                    prefetchable: false,
                },
                AccessSize::new(0x1000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    endpoint
}

fn pci_host() -> Arc<Mutex<PciHostBridge>> {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 2).unwrap();
    let bases = PciHostAddressBases::new(
        Address::new(0x1000_0000),
        Address::new(0x8000_0000),
        Address::new(0xa000_0000),
    );
    let mut host = PciHostBridge::with_address_bases(aperture, bases);
    host.register_bridge(pci_bridge(PciFunctionAddress::new(0, 1, 0).unwrap(), 1))
        .unwrap();
    host.register_endpoint(pci_endpoint(PciFunctionAddress::new(1, 2, 0).unwrap()))
        .unwrap();
    Arc::new(Mutex::new(host))
}

fn root_function() -> PciFunctionAddress {
    PciFunctionAddress::new(0, 1, 0).unwrap()
}

fn legacy_router_state(intd_line: Option<u64>) -> PciLegacyInterruptRouter {
    let mapper = PciLegacyInterruptMapper::new(
        InterruptLineId::new(32),
        4,
        PciLegacyInterruptPolicy::DeviceModulo,
    )
    .unwrap();
    let mut table = PciLegacyInterruptRoutingTable::new(mapper)
        .with_entry(
            PciLegacyInterruptRoutingEntry::new(
                root_function(),
                PciInterruptPin::IntC,
                InterruptLineId::new(48),
            )
            .unwrap(),
        )
        .unwrap();
    if let Some(line) = intd_line {
        table = table
            .with_entry(
                PciLegacyInterruptRoutingEntry::new(
                    root_function(),
                    PciInterruptPin::IntD,
                    InterruptLineId::new(line),
                )
                .unwrap(),
            )
            .unwrap();
    }
    PciLegacyInterruptRouter::new(
        table,
        InterruptTargetId::new(7),
        PartitionId::new(0),
        2,
        Arc::new(Mutex::new(InterruptController::new())),
    )
    .unwrap()
}

fn legacy_router() -> Arc<Mutex<PciLegacyInterruptRouter>> {
    Arc::new(Mutex::new(legacy_router_state(None)))
}

fn legacy_router_with_intd(line: u64) -> Arc<Mutex<PciLegacyInterruptRouter>> {
    Arc::new(Mutex::new(legacy_router_state(Some(line))))
}

fn replace_router_intd(router: &Arc<Mutex<PciLegacyInterruptRouter>>, line: u64) {
    *router.lock().unwrap() = legacy_router_state(Some(line));
}

fn router_line(router: &Arc<Mutex<PciLegacyInterruptRouter>>, pin: PciInterruptPin) -> u64 {
    router
        .lock()
        .unwrap()
        .line(root_function(), pin)
        .unwrap()
        .get()
}

#[test]
fn topology_host_controller_attaches_existing_pci_host_checkpoint_port() {
    let source = GuestSourceId::new(121);
    let component = CheckpointComponentId::new("pci.host0").unwrap();
    let pci_host = pci_host();
    let system = base_system()
        .with_pci_host_checkpoint_port(PciHostCheckpointPort::new(
            component.clone(),
            Arc::clone(&pci_host),
        ))
        .unwrap()
        .with_host_controller(host_config(source), StatsRegistry::new())
        .unwrap();
    let host = system.host_controller().unwrap();

    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .pci_host_checkpoint_bank()
            .unwrap()
            .components(),
        vec![component.clone()]
    );

    let checkpoint = HostActionRecord::new(
        20,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(410),
        source,
        HostAction::Checkpoint {
            label: "topology-pci-host".to_string(),
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

    assert!(manifest.states().iter().any(|state| {
        state.component() == &component
            && ["host-topology", "host-endpoint-config-space"]
                .into_iter()
                .all(|name| state.chunks().iter().any(|chunk| chunk.name() == name))
    }));
}

#[test]
fn topology_host_controller_attaches_existing_pci_legacy_router_checkpoint_port() {
    let source = GuestSourceId::new(122);
    let component = CheckpointComponentId::new("pci.intx-router0").unwrap();
    let router = legacy_router_with_intd(60);
    let system = base_system()
        .with_pci_legacy_interrupt_router_checkpoint_port(
            PciLegacyInterruptRouterCheckpointPort::new(component.clone(), Arc::clone(&router)),
        )
        .unwrap()
        .with_host_controller(host_config(source), StatsRegistry::new())
        .unwrap();
    let host = system.host_controller().unwrap();

    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .pci_legacy_interrupt_router_checkpoint_bank()
            .unwrap()
            .components(),
        vec![component.clone()]
    );

    let checkpoint = HostActionRecord::new(
        21,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(411),
        source,
        HostAction::Checkpoint {
            label: "topology-pci-intx".to_string(),
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
    assert!(manifest.states().iter().any(|state| {
        state.component() == &component
            && state
                .chunks()
                .iter()
                .any(|chunk| chunk.name() == "legacy-intx-router")
    }));

    replace_router_intd(&router, 61);
    let restore = HostActionRecord::new(
        31,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(412),
        source,
        HostAction::RestoreCheckpoint { manifest },
    );
    host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(router_line(&router, PciInterruptPin::IntD), 60);
}

#[test]
fn topology_host_controller_attaches_late_pci_checkpoint_ports() {
    let source = GuestSourceId::new(123);
    let host_component = CheckpointComponentId::new("pci.host1").unwrap();
    let router_component = CheckpointComponentId::new("pci.intx-router1").unwrap();
    let pci_host = pci_host();
    let router = legacy_router();
    let system = base_system()
        .with_host_controller(host_config(source), StatsRegistry::new())
        .unwrap()
        .with_pci_host_checkpoint_port(PciHostCheckpointPort::new(host_component.clone(), pci_host))
        .unwrap()
        .with_pci_legacy_interrupt_router_checkpoint_port(
            PciLegacyInterruptRouterCheckpointPort::new(router_component.clone(), router),
        )
        .unwrap();
    let host = system.host_controller().unwrap();

    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .pci_host_checkpoint_bank()
            .unwrap()
            .components(),
        vec![host_component]
    );
    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .pci_legacy_interrupt_router_checkpoint_bank()
            .unwrap()
            .components(),
        vec![router_component]
    );
}

#[test]
fn topology_rejects_duplicate_pci_host_checkpoint_component() {
    let component = CheckpointComponentId::new("pci.host2").unwrap();
    let error = match base_system()
        .with_pci_host_checkpoint_port(PciHostCheckpointPort::new(component.clone(), pci_host()))
        .unwrap()
        .with_pci_host_checkpoint_port(PciHostCheckpointPort::new(component.clone(), pci_host()))
    {
        Ok(_) => panic!("duplicate PCI host checkpoint component succeeded"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        RiscvTopologySystemError::System(SystemError::Checkpoint(
            CheckpointError::DuplicateComponent { component: duplicate }
        )) if duplicate == component
    ));
}

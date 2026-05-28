use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarSpec, PciBridgeBusRange, PciBridgeConfig, PciClassCode,
    PciConfigAperture, PciDeviceIdentity, PciEndpointConfig, PciError, PciFunctionAddress,
    PciHostAddressBases, PciHostBridge, PciHostBridgeTopologySnapshot,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, PciHostCheckpointBank,
    PciHostCheckpointError, PciHostCheckpointPort, PciHostCheckpointRecord, SystemActionExecutor,
    SystemActionOutcome, SystemError,
};

fn bridge(function: PciFunctionAddress, secondary: u8) -> PciBridgeConfig {
    PciBridgeConfig::new(
        function,
        PciDeviceIdentity::new(0x1011, 0x0026),
        PciClassCode::new(0x06, 0x04, 0x00, 0x00),
        PciBridgeBusRange::new(function.bus(), secondary, secondary).unwrap(),
    )
}

fn endpoint(function: PciFunctionAddress) -> PciEndpointConfig {
    let mut endpoint = PciEndpointConfig::new(
        function,
        PciDeviceIdentity::new(0x1af4, 0x1001),
        PciClassCode::new(0x01, 0x00, 0x00, 0x00),
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

fn host_with_endpoint() -> PciHostBridge {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 2).unwrap();
    let bases = PciHostAddressBases::new(
        Address::new(0x1000_0000),
        Address::new(0x8000_0000),
        Address::new(0xa000_0000),
    );
    let bridge_function = PciFunctionAddress::new(0, 1, 0).unwrap();
    let endpoint_function = PciFunctionAddress::new(1, 2, 0).unwrap();
    let mut host = PciHostBridge::with_address_bases(aperture, bases);
    host.register_bridge(bridge(bridge_function, 1)).unwrap();
    host.register_endpoint(endpoint(endpoint_function)).unwrap();
    host
}

fn host_missing_endpoint() -> PciHostBridge {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 2).unwrap();
    let bases = PciHostAddressBases::new(
        Address::new(0x1000_0000),
        Address::new(0x8000_0000),
        Address::new(0xa000_0000),
    );
    let bridge_function = PciFunctionAddress::new(0, 1, 0).unwrap();
    let mut host = PciHostBridge::with_address_bases(aperture, bases);
    host.register_bridge(bridge(bridge_function, 1)).unwrap();
    host
}

#[test]
fn pci_host_checkpoint_captures_and_validates_topology_payload() {
    let live = Arc::new(Mutex::new(host_with_endpoint()));
    let topology = live.lock().unwrap().topology_snapshot();
    let component = CheckpointComponentId::new("pci.host0").unwrap();
    let port = PciHostCheckpointPort::new(component.clone(), Arc::clone(&live));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        PciHostCheckpointRecord::new(component.clone(), topology.clone())
    );
    assert_eq!(
        PciHostBridgeTopologySnapshot::from_bytes(
            registry.chunk(&component, "host-topology").unwrap()
        )
        .unwrap(),
        topology
    );
    assert_eq!(port.restore_from(&registry).unwrap(), captured);
    assert_eq!(live.lock().unwrap().topology_snapshot(), topology);
}

#[test]
fn system_action_executor_checkpoints_and_prevalidates_pci_host_topology() {
    let live = Arc::new(Mutex::new(host_with_endpoint()));
    let component = CheckpointComponentId::new("pci.host0").unwrap();
    let expected_topology = live.lock().unwrap().topology_snapshot();
    let bank = PciHostCheckpointBank::new([PciHostCheckpointPort::new(
        component.clone(),
        Arc::clone(&live),
    )])
    .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor.attach_pci_host_checkpoint_bank(bank).unwrap();

    let checkpoint = HostActionRecord::new(
        18,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(1),
        GuestSourceId::new(9),
        HostAction::Checkpoint {
            label: "pci-ready".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    assert!(manifest.states().iter().any(|state| {
        state.component() == &component
            && state
                .chunks()
                .iter()
                .any(|chunk| chunk.name() == "host-topology")
    }));

    *live.lock().unwrap() = host_missing_endpoint();
    let missing_endpoint_topology = live.lock().unwrap().topology_snapshot();

    let restore = HostActionRecord::new(
        24,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(2),
        GuestSourceId::new(9),
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );
    assert_eq!(
        executor.apply(&restore).unwrap_err(),
        SystemError::PciHostCheckpoint(PciHostCheckpointError::Pci {
            component,
            error: PciError::SnapshotHostBridgeMismatch,
        })
    );
    assert_eq!(
        PciHostBridgeTopologySnapshot::from_bytes(
            executor
                .checkpoints()
                .chunk(
                    &CheckpointComponentId::new("pci.host0").unwrap(),
                    "host-topology"
                )
                .unwrap()
        )
        .unwrap(),
        expected_topology
    );
    assert_eq!(
        live.lock().unwrap().topology_snapshot(),
        missing_endpoint_topology
    );
}

use std::sync::{Arc, Mutex};

use rem6_checkpoint::{
    CheckpointChunk, CheckpointComponentId, CheckpointManifest, CheckpointRegistry, CheckpointState,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarSpec, PciBridgeBusRange, PciBridgeConfig, PciClassCode,
    PciConfigAperture, PciConfigOffset, PciDeviceIdentity, PciEndpointConfig, PciError,
    PciFunctionAddress, PciHostAddressBases, PciHostBridge, PciHostBridgeTopologySnapshot,
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

fn host_with_other_endpoint() -> PciHostBridge {
    let aperture = PciConfigAperture::ecam(Address::new(0x4000_0000), 3).unwrap();
    let bases = PciHostAddressBases::new(
        Address::new(0x2000_0000),
        Address::new(0x9000_0000),
        Address::new(0xb000_0000),
    );
    let bridge_function = PciFunctionAddress::new(0, 3, 0).unwrap();
    let endpoint_function = PciFunctionAddress::new(2, 4, 0).unwrap();
    let mut host = PciHostBridge::with_address_bases(aperture, bases);
    host.register_bridge(bridge(bridge_function, 2)).unwrap();
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

fn corrupt_manifest_chunk(
    manifest: &CheckpointManifest,
    component: &CheckpointComponentId,
    chunk_name: &str,
) -> CheckpointManifest {
    CheckpointManifest::new(
        manifest.label().to_string(),
        manifest.tick(),
        manifest
            .states()
            .iter()
            .map(|state| {
                let chunks = state
                    .chunks()
                    .iter()
                    .map(|chunk| {
                        let mut payload = chunk.payload().to_vec();
                        if state.component() == component && chunk.name() == chunk_name {
                            let last = payload.last_mut().expect("checkpoint chunk is non-empty");
                            *last ^= 0x01;
                        }
                        CheckpointChunk::new(chunk.name().to_string(), payload)
                    })
                    .collect();
                CheckpointState::new(state.component().clone(), chunks)
            })
            .collect(),
    )
}

fn malformed_bar_payload_with_slot_count(slot_count: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"R6PHBAR1");
    payload.extend_from_slice(&1u16.to_le_bytes());
    payload.extend_from_slice(&1u32.to_le_bytes());
    payload.extend_from_slice(&[0, 1, 0]);
    payload.extend_from_slice(&slot_count.to_le_bytes());
    payload
}

#[test]
fn pci_host_checkpoint_captures_and_validates_topology_payload() {
    let live = Arc::new(Mutex::new(host_with_endpoint()));
    let endpoint_function = PciFunctionAddress::new(1, 2, 0).unwrap();
    live.lock()
        .unwrap()
        .endpoint_mut(endpoint_function)
        .unwrap()
        .write_config(PciConfigOffset::new(0x04).unwrap(), &[0x02, 0x00])
        .unwrap();
    let snapshot = live.lock().unwrap().snapshot();
    let topology = snapshot.topology_snapshot();
    let bridge_config_space = snapshot.bridge_config_space_payloads();
    let endpoint_config_space = snapshot.endpoint_config_space_payloads();
    let bridge_bars = snapshot.bridge_bar_payloads();
    let endpoint_bars = snapshot.endpoint_bar_payloads();
    let component = CheckpointComponentId::new("pci.host0").unwrap();
    let port = PciHostCheckpointPort::new(component.clone(), Arc::clone(&live));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        PciHostCheckpointRecord::new(
            component.clone(),
            topology.clone(),
            bridge_config_space.clone(),
            endpoint_config_space.clone(),
            bridge_bars.clone(),
            endpoint_bars.clone()
        )
    );
    assert_eq!(
        captured.bridge_config_space_payloads(),
        &bridge_config_space
    );
    assert_eq!(
        captured.endpoint_config_space_payloads(),
        &endpoint_config_space
    );
    assert_eq!(captured.bridge_bar_payloads(), &bridge_bars);
    assert_eq!(captured.endpoint_bar_payloads(), &endpoint_bars);
    assert_eq!(
        PciHostBridgeTopologySnapshot::from_bytes(
            registry.chunk(&component, "host-topology").unwrap()
        )
        .unwrap(),
        topology
    );
    assert!(registry
        .chunk(&component, "host-bridge-config-space")
        .is_some());
    assert!(registry
        .chunk(&component, "host-endpoint-config-space")
        .is_some());
    assert!(registry.chunk(&component, "host-bridge-bars").is_some());
    assert!(registry.chunk(&component, "host-endpoint-bars").is_some());
    assert_eq!(port.restore_from(&registry).unwrap(), captured);
    assert_eq!(live.lock().unwrap().topology_snapshot(), topology);
}

#[test]
fn pci_host_checkpoint_accepts_legacy_topology_only_payloads() {
    let live = Arc::new(Mutex::new(host_with_endpoint()));
    let topology = live.lock().unwrap().topology_snapshot();
    let component = CheckpointComponentId::new("pci.host0").unwrap();
    let port = PciHostCheckpointPort::new(component.clone(), Arc::clone(&live));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    registry
        .write_chunk(&component, "host-topology", topology.to_bytes())
        .unwrap();

    let restored = port.restore_from(&registry).unwrap();

    assert!(!restored.has_config_space_payloads());
    assert_eq!(restored.topology(), &topology);
    assert!(restored.bridge_config_space_payloads().is_empty());
    assert!(restored.endpoint_config_space_payloads().is_empty());
    assert!(!restored.has_bar_payloads());
    assert!(restored.bridge_bar_payloads().is_empty());
    assert!(restored.endpoint_bar_payloads().is_empty());
}

#[test]
fn pci_host_checkpoint_accepts_config_only_payloads() {
    let live = Arc::new(Mutex::new(host_with_endpoint()));
    let component = CheckpointComponentId::new("pci.host0").unwrap();
    let port = PciHostCheckpointPort::new(component.clone(), Arc::clone(&live));
    let mut captured = CheckpointRegistry::new();

    port.register(&mut captured).unwrap();
    let record = port.capture_into(&mut captured).unwrap();

    let mut config_only = CheckpointRegistry::new();
    port.register(&mut config_only).unwrap();
    for chunk_name in [
        "host-topology",
        "host-bridge-config-space",
        "host-endpoint-config-space",
    ] {
        config_only
            .write_chunk(
                &component,
                chunk_name,
                captured.chunk(&component, chunk_name).unwrap().to_vec(),
            )
            .unwrap();
    }

    let restored = port.restore_from(&config_only).unwrap();

    assert!(restored.has_config_space_payloads());
    assert_eq!(
        restored.bridge_config_space_payloads(),
        record.bridge_config_space_payloads()
    );
    assert_eq!(
        restored.endpoint_config_space_payloads(),
        record.endpoint_config_space_payloads()
    );
    assert!(!restored.has_bar_payloads());
    assert!(restored.bridge_bar_payloads().is_empty());
    assert!(restored.endpoint_bar_payloads().is_empty());
}

#[test]
fn pci_host_checkpoint_rejects_partially_missing_config_space_payloads() {
    let live = Arc::new(Mutex::new(host_with_endpoint()));
    let component = CheckpointComponentId::new("pci.host0").unwrap();
    let port = PciHostCheckpointPort::new(component.clone(), Arc::clone(&live));
    let mut captured = CheckpointRegistry::new();

    port.register(&mut captured).unwrap();
    port.capture_into(&mut captured).unwrap();

    let mut partial = CheckpointRegistry::new();
    port.register(&mut partial).unwrap();
    partial
        .write_chunk(
            &component,
            "host-topology",
            captured
                .chunk(&component, "host-topology")
                .unwrap()
                .to_vec(),
        )
        .unwrap();
    partial
        .write_chunk(
            &component,
            "host-endpoint-config-space",
            captured
                .chunk(&component, "host-endpoint-config-space")
                .unwrap()
                .to_vec(),
        )
        .unwrap();

    assert_eq!(
        port.restore_from(&partial).unwrap_err(),
        PciHostCheckpointError::MissingChunk {
            component,
            name: "host-bridge-config-space".to_string(),
        }
    );
}

#[test]
fn pci_host_checkpoint_rejects_partially_missing_bar_payloads() {
    let live = Arc::new(Mutex::new(host_with_endpoint()));
    let component = CheckpointComponentId::new("pci.host0").unwrap();
    let port = PciHostCheckpointPort::new(component.clone(), Arc::clone(&live));
    let mut captured = CheckpointRegistry::new();

    port.register(&mut captured).unwrap();
    port.capture_into(&mut captured).unwrap();

    let mut partial = CheckpointRegistry::new();
    port.register(&mut partial).unwrap();
    for chunk_name in [
        "host-topology",
        "host-bridge-config-space",
        "host-endpoint-config-space",
        "host-endpoint-bars",
    ] {
        partial
            .write_chunk(
                &component,
                chunk_name,
                captured.chunk(&component, chunk_name).unwrap().to_vec(),
            )
            .unwrap();
    }

    assert_eq!(
        port.restore_from(&partial).unwrap_err(),
        PciHostCheckpointError::MissingChunk {
            component,
            name: "host-bridge-bars".to_string(),
        }
    );
}

#[test]
fn pci_host_checkpoint_rejects_bar_payloads_without_config_space_payloads() {
    let live = Arc::new(Mutex::new(host_with_endpoint()));
    let component = CheckpointComponentId::new("pci.host0").unwrap();
    let port = PciHostCheckpointPort::new(component.clone(), Arc::clone(&live));
    let mut captured = CheckpointRegistry::new();

    port.register(&mut captured).unwrap();
    port.capture_into(&mut captured).unwrap();

    let mut bar_only = CheckpointRegistry::new();
    port.register(&mut bar_only).unwrap();
    for chunk_name in ["host-topology", "host-bridge-bars", "host-endpoint-bars"] {
        bar_only
            .write_chunk(
                &component,
                chunk_name,
                captured.chunk(&component, chunk_name).unwrap().to_vec(),
            )
            .unwrap();
    }

    assert_eq!(
        port.restore_from(&bar_only).unwrap_err(),
        PciHostCheckpointError::MissingChunk {
            component,
            name: "host-bridge-config-space".to_string(),
        }
    );
}

#[test]
fn pci_host_checkpoint_rejects_bar_payloads_with_impossible_slot_count() {
    let live = Arc::new(Mutex::new(host_with_endpoint()));
    let component = CheckpointComponentId::new("pci.host0").unwrap();
    let port = PciHostCheckpointPort::new(component.clone(), Arc::clone(&live));
    let mut captured = CheckpointRegistry::new();

    port.register(&mut captured).unwrap();
    port.capture_into(&mut captured).unwrap();

    let mut malformed = CheckpointRegistry::new();
    port.register(&mut malformed).unwrap();
    for chunk_name in [
        "host-topology",
        "host-bridge-config-space",
        "host-endpoint-config-space",
        "host-bridge-bars",
    ] {
        malformed
            .write_chunk(
                &component,
                chunk_name,
                captured.chunk(&component, chunk_name).unwrap().to_vec(),
            )
            .unwrap();
    }
    malformed
        .write_chunk(
            &component,
            "host-endpoint-bars",
            malformed_bar_payload_with_slot_count(u32::MAX),
        )
        .unwrap();

    assert_eq!(
        port.restore_from(&malformed).unwrap_err(),
        PciHostCheckpointError::InvalidChunk {
            component,
            reason: PciError::InvalidBarSnapshot.to_string(),
        }
    );
}

#[test]
fn pci_host_checkpoint_bank_decodes_records_for_manifest_audit() {
    let first = Arc::new(Mutex::new(host_with_endpoint()));
    let second = Arc::new(Mutex::new(host_with_other_endpoint()));
    let first_component = CheckpointComponentId::new("pci.host0").unwrap();
    let second_component = CheckpointComponentId::new("pci.host1").unwrap();
    let first_snapshot = first.lock().unwrap().snapshot();
    let second_snapshot = second.lock().unwrap().snapshot();
    let bank = PciHostCheckpointBank::new([
        PciHostCheckpointPort::new(second_component.clone(), Arc::clone(&second)),
        PciHostCheckpointPort::new(first_component.clone(), Arc::clone(&first)),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();

    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();

    assert_eq!(
        bank.decode_all_from(&registry).unwrap(),
        vec![
            PciHostCheckpointRecord::new(
                first_component,
                first_snapshot.topology_snapshot(),
                first_snapshot.bridge_config_space_payloads(),
                first_snapshot.endpoint_config_space_payloads(),
                first_snapshot.bridge_bar_payloads(),
                first_snapshot.endpoint_bar_payloads()
            ),
            PciHostCheckpointRecord::new(
                second_component,
                second_snapshot.topology_snapshot(),
                second_snapshot.bridge_config_space_payloads(),
                second_snapshot.endpoint_config_space_payloads(),
                second_snapshot.bridge_bar_payloads(),
                second_snapshot.endpoint_bar_payloads()
            ),
        ]
    );
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
            && [
                "host-bridge-bars",
                "host-bridge-config-space",
                "host-endpoint-bars",
                "host-endpoint-config-space",
                "host-topology",
            ]
            .into_iter()
            .all(|name| state.chunks().iter().any(|chunk| chunk.name() == name))
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

#[test]
fn system_action_executor_constructs_with_pci_host_checkpoint_bank() {
    let live = Arc::new(Mutex::new(host_with_endpoint()));
    let component = CheckpointComponentId::new("pci.host0").unwrap();
    let bank = PciHostCheckpointBank::new([PciHostCheckpointPort::new(
        component.clone(),
        Arc::clone(&live),
    )])
    .unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    bank.register_all(&mut checkpoints).unwrap();
    let mut executor = SystemActionExecutor::with_pci_host_checkpoint_bank(
        StatsRegistry::new(),
        checkpoints,
        bank,
    );

    let checkpoint = HostActionRecord::new(
        31,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(5),
        GuestSourceId::new(9),
        HostAction::Checkpoint {
            label: "pci-constructor".to_string(),
        },
    );
    let SystemActionOutcome::Checkpoint { manifest, .. } = executor.apply(&checkpoint).unwrap()
    else {
        panic!("checkpoint outcome expected");
    };

    assert!(manifest.states().iter().any(|state| {
        state.component() == &component
            && [
                "host-bridge-bars",
                "host-bridge-config-space",
                "host-endpoint-bars",
                "host-endpoint-config-space",
                "host-topology",
            ]
            .into_iter()
            .all(|name| state.chunks().iter().any(|chunk| chunk.name() == name))
    }));
    assert!(executor.pci_host_checkpoint_bank().is_some());
}

#[test]
fn system_action_executor_rejects_pci_host_bar_payload_malformed() {
    let live = Arc::new(Mutex::new(host_with_endpoint()));
    let component = CheckpointComponentId::new("pci.host0").unwrap();
    let bank = PciHostCheckpointBank::new([PciHostCheckpointPort::new(
        component.clone(),
        Arc::clone(&live),
    )])
    .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor.attach_pci_host_checkpoint_bank(bank).unwrap();

    let checkpoint = HostActionRecord::new(
        53,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(10),
        GuestSourceId::new(9),
        HostAction::Checkpoint {
            label: "pci-bars".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let original_payload = executor
        .checkpoints()
        .chunk(&component, "host-endpoint-bars")
        .unwrap()
        .to_vec();
    let corrupted = corrupt_manifest_chunk(&manifest, &component, "host-endpoint-bars");

    let restore = HostActionRecord::new(
        59,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(11),
        GuestSourceId::new(9),
        HostAction::RestoreCheckpoint {
            manifest: corrupted,
        },
    );

    assert_eq!(
        executor.apply(&restore).unwrap_err(),
        SystemError::PciHostCheckpoint(PciHostCheckpointError::InvalidChunk {
            component: component.clone(),
            reason: PciError::InvalidBarSnapshot.to_string(),
        })
    );
    assert_eq!(
        executor
            .checkpoints()
            .chunk(&component, "host-endpoint-bars")
            .unwrap(),
        original_payload
    );
}

#[test]
fn system_action_executor_rejects_pci_host_config_space_payload_mismatch() {
    let live = Arc::new(Mutex::new(host_with_endpoint()));
    let component = CheckpointComponentId::new("pci.host0").unwrap();
    let bank = PciHostCheckpointBank::new([PciHostCheckpointPort::new(
        component.clone(),
        Arc::clone(&live),
    )])
    .unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor.attach_pci_host_checkpoint_bank(bank).unwrap();

    let checkpoint = HostActionRecord::new(
        42,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(7),
        GuestSourceId::new(9),
        HostAction::Checkpoint {
            label: "pci-config".to_string(),
        },
    );
    let manifest = match executor.apply(&checkpoint).unwrap() {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let original_payload = executor
        .checkpoints()
        .chunk(&component, "host-endpoint-config-space")
        .unwrap()
        .to_vec();
    let corrupted = corrupt_manifest_chunk(&manifest, &component, "host-endpoint-config-space");

    let restore = HostActionRecord::new(
        48,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(8),
        GuestSourceId::new(9),
        HostAction::RestoreCheckpoint {
            manifest: corrupted,
        },
    );

    assert_eq!(
        executor.apply(&restore).unwrap_err(),
        SystemError::PciHostCheckpoint(PciHostCheckpointError::Pci {
            component: component.clone(),
            error: PciError::SnapshotConfigSpaceMismatch,
        })
    );
    assert_eq!(
        executor
            .checkpoints()
            .chunk(&component, "host-endpoint-config-space")
            .unwrap(),
        original_payload
    );
}

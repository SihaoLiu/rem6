use std::collections::BTreeMap;

use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarSpec, PciBridgeBusRange, PciBridgeConfig, PciClassCode,
    PciConfigAperture, PciConfigOffset, PciDeviceIdentity, PciEndpointConfig, PciError,
    PciFunctionAddress, PciHostAddressBases, PciHostBridge, PciHostBridgeTopologySnapshot,
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

#[test]
fn pci_host_bridge_topology_snapshot_codec_round_trips_sorted_functions() {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 4).unwrap();
    let bases = PciHostAddressBases::new(
        Address::new(0x1000_0000),
        Address::new(0x8000_0000),
        Address::new(0xa000_0000),
    );
    let bridge0 = PciFunctionAddress::new(0, 3, 0).unwrap();
    let bridge1 = PciFunctionAddress::new(0, 1, 0).unwrap();
    let endpoint0 = PciFunctionAddress::new(1, 2, 0).unwrap();
    let endpoint1 = PciFunctionAddress::new(3, 4, 0).unwrap();
    let mut host = PciHostBridge::with_address_bases(aperture, bases);

    host.register_bridge(bridge(bridge0, 3)).unwrap();
    host.register_endpoint(endpoint(endpoint1)).unwrap();
    host.register_bridge(bridge(bridge1, 1)).unwrap();
    host.register_endpoint(endpoint(endpoint0)).unwrap();

    let topology = host.topology_snapshot();
    let decoded = PciHostBridgeTopologySnapshot::from_bytes(&topology.to_bytes()).unwrap();

    assert_eq!(decoded, topology);
    assert_eq!(decoded.aperture(), aperture);
    assert_eq!(decoded.address_bases(), bases);
    assert_eq!(decoded.bridge_functions(), &[bridge1, bridge0]);
    assert_eq!(decoded.endpoint_functions(), &[endpoint0, endpoint1]);
}

#[test]
fn pci_host_bridge_topology_snapshot_validates_live_host_shape() {
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
    let topology =
        PciHostBridgeTopologySnapshot::from_bytes(&host.topology_snapshot().to_bytes()).unwrap();

    assert_eq!(host.validate_topology_snapshot(&topology), Ok(()));

    let mut missing_endpoint_host = PciHostBridge::with_address_bases(aperture, bases);
    missing_endpoint_host
        .register_bridge(bridge(bridge_function, 1))
        .unwrap();
    assert_eq!(
        missing_endpoint_host.validate_topology_snapshot(&topology),
        Err(PciError::SnapshotHostBridgeMismatch)
    );
}

#[test]
fn pci_host_bridge_topology_snapshot_rejects_functions_outside_aperture() {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 2).unwrap();
    let bases = PciHostAddressBases::zero();
    let outside = PciFunctionAddress::new(3, 0, 0).unwrap();

    assert_eq!(
        PciHostBridgeTopologySnapshot::new(aperture, bases, Vec::new(), vec![outside]),
        Err(PciError::InvalidHostBridgeTopologySnapshot)
    );
    assert_eq!(
        PciHostBridgeTopologySnapshot::new(aperture, bases, vec![outside], Vec::new()),
        Err(PciError::InvalidHostBridgeTopologySnapshot)
    );
}

#[test]
fn pci_host_bridge_snapshot_exposes_config_space_payloads_for_checkpoint_audit() {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 4).unwrap();
    let bases = PciHostAddressBases::new(
        Address::new(0x1000_0000),
        Address::new(0x8000_0000),
        Address::new(0xa000_0000),
    );
    let bridge0 = PciFunctionAddress::new(0, 3, 0).unwrap();
    let bridge1 = PciFunctionAddress::new(0, 1, 0).unwrap();
    let endpoint0 = PciFunctionAddress::new(1, 2, 0).unwrap();
    let endpoint1 = PciFunctionAddress::new(3, 4, 0).unwrap();
    let mut host = PciHostBridge::with_address_bases(aperture, bases);
    let mut bridge0_config = bridge(bridge0, 3);
    bridge0_config
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

    host.register_bridge(bridge0_config).unwrap();
    host.register_endpoint(endpoint(endpoint1)).unwrap();
    host.register_bridge(bridge(bridge1, 1)).unwrap();
    host.register_endpoint(endpoint(endpoint0)).unwrap();

    let bridge0_bar_addr = aperture
        .config_address(bridge0, PciConfigOffset::new(0x10).unwrap())
        .unwrap();
    host.write_config_address(bridge0_bar_addr, &0x0040_1234_u32.to_le_bytes())
        .unwrap();
    let endpoint1_command = aperture
        .config_address(endpoint1, PciConfigOffset::new(0x04).unwrap())
        .unwrap();
    host.write_config_address(endpoint1_command, &0x0002_u16.to_le_bytes())
        .unwrap();
    let endpoint1_bar = aperture
        .config_address(endpoint1, PciConfigOffset::new(0x10).unwrap())
        .unwrap();
    host.write_config_address(endpoint1_bar, &0x9000_1234_u32.to_le_bytes())
        .unwrap();
    let snapshot = host.snapshot();

    let bridge_payloads = snapshot.bridge_config_space_payloads();
    let endpoint_payloads = snapshot.endpoint_config_space_payloads();
    let bridge_bar_payloads = snapshot.bridge_bar_payloads();
    let endpoint_bar_payloads = snapshot.endpoint_bar_payloads();

    assert_eq!(
        bridge_payloads.keys().copied().collect::<Vec<_>>(),
        vec![bridge1, bridge0]
    );
    assert_eq!(
        endpoint_payloads.keys().copied().collect::<Vec<_>>(),
        vec![endpoint0, endpoint1]
    );
    assert_eq!(
        bridge_bar_payloads.keys().copied().collect::<Vec<_>>(),
        vec![bridge1, bridge0]
    );
    assert_eq!(
        endpoint_bar_payloads.keys().copied().collect::<Vec<_>>(),
        vec![endpoint0, endpoint1]
    );
    assert_eq!(
        bridge_bar_payloads.get(&bridge0).unwrap(),
        &snapshot.bridges().get(&bridge0).unwrap().bar_payloads()
    );
    assert_eq!(
        endpoint_bar_payloads.get(&endpoint1).unwrap(),
        &snapshot.endpoints().get(&endpoint1).unwrap().bar_payloads()
    );
    assert_eq!(
        &bridge_payloads.get(&bridge0).unwrap()[0x10..0x14],
        &0x0040_1000_u32.to_le_bytes()
    );
    assert_eq!(
        &endpoint_payloads.get(&endpoint1).unwrap()[0x04..0x06],
        &0x0002_u16.to_le_bytes()
    );
    assert_eq!(
        &endpoint_payloads.get(&endpoint1).unwrap()[0x10..0x14],
        &0x9000_1000_u32.to_le_bytes()
    );
    assert_eq!(
        snapshot.validate_bridge_config_space_payloads(&bridge_payloads),
        Ok(())
    );
    assert_eq!(
        snapshot.validate_endpoint_config_space_payloads(&endpoint_payloads),
        Ok(())
    );
    assert_eq!(
        snapshot.validate_bridge_bar_payloads(&bridge_bar_payloads),
        Ok(())
    );
    assert_eq!(
        snapshot.validate_endpoint_bar_payloads(&endpoint_bar_payloads),
        Ok(())
    );

    let missing_bridge: BTreeMap<_, _> = bridge_payloads
        .iter()
        .filter(|(function, _)| **function != bridge1)
        .map(|(function, payload)| (*function, payload.clone()))
        .collect();
    assert_eq!(
        snapshot.validate_bridge_config_space_payloads(&missing_bridge),
        Err(PciError::SnapshotHostBridgeMismatch)
    );

    let missing_bridge_bar: BTreeMap<_, _> = bridge_bar_payloads
        .iter()
        .filter(|(function, _)| **function != bridge1)
        .map(|(function, payload)| (*function, payload.clone()))
        .collect();
    assert_eq!(
        snapshot.validate_bridge_bar_payloads(&missing_bridge_bar),
        Err(PciError::SnapshotHostBridgeMismatch)
    );

    let mut truncated_endpoint = endpoint_payloads.clone();
    truncated_endpoint.get_mut(&endpoint1).unwrap().pop();
    assert_eq!(
        snapshot.validate_endpoint_config_space_payloads(&truncated_endpoint),
        Err(PciError::InvalidConfigSpaceSnapshot)
    );

    let mut truncated_endpoint_bar = endpoint_bar_payloads.clone();
    truncated_endpoint_bar.get_mut(&endpoint1).unwrap()[0]
        .as_mut()
        .unwrap()
        .pop();
    assert_eq!(
        snapshot.validate_endpoint_bar_payloads(&truncated_endpoint_bar),
        Err(PciError::InvalidBarSnapshot)
    );

    let mut mismatched_endpoint = endpoint_payloads;
    mismatched_endpoint.get_mut(&endpoint1).unwrap()[0x04] = 0;
    assert_eq!(
        snapshot.validate_endpoint_config_space_payloads(&mismatched_endpoint),
        Err(PciError::SnapshotConfigSpaceMismatch)
    );

    let mut mismatched_bridge_bar = bridge_bar_payloads;
    mismatched_bridge_bar.get_mut(&bridge0).unwrap()[0] = None;
    assert_eq!(
        snapshot.validate_bridge_bar_payloads(&mismatched_bridge_bar),
        Err(PciError::SnapshotBarMismatch {
            index: PciBarIndex::new(0).unwrap(),
        })
    );
}

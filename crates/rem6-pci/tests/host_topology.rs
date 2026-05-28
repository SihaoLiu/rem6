use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarSpec, PciBridgeBusRange, PciBridgeConfig, PciClassCode,
    PciConfigAperture, PciDeviceIdentity, PciEndpointConfig, PciError, PciFunctionAddress,
    PciHostAddressBases, PciHostBridge, PciHostBridgeTopologySnapshot,
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

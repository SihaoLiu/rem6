use std::collections::BTreeMap;

use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarSpec, PciBridgeBusRange, PciBridgeConfig, PciClassCode,
    PciConfigAperture, PciConfigOffset, PciDeviceIdentity, PciEndpointConfig, PciError,
    PciExpressCapabilitySpec, PciExpressDeviceCapabilitySpec, PciExpressLinkCapabilitySpec,
    PciFunctionAddress, PciHostAddressBases, PciHostBridge, PciHostBridgeTopologySnapshot,
    PciMsiCapabilitySpec, PciMsixCapabilitySpec, PciPowerManagementCapabilitySpec,
    PciRawCapabilitySpec,
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

fn endpoint_with_capabilities(function: PciFunctionAddress) -> PciEndpointConfig {
    let mut endpoint = endpoint(function);
    endpoint.install_pm_capability(pm(0x44)).unwrap();
    endpoint.install_msi_capability(msi(0x50)).unwrap();
    endpoint
        .install_raw_capability(raw_capability(0x70))
        .unwrap();
    endpoint.install_msix_capability(msix(0x90)).unwrap();
    endpoint.install_pcie_capability(pcie(0xa0)).unwrap();
    endpoint
}

fn pm(offset: u16) -> PciPowerManagementCapabilitySpec {
    PciPowerManagementCapabilitySpec::new(PciConfigOffset::new(offset).unwrap(), 0x0003, 0x0001)
        .unwrap()
}

fn msi(offset: u16) -> PciMsiCapabilitySpec {
    PciMsiCapabilitySpec::new(PciConfigOffset::new(offset).unwrap(), 4, true, true).unwrap()
}

fn msix(offset: u16) -> PciMsixCapabilitySpec {
    PciMsixCapabilitySpec::new(
        PciConfigOffset::new(offset).unwrap(),
        4,
        PciBarIndex::new(2).unwrap(),
        Address::new(0x100),
        PciBarIndex::new(2).unwrap(),
        Address::new(0x180),
    )
    .unwrap()
}

fn pcie(offset: u16) -> PciExpressCapabilitySpec {
    PciExpressCapabilitySpec::new(
        PciConfigOffset::new(offset).unwrap(),
        0x0002,
        PciExpressDeviceCapabilitySpec::new(0x0000_1234, 0x0011, 0x0022),
        PciExpressLinkCapabilitySpec::new(0x0100_0001, 0x0003, 0x2001),
    )
    .unwrap()
}

fn raw_capability(offset: u16) -> PciRawCapabilitySpec {
    PciRawCapabilitySpec::new(
        PciConfigOffset::new(offset).unwrap(),
        [0x09, 0xff, 0x10, 0x08, 0xaa, 0xbb, 0xcc, 0xdd],
    )
    .unwrap()
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

#[test]
fn pci_host_bridge_snapshot_exposes_endpoint_capability_payloads_for_checkpoint_audit() {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 4).unwrap();
    let bases = PciHostAddressBases::new(
        Address::new(0x1000_0000),
        Address::new(0x8000_0000),
        Address::new(0xa000_0000),
    );
    let endpoint0 = PciFunctionAddress::new(1, 2, 0).unwrap();
    let endpoint1 = PciFunctionAddress::new(3, 4, 0).unwrap();
    let mut host = PciHostBridge::with_address_bases(aperture, bases);

    host.register_endpoint(endpoint(endpoint1)).unwrap();
    host.register_endpoint(endpoint_with_capabilities(endpoint0))
        .unwrap();

    let snapshot = host.snapshot();
    let raw_payloads = snapshot.endpoint_raw_capability_payloads();
    let pm_payloads = snapshot.endpoint_power_management_payloads();
    let pcie_payloads = snapshot.endpoint_pcie_payloads();
    let msi_payloads = snapshot.endpoint_msi_payloads();
    let msix_payloads = snapshot.endpoint_msix_payloads();

    assert_eq!(
        raw_payloads.keys().copied().collect::<Vec<_>>(),
        vec![endpoint0, endpoint1]
    );
    assert_eq!(
        pm_payloads.keys().copied().collect::<Vec<_>>(),
        vec![endpoint0, endpoint1]
    );
    assert_eq!(
        pcie_payloads.keys().copied().collect::<Vec<_>>(),
        vec![endpoint0, endpoint1]
    );
    assert_eq!(
        msi_payloads.keys().copied().collect::<Vec<_>>(),
        vec![endpoint0, endpoint1]
    );
    assert_eq!(
        msix_payloads.keys().copied().collect::<Vec<_>>(),
        vec![endpoint0, endpoint1]
    );

    let endpoint0_snapshot = snapshot.endpoints().get(&endpoint0).unwrap();
    assert_eq!(
        raw_payloads.get(&endpoint0).unwrap(),
        &endpoint0_snapshot.raw_capability_payloads()
    );
    assert_eq!(
        pm_payloads.get(&endpoint0).unwrap(),
        &endpoint0_snapshot.power_management_payload()
    );
    assert_eq!(
        pcie_payloads.get(&endpoint0).unwrap(),
        &endpoint0_snapshot.pcie_payload()
    );
    assert_eq!(
        msi_payloads.get(&endpoint0).unwrap(),
        &endpoint0_snapshot.msi_payload()
    );
    assert_eq!(
        msix_payloads.get(&endpoint0).unwrap(),
        &endpoint0_snapshot.msix_payload()
    );
    assert!(raw_payloads.get(&endpoint1).unwrap().is_empty());
    assert_eq!(pm_payloads.get(&endpoint1).unwrap(), &None);
    assert_eq!(pcie_payloads.get(&endpoint1).unwrap(), &None);
    assert_eq!(msi_payloads.get(&endpoint1).unwrap(), &None);
    assert_eq!(msix_payloads.get(&endpoint1).unwrap(), &None);

    assert_eq!(
        snapshot.validate_endpoint_raw_capability_payloads(&raw_payloads),
        Ok(())
    );
    assert_eq!(
        snapshot.validate_endpoint_power_management_payloads(&pm_payloads),
        Ok(())
    );
    assert_eq!(
        snapshot.validate_endpoint_pcie_payloads(&pcie_payloads),
        Ok(())
    );
    assert_eq!(
        snapshot.validate_endpoint_msi_payloads(&msi_payloads),
        Ok(())
    );
    assert_eq!(
        snapshot.validate_endpoint_msix_payloads(&msix_payloads),
        Ok(())
    );

    let missing_endpoint: BTreeMap<_, _> = raw_payloads
        .iter()
        .filter(|(function, _)| **function != endpoint1)
        .map(|(function, payload)| (*function, payload.clone()))
        .collect();
    assert_eq!(
        snapshot.validate_endpoint_raw_capability_payloads(&missing_endpoint),
        Err(PciError::SnapshotHostBridgeMismatch)
    );

    let mut malformed_raw = raw_payloads.clone();
    malformed_raw.get_mut(&endpoint0).unwrap()[0].pop();
    assert_eq!(
        snapshot.validate_endpoint_raw_capability_payloads(&malformed_raw),
        Err(PciError::InvalidRawCapabilitySnapshot)
    );

    let mut missing_pm = pm_payloads.clone();
    *missing_pm.get_mut(&endpoint0).unwrap() = None;
    assert_eq!(
        snapshot.validate_endpoint_power_management_payloads(&missing_pm),
        Err(PciError::SnapshotPowerManagementCapabilityMismatch)
    );

    let mut unexpected_msi = msi_payloads.clone();
    *unexpected_msi.get_mut(&endpoint1).unwrap() = msi_payloads.get(&endpoint0).unwrap().clone();
    assert_eq!(
        snapshot.validate_endpoint_msi_payloads(&unexpected_msi),
        Err(PciError::SnapshotMsiCapabilityMismatch)
    );

    let mut malformed_msix = msix_payloads.clone();
    malformed_msix
        .get_mut(&endpoint0)
        .unwrap()
        .as_mut()
        .unwrap()
        .pop();
    assert_eq!(
        snapshot.validate_endpoint_msix_payloads(&malformed_msix),
        Err(PciError::InvalidMsixCapabilitySnapshot)
    );
}

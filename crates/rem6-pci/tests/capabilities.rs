use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciClassCode, PciConfigOffset, PciDeviceIdentity, PciEndpointConfig, PciError,
    PciExpressCapabilitySpec, PciExpressDeviceCapabilitySpec, PciExpressLinkCapabilitySpec,
    PciFunctionAddress, PciMsiCapabilitySpec, PciMsixCapabilitySpec,
    PciPowerManagementCapabilitySpec,
};

fn storage_endpoint() -> PciEndpointConfig {
    PciEndpointConfig::new(
        PciFunctionAddress::new(0, 10, 0).unwrap(),
        PciDeviceIdentity::new(0x1af4, 0x1001),
        PciClassCode::new(0x01, 0x00, 0x00, 0x00),
    )
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

fn pm(offset: u16) -> PciPowerManagementCapabilitySpec {
    PciPowerManagementCapabilitySpec::new(PciConfigOffset::new(offset).unwrap(), 0x0003, 0x0000)
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

#[test]
fn pci_endpoint_links_multiple_capabilities_in_install_order() {
    let mut endpoint = storage_endpoint();

    endpoint.install_msi_capability(msi(0x50)).unwrap();
    endpoint.install_msix_capability(msix(0x70)).unwrap();

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x34).unwrap(),
            AccessSize::new(1).unwrap(),
        ),
        Ok(vec![0x50])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x50).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x05, 0x70, 0x84, 0x01])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x70).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x11, 0x00, 0x03, 0x00])
    );

    endpoint
        .write_config(
            PciConfigOffset::new(0x52).unwrap(),
            &0x0021_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0x72).unwrap(),
            &0x8000_u16.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x50).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x05, 0x70, 0xa5, 0x01])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x70).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x11, 0x00, 0x03, 0x80])
    );
}

#[test]
fn pci_endpoint_power_management_capability_links_writes_and_snapshots_pmcsr() {
    let mut endpoint = storage_endpoint();

    endpoint.install_pm_capability(pm(0x44)).unwrap();
    endpoint.install_msi_capability(msi(0x50)).unwrap();

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x34).unwrap(),
            AccessSize::new(1).unwrap(),
        ),
        Ok(vec![0x44])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x44).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x01, 0x50, 0x03, 0x00])
    );

    endpoint
        .write_config(
            PciConfigOffset::new(0x48).unwrap(),
            &0x8103_u16.to_le_bytes(),
        )
        .unwrap();
    let snapshot = endpoint.snapshot();
    endpoint
        .write_config(
            PciConfigOffset::new(0x48).unwrap(),
            &0x0000_u16.to_le_bytes(),
        )
        .unwrap();

    endpoint.restore(&snapshot).unwrap();

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x48).unwrap(),
            AccessSize::new(2).unwrap(),
        ),
        Ok(vec![0x03, 0x81])
    );
    assert_eq!(
        endpoint.write_config(
            PciConfigOffset::new(0x46).unwrap(),
            &0x0000_u16.to_le_bytes(),
        ),
        Err(PciError::ReadOnlyPowerManagementCapabilityWrite {
            offset: PciConfigOffset::new(0x46).unwrap(),
            size: AccessSize::new(2).unwrap(),
        })
    );
}

#[test]
fn pci_endpoint_pcie_capability_links_writes_and_snapshots_control_status() {
    let mut endpoint = storage_endpoint();

    endpoint.install_pm_capability(pm(0x44)).unwrap();
    endpoint.install_pcie_capability(pcie(0x80)).unwrap();
    endpoint.install_msi_capability(msi(0xc0)).unwrap();

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x44).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x01, 0x80, 0x03, 0x00])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x80).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x10, 0xc0, 0x02, 0x00])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x84).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x34, 0x12, 0x00, 0x00])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x88).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x11, 0x00, 0x22, 0x00])
    );

    endpoint
        .write_config(
            PciConfigOffset::new(0x88).unwrap(),
            &0x000f_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0x8a).unwrap(),
            &0x0040_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0x90).unwrap(),
            &0x0001_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0x92).unwrap(),
            &0x1001_u16.to_le_bytes(),
        )
        .unwrap();
    let snapshot = endpoint.snapshot();
    endpoint
        .write_config(
            PciConfigOffset::new(0x88).unwrap(),
            &0x0000_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint.restore(&snapshot).unwrap();

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x88).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x0f, 0x00, 0x40, 0x00])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x90).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x01, 0x00, 0x01, 0x10])
    );
    assert_eq!(
        endpoint.write_config(
            PciConfigOffset::new(0x84).unwrap(),
            &0x0000_0000_u32.to_le_bytes(),
        ),
        Err(PciError::ReadOnlyPciExpressCapabilityWrite {
            offset: PciConfigOffset::new(0x84).unwrap(),
            size: AccessSize::new(4).unwrap(),
        })
    );
}

#[test]
fn pci_endpoint_rejects_overlapping_capabilities_without_mutating_chain() {
    let mut endpoint = storage_endpoint();

    endpoint.install_msi_capability(msi(0x50)).unwrap();

    assert_eq!(
        endpoint.install_msix_capability(msix(0x60)),
        Err(PciError::OverlappingCapability {
            existing_offset: PciConfigOffset::new(0x50).unwrap(),
            existing_size: AccessSize::new(0x18).unwrap(),
            requested_offset: PciConfigOffset::new(0x60).unwrap(),
            requested_size: AccessSize::new(0x0c).unwrap(),
        })
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x34).unwrap(),
            AccessSize::new(1).unwrap(),
        ),
        Ok(vec![0x50])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x50).unwrap(),
            AccessSize::new(2).unwrap(),
        ),
        Ok(vec![0x05, 0x00])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x60).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x00, 0x00, 0x00, 0x00])
    );
}

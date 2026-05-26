use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciClassCode, PciConfigOffset, PciDeviceIdentity, PciEndpointConfig, PciError,
    PciFunctionAddress, PciMsiCapabilitySpec, PciMsixCapabilitySpec,
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

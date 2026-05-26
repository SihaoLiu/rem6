use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciClassCode, PciConfigOffset, PciDeviceIdentity, PciEndpointConfig, PciError,
    PciExpressCapability2Spec, PciExpressCapabilitySpec, PciExpressDeviceCapabilitySpec,
    PciExpressLinkCapabilitySpec, PciExpressRootCapabilitySpec, PciExpressSlotCapabilitySpec,
    PciFunctionAddress, PciMsiCapabilitySpec, PciMsixCapabilitySpec,
    PciPowerManagementCapabilitySpec, PciRawCapabilitySpec,
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

fn pcie_with_extended_registers(offset: u16) -> PciExpressCapabilitySpec {
    pcie(offset)
        .with_slot(PciExpressSlotCapabilitySpec::new(
            0x1122_3344,
            0x5566,
            0x7788,
        ))
        .with_root(PciExpressRootCapabilitySpec::new(
            0x99aa,
            0xbbcc,
            0xddee_ff00,
        ))
        .with_capability2(PciExpressCapability2Spec::new(
            PciExpressDeviceCapabilitySpec::new(0x0102_0304, 0x0506, 0x0708),
            PciExpressLinkCapabilitySpec::new(0x1112_1314, 0x1516, 0x1718),
            PciExpressSlotCapabilitySpec::new(0x2122_2324, 0x2526, 0x2728),
        ))
}

fn virtio_shared_memory_cap(offset: u16) -> PciRawCapabilitySpec {
    virtio_shared_memory_cap_with_next(offset, 0xff)
}

fn virtio_shared_memory_cap_with_next(offset: u16, next: u8) -> PciRawCapabilitySpec {
    PciRawCapabilitySpec::new(
        PciConfigOffset::new(offset).unwrap(),
        [
            0x09, next, 0x18, 0x08, 0x04, 0x07, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x10, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
        ],
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
fn pci_endpoint_raw_capability_links_read_only_bytes_and_snapshots_shape() {
    let mut endpoint = storage_endpoint();

    endpoint.install_pm_capability(pm(0x44)).unwrap();
    endpoint
        .install_raw_capability(virtio_shared_memory_cap(0x60))
        .unwrap();
    endpoint.install_msi_capability(msi(0x80)).unwrap();

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
            AccessSize::new(2).unwrap(),
        ),
        Ok(vec![0x01, 0x60])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x60).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x09, 0x80, 0x18, 0x08])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x68).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x08, 0x00, 0x00, 0x00])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x6c).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x10, 0x00, 0x00, 0x00])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x70).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x01, 0x00, 0x00, 0x00])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x74).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x02, 0x00, 0x00, 0x00])
    );
    assert_eq!(
        endpoint.write_config(PciConfigOffset::new(0x62).unwrap(), &[0x20]),
        Err(PciError::ReadOnlyConfigWrite {
            offset: PciConfigOffset::new(0x62).unwrap(),
            size: AccessSize::new(1).unwrap(),
        })
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x60).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x09, 0x80, 0x18, 0x08])
    );

    let snapshot = endpoint.snapshot();
    endpoint
        .write_config(
            PciConfigOffset::new(0x48).unwrap(),
            &0x0001_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint.restore(&snapshot).unwrap();
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x48).unwrap(),
            AccessSize::new(2).unwrap(),
        ),
        Ok(vec![0x00, 0x00])
    );

    let mut same_shape = storage_endpoint();
    same_shape.install_pm_capability(pm(0x44)).unwrap();
    same_shape
        .install_raw_capability(virtio_shared_memory_cap_with_next(0x60, 0x00))
        .unwrap();
    same_shape.install_msi_capability(msi(0x80)).unwrap();
    same_shape.restore(&snapshot).unwrap();
    assert_eq!(
        same_shape.read_config(
            PciConfigOffset::new(0x60).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x09, 0x80, 0x18, 0x08])
    );

    let mut other = storage_endpoint();
    other.install_pm_capability(pm(0x44)).unwrap();
    other
        .install_raw_capability(virtio_shared_memory_cap(0x64))
        .unwrap();
    other.install_msi_capability(msi(0x80)).unwrap();
    assert_eq!(
        other.restore(&snapshot),
        Err(PciError::SnapshotRawCapabilityMismatch)
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
fn pci_endpoint_pcie_extended_registers_are_typed_and_snapshot_restored() {
    let mut endpoint = storage_endpoint();

    endpoint
        .install_pcie_capability(pcie_with_extended_registers(0x80))
        .unwrap();

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x94).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x44, 0x33, 0x22, 0x11])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x98).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x66, 0x55, 0x88, 0x77])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x9c).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0xcc, 0xbb, 0xaa, 0x99])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0xa0).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x00, 0xff, 0xee, 0xdd])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0xa4).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x04, 0x03, 0x02, 0x01])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0xa8).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x06, 0x05, 0x08, 0x07])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0xac).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x14, 0x13, 0x12, 0x11])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0xb0).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x16, 0x15, 0x18, 0x17])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0xb4).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x24, 0x23, 0x22, 0x21])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0xb8).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x26, 0x25, 0x28, 0x27])
    );

    endpoint
        .write_config(
            PciConfigOffset::new(0x98).unwrap(),
            &0x0102_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0x9a).unwrap(),
            &0x0304_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0x9c).unwrap(),
            &0x0506_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0xa0).unwrap(),
            &0x0708_090a_u32.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0xa8).unwrap(),
            &0x0b0c_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0xaa).unwrap(),
            &0x0d0e_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0xb0).unwrap(),
            &0x0f10_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0xb2).unwrap(),
            &0x1112_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0xb8).unwrap(),
            &0x1314_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0xba).unwrap(),
            &0x1516_u16.to_le_bytes(),
        )
        .unwrap();
    let snapshot = endpoint.snapshot();
    endpoint
        .write_config(
            PciConfigOffset::new(0x98).unwrap(),
            &0x0000_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0xa0).unwrap(),
            &0x0000_0000_u32.to_le_bytes(),
        )
        .unwrap();
    endpoint.restore(&snapshot).unwrap();

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x98).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x02, 0x01, 0x04, 0x03])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x9c).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x06, 0x05, 0xaa, 0x99])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0xa0).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x0a, 0x09, 0x08, 0x07])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0xa8).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x0c, 0x0b, 0x0e, 0x0d])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0xb0).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x10, 0x0f, 0x12, 0x11])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0xb8).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x14, 0x13, 0x16, 0x15])
    );
    assert_eq!(
        endpoint.write_config(
            PciConfigOffset::new(0xb4).unwrap(),
            &0x0000_0000_u32.to_le_bytes(),
        ),
        Err(PciError::ReadOnlyPciExpressCapabilityWrite {
            offset: PciConfigOffset::new(0xb4).unwrap(),
            size: AccessSize::new(4).unwrap(),
        })
    );
    assert_eq!(
        endpoint.write_config(
            PciConfigOffset::new(0xa0).unwrap(),
            &0x0000_u16.to_le_bytes(),
        ),
        Err(PciError::UnalignedPciExpressCapabilityWrite {
            offset: PciConfigOffset::new(0xa0).unwrap(),
            size: AccessSize::new(2).unwrap(),
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

#[test]
fn pci_endpoint_rejects_invalid_raw_capabilities_without_mutating_chain() {
    let mut endpoint = storage_endpoint();

    endpoint.install_msi_capability(msi(0x50)).unwrap();

    assert_eq!(
        PciRawCapabilitySpec::new(PciConfigOffset::new(0x3c).unwrap(), [0x09, 0x00]),
        Err(PciError::InvalidRawCapabilityOffset {
            offset: PciConfigOffset::new(0x3c).unwrap(),
            size: AccessSize::new(2).unwrap(),
        })
    );
    assert_eq!(
        PciRawCapabilitySpec::new(PciConfigOffset::new(0x40).unwrap(), [0x09]),
        Err(PciError::InvalidRawCapabilitySize {
            offset: PciConfigOffset::new(0x40).unwrap(),
            size: AccessSize::new(1).unwrap(),
        })
    );
    assert_eq!(
        endpoint.install_raw_capability(virtio_shared_memory_cap(0x60)),
        Err(PciError::OverlappingCapability {
            existing_offset: PciConfigOffset::new(0x50).unwrap(),
            existing_size: AccessSize::new(0x18).unwrap(),
            requested_offset: PciConfigOffset::new(0x60).unwrap(),
            requested_size: AccessSize::new(0x18).unwrap(),
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
            PciConfigOffset::new(0x60).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x00, 0x00, 0x00, 0x00])
    );
}

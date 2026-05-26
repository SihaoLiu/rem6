use rem6_memory::AccessSize;
use rem6_pci::{
    PciClassCode, PciConfigOffset, PciDeviceIdentity, PciEndpointConfig, PciFunctionAddress,
};
use rem6_virtio::{
    VirtioPciBarIndex, VirtioPciCapabilityEntry, VirtioPciCapabilityKind,
    VirtioPciCapabilityOffset, VirtioPciNotifyCapabilityEntry, VirtioPciSharedMemoryCapabilities,
    VirtioPciSharedMemoryRegionSpec, VirtioPciSharedMemoryRegistry,
};

fn bar(index: u8) -> VirtioPciBarIndex {
    VirtioPciBarIndex::new(index).unwrap()
}

fn region_id(value: u8) -> rem6_virtio::VirtioPciSharedMemoryId {
    rem6_virtio::VirtioPciSharedMemoryId::new(value)
}

fn endpoint() -> PciEndpointConfig {
    PciEndpointConfig::new(
        PciFunctionAddress::new(0, 10, 0).unwrap(),
        PciDeviceIdentity::new(0x1af4, 0x1042),
        PciClassCode::new(0x02, 0x00, 0x00, 0x00),
    )
}

#[test]
fn virtio_pci_capability_entry_exports_standard_vendor_bytes() {
    let common = VirtioPciCapabilityEntry::new(
        VirtioPciCapabilityOffset::new(0x70).unwrap(),
        Some(VirtioPciCapabilityOffset::new(0x80).unwrap()),
        VirtioPciCapabilityKind::CommonConfig,
        bar(2),
        0,
        0x1000_0004,
        0x40,
    )
    .unwrap();

    assert_eq!(VirtioPciCapabilityKind::CommonConfig.cfg_type(), 1);
    assert_eq!(
        common.offset(),
        VirtioPciCapabilityOffset::new(0x70).unwrap()
    );
    assert_eq!(
        common.next(),
        Some(VirtioPciCapabilityOffset::new(0x80).unwrap())
    );
    assert_eq!(common.kind(), VirtioPciCapabilityKind::CommonConfig);
    assert_eq!(common.bar(), bar(2));
    assert_eq!(common.id(), 0);
    assert_eq!(common.region_offset(), 0x1000_0004);
    assert_eq!(common.length(), 0x40);
    assert_eq!(
        common.bytes(),
        [
            0x09, 0x80, 0x10, 0x01, 0x02, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x10, 0x40, 0x00,
            0x00, 0x00,
        ]
    );
}

#[test]
fn virtio_pci_notify_capability_entry_exports_multiplier_extension() {
    let notify = VirtioPciNotifyCapabilityEntry::new(
        VirtioPciCapabilityEntry::new(
            VirtioPciCapabilityOffset::new(0x80).unwrap(),
            None,
            VirtioPciCapabilityKind::NotifyConfig,
            bar(4),
            2,
            0x200,
            0x100,
        )
        .unwrap(),
        0x20,
    )
    .unwrap();

    assert_eq!(notify.base().kind(), VirtioPciCapabilityKind::NotifyConfig);
    assert_eq!(notify.notify_off_multiplier(), 0x20);
    assert_eq!(
        notify.bytes(),
        [
            0x09, 0x00, 0x14, 0x02, 0x04, 0x02, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x20, 0x00, 0x00, 0x00,
        ]
    );
}

#[test]
fn virtio_pci_capabilities_install_as_pci_raw_capabilities() {
    let common = VirtioPciCapabilityEntry::new(
        VirtioPciCapabilityOffset::new(0x70).unwrap(),
        Some(VirtioPciCapabilityOffset::new(0x80).unwrap()),
        VirtioPciCapabilityKind::CommonConfig,
        bar(2),
        0,
        0x1000,
        0x40,
    )
    .unwrap();
    let notify = VirtioPciNotifyCapabilityEntry::new(
        VirtioPciCapabilityEntry::new(
            VirtioPciCapabilityOffset::new(0x80).unwrap(),
            Some(VirtioPciCapabilityOffset::new(0xa0).unwrap()),
            VirtioPciCapabilityKind::NotifyConfig,
            bar(4),
            0,
            0x2000,
            0x80,
        )
        .unwrap(),
        4,
    )
    .unwrap();
    let registry = VirtioPciSharedMemoryRegistry::new(
        [(bar(5), AccessSize::new(0x4000).unwrap())],
        [VirtioPciSharedMemoryRegionSpec::new(region_id(9), bar(5), 0x3000, 0x400).unwrap()],
    )
    .unwrap();
    let shared = VirtioPciSharedMemoryCapabilities::new(
        VirtioPciCapabilityOffset::new(0xa0).unwrap(),
        &registry,
    )
    .unwrap();

    let mut endpoint = endpoint();
    endpoint
        .install_raw_capability(common.raw_capability_spec())
        .unwrap();
    endpoint
        .install_raw_capability(notify.raw_capability_spec())
        .unwrap();
    endpoint
        .install_raw_capability(shared.entries()[0].raw_capability_spec())
        .unwrap();

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x34).unwrap(),
            AccessSize::new(1).unwrap()
        ),
        Ok(vec![0x70])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x70).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x09, 0x80, 0x10, 0x01])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x80).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x09, 0xa0, 0x14, 0x02])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x90).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x04, 0x00, 0x00, 0x00])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0xa0).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x09, 0x00, 0x18, 0x08])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0xa4).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x05, 0x09, 0x00, 0x00])
    );
}

#[test]
fn virtio_pci_capability_entries_reject_invalid_layouts() {
    assert!(matches!(
        VirtioPciCapabilityEntry::new(
            VirtioPciCapabilityOffset::new(0xf4).unwrap(),
            None,
            VirtioPciCapabilityKind::DeviceConfig,
            bar(1),
            0,
            0,
            4,
        ),
        Err(error) if error.to_string().contains("configuration space")
    ));
    assert!(matches!(
        VirtioPciCapabilityEntry::new(
            VirtioPciCapabilityOffset::new(0x40).unwrap(),
            None,
            VirtioPciCapabilityKind::DeviceConfig,
            bar(1),
            0,
            0,
            0,
        ),
        Err(error) if error.to_string().contains("zero length")
    ));
    assert!(matches!(
        VirtioPciNotifyCapabilityEntry::new(
            VirtioPciCapabilityEntry::new(
                VirtioPciCapabilityOffset::new(0x40).unwrap(),
                None,
                VirtioPciCapabilityKind::DeviceConfig,
                bar(1),
                0,
                0,
                4,
            )
            .unwrap(),
            0,
        ),
        Err(error) if error.to_string().contains("notify")
    ));
    assert!(matches!(
        VirtioPciNotifyCapabilityEntry::new(
            VirtioPciCapabilityEntry::new(
                VirtioPciCapabilityOffset::new(0xf0).unwrap(),
                None,
                VirtioPciCapabilityKind::NotifyConfig,
                bar(1),
                0,
                0,
                4,
            )
            .unwrap(),
            0,
        ),
        Err(error) if error.to_string().contains("configuration space")
    ));
}

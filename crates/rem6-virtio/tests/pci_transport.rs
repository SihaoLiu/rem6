use rem6_memory::AccessSize;
use rem6_pci::{
    PciBarKind, PciClassCode, PciConfigOffset, PciDeviceIdentity, PciEndpointConfig,
    PciFunctionAddress,
};
use rem6_virtio::{
    VirtioPciBarIndex, VirtioPciCapabilityOffset, VirtioPciModernTransportSpec,
    VirtioPciNotifyRegion, VirtioPciSharedMemoryId, VirtioPciSharedMemoryRegionSpec,
    VirtioPciSharedMemoryRegistry, VirtioPciTransportBarSpec, VirtioPciTransportEndpointSpec,
    VirtioPciTransportRegion, VIRTIO_PCI_ISR_STATUS_SIZE,
};

fn bar(index: u8) -> VirtioPciBarIndex {
    VirtioPciBarIndex::new(index).unwrap()
}

fn function() -> PciFunctionAddress {
    PciFunctionAddress::new(0, 12, 0).unwrap()
}

fn identity() -> PciDeviceIdentity {
    PciDeviceIdentity::new(0x1af4, 0x1042)
}

fn class() -> PciClassCode {
    PciClassCode::new(0x02, 0x00, 0x00, 0x00)
}

fn endpoint_spec() -> VirtioPciTransportEndpointSpec {
    VirtioPciTransportEndpointSpec::new(function(), identity(), class())
}

fn memory_bar(index: u8, size: u64) -> VirtioPciTransportBarSpec {
    VirtioPciTransportBarSpec::new(
        bar(index),
        PciBarKind::Memory32 {
            prefetchable: false,
        },
        AccessSize::new(size).unwrap(),
    )
}

fn region(index: u8, offset: u64, length: u32) -> VirtioPciTransportRegion {
    VirtioPciTransportRegion::new(bar(index), offset, length)
}

fn base_transport(
    common: VirtioPciTransportRegion,
    notify: VirtioPciNotifyRegion,
    isr: VirtioPciTransportRegion,
) -> VirtioPciModernTransportSpec {
    VirtioPciModernTransportSpec::new(
        endpoint_spec(),
        VirtioPciCapabilityOffset::new(0x70).unwrap(),
        [memory_bar(0, 0x1000)],
        common,
        notify,
        isr,
    )
}

fn read4(endpoint: &PciEndpointConfig, offset: u16) -> Vec<u8> {
    endpoint
        .read_config(
            PciConfigOffset::new(offset).unwrap(),
            AccessSize::new(4).unwrap(),
        )
        .unwrap()
}

#[test]
fn virtio_pci_modern_transport_builds_endpoint_bars_and_capabilities() {
    let shared = VirtioPciSharedMemoryRegistry::new(
        [(bar(0), AccessSize::new(0x1000).unwrap())],
        [VirtioPciSharedMemoryRegionSpec::new(
            VirtioPciSharedMemoryId::new(7),
            bar(0),
            0x800,
            0x100,
        )
        .unwrap()],
    )
    .unwrap();
    let transport = base_transport(
        region(0, 0x000, 0x40),
        VirtioPciNotifyRegion::new(region(0, 0x100, 0x100), 4).unwrap(),
        region(0, 0x200, VIRTIO_PCI_ISR_STATUS_SIZE as u32),
    )
    .with_device_config(region(0, 0x300, 0x20))
    .with_shared_memory(shared);

    let endpoint = transport.build_endpoint().unwrap();

    assert_eq!(endpoint.function(), function());
    assert_eq!(endpoint.identity(), identity());
    assert_eq!(endpoint.class(), class());
    assert_eq!(
        endpoint
            .read_config(
                PciConfigOffset::new(0x34).unwrap(),
                AccessSize::new(1).unwrap(),
            )
            .unwrap(),
        vec![0x70]
    );
    assert_eq!(read4(&endpoint, 0x70), vec![0x09, 0x80, 0x10, 0x01]);
    assert_eq!(read4(&endpoint, 0x78), vec![0x00, 0x00, 0x00, 0x00]);
    assert_eq!(read4(&endpoint, 0x7c), vec![0x40, 0x00, 0x00, 0x00]);
    assert_eq!(read4(&endpoint, 0x80), vec![0x09, 0x94, 0x14, 0x02]);
    assert_eq!(read4(&endpoint, 0x88), vec![0x00, 0x01, 0x00, 0x00]);
    assert_eq!(read4(&endpoint, 0x8c), vec![0x00, 0x01, 0x00, 0x00]);
    assert_eq!(read4(&endpoint, 0x90), vec![0x04, 0x00, 0x00, 0x00]);
    assert_eq!(read4(&endpoint, 0x94), vec![0x09, 0xa4, 0x10, 0x03]);
    assert_eq!(read4(&endpoint, 0xa4), vec![0x09, 0xb4, 0x10, 0x04]);
    assert_eq!(read4(&endpoint, 0xb4), vec![0x09, 0x00, 0x18, 0x08]);
    assert_eq!(read4(&endpoint, 0xb8), vec![0x00, 0x07, 0x00, 0x00]);
    assert_eq!(read4(&endpoint, 0xbc), vec![0x00, 0x08, 0x00, 0x00]);
    assert_eq!(read4(&endpoint, 0xc0), vec![0x00, 0x01, 0x00, 0x00]);
}

#[test]
fn virtio_pci_modern_transport_rejects_invalid_bar_regions() {
    let missing = VirtioPciModernTransportSpec::new(
        endpoint_spec(),
        VirtioPciCapabilityOffset::new(0x70).unwrap(),
        [memory_bar(0, 0x1000)],
        region(1, 0x000, 0x40),
        VirtioPciNotifyRegion::new(region(0, 0x100, 0x100), 4).unwrap(),
        region(0, 0x200, VIRTIO_PCI_ISR_STATUS_SIZE as u32),
    );
    assert!(matches!(
        missing.build_endpoint(),
        Err(error) if error.to_string().contains("undeclared BAR")
    ));

    let outside = base_transport(
        region(0, 0xf80, 0x100),
        VirtioPciNotifyRegion::new(region(0, 0x100, 0x100), 4).unwrap(),
        region(0, 0x200, VIRTIO_PCI_ISR_STATUS_SIZE as u32),
    );
    assert!(matches!(
        outside.build_endpoint(),
        Err(error) if error.to_string().contains("contained within BAR")
    ));

    let overlapping = base_transport(
        region(0, 0x000, 0x80),
        VirtioPciNotifyRegion::new(region(0, 0x040, 0x100), 4).unwrap(),
        region(0, 0x200, VIRTIO_PCI_ISR_STATUS_SIZE as u32),
    );
    assert!(matches!(
        overlapping.build_endpoint(),
        Err(error) if error.to_string().contains("overlaps")
    ));
}

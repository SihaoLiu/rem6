use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarRange, PciBarSpec, PciClassCode, PciConfigAperture,
    PciConfigOffset, PciDeviceIdentity, PciEndpointConfig, PciError, PciFunctionAddress,
    PciHostBridge,
};

fn storage_endpoint(function: PciFunctionAddress) -> PciEndpointConfig {
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
                AccessSize::new(0x2000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    endpoint
}

#[test]
fn pci_host_ecam_routes_config_accesses_to_registered_endpoint() {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 1).unwrap();
    let mut host = PciHostBridge::new(aperture);
    let function = PciFunctionAddress::new(0, 3, 0).unwrap();
    host.register_endpoint(storage_endpoint(function)).unwrap();

    let config_range = host.endpoint_config_range(function).unwrap();
    assert_eq!(config_range.start(), Address::new(0x3001_8000));
    assert_eq!(config_range.size(), AccessSize::new(0x1000).unwrap());
    assert_eq!(
        host.read_config_address(config_range.start(), AccessSize::new(4).unwrap()),
        Ok(vec![0xf4, 0x1a, 0x01, 0x10])
    );

    let bar0 = Address::new(config_range.start().get() + 0x10);
    host.write_config_address(bar0, &0x9000_1234_u32.to_le_bytes())
        .unwrap();
    host.write_config_address(
        Address::new(config_range.start().get() + 0x04),
        &0x0002_u16.to_le_bytes(),
    )
    .unwrap();

    assert_eq!(
        host.endpoint(function).unwrap().active_bar_ranges(),
        vec![PciBarRange::new(
            PciBarIndex::new(0).unwrap(),
            PciBarKind::Memory32 {
                prefetchable: false,
            },
            Address::new(0x9000_0000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap()]
    );
}

#[test]
fn pci_host_supports_cam_sized_config_slots_and_absent_device_reads() {
    let aperture = PciConfigAperture::cam(Address::new(0x4000_0000), 1).unwrap();
    let mut host = PciHostBridge::new(aperture);
    let function = PciFunctionAddress::new(0, 3, 2).unwrap();
    host.register_endpoint(storage_endpoint(function)).unwrap();

    let config_range = host.endpoint_config_range(function).unwrap();
    assert_eq!(config_range.start(), Address::new(0x4000_1a00));
    assert_eq!(config_range.size(), AccessSize::new(0x100).unwrap());

    let missing = host
        .aperture()
        .config_address(
            PciFunctionAddress::new(0, 4, 0).unwrap(),
            PciConfigOffset::new(0x00).unwrap(),
        )
        .unwrap();
    assert_eq!(
        host.read_config_address(missing, AccessSize::new(4).unwrap()),
        Ok(vec![0xff, 0xff, 0xff, 0xff])
    );
    assert_eq!(
        host.write_config_address(missing, &0xffff_ffff_u32.to_le_bytes()),
        Ok(())
    );
}

#[test]
fn pci_host_rejects_duplicate_or_out_of_aperture_devices() {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 1).unwrap();
    let mut host = PciHostBridge::new(aperture);
    let function = PciFunctionAddress::new(0, 2, 0).unwrap();
    host.register_endpoint(storage_endpoint(function)).unwrap();

    assert_eq!(
        host.register_endpoint(storage_endpoint(function)),
        Err(PciError::DuplicateFunction { function })
    );

    let out_of_range = PciFunctionAddress::new(1, 0, 0).unwrap();
    assert_eq!(
        host.register_endpoint(storage_endpoint(out_of_range)),
        Err(PciError::FunctionOutsideAperture {
            function: out_of_range,
            bus_count: 1,
        })
    );
}

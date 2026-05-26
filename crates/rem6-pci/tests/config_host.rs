use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarRange, PciBarSpec, PciClassCode, PciConfigAperture,
    PciConfigOffset, PciDeviceIdentity, PciEndpointConfig, PciError, PciFunctionAddress,
    PciHostAddressBases, PciHostAddressSpace, PciHostBarRange, PciHostBridge,
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

fn multi_bar_endpoint(function: PciFunctionAddress) -> PciEndpointConfig {
    let mut endpoint = storage_endpoint(function);
    endpoint
        .install_bar(
            PciBarSpec::new(
                PciBarIndex::new(1).unwrap(),
                PciBarKind::Memory32 { prefetchable: true },
                AccessSize::new(0x4000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    endpoint
        .install_bar(
            PciBarSpec::new(
                PciBarIndex::new(2).unwrap(),
                PciBarKind::Io,
                AccessSize::new(0x100).unwrap(),
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
fn pci_host_maps_active_bar_ranges_into_host_address_spaces() {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 1).unwrap();
    let bases = PciHostAddressBases::new(
        Address::new(0x1000_0000),
        Address::new(0x8000_0000),
        Address::new(0xa000_0000),
    );
    let mut host = PciHostBridge::with_address_bases(aperture, bases);
    let function = PciFunctionAddress::new(0, 3, 0).unwrap();
    host.register_endpoint(multi_bar_endpoint(function))
        .unwrap();

    let config_range = host.endpoint_config_range(function).unwrap();
    host.write_config_address(
        Address::new(config_range.start().get() + 0x10),
        &0x0000_2000_u32.to_le_bytes(),
    )
    .unwrap();
    host.write_config_address(
        Address::new(config_range.start().get() + 0x14),
        &0x0001_0000_u32.to_le_bytes(),
    )
    .unwrap();
    host.write_config_address(
        Address::new(config_range.start().get() + 0x18),
        &0x0000_0181_u32.to_le_bytes(),
    )
    .unwrap();
    host.write_config_address(
        Address::new(config_range.start().get() + 0x04),
        &0x0003_u16.to_le_bytes(),
    )
    .unwrap();

    assert_eq!(
        host.active_host_bar_ranges().unwrap(),
        vec![
            PciHostBarRange::new(
                function,
                PciBarIndex::new(0).unwrap(),
                PciHostAddressSpace::Memory,
                Address::new(0x0000_2000),
                Address::new(0x8000_2000),
                AccessSize::new(0x2000).unwrap(),
            )
            .unwrap(),
            PciHostBarRange::new(
                function,
                PciBarIndex::new(1).unwrap(),
                PciHostAddressSpace::PrefetchableMemory,
                Address::new(0x0001_0000),
                Address::new(0xa001_0000),
                AccessSize::new(0x4000).unwrap(),
            )
            .unwrap(),
            PciHostBarRange::new(
                function,
                PciBarIndex::new(2).unwrap(),
                PciHostAddressSpace::Io,
                Address::new(0x0000_0100),
                Address::new(0x1000_0100),
                AccessSize::new(0x100).unwrap(),
            )
            .unwrap(),
        ]
    );
}

#[test]
fn pci_host_maps_memory64_bar_ranges_into_host_memory_space() {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 1).unwrap();
    let bases = PciHostAddressBases::new(
        Address::new(0x1000_0000),
        Address::new(0x20_0000_0000),
        Address::new(0x40_0000_0000),
    );
    let mut endpoint = storage_endpoint(PciFunctionAddress::new(0, 4, 0).unwrap());
    endpoint
        .install_bar(
            PciBarSpec::new(
                PciBarIndex::new(2).unwrap(),
                PciBarKind::Memory64 {
                    prefetchable: false,
                },
                AccessSize::new(0x2000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

    let function = endpoint.function();
    let mut host = PciHostBridge::with_address_bases(aperture, bases);
    host.register_endpoint(endpoint).unwrap();
    let config_range = host.endpoint_config_range(function).unwrap();
    host.write_config_address(
        Address::new(config_range.start().get() + 0x18),
        &0x0000_2345_u32.to_le_bytes(),
    )
    .unwrap();
    host.write_config_address(
        Address::new(config_range.start().get() + 0x1c),
        &0x0000_0001_u32.to_le_bytes(),
    )
    .unwrap();
    host.write_config_address(
        Address::new(config_range.start().get() + 0x04),
        &0x0002_u16.to_le_bytes(),
    )
    .unwrap();

    assert_eq!(
        host.active_host_bar_ranges().unwrap(),
        vec![
            PciHostBarRange::new(
                function,
                PciBarIndex::new(0).unwrap(),
                PciHostAddressSpace::Memory,
                Address::new(0),
                Address::new(0x20_0000_0000),
                AccessSize::new(0x2000).unwrap(),
            )
            .unwrap(),
            PciHostBarRange::new(
                function,
                PciBarIndex::new(2).unwrap(),
                PciHostAddressSpace::Memory,
                Address::new(0x1_0000_2000),
                Address::new(0x21_0000_2000),
                AccessSize::new(0x2000).unwrap(),
            )
            .unwrap(),
        ]
    );
}

#[test]
fn pci_host_rejects_overlapping_active_host_bar_ranges() {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 1).unwrap();
    let mut host = PciHostBridge::with_address_bases(
        aperture,
        PciHostAddressBases::new(
            Address::new(0x1000_0000),
            Address::new(0x8000_0000),
            Address::new(0xa000_0000),
        ),
    );
    let first = PciFunctionAddress::new(0, 2, 0).unwrap();
    let second = PciFunctionAddress::new(0, 3, 0).unwrap();
    host.register_endpoint(storage_endpoint(first)).unwrap();
    host.register_endpoint(storage_endpoint(second)).unwrap();

    for function in [first, second] {
        let config_range = host.endpoint_config_range(function).unwrap();
        host.write_config_address(
            Address::new(config_range.start().get() + 0x10),
            &0x0000_2000_u32.to_le_bytes(),
        )
        .unwrap();
        host.write_config_address(
            Address::new(config_range.start().get() + 0x04),
            &0x0002_u16.to_le_bytes(),
        )
        .unwrap();
    }

    assert_eq!(
        host.active_host_bar_ranges(),
        Err(PciError::OverlappingHostBarRange {
            existing_function: first,
            existing_bar: PciBarIndex::new(0).unwrap(),
            requested_function: second,
            requested_bar: PciBarIndex::new(0).unwrap(),
        })
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

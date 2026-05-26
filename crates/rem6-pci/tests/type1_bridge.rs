use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarSpec, PciBridgeBusRange, PciBridgeConfig, PciClassCode,
    PciConfigAperture, PciConfigOffset, PciDeviceIdentity, PciEndpointConfig, PciError,
    PciFunctionAddress, PciHostAddressBases, PciHostAddressSpace, PciHostBarRange, PciHostBridge,
    PciInterruptPin, PciType1HeaderFields,
};

fn bridge_config(function: PciFunctionAddress) -> PciBridgeConfig {
    PciBridgeConfig::new(
        function,
        PciDeviceIdentity::new(0x1011, 0x0026),
        PciClassCode::new(0x06, 0x04, 0x00, 0x00),
        PciBridgeBusRange::new(0, 1, 2).unwrap(),
    )
}

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

fn legacy_io_endpoint(function: PciFunctionAddress) -> PciEndpointConfig {
    let mut endpoint = PciEndpointConfig::new(
        function,
        PciDeviceIdentity::new(0x8086, 0x100e),
        PciClassCode::new(0x02, 0x00, 0x00, 0x00),
    );
    endpoint
        .install_bar(
            PciBarSpec::new(
                PciBarIndex::new(0).unwrap(),
                PciBarKind::LegacyIo {
                    address: Address::new(0x3000),
                },
                AccessSize::new(0x10).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    endpoint
}

#[test]
fn pci_type1_bridge_config_exposes_header_bus_numbers_and_windows() {
    let function = PciFunctionAddress::new(0, 1, 0).unwrap();
    let mut bridge = bridge_config(function);

    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x00).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x11, 0x10, 0x26, 0x00])
    );
    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x08).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x00, 0x00, 0x04, 0x06])
    );
    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x0e).unwrap(),
            AccessSize::new(1).unwrap()
        ),
        Ok(vec![0x01])
    );
    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x18).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0, 1, 2, 0])
    );

    bridge
        .write_config(PciConfigOffset::new(0x18).unwrap(), &[0, 2, 3, 0x20])
        .unwrap();
    bridge
        .write_config(
            PciConfigOffset::new(0x20).unwrap(),
            &0x0020_0010_u32.to_le_bytes(),
        )
        .unwrap();

    assert_eq!(bridge.bus_range(), PciBridgeBusRange::new(0, 2, 3).unwrap());
    assert!(bridge.routes_bus(2));
    assert!(bridge.routes_bus(3));
    assert!(!bridge.routes_bus(1));
    assert!(bridge.allows_bar_range(
        PciBarKind::Memory32 {
            prefetchable: false,
        },
        rem6_memory::AddressRange::new(Address::new(0x0010_0000), AccessSize::new(0x2000).unwrap())
            .unwrap(),
    ));
    assert!(!bridge.allows_bar_range(
        PciBarKind::Memory32 {
            prefetchable: false,
        },
        rem6_memory::AddressRange::new(Address::new(0x0030_0000), AccessSize::new(0x2000).unwrap())
            .unwrap(),
    ));
}

#[test]
fn pci_type1_bridge_header_exposes_interrupt_rom_and_control_fields() {
    let function = PciFunctionAddress::new(0, 1, 0).unwrap();
    let mut bridge = bridge_config(function).with_type1_header(PciType1HeaderFields::new(
        0x8000_0001,
        7,
        PciInterruptPin::IntB,
        0x0040,
    ));

    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x38).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x01, 0x00, 0x00, 0x80])
    );
    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x3c).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![7, 2, 0x40, 0x00])
    );

    bridge
        .write_config(
            PciConfigOffset::new(0x38).unwrap(),
            &0x9000_0001_u32.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x38).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x01, 0x00, 0x00, 0x90])
    );

    bridge
        .write_config(
            PciConfigOffset::new(0x38).unwrap(),
            &0xffff_fffe_u32.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x38).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0xff, 0xff, 0xff, 0xff])
    );

    bridge
        .write_config(PciConfigOffset::new(0x3c).unwrap(), &[9])
        .unwrap();
    bridge
        .write_config(
            PciConfigOffset::new(0x3e).unwrap(),
            &0x0080_u16.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x3c).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![9, 2, 0x80, 0x00])
    );
}

#[test]
fn pci_type1_bridge_common_header_writes_cache_line_latency_and_snapshots() {
    let function = PciFunctionAddress::new(0, 1, 0).unwrap();
    let mut bridge = bridge_config(function);

    bridge
        .write_config(PciConfigOffset::new(0x0c).unwrap(), &[0x80])
        .unwrap();
    bridge
        .write_config(PciConfigOffset::new(0x0d).unwrap(), &[0x10])
        .unwrap();

    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x0c).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x80, 0x10, 0x01, 0x00])
    );

    bridge
        .write_config(
            PciConfigOffset::new(0x0c).unwrap(),
            &0x5544_u16.to_le_bytes(),
        )
        .unwrap();
    let snapshot = bridge.snapshot();
    bridge
        .write_config(
            PciConfigOffset::new(0x0c).unwrap(),
            &0x0000_u16.to_le_bytes(),
        )
        .unwrap();
    bridge.restore(&snapshot).unwrap();

    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x0c).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x44, 0x55, 0x01, 0x00])
    );
    assert_eq!(
        bridge.write_config(
            PciConfigOffset::new(0x0d).unwrap(),
            &0x0000_u16.to_le_bytes(),
        ),
        Err(PciError::ReadOnlyConfigWrite {
            offset: PciConfigOffset::new(0x0d).unwrap(),
            size: AccessSize::new(2).unwrap(),
        })
    );
}

#[test]
fn pci_type1_bridge_status_writes_do_not_create_status_bits() {
    let function = PciFunctionAddress::new(0, 1, 0).unwrap();
    let mut bridge = bridge_config(function);

    bridge
        .write_config(
            PciConfigOffset::new(0x04).unwrap(),
            &0x0003_u16.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x04).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x03, 0x00, 0x00, 0x00])
    );

    bridge
        .write_config(
            PciConfigOffset::new(0x06).unwrap(),
            &0xffff_u16.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x04).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x03, 0x00, 0x00, 0x00])
    );

    bridge
        .write_config(
            PciConfigOffset::new(0x04).unwrap(),
            &0xffff_0002_u32.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x04).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x02, 0x00, 0x00, 0x00])
    );
}

#[test]
fn pci_type1_bridge_bars_map_on_primary_bus_when_command_bits_enable_space() {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 3).unwrap();
    let bases = PciHostAddressBases::new(
        Address::new(0x1000_0000),
        Address::new(0x8000_0000),
        Address::new(0xa000_0000),
    );
    let mut host = PciHostBridge::with_address_bases(aperture, bases);
    let bridge_function = PciFunctionAddress::new(0, 1, 0).unwrap();
    let mut bridge = bridge_config(bridge_function);
    bridge
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
    host.register_bridge(bridge).unwrap();

    let bridge_bar_addr = aperture
        .config_address(bridge_function, PciConfigOffset::new(0x10).unwrap())
        .unwrap();
    let bridge_command_addr = aperture
        .config_address(bridge_function, PciConfigOffset::new(0x04).unwrap())
        .unwrap();

    host.write_config_address(bridge_bar_addr, &0x0040_0000_u32.to_le_bytes())
        .unwrap();
    assert_eq!(host.active_host_bar_ranges(), Ok(Vec::new()));

    host.write_config_address(bridge_command_addr, &0x0002_u16.to_le_bytes())
        .unwrap();
    assert_eq!(
        host.read_config_address(bridge_bar_addr, AccessSize::new(4).unwrap()),
        Ok(vec![0x00, 0x00, 0x40, 0x00])
    );
    assert_eq!(
        host.active_host_bar_ranges(),
        Ok(vec![PciHostBarRange::new(
            bridge_function,
            PciBarIndex::new(0).unwrap(),
            PciHostAddressSpace::Memory,
            Address::new(0x0040_0000),
            Address::new(0x8040_0000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap()])
    );
}

#[test]
fn pci_type1_bridge_snapshot_restore_preserves_config_and_bar_state() {
    let function = PciFunctionAddress::new(0, 1, 0).unwrap();
    let mut bridge = bridge_config(function).with_type1_header(PciType1HeaderFields::new(
        0x8000_0001,
        7,
        PciInterruptPin::IntB,
        0x0040,
    ));
    bridge
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
    bridge
        .write_config(
            PciConfigOffset::new(0x10).unwrap(),
            &0x0040_0000_u32.to_le_bytes(),
        )
        .unwrap();
    bridge
        .write_config(
            PciConfigOffset::new(0x04).unwrap(),
            &0x0002_u16.to_le_bytes(),
        )
        .unwrap();
    bridge
        .write_config(PciConfigOffset::new(0x18).unwrap(), &[0, 2, 3, 0x20])
        .unwrap();

    let snapshot = bridge.snapshot();
    bridge
        .write_config(
            PciConfigOffset::new(0x10).unwrap(),
            &0x0080_0000_u32.to_le_bytes(),
        )
        .unwrap();
    bridge
        .write_config(
            PciConfigOffset::new(0x04).unwrap(),
            &0x0000_u16.to_le_bytes(),
        )
        .unwrap();
    bridge
        .write_config(PciConfigOffset::new(0x18).unwrap(), &[0, 1, 1, 0])
        .unwrap();

    bridge.restore(&snapshot).unwrap();

    assert_eq!(bridge.bus_range(), PciBridgeBusRange::new(0, 2, 3).unwrap());
    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x10).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x00, 0x00, 0x40, 0x00])
    );
    assert_eq!(
        bridge.read_config(
            PciConfigOffset::new(0x38).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x01, 0x00, 0x00, 0x80])
    );
    assert_eq!(
        bridge.active_bar_ranges(),
        vec![rem6_pci::PciBarRange::new(
            PciBarIndex::new(0).unwrap(),
            PciBarKind::Memory32 {
                prefetchable: false,
            },
            Address::new(0x0040_0000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap()]
    );
}

#[test]
fn pci_host_routes_subordinate_config_only_through_declared_bridge_bus_numbers() {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 4).unwrap();
    let mut host = PciHostBridge::new(aperture);
    let bridge_function = PciFunctionAddress::new(0, 1, 0).unwrap();
    let bus1_function = PciFunctionAddress::new(1, 2, 0).unwrap();
    let bus3_function = PciFunctionAddress::new(3, 2, 0).unwrap();

    host.register_bridge(bridge_config(bridge_function))
        .unwrap();
    host.register_endpoint(storage_endpoint(bus1_function))
        .unwrap();
    host.register_endpoint(storage_endpoint(bus3_function))
        .unwrap();

    let bus1_addr = aperture
        .config_address(bus1_function, PciConfigOffset::new(0).unwrap())
        .unwrap();
    let bus3_addr = aperture
        .config_address(bus3_function, PciConfigOffset::new(0).unwrap())
        .unwrap();
    assert_eq!(
        host.read_config_address(bus1_addr, AccessSize::new(4).unwrap()),
        Ok(vec![0xf4, 0x1a, 0x01, 0x10])
    );
    assert_eq!(
        host.read_config_address(bus3_addr, AccessSize::new(4).unwrap()),
        Ok(vec![0xff, 0xff, 0xff, 0xff])
    );

    let bridge_bus_number_addr = aperture
        .config_address(bridge_function, PciConfigOffset::new(0x18).unwrap())
        .unwrap();
    host.write_config_address(bridge_bus_number_addr, &[0, 3, 3, 0])
        .unwrap();

    assert_eq!(
        host.read_config_address(bus1_addr, AccessSize::new(4).unwrap()),
        Ok(vec![0xff, 0xff, 0xff, 0xff])
    );
    assert_eq!(
        host.read_config_address(bus3_addr, AccessSize::new(4).unwrap()),
        Ok(vec![0xf4, 0x1a, 0x01, 0x10])
    );
}

#[test]
fn pci_host_filters_downstream_bar_ranges_through_type1_memory_window() {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 3).unwrap();
    let bases = PciHostAddressBases::new(
        Address::new(0x1000_0000),
        Address::new(0x8000_0000),
        Address::new(0xa000_0000),
    );
    let mut host = PciHostBridge::with_address_bases(aperture, bases);
    let bridge_function = PciFunctionAddress::new(0, 1, 0).unwrap();
    let endpoint_function = PciFunctionAddress::new(1, 2, 0).unwrap();

    host.register_bridge(bridge_config(bridge_function))
        .unwrap();
    host.register_endpoint(storage_endpoint(endpoint_function))
        .unwrap();

    let endpoint_bar_addr = aperture
        .config_address(endpoint_function, PciConfigOffset::new(0x10).unwrap())
        .unwrap();
    let endpoint_command_addr = aperture
        .config_address(endpoint_function, PciConfigOffset::new(0x04).unwrap())
        .unwrap();
    host.write_config_address(endpoint_bar_addr, &0x0020_1000_u32.to_le_bytes())
        .unwrap();
    host.write_config_address(endpoint_command_addr, &0x0002_u16.to_le_bytes())
        .unwrap();

    assert_eq!(host.active_host_bar_ranges(), Ok(Vec::new()));

    let bridge_memory_window_addr = aperture
        .config_address(bridge_function, PciConfigOffset::new(0x20).unwrap())
        .unwrap();
    host.write_config_address(bridge_memory_window_addr, &0x0020_0020_u32.to_le_bytes())
        .unwrap();
    assert_eq!(
        host.active_host_bar_ranges(),
        Ok(vec![PciHostBarRange::new(
            endpoint_function,
            PciBarIndex::new(0).unwrap(),
            PciHostAddressSpace::Memory,
            Address::new(0x0020_0000),
            Address::new(0x8020_0000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap()])
    );

    host.write_config_address(bridge_memory_window_addr, &0x0040_0040_u32.to_le_bytes())
        .unwrap();
    assert_eq!(host.active_host_bar_ranges(), Ok(Vec::new()));
}

#[test]
fn pci_host_filters_downstream_legacy_io_bar_ranges_through_type1_io_window() {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 3).unwrap();
    let bases = PciHostAddressBases::new(
        Address::new(0x1000_0000),
        Address::new(0x8000_0000),
        Address::new(0xa000_0000),
    );
    let mut host = PciHostBridge::with_address_bases(aperture, bases);
    let bridge_function = PciFunctionAddress::new(0, 1, 0).unwrap();
    let endpoint_function = PciFunctionAddress::new(1, 2, 0).unwrap();

    host.register_bridge(bridge_config(bridge_function))
        .unwrap();
    host.register_endpoint(legacy_io_endpoint(endpoint_function))
        .unwrap();

    let endpoint_command_addr = aperture
        .config_address(endpoint_function, PciConfigOffset::new(0x04).unwrap())
        .unwrap();
    host.write_config_address(endpoint_command_addr, &0x0001_u16.to_le_bytes())
        .unwrap();
    assert_eq!(host.active_host_bar_ranges(), Ok(Vec::new()));

    let bridge_io_base_addr = aperture
        .config_address(bridge_function, PciConfigOffset::new(0x1c).unwrap())
        .unwrap();
    let bridge_io_limit_addr = aperture
        .config_address(bridge_function, PciConfigOffset::new(0x1d).unwrap())
        .unwrap();
    host.write_config_address(bridge_io_base_addr, &[0x30])
        .unwrap();
    host.write_config_address(bridge_io_limit_addr, &[0x30])
        .unwrap();
    assert_eq!(
        host.active_host_bar_ranges(),
        Ok(vec![PciHostBarRange::new(
            endpoint_function,
            PciBarIndex::new(0).unwrap(),
            PciHostAddressSpace::Io,
            Address::new(0x3000),
            Address::new(0x1000_3000),
            AccessSize::new(0x10).unwrap(),
        )
        .unwrap()])
    );

    host.write_config_address(bridge_io_base_addr, &[0x40])
        .unwrap();
    host.write_config_address(bridge_io_limit_addr, &[0x40])
        .unwrap();
    assert_eq!(host.active_host_bar_ranges(), Ok(Vec::new()));
}

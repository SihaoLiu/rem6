use rem6_interrupt::InterruptLineId;
use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarRange, PciBarSpec, PciClassCode, PciConfigOffset,
    PciDeviceIdentity, PciEndpointConfig, PciError, PciFunctionAddress, PciInterruptPin,
    PciPowerManagementCapabilitySpec, PciType0HeaderFields,
};

fn network_endpoint() -> PciEndpointConfig {
    PciEndpointConfig::new(
        PciFunctionAddress::new(0, 3, 0).unwrap(),
        PciDeviceIdentity::new(0x1234, 0xabcd),
        PciClassCode::new(0x02, 0x00, 0x01, 0x07),
    )
    .with_interrupt(11, PciInterruptPin::IntA)
}

#[test]
fn pci_endpoint_config_exposes_type0_identity_and_rejects_read_only_writes() {
    let mut endpoint = network_endpoint();

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x00).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x34, 0x12, 0xcd, 0xab])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x08).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x07, 0x01, 0x00, 0x02])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x0e).unwrap(),
            AccessSize::new(1).unwrap()
        ),
        Ok(vec![0x00])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x3c).unwrap(),
            AccessSize::new(2).unwrap()
        ),
        Ok(vec![11, 1])
    );

    assert_eq!(
        endpoint.write_config(PciConfigOffset::new(0x00).unwrap(), &[0x00, 0x00]),
        Err(PciError::ReadOnlyConfigWrite {
            offset: PciConfigOffset::new(0x00).unwrap(),
            size: AccessSize::new(2).unwrap(),
        })
    );
}

#[test]
fn pci_endpoint_config_exposes_typed_legacy_interrupt_path() {
    let endpoint = network_endpoint();

    assert_eq!(endpoint.legacy_interrupt_line(), 11);
    assert_eq!(endpoint.legacy_interrupt_pin(), Ok(PciInterruptPin::IntA));
    let path = endpoint.legacy_interrupt_path().unwrap();
    assert_eq!(path.endpoint_function(), endpoint.function());
    assert_eq!(path.endpoint_pin(), PciInterruptPin::IntA);
    assert_eq!(path.root_function(), endpoint.function());
    assert_eq!(path.root_pin(), PciInterruptPin::IntA);
    assert!(path.upstream_bridges().is_empty());

    let mut reassigned = endpoint.clone();
    reassigned
        .write_config(PciConfigOffset::new(0x3c).unwrap(), &[17])
        .unwrap();
    assert_eq!(reassigned.legacy_interrupt_line(), 17);
    assert_eq!(reassigned.legacy_interrupt_pin(), Ok(PciInterruptPin::IntA));
    reassigned
        .assign_legacy_interrupt_line(InterruptLineId::new(48))
        .unwrap();
    assert_eq!(reassigned.legacy_interrupt_line(), 48);
    assert_eq!(
        reassigned.read_config(
            PciConfigOffset::new(0x3c).unwrap(),
            AccessSize::new(1).unwrap()
        ),
        Ok(vec![48])
    );
    assert_eq!(
        reassigned.assign_legacy_interrupt_line(InterruptLineId::new(256)),
        Err(PciError::LegacyInterruptConfigLineOverflow {
            line: InterruptLineId::new(256),
        })
    );

    let no_pin = PciEndpointConfig::new(
        PciFunctionAddress::new(0, 4, 0).unwrap(),
        PciDeviceIdentity::new(0x1234, 0xabcd),
        PciClassCode::new(0x02, 0x00, 0x01, 0x07),
    );
    assert_eq!(no_pin.legacy_interrupt_pin(), Ok(PciInterruptPin::None));
    assert_eq!(
        no_pin.legacy_interrupt_path(),
        Err(PciError::MissingLegacyInterruptPin {
            function: no_pin.function(),
        })
    );
    assert_eq!(
        PciInterruptPin::from_config_value(5),
        Err(PciError::InvalidLegacyInterruptPinValue { value: 5 })
    );
}

#[test]
fn pci_endpoint_type0_header_exposes_subsystem_rom_and_latency_fields() {
    let mut endpoint = network_endpoint().with_type0_header(PciType0HeaderFields::new(
        0x1122_3344,
        0x1af4,
        0x1001,
        0x8000_0001,
        3,
        9,
    ));

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x28).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x44, 0x33, 0x22, 0x11])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x2c).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0xf4, 0x1a, 0x01, 0x10])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x30).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x01, 0x00, 0x00, 0x80])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x3e).unwrap(),
            AccessSize::new(2).unwrap()
        ),
        Ok(vec![3, 9])
    );

    endpoint
        .write_u32(PciConfigOffset::new(0x30).unwrap(), 0x9000_0001)
        .unwrap();
    assert_eq!(
        endpoint.read_u32(PciConfigOffset::new(0x30).unwrap()),
        Ok(0x9000_0001)
    );
    endpoint
        .write_u32(PciConfigOffset::new(0x30).unwrap(), 0xffff_fffe)
        .unwrap();
    assert_eq!(
        endpoint.read_u32(PciConfigOffset::new(0x30).unwrap()),
        Ok(0xffff_ffff)
    );
    endpoint
        .write_config(PciConfigOffset::new(0x3e).unwrap(), &[0xaa])
        .unwrap();
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x3e).unwrap(),
            AccessSize::new(2).unwrap()
        ),
        Ok(vec![3, 9])
    );
}

#[test]
fn pci_endpoint_common_header_writes_cache_line_latency_and_snapshots() {
    let mut endpoint = network_endpoint();

    endpoint
        .write_config(PciConfigOffset::new(0x0c).unwrap(), &[0x40])
        .unwrap();
    endpoint
        .write_config(PciConfigOffset::new(0x0d).unwrap(), &[0x20])
        .unwrap();

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x0c).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x40, 0x20, 0x00, 0x00])
    );

    endpoint
        .write_config(
            PciConfigOffset::new(0x0c).unwrap(),
            &0x3322_u16.to_le_bytes(),
        )
        .unwrap();
    let snapshot = endpoint.snapshot();
    endpoint
        .write_config(
            PciConfigOffset::new(0x0c).unwrap(),
            &0x0000_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint.restore(&snapshot).unwrap();

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x0c).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x22, 0x33, 0x00, 0x00])
    );
    assert_eq!(
        endpoint.write_config(
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
fn pci_endpoint_common_header_writes_bist_byte_and_snapshots() {
    let mut endpoint = network_endpoint();

    endpoint
        .write_config(PciConfigOffset::new(0x0f).unwrap(), &[0x40])
        .unwrap();

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x0c).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x00, 0x00, 0x00, 0x40])
    );

    let snapshot = endpoint.snapshot();
    endpoint
        .write_config(PciConfigOffset::new(0x0f).unwrap(), &[0x00])
        .unwrap();
    endpoint.restore(&snapshot).unwrap();

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x0c).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x00, 0x00, 0x00, 0x40])
    );
    assert_eq!(
        endpoint.write_config(PciConfigOffset::new(0x0e).unwrap(), &[0x01, 0x80]),
        Err(PciError::ReadOnlyConfigWrite {
            offset: PciConfigOffset::new(0x0e).unwrap(),
            size: AccessSize::new(2).unwrap(),
        })
    );
}

#[test]
fn pci_endpoint_command_writes_mask_reserved_bits() {
    let mut endpoint = network_endpoint();

    endpoint
        .write_config(
            PciConfigOffset::new(0x04).unwrap(),
            &0xffff_u16.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x04).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0xff, 0x03, 0x00, 0x00])
    );

    endpoint
        .write_config(
            PciConfigOffset::new(0x04).unwrap(),
            &0xffff_0002_u32.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x04).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x02, 0x00, 0x00, 0x00])
    );
}

#[test]
fn pci_endpoint_status_writes_preserve_read_only_capability_list_bit() {
    let mut endpoint = network_endpoint();
    endpoint
        .install_pm_capability(
            PciPowerManagementCapabilitySpec::new(
                PciConfigOffset::new(0x44).unwrap(),
                0x0003,
                0x0000,
            )
            .unwrap(),
        )
        .unwrap();

    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x04).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x00, 0x00, 0x10, 0x00])
    );

    endpoint
        .write_config(
            PciConfigOffset::new(0x06).unwrap(),
            &0xffff_u16.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x04).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x00, 0x00, 0x10, 0x00])
    );

    endpoint
        .write_config(
            PciConfigOffset::new(0x04).unwrap(),
            &0xffff_0003_u32.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x04).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
        Ok(vec![0x03, 0x00, 0x10, 0x00])
    );
}

#[test]
fn pci_endpoint_bar_writes_apply_size_masks_and_enable_ranges() {
    let mut endpoint = network_endpoint();
    endpoint
        .install_bar(
            PciBarSpec::new(
                PciBarIndex::new(0).unwrap(),
                PciBarKind::Memory32 { prefetchable: true },
                AccessSize::new(0x1000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

    assert_eq!(
        endpoint.read_u32(PciConfigOffset::new(0x10).unwrap()),
        Ok(0x8)
    );
    endpoint
        .write_u32(PciConfigOffset::new(0x10).unwrap(), 0xffff_ffff)
        .unwrap();
    assert_eq!(
        endpoint.read_u32(PciConfigOffset::new(0x10).unwrap()),
        Ok(0xffff_f008)
    );

    endpoint
        .write_u32(PciConfigOffset::new(0x10).unwrap(), 0x8000_1234)
        .unwrap();
    assert_eq!(
        endpoint.read_u32(PciConfigOffset::new(0x10).unwrap()),
        Ok(0x8000_1008)
    );
    assert_eq!(endpoint.active_bar_ranges(), Vec::new());

    endpoint
        .write_config(
            PciConfigOffset::new(0x04).unwrap(),
            &0x0002_u16.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        endpoint.active_bar_ranges(),
        vec![PciBarRange::new(
            PciBarIndex::new(0).unwrap(),
            PciBarKind::Memory32 { prefetchable: true },
            Address::new(0x8000_1000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap()]
    );
}

#[test]
fn pci_endpoint_legacy_io_bar_ignores_writes_and_uses_fixed_range() {
    let mut endpoint = network_endpoint();
    endpoint
        .install_bar(
            PciBarSpec::new(
                PciBarIndex::new(0).unwrap(),
                PciBarKind::LegacyIo {
                    address: Address::new(0x03f8),
                },
                AccessSize::new(8).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

    assert_eq!(
        endpoint.read_u32(PciConfigOffset::new(0x10).unwrap()),
        Ok(0)
    );
    endpoint
        .write_u32(PciConfigOffset::new(0x10).unwrap(), 0xffff_ffff)
        .unwrap();
    assert_eq!(
        endpoint.read_u32(PciConfigOffset::new(0x10).unwrap()),
        Ok(0)
    );
    assert_eq!(endpoint.active_bar_ranges(), Vec::new());

    endpoint
        .write_config(
            PciConfigOffset::new(0x04).unwrap(),
            &0x0001_u16.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        endpoint.active_bar_ranges(),
        vec![PciBarRange::new(
            PciBarIndex::new(0).unwrap(),
            PciBarKind::LegacyIo {
                address: Address::new(0x03f8),
            },
            Address::new(0x03f8),
            AccessSize::new(8).unwrap(),
        )
        .unwrap()]
    );
}

#[test]
fn pci_endpoint_memory64_bar_writes_lower_and_upper_config_dwords() {
    let mut endpoint = network_endpoint();
    endpoint
        .install_bar(
            PciBarSpec::new(
                PciBarIndex::new(0).unwrap(),
                PciBarKind::Memory64 { prefetchable: true },
                AccessSize::new(0x2000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

    assert_eq!(
        endpoint.read_u32(PciConfigOffset::new(0x10).unwrap()),
        Ok(0x0c)
    );
    assert_eq!(
        endpoint.read_u32(PciConfigOffset::new(0x14).unwrap()),
        Ok(0)
    );

    endpoint
        .write_u32(PciConfigOffset::new(0x10).unwrap(), 0x0000_2345)
        .unwrap();
    endpoint
        .write_u32(PciConfigOffset::new(0x14).unwrap(), 0x0000_0001)
        .unwrap();

    assert_eq!(
        endpoint.read_u32(PciConfigOffset::new(0x10).unwrap()),
        Ok(0x0000_200c)
    );
    assert_eq!(
        endpoint.read_u32(PciConfigOffset::new(0x14).unwrap()),
        Ok(0x0000_0001)
    );
}

#[test]
fn pci_endpoint_memory64_bar_consumes_upper_slot_and_rejects_bad_pairing() {
    let mut endpoint = network_endpoint();
    endpoint
        .install_bar(
            PciBarSpec::new(
                PciBarIndex::new(0).unwrap(),
                PciBarKind::Memory64 {
                    prefetchable: false,
                },
                AccessSize::new(0x1000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

    assert_eq!(
        endpoint.install_bar(
            PciBarSpec::new(
                PciBarIndex::new(1).unwrap(),
                PciBarKind::Io,
                AccessSize::new(0x100).unwrap(),
            )
            .unwrap(),
        ),
        Err(PciError::ReservedBar {
            index: PciBarIndex::new(1).unwrap(),
            owner: PciBarIndex::new(0).unwrap(),
        })
    );
    assert_eq!(
        PciBarSpec::new(
            PciBarIndex::new(5).unwrap(),
            PciBarKind::Memory64 {
                prefetchable: false,
            },
            AccessSize::new(0x1000).unwrap(),
        ),
        Err(PciError::InvalidBarPair {
            index: PciBarIndex::new(5).unwrap(),
        })
    );
}

#[test]
fn pci_endpoint_memory64_bar_enables_single_active_range() {
    let mut endpoint = network_endpoint();
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

    endpoint
        .write_u32(PciConfigOffset::new(0x18).unwrap(), 0x0000_2345)
        .unwrap();
    endpoint
        .write_u32(PciConfigOffset::new(0x1c).unwrap(), 0x0000_0001)
        .unwrap();
    assert_eq!(endpoint.active_bar_ranges(), Vec::new());

    endpoint
        .write_config(
            PciConfigOffset::new(0x04).unwrap(),
            &0x0002_u16.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        endpoint.active_bar_ranges(),
        vec![PciBarRange::new(
            PciBarIndex::new(2).unwrap(),
            PciBarKind::Memory64 {
                prefetchable: false,
            },
            Address::new(0x1_0000_2000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap()]
    );
}

#[test]
fn pci_endpoint_rejects_oversized_32_bit_bar() {
    assert_eq!(
        PciBarSpec::new(
            PciBarIndex::new(0).unwrap(),
            PciBarKind::Memory64 {
                prefetchable: false,
            },
            AccessSize::new(0x1_0000_0000).unwrap(),
        )
        .unwrap()
        .size(),
        AccessSize::new(0x1_0000_0000).unwrap()
    );
    assert_eq!(
        PciBarSpec::new(
            PciBarIndex::new(0).unwrap(),
            PciBarKind::Memory32 {
                prefetchable: false,
            },
            AccessSize::new(0x1_0000_0000).unwrap(),
        ),
        Err(PciError::InvalidBarSize {
            index: PciBarIndex::new(0).unwrap(),
            kind: PciBarKind::Memory32 {
                prefetchable: false,
            },
            size: AccessSize::new(0x1_0000_0000).unwrap(),
        })
    );
}

#[test]
fn pci_endpoint_snapshot_restore_preserves_command_and_bar_state() {
    let mut endpoint = network_endpoint();
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
        .write_u32(PciConfigOffset::new(0x18).unwrap(), 0x0000_c123)
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0x04).unwrap(),
            &0x0001_u16.to_le_bytes(),
        )
        .unwrap();
    let snapshot = endpoint.snapshot();

    endpoint
        .write_u32(PciConfigOffset::new(0x18).unwrap(), 0x0000_d123)
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0x04).unwrap(),
            &0x0000_u16.to_le_bytes(),
        )
        .unwrap();

    endpoint.restore(&snapshot).unwrap();
    assert_eq!(
        endpoint.active_bar_ranges(),
        vec![PciBarRange::new(
            PciBarIndex::new(2).unwrap(),
            PciBarKind::Io,
            Address::new(0x0000_c100),
            AccessSize::new(0x100).unwrap(),
        )
        .unwrap()]
    );

    let mut other = PciEndpointConfig::new(
        PciFunctionAddress::new(0, 4, 0).unwrap(),
        PciDeviceIdentity::new(0x1234, 0xabcd),
        PciClassCode::new(0x02, 0x00, 0x01, 0x07),
    );
    assert_eq!(
        other.restore(&snapshot),
        Err(PciError::SnapshotFunctionMismatch {
            expected: PciFunctionAddress::new(0, 4, 0).unwrap(),
            actual: PciFunctionAddress::new(0, 3, 0).unwrap(),
        })
    );
}

#[test]
fn pci_endpoint_snapshot_exposes_bar_payloads_for_checkpoint_audit() {
    let mut endpoint = network_endpoint();
    endpoint
        .install_bar(
            PciBarSpec::new(
                PciBarIndex::new(0).unwrap(),
                PciBarKind::Io,
                AccessSize::new(0x100).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    endpoint
        .install_bar(
            PciBarSpec::new(
                PciBarIndex::new(2).unwrap(),
                PciBarKind::Memory64 { prefetchable: true },
                AccessSize::new(0x2000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    endpoint
        .write_u32(PciConfigOffset::new(0x10).unwrap(), 0x0000_c123)
        .unwrap();
    endpoint
        .write_u32(PciConfigOffset::new(0x18).unwrap(), 0x0000_2345)
        .unwrap();
    endpoint
        .write_u32(PciConfigOffset::new(0x1c).unwrap(), 0x0000_0001)
        .unwrap();
    let snapshot = endpoint.snapshot();

    let payloads = snapshot.bar_payloads();

    assert_eq!(payloads.len(), 6);
    assert!(payloads[0].is_some());
    assert_eq!(payloads[1], None);
    assert!(payloads[2].is_some());
    assert!(payloads[3].is_some());
    assert_eq!(snapshot.validate_bar_payloads(&payloads), Ok(()));

    let no_bars = network_endpoint().snapshot();
    assert_eq!(
        no_bars.validate_bar_payloads(&payloads),
        Err(PciError::SnapshotBarMismatch {
            index: PciBarIndex::new(0).unwrap(),
        })
    );

    let different_state = {
        let mut endpoint = network_endpoint();
        endpoint
            .install_bar(
                PciBarSpec::new(
                    PciBarIndex::new(0).unwrap(),
                    PciBarKind::Io,
                    AccessSize::new(0x100).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
        endpoint
            .install_bar(
                PciBarSpec::new(
                    PciBarIndex::new(2).unwrap(),
                    PciBarKind::Memory64 { prefetchable: true },
                    AccessSize::new(0x2000).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
        endpoint
            .write_u32(PciConfigOffset::new(0x10).unwrap(), 0x0000_c123)
            .unwrap();
        endpoint
            .write_u32(PciConfigOffset::new(0x18).unwrap(), 0x0000_4000)
            .unwrap();
        endpoint.snapshot()
    };
    assert_eq!(
        different_state.validate_bar_payloads(&payloads),
        Err(PciError::SnapshotBarMismatch {
            index: PciBarIndex::new(2).unwrap(),
        })
    );

    let mut corrupted = payloads;
    corrupted[2].as_mut().unwrap().push(0);
    assert_eq!(
        snapshot.validate_bar_payloads(&corrupted),
        Err(PciError::InvalidBarSnapshot)
    );
}

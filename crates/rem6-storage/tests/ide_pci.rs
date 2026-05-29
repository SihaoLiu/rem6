use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarRange, PciClassCode, PciConfigOffset, PciDeviceIdentity,
    PciFunctionAddress, PciInterruptPin,
};
use rem6_storage::{
    IdePciEndpointSpec, IDE_PCI_BUS_MASTER_BAR_BYTES, IDE_PCI_COMMAND_BAR_BYTES,
    IDE_PCI_CONTROL_BAR_BYTES, IDE_PCI_DEVICE_ID, IDE_PCI_INTERRUPT_LINE, IDE_PCI_MAX_BAR_INDEX,
    IDE_PCI_PROG_IF, IDE_PCI_STATUS, IDE_PCI_VENDOR_ID,
};

fn function() -> PciFunctionAddress {
    PciFunctionAddress::new(0, 4, 0).unwrap()
}

#[test]
fn ide_pci_endpoint_matches_gem5_piix4_identity_and_bar_shape() {
    let spec = IdePciEndpointSpec::new(function());

    assert_eq!(spec.function(), function());
    assert_eq!(
        spec.identity(),
        PciDeviceIdentity::new(IDE_PCI_VENDOR_ID, IDE_PCI_DEVICE_ID)
    );
    assert_eq!(
        spec.class(),
        PciClassCode::new(0x01, 0x01, IDE_PCI_PROG_IF, 0)
    );
    assert_eq!(spec.status(), IDE_PCI_STATUS);
    assert_eq!(spec.interrupt_line(), IDE_PCI_INTERRUPT_LINE);
    assert_eq!(spec.interrupt_pin(), PciInterruptPin::IntA);
    assert_eq!(spec.io_shift(), 0);
    assert_eq!(spec.control_offset(), 0);
    assert_eq!(spec.primary_command_bar(), PciBarIndex::new(0).unwrap());
    assert_eq!(spec.primary_control_bar(), PciBarIndex::new(1).unwrap());
    assert_eq!(spec.secondary_command_bar(), PciBarIndex::new(2).unwrap());
    assert_eq!(spec.secondary_control_bar(), PciBarIndex::new(3).unwrap());
    assert_eq!(spec.bus_master_bar(), PciBarIndex::new(4).unwrap());
    assert_eq!(
        spec.max_bar_index(),
        PciBarIndex::new(IDE_PCI_MAX_BAR_INDEX).unwrap()
    );

    let mut endpoint = spec.build_endpoint().unwrap();
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x00).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x86, 0x80, 0x11, 0x71])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x04).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x00, 0x00, 0x80, 0x02])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x08).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x00, IDE_PCI_PROG_IF, 0x01, 0x01])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x3c).unwrap(),
            AccessSize::new(2).unwrap()
        ),
        Ok(vec![IDE_PCI_INTERRUPT_LINE, 1])
    );

    endpoint
        .write_config(
            PciConfigOffset::new(0x04).unwrap(),
            &0x0001_u16.to_le_bytes(),
        )
        .unwrap();
    assert_bar(
        &mut endpoint,
        spec.primary_command_bar(),
        IDE_PCI_COMMAND_BAR_BYTES,
        0xffff_fff9,
        0x1000,
    );
    assert_bar(
        &mut endpoint,
        spec.primary_control_bar(),
        IDE_PCI_CONTROL_BAR_BYTES,
        0xffff_fffd,
        0x2000,
    );
    assert_bar(
        &mut endpoint,
        spec.secondary_command_bar(),
        IDE_PCI_COMMAND_BAR_BYTES,
        0xffff_fff9,
        0x3000,
    );
    assert_bar(
        &mut endpoint,
        spec.secondary_control_bar(),
        IDE_PCI_CONTROL_BAR_BYTES,
        0xffff_fffd,
        0x4000,
    );
    assert_bar(
        &mut endpoint,
        spec.bus_master_bar(),
        IDE_PCI_BUS_MASTER_BAR_BYTES,
        0xffff_fff1,
        0x5000,
    );
    assert_eq!(
        endpoint.active_bar_ranges(),
        vec![
            PciBarRange::new(
                spec.primary_command_bar(),
                PciBarKind::Io,
                Address::new(0x1000),
                AccessSize::new(IDE_PCI_COMMAND_BAR_BYTES).unwrap(),
            )
            .unwrap(),
            PciBarRange::new(
                spec.primary_control_bar(),
                PciBarKind::Io,
                Address::new(0x2000),
                AccessSize::new(IDE_PCI_CONTROL_BAR_BYTES).unwrap(),
            )
            .unwrap(),
            PciBarRange::new(
                spec.secondary_command_bar(),
                PciBarKind::Io,
                Address::new(0x3000),
                AccessSize::new(IDE_PCI_COMMAND_BAR_BYTES).unwrap(),
            )
            .unwrap(),
            PciBarRange::new(
                spec.secondary_control_bar(),
                PciBarKind::Io,
                Address::new(0x4000),
                AccessSize::new(IDE_PCI_CONTROL_BAR_BYTES).unwrap(),
            )
            .unwrap(),
            PciBarRange::new(
                spec.bus_master_bar(),
                PciBarKind::Io,
                Address::new(0x5000),
                AccessSize::new(IDE_PCI_BUS_MASTER_BAR_BYTES).unwrap(),
            )
            .unwrap(),
        ]
    );
}

#[test]
fn ide_pci_endpoint_builds_dispatch_from_spec_layout_policy() {
    let spec = IdePciEndpointSpec::new(function())
        .with_io_shift(1)
        .unwrap()
        .with_control_offset(2);

    assert_eq!(spec.io_shift(), 1);
    assert_eq!(spec.control_offset(), 2);
    assert_eq!(
        spec.dispatch(false).unwrap(),
        rem6_storage::IdeControllerDispatch::new(1, 2).unwrap()
    );
    assert_eq!(
        spec.dispatch(true).unwrap(),
        rem6_storage::IdeControllerDispatch::new(1, 2)
            .unwrap()
            .with_bus_master_enabled(true)
    );

    assert!(IdePciEndpointSpec::new(function())
        .with_io_shift(8)
        .is_err());
}

fn assert_bar(
    endpoint: &mut rem6_pci::PciEndpointConfig,
    index: PciBarIndex,
    expected_size: u64,
    expected_probe: u32,
    base: u32,
) {
    let offset = PciConfigOffset::new(0x10 + u16::from(index.get()) * 4).unwrap();
    assert_eq!(endpoint.read_u32(offset), Ok(0x1));
    endpoint.write_u32(offset, 0xffff_ffff).unwrap();
    assert_eq!(endpoint.read_u32(offset), Ok(expected_probe));
    endpoint.write_u32(offset, base).unwrap();
    assert_eq!(endpoint.read_u32(offset), Ok(base | 0x1));
    assert_eq!(
        AccessSize::new(expected_size).unwrap().bytes(),
        expected_size
    );
}

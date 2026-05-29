use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptEvent, InterruptEventKind, InterruptLineId, InterruptRoute,
    InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarRange, PciClassCode, PciConfigOffset, PciDeviceIdentity,
    PciFunctionAddress, PciInterruptPin, PciLegacyInterruptPort, PciLegacyInterruptRoute,
};
use rem6_storage::{
    IdeChannelId, IdeController, IdeDeviceId, IdeDisk, IdePciEndpointSpec, RawStorageImage,
    StorageImageLayer, IDE_COMMAND_IDENTIFY, IDE_PCI_BUS_MASTER_BAR_BYTES,
    IDE_PCI_COMMAND_BAR_BYTES, IDE_PCI_CONTROL_BAR_BYTES, IDE_PCI_DEVICE_ID,
    IDE_PCI_INTERRUPT_LINE, IDE_PCI_MAX_BAR_INDEX, IDE_PCI_PROG_IF, IDE_PCI_STATUS,
    IDE_PCI_VENDOR_ID, IDE_STATUS_DRDY, IDE_STATUS_DRQ, IDE_STATUS_OFFSET,
};

fn function() -> PciFunctionAddress {
    PciFunctionAddress::new(0, 4, 0).unwrap()
}

fn ide_source() -> InterruptSourceId {
    InterruptSourceId::new(u32::from(IDE_PCI_DEVICE_ID))
}

fn disk(seed: u8, device: IdeDeviceId) -> IdeDisk {
    let image = Arc::new(RawStorageImage::from_bytes(vec![seed; 512]).unwrap());
    IdeDisk::new(image as Arc<dyn StorageImageLayer>, device).unwrap()
}

fn legacy_interrupt_port(
    target: PartitionId,
    signal_latency: u64,
) -> (
    Arc<Mutex<InterruptController>>,
    PciLegacyInterruptPort,
    InterruptSourceId,
) {
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let route = PciLegacyInterruptRoute::new(
        function(),
        IdePciEndpointSpec::new(function()).interrupt_pin(),
        InterruptRoute::new(InterruptLineId::new(47), InterruptTargetId::new(0), target),
        signal_latency,
    )
    .unwrap();
    controller
        .lock()
        .unwrap()
        .register_route(route.interrupt_route())
        .unwrap();
    let port = PciLegacyInterruptPort::new(route, Arc::clone(&controller)).unwrap();
    let source = ide_source();
    (controller, port, source)
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

#[test]
fn ide_pci_interrupt_port_syncs_shared_intx_in_parallel() {
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let (interrupts, legacy_port, source) = legacy_interrupt_port(cpu, 2);
    let ide_port = IdePciEndpointSpec::new(function())
        .build_legacy_interrupt_port(legacy_port, source)
        .unwrap();
    let controller = Arc::new(Mutex::new(
        IdeController::new([Some(disk(0x5a, IdeDeviceId::Device0)), None, None, None]).unwrap(),
    ));

    let post_controller = Arc::clone(&controller);
    let post_port = ide_port.clone();
    let clear_controller = Arc::clone(&controller);
    let clear_port = ide_port.clone();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    scheduler
        .schedule_parallel_at(pci, 5, move |context| {
            let mut controller = post_controller.lock().unwrap();
            controller
                .write_command_u8(
                    IdeChannelId::Primary,
                    IDE_STATUS_OFFSET,
                    IDE_COMMAND_IDENTIFY,
                )
                .unwrap();
            assert!(post_port
                .sync_controller_parallel(context, &controller)
                .unwrap()
                .is_some());
            assert_eq!(
                post_port.sync_controller_parallel(context, &controller),
                Ok(None)
            );
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(pci, 12, move |context| {
            let mut controller = clear_controller.lock().unwrap();
            assert_eq!(
                controller.read_command_u8(IdeChannelId::Primary, IDE_STATUS_OFFSET),
                Ok(IDE_STATUS_DRDY | IDE_STATUS_DRQ)
            );
            assert!(clear_port
                .sync_controller_parallel(context, &controller)
                .unwrap()
                .is_some());
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        interrupts.lock().unwrap().history(),
        &[
            InterruptEvent::routed(
                7,
                InterruptLineId::new(47),
                InterruptTargetId::new(0),
                cpu,
                source,
                InterruptEventKind::Assert,
            ),
            InterruptEvent::routed(
                14,
                InterruptLineId::new(47),
                InterruptTargetId::new(0),
                cpu,
                source,
                InterruptEventKind::Deassert,
            ),
        ]
    );
    assert!(!ide_port.line_asserted());
    assert!(ide_port.delivery_errors().lock().unwrap().is_empty());
}

#[test]
fn ide_pci_interrupt_port_syncs_shared_intx_in_serial() {
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let (interrupts, legacy_port, source) = legacy_interrupt_port(cpu, 2);
    let ide_port = IdePciEndpointSpec::new(function())
        .build_legacy_interrupt_port(legacy_port, source)
        .unwrap();
    let controller = Arc::new(Mutex::new(
        IdeController::new([Some(disk(0x3c, IdeDeviceId::Device0)), None, None, None]).unwrap(),
    ));

    let post_controller = Arc::clone(&controller);
    let post_port = ide_port.clone();
    let clear_controller = Arc::clone(&controller);
    let clear_port = ide_port.clone();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    scheduler
        .schedule_at(pci, 3, move |context| {
            let mut controller = post_controller.lock().unwrap();
            controller
                .write_command_u8(
                    IdeChannelId::Primary,
                    IDE_STATUS_OFFSET,
                    IDE_COMMAND_IDENTIFY,
                )
                .unwrap();
            assert!(post_port
                .sync_controller(context, &controller)
                .unwrap()
                .is_some());
        })
        .unwrap();
    scheduler
        .schedule_at(pci, 8, move |context| {
            let mut controller = clear_controller.lock().unwrap();
            assert_eq!(
                controller.read_command_u8(IdeChannelId::Primary, IDE_STATUS_OFFSET),
                Ok(IDE_STATUS_DRDY | IDE_STATUS_DRQ)
            );
            assert!(clear_port
                .sync_controller(context, &controller)
                .unwrap()
                .is_some());
        })
        .unwrap();

    scheduler.run_until_idle();

    assert_eq!(
        interrupts.lock().unwrap().history(),
        &[
            InterruptEvent::routed(
                5,
                InterruptLineId::new(47),
                InterruptTargetId::new(0),
                cpu,
                source,
                InterruptEventKind::Assert,
            ),
            InterruptEvent::routed(
                10,
                InterruptLineId::new(47),
                InterruptTargetId::new(0),
                cpu,
                source,
                InterruptEventKind::Deassert,
            ),
        ]
    );
    assert!(!ide_port.line_asserted());
    assert!(ide_port.delivery_errors().lock().unwrap().is_empty());
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

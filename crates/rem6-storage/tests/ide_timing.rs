use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptEvent, InterruptEventKind, InterruptLineId, InterruptRoute,
    InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_pci::{
    PciFunctionAddress, PciInterruptPin, PciLegacyInterruptPort, PciLegacyInterruptRoute,
};
use rem6_storage::{
    IdeChannelId, IdeController, IdeControllerTimingPort, IdeDeviceId, IdeDisk, IdePciEndpointSpec,
    RawStorageImage, StorageImageLayer, IDE_COMMAND_READ, IDE_DRIVE_LBA, IDE_DRIVE_OFFSET,
    IDE_HCYL_OFFSET, IDE_LCYL_OFFSET, IDE_NSECTOR_OFFSET, IDE_PCI_DEVICE_ID, IDE_SECTOR_OFFSET,
    IDE_STATUS_BSY, IDE_STATUS_DRDY, IDE_STATUS_DRQ, IDE_STATUS_OFFSET,
};

fn function() -> PciFunctionAddress {
    PciFunctionAddress::new(0, 4, 0).unwrap()
}

fn disk(byte: u8, device_id: IdeDeviceId) -> IdeDisk {
    let image = Arc::new(RawStorageImage::from_bytes(vec![byte; 512]).unwrap());
    IdeDisk::new(image as Arc<dyn StorageImageLayer>, device_id).unwrap()
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
        PciInterruptPin::IntA,
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
    let source = InterruptSourceId::new(u32::from(IDE_PCI_DEVICE_ID));
    (controller, port, source)
}

fn issue_lba_read_registers(controller: &mut IdeController) {
    controller
        .write_command_u8(IdeChannelId::Primary, IDE_DRIVE_OFFSET, IDE_DRIVE_LBA)
        .unwrap();
    controller
        .write_command_u8(IdeChannelId::Primary, IDE_NSECTOR_OFFSET, 1)
        .unwrap();
    controller
        .write_command_u8(IdeChannelId::Primary, IDE_SECTOR_OFFSET, 0)
        .unwrap();
    controller
        .write_command_u8(IdeChannelId::Primary, IDE_LCYL_OFFSET, 0)
        .unwrap();
    controller
        .write_command_u8(IdeChannelId::Primary, IDE_HCYL_OFFSET, 0)
        .unwrap();
}

fn read_sector(controller: &mut IdeController) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(512);
    for _ in 0..256 {
        bytes.extend_from_slice(
            &controller
                .read_data_u16(IdeChannelId::Primary)
                .unwrap()
                .to_le_bytes(),
        );
    }
    bytes
}

#[test]
fn ide_timing_port_delays_media_read_ready_and_interrupt_delivery_in_parallel() {
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let controller = Arc::new(Mutex::new(
        IdeController::new([Some(disk(0x7a, IdeDeviceId::Device0)), None, None, None]).unwrap(),
    ));
    issue_lba_read_registers(&mut controller.lock().unwrap());
    let (interrupts, legacy_port, source) = legacy_interrupt_port(cpu, 2);
    let ide_port = IdePciEndpointSpec::new(function())
        .build_legacy_interrupt_port(legacy_port, source)
        .unwrap();
    let timing = IdeControllerTimingPort::new(Arc::clone(&controller), 4)
        .unwrap()
        .with_interrupt_port(ide_port.clone());

    let issue_timing = timing.clone();
    let issue_controller = Arc::clone(&controller);
    let observe_controller = Arc::clone(&controller);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    scheduler
        .schedule_parallel_at(pci, 5, move |context| {
            assert!(issue_timing
                .write_command_u8_parallel(
                    context,
                    IdeChannelId::Primary,
                    IDE_STATUS_OFFSET,
                    IDE_COMMAND_READ,
                )
                .unwrap()
                .is_some());
            assert_eq!(
                issue_controller
                    .lock()
                    .unwrap()
                    .read_control_u8(IdeChannelId::Primary, rem6_storage::IDE_ALTSTAT_OFFSET),
                Ok(IDE_STATUS_DRDY | IDE_STATUS_BSY)
            );
            assert!(!issue_controller.lock().unwrap().shared_interrupt_asserted());
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(pci, 6, move |_| {
            assert_eq!(
                observe_controller
                    .lock()
                    .unwrap()
                    .read_control_u8(IdeChannelId::Primary, rem6_storage::IDE_ALTSTAT_OFFSET),
                Ok(IDE_STATUS_DRDY | IDE_STATUS_BSY)
            );
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        controller
            .lock()
            .unwrap()
            .read_control_u8(IdeChannelId::Primary, rem6_storage::IDE_ALTSTAT_OFFSET),
        Ok(IDE_STATUS_DRDY | IDE_STATUS_DRQ)
    );
    assert_eq!(
        read_sector(&mut controller.lock().unwrap()),
        vec![0x7a; 512]
    );
    assert_eq!(
        interrupts.lock().unwrap().history(),
        &[InterruptEvent::routed(
            11,
            InterruptLineId::new(47),
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
    assert!(timing.completion_errors().lock().unwrap().is_empty());
    assert!(ide_port.delivery_errors().lock().unwrap().is_empty());
}

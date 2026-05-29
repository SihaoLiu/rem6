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
    IdeChannelId, IdeController, IdeControllerError, IdeControllerGuestMemory,
    IdeControllerTimingPort, IdeDeviceId, IdeDisk, IdeDiskError, IdePciEndpointSpec,
    RawStorageImage, StorageImageLayer, StorageSectorId, IDE_BMI_COMMAND_OFFSET,
    IDE_BMI_COMMAND_RW, IDE_BMI_COMMAND_START, IDE_BMI_PRD_TABLE_OFFSET, IDE_BMI_STATUS_ACTIVE,
    IDE_BMI_STATUS_DMA_CAP0, IDE_BMI_STATUS_DMA_CAP1, IDE_BMI_STATUS_INTERRUPT,
    IDE_BMI_STATUS_OFFSET, IDE_COMMAND_READ, IDE_COMMAND_READ_DMA, IDE_COMMAND_WRITE,
    IDE_CONTROL_IEN, IDE_CONTROL_OFFSET, IDE_DRIVE_LBA, IDE_DRIVE_OFFSET, IDE_HCYL_OFFSET,
    IDE_LCYL_OFFSET, IDE_NSECTOR_OFFSET, IDE_PCI_DEVICE_ID, IDE_SECTOR_OFFSET, IDE_STATUS_BSY,
    IDE_STATUS_DRDY, IDE_STATUS_DRQ, IDE_STATUS_OFFSET,
};

fn function() -> PciFunctionAddress {
    PciFunctionAddress::new(0, 4, 0).unwrap()
}

fn disk(byte: u8, device_id: IdeDeviceId) -> IdeDisk {
    let image = Arc::new(RawStorageImage::from_bytes(vec![byte; 512]).unwrap());
    IdeDisk::new(image as Arc<dyn StorageImageLayer>, device_id).unwrap()
}

fn two_sector_disk(first: u8, second: u8, device_id: IdeDeviceId) -> IdeDisk {
    let mut bytes = vec![first; 512];
    bytes.extend(vec![second; 512]);
    let image = Arc::new(RawStorageImage::from_bytes(bytes).unwrap());
    IdeDisk::new(image as Arc<dyn StorageImageLayer>, device_id).unwrap()
}

fn two_sector_image(first: u8, second: u8) -> Arc<RawStorageImage> {
    let mut bytes = vec![first; 512];
    bytes.extend(vec![second; 512]);
    Arc::new(RawStorageImage::from_bytes(bytes).unwrap())
}

#[derive(Debug)]
struct GuestMemory {
    bytes: Vec<u8>,
}

impl GuestMemory {
    fn filled(bytes: usize, value: u8) -> Self {
        Self {
            bytes: vec![value; bytes],
        }
    }

    fn write_seed(&mut self, address: u64, data: &[u8]) {
        let address = address as usize;
        self.bytes[address..address + data.len()].copy_from_slice(data);
    }

    fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn range(
        &self,
        operation: &'static str,
        address: u64,
        bytes: u64,
    ) -> Result<std::ops::Range<usize>, IdeControllerError> {
        let end = address
            .checked_add(bytes)
            .ok_or(IdeControllerError::GuestAddressOverflow { address, bytes })?;
        if end > self.bytes.len() as u64 {
            return Err(IdeControllerError::GuestMemory {
                operation,
                address,
                bytes,
                capacity_bytes: self.bytes.len() as u64,
            });
        }
        Ok(address as usize..end as usize)
    }
}

impl IdeControllerGuestMemory for GuestMemory {
    fn validate_read(&self, address: u64, bytes: u64) -> Result<(), IdeControllerError> {
        self.range("read", address, bytes).map(|_| ())
    }

    fn validate_write(&self, address: u64, bytes: u64) -> Result<(), IdeControllerError> {
        self.range("write", address, bytes).map(|_| ())
    }

    fn read_bytes(&mut self, address: u64, bytes: u64) -> Result<Vec<u8>, IdeControllerError> {
        let range = self.range("read", address, bytes)?;
        Ok(self.bytes[range].to_vec())
    }

    fn write_bytes(&mut self, address: u64, data: &[u8]) -> Result<(), IdeControllerError> {
        let range = self.range("write", address, data.len() as u64)?;
        self.bytes[range].copy_from_slice(data);
        Ok(())
    }
}

fn write_prd(guest: &mut GuestMemory, table: u64, base: u32, byte_count: u16, end: bool) {
    let mut entry = [0_u8; 8];
    entry[0..4].copy_from_slice(&base.to_le_bytes());
    entry[4..6].copy_from_slice(&byte_count.to_le_bytes());
    entry[6..8].copy_from_slice(&(if end { 0x8000_u16 } else { 0 }).to_le_bytes());
    guest.write_seed(table, &entry);
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
    issue_lba_read_registers_with_sectors(controller, 1);
}

fn issue_lba_read_registers_with_sectors(controller: &mut IdeController, sectors: u8) {
    controller
        .write_command_u8(IdeChannelId::Primary, IDE_DRIVE_OFFSET, IDE_DRIVE_LBA)
        .unwrap();
    controller
        .write_command_u8(IdeChannelId::Primary, IDE_NSECTOR_OFFSET, sectors)
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

fn assert_data_read_not_ready(error: IdeControllerError) {
    assert!(matches!(
        error,
        IdeControllerError::Disk {
            source: IdeDiskError::DataReadNotReady,
            ..
        }
    ));
}

fn assert_data_write_not_ready(error: IdeControllerError) {
    assert!(matches!(
        error,
        IdeControllerError::Disk {
            source: IdeDiskError::DataWriteNotReady,
            ..
        }
    ));
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

fn sector(byte: u8) -> [u8; 512] {
    [byte; 512]
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

#[test]
fn ide_timing_port_delays_next_pio_read_sector_in_parallel() {
    let pci = PartitionId::new(1);
    let controller = Arc::new(Mutex::new(
        IdeController::new([
            Some(two_sector_disk(0x11, 0x22, IdeDeviceId::Device0)),
            None,
            None,
            None,
        ])
        .unwrap(),
    ));
    {
        let mut controller = controller.lock().unwrap();
        issue_lba_read_registers_with_sectors(&mut controller, 2);
        controller
            .write_control_u8(IdeChannelId::Primary, IDE_CONTROL_OFFSET, IDE_CONTROL_IEN)
            .unwrap();
    }
    let timing = IdeControllerTimingPort::new(Arc::clone(&controller), 3).unwrap();
    let issue_timing = timing.clone();
    let read_timing = timing.clone();
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
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(pci, 9, move |context| {
            for word in 0..256 {
                let read = read_timing
                    .read_data_u16_parallel(context, IdeChannelId::Primary)
                    .unwrap();
                assert_eq!(read.word(), 0x1111);
                if word == 255 {
                    assert!(read.completion_event().is_some());
                } else {
                    assert!(read.completion_event().is_none());
                }
            }
            assert_eq!(
                read_timing
                    .controller()
                    .lock()
                    .unwrap()
                    .read_control_u8(IdeChannelId::Primary, rem6_storage::IDE_ALTSTAT_OFFSET),
                Ok(IDE_STATUS_DRDY | IDE_STATUS_BSY)
            );
            assert_data_read_not_ready(
                read_timing
                    .read_data_u16_parallel(context, IdeChannelId::Primary)
                    .unwrap_err(),
            );
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(pci, 10, move |_| {
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
        controller
            .lock()
            .unwrap()
            .read_data_u16(IdeChannelId::Primary),
        Ok(0x2222)
    );
    assert!(timing.completion_errors().lock().unwrap().is_empty());
}

#[test]
fn ide_timing_port_delays_dma_read_until_media_latency_elapsed_in_parallel() {
    let pci = PartitionId::new(1);
    let controller = Arc::new(Mutex::new(
        IdeController::new([Some(disk(0x5a, IdeDeviceId::Device0)), None, None, None]).unwrap(),
    ));
    let guest = Arc::new(Mutex::new(GuestMemory::filled(4096, 0xee)));
    write_prd(&mut guest.lock().unwrap(), 0x80, 0x200, 512, true);
    let timing = IdeControllerTimingPort::new(Arc::clone(&controller), 4).unwrap();

    let issue_timing = timing.clone();
    let issue_controller = Arc::clone(&controller);
    let dma_timing = timing.clone();
    let dma_controller = Arc::clone(&controller);
    let dma_guest = Arc::clone(&guest);
    let observe_controller = Arc::clone(&controller);
    let observe_guest = Arc::clone(&guest);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    scheduler
        .schedule_parallel_at(pci, 5, move |context| {
            let mut controller = issue_controller.lock().unwrap();
            controller
                .write_bmi_u32(IdeChannelId::Primary, IDE_BMI_PRD_TABLE_OFFSET, 0x80)
                .unwrap();
            issue_lba_read_registers(&mut controller);
            drop(controller);
            assert!(issue_timing
                .write_command_u8_parallel(
                    context,
                    IdeChannelId::Primary,
                    IDE_STATUS_OFFSET,
                    IDE_COMMAND_READ_DMA,
                )
                .unwrap()
                .is_some());
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(pci, 10, move |context| {
            dma_controller
                .lock()
                .unwrap()
                .write_bmi_u8(
                    IdeChannelId::Primary,
                    IDE_BMI_COMMAND_OFFSET,
                    IDE_BMI_COMMAND_START | IDE_BMI_COMMAND_RW,
                )
                .unwrap();
            assert!(dma_timing
                .execute_dma_parallel(context, IdeChannelId::Primary, Arc::clone(&dma_guest))
                .unwrap()
                .is_some());
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(pci, 14, move |_| {
            assert_eq!(
                &observe_guest.lock().unwrap().bytes()[0x200..0x400],
                [0xee; 512].as_slice()
            );
            assert_eq!(
                observe_controller
                    .lock()
                    .unwrap()
                    .read_bmi_u8(IdeChannelId::Primary, IDE_BMI_STATUS_OFFSET),
                Ok(IDE_BMI_STATUS_DMA_CAP0 | IDE_BMI_STATUS_DMA_CAP1 | IDE_BMI_STATUS_ACTIVE)
            );
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        &guest.lock().unwrap().bytes()[0x200..0x400],
        sector(0x5a).as_slice()
    );
    assert_eq!(
        controller
            .lock()
            .unwrap()
            .read_bmi_u8(IdeChannelId::Primary, IDE_BMI_STATUS_OFFSET),
        Ok(IDE_BMI_STATUS_DMA_CAP0 | IDE_BMI_STATUS_DMA_CAP1 | IDE_BMI_STATUS_INTERRUPT)
    );
    assert!(controller.lock().unwrap().shared_interrupt_asserted());
    assert!(timing.completion_errors().lock().unwrap().is_empty());
}

#[test]
fn ide_timing_port_syncs_dma_read_interrupt_delivery_in_parallel() {
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let controller = Arc::new(Mutex::new(
        IdeController::new([Some(disk(0x61, IdeDeviceId::Device0)), None, None, None]).unwrap(),
    ));
    let guest = Arc::new(Mutex::new(GuestMemory::filled(4096, 0xee)));
    write_prd(&mut guest.lock().unwrap(), 0x80, 0x200, 512, true);
    let (interrupts, legacy_port, source) = legacy_interrupt_port(cpu, 2);
    let ide_port = IdePciEndpointSpec::new(function())
        .build_legacy_interrupt_port(legacy_port, source)
        .unwrap();
    let timing = IdeControllerTimingPort::new(Arc::clone(&controller), 4)
        .unwrap()
        .with_interrupt_port(ide_port.clone());

    let issue_timing = timing.clone();
    let issue_controller = Arc::clone(&controller);
    let dma_timing = timing.clone();
    let dma_controller = Arc::clone(&controller);
    let dma_guest = Arc::clone(&guest);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    scheduler
        .schedule_parallel_at(pci, 5, move |context| {
            let mut controller = issue_controller.lock().unwrap();
            controller
                .write_bmi_u32(IdeChannelId::Primary, IDE_BMI_PRD_TABLE_OFFSET, 0x80)
                .unwrap();
            issue_lba_read_registers(&mut controller);
            drop(controller);
            assert!(issue_timing
                .write_command_u8_parallel(
                    context,
                    IdeChannelId::Primary,
                    IDE_STATUS_OFFSET,
                    IDE_COMMAND_READ_DMA,
                )
                .unwrap()
                .is_some());
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(pci, 10, move |context| {
            dma_controller
                .lock()
                .unwrap()
                .write_bmi_u8(
                    IdeChannelId::Primary,
                    IDE_BMI_COMMAND_OFFSET,
                    IDE_BMI_COMMAND_START | IDE_BMI_COMMAND_RW,
                )
                .unwrap();
            assert!(dma_timing
                .execute_dma_parallel(context, IdeChannelId::Primary, Arc::clone(&dma_guest))
                .unwrap()
                .is_some());
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        interrupts.lock().unwrap().history(),
        &[InterruptEvent::routed(
            17,
            InterruptLineId::new(47),
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
    assert!(ide_port.line_asserted());
    assert!(timing.completion_errors().lock().unwrap().is_empty());
    assert!(ide_port.delivery_errors().lock().unwrap().is_empty());
}

#[test]
fn ide_timing_port_delays_next_pio_write_sector_in_parallel() {
    let pci = PartitionId::new(1);
    let image = two_sector_image(0x00, 0x00);
    let controller = Arc::new(Mutex::new(
        IdeController::new([
            Some(
                IdeDisk::new(
                    image.clone() as Arc<dyn StorageImageLayer>,
                    IdeDeviceId::Device0,
                )
                .unwrap(),
            ),
            None,
            None,
            None,
        ])
        .unwrap(),
    ));
    {
        let mut controller = controller.lock().unwrap();
        issue_lba_read_registers_with_sectors(&mut controller, 2);
        controller
            .write_control_u8(IdeChannelId::Primary, IDE_CONTROL_OFFSET, IDE_CONTROL_IEN)
            .unwrap();
    }
    let timing = IdeControllerTimingPort::new(Arc::clone(&controller), 3).unwrap();
    let issue_timing = timing.clone();
    let write_first_timing = timing.clone();
    let write_second_timing = timing.clone();
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
                    IDE_COMMAND_WRITE,
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
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(pci, 9, move |context| {
            for word in 0..256 {
                let write = write_first_timing
                    .write_data_u16_parallel(context, IdeChannelId::Primary, 0x1111)
                    .unwrap();
                if word == 255 {
                    assert!(write.completion_event().is_some());
                } else {
                    assert!(write.completion_event().is_none());
                }
            }
            assert_eq!(
                write_first_timing
                    .controller()
                    .lock()
                    .unwrap()
                    .read_control_u8(IdeChannelId::Primary, rem6_storage::IDE_ALTSTAT_OFFSET),
                Ok(IDE_STATUS_DRDY | IDE_STATUS_BSY)
            );
            assert_data_write_not_ready(
                write_first_timing
                    .write_data_u16_parallel(context, IdeChannelId::Primary, 0x2222)
                    .unwrap_err(),
            );
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(pci, 10, move |_| {
            assert_eq!(
                observe_controller
                    .lock()
                    .unwrap()
                    .read_control_u8(IdeChannelId::Primary, rem6_storage::IDE_ALTSTAT_OFFSET),
                Ok(IDE_STATUS_DRDY | IDE_STATUS_BSY)
            );
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(pci, 13, move |context| {
            for word in 0..256 {
                let write = write_second_timing
                    .write_data_u16_parallel(context, IdeChannelId::Primary, 0x2222)
                    .unwrap();
                assert!(write.completion_event().is_none(), "{word}");
            }
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        controller
            .lock()
            .unwrap()
            .read_control_u8(IdeChannelId::Primary, rem6_storage::IDE_ALTSTAT_OFFSET),
        Ok(IDE_STATUS_DRDY | rem6_storage::IDE_STATUS_SEEK)
    );
    assert_eq!(
        image.read_sector(StorageSectorId::new(0)).unwrap(),
        [0x11; 512]
    );
    assert_eq!(
        image.read_sector(StorageSectorId::new(1)).unwrap(),
        [0x22; 512]
    );
    assert!(timing.completion_errors().lock().unwrap().is_empty());
}

use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{
    MmioAccess, MmioBus, MmioCompletion, MmioError, MmioOperation, MmioRequest, MmioRequestId,
    MmioResponse, MmioRoute,
};
use rem6_virtio::{
    VirtioPciIsrDevice, VirtioPciIsrEvent, VirtioPciIsrEventKind, VirtioPciIsrStatus,
    VIRTIO_PCI_ISR_STATUS_SIZE,
};

fn isr_read(id: u64, address: u64) -> MmioRequest {
    MmioRequest::read(
        MmioRequestId::new(id),
        Address::new(address),
        AccessSize::new(1).unwrap(),
    )
    .unwrap()
}

fn completion_for(
    completions: &[MmioCompletion],
    request: MmioRequestId,
) -> &Result<MmioResponse, MmioError> {
    completions
        .iter()
        .find_map(|completion| match completion.response() {
            Ok(response) if response.request() == request => Some(completion.response()),
            Err(MmioError::AccessDenied {
                request: failed, ..
            })
            | Err(MmioError::DeviceError {
                request: failed, ..
            }) if *failed == request => Some(completion.response()),
            _ => None,
        })
        .expect("completion for request")
}

#[test]
fn virtio_pci_isr_records_serial_and_parallel_read_clear_status() {
    let isr = VirtioPciIsrDevice::new();
    assert_eq!(
        isr.range(),
        AddressRange::new(
            Address::new(0),
            AccessSize::new(VIRTIO_PCI_ISR_STATUS_SIZE).unwrap(),
        )
        .unwrap()
    );
    assert_eq!(isr.status(), VirtioPciIsrStatus::empty());

    isr.raise_queue_interrupt(4);
    isr.raise_configuration_change_interrupt(6);
    assert_eq!(isr.status(), VirtioPciIsrStatus::queue_and_config());

    let snapshot = isr.snapshot();
    let cpu = PartitionId::new(0);
    let virtio = PartitionId::new(1);
    let route = MmioRoute::new(cpu, virtio, 2, 1).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(isr.range(), route, isr.clone()).unwrap();
    let bus = Arc::new(bus);
    let completions = Arc::new(Mutex::new(Vec::new()));

    let serial_bus = Arc::clone(&bus);
    let serial_completed = Arc::clone(&completions);
    let mut serial_scheduler = PartitionedScheduler::new(2).unwrap();
    serial_scheduler
        .schedule_at(cpu, 10, move |context| {
            serial_bus
                .submit(context, isr_read(1, 0), move |completion| {
                    serial_completed.lock().unwrap().push(completion)
                })
                .unwrap();
        })
        .unwrap();
    serial_scheduler.run_until_idle();

    assert_eq!(isr.status(), VirtioPciIsrStatus::empty());
    assert_eq!(
        isr.events(),
        vec![
            VirtioPciIsrEvent::new(
                4,
                VirtioPciIsrEventKind::QueueInterrupt,
                VirtioPciIsrStatus::empty(),
                VirtioPciIsrStatus::queue_interrupt(),
            ),
            VirtioPciIsrEvent::new(
                6,
                VirtioPciIsrEventKind::ConfigurationChangeInterrupt,
                VirtioPciIsrStatus::queue_interrupt(),
                VirtioPciIsrStatus::queue_and_config(),
            ),
            VirtioPciIsrEvent::new(
                12,
                VirtioPciIsrEventKind::DriverReadClear,
                VirtioPciIsrStatus::queue_and_config(),
                VirtioPciIsrStatus::empty(),
            ),
        ]
    );

    isr.restore(&snapshot);
    assert_eq!(isr.status(), VirtioPciIsrStatus::queue_and_config());

    let parallel_bus = Arc::clone(&bus);
    let parallel_completed = Arc::clone(&completions);
    let mut parallel_scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    parallel_scheduler
        .schedule_parallel_at(cpu, 20, move |context| {
            parallel_bus
                .submit_parallel(context, isr_read(2, 0), move |completion| {
                    parallel_completed.lock().unwrap().push(completion)
                })
                .unwrap();
        })
        .unwrap();
    parallel_scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(isr.status(), VirtioPciIsrStatus::empty());
    assert_eq!(
        isr.events(),
        vec![
            VirtioPciIsrEvent::new(
                4,
                VirtioPciIsrEventKind::QueueInterrupt,
                VirtioPciIsrStatus::empty(),
                VirtioPciIsrStatus::queue_interrupt(),
            ),
            VirtioPciIsrEvent::new(
                6,
                VirtioPciIsrEventKind::ConfigurationChangeInterrupt,
                VirtioPciIsrStatus::queue_interrupt(),
                VirtioPciIsrStatus::queue_and_config(),
            ),
            VirtioPciIsrEvent::new(
                22,
                VirtioPciIsrEventKind::DriverReadClear,
                VirtioPciIsrStatus::queue_and_config(),
                VirtioPciIsrStatus::empty(),
            ),
        ]
    );

    let completions = completions.lock().unwrap();
    assert_eq!(
        completion_for(&completions, MmioRequestId::new(1)),
        &Ok(MmioResponse::completed(
            MmioRequestId::new(1),
            Some(vec![0x03]),
        ))
    );
    assert_eq!(
        completion_for(&completions, MmioRequestId::new(2)),
        &Ok(MmioResponse::completed(
            MmioRequestId::new(2),
            Some(vec![0x03]),
        ))
    );
}

#[test]
fn virtio_pci_isr_rejects_writes_and_invalid_accesses() {
    let isr = VirtioPciIsrDevice::new();
    assert_eq!(
        VirtioPciIsrStatus::from_bits_truncate(0xff),
        VirtioPciIsrStatus::queue_and_config()
    );

    assert_eq!(
        isr.read_local(Address::new(0), AccessSize::new(2).unwrap()),
        Err(MmioError::AccessSizeMismatch {
            request: MmioRequestId::new(0),
            expected: 1,
            actual: 2,
        })
    );
    assert!(matches!(
        isr.read_local(Address::new(1), AccessSize::new(1).unwrap()),
        Err(MmioError::DeviceBoundaryCrossed { .. })
    ));
    assert_eq!(
        isr.write_local(
            Address::new(0),
            vec![0xff],
            ByteMask::from_bits(vec![true]).unwrap(),
        ),
        Err(MmioError::AccessDenied {
            request: MmioRequestId::new(0),
            operation: MmioOperation::Write,
            access: MmioAccess::ReadOnly,
        })
    );

    assert_eq!(
        isr.read_local(Address::new(0), AccessSize::new(1).unwrap()),
        Ok(vec![0x00])
    );
    assert!(isr.events().is_empty());
}

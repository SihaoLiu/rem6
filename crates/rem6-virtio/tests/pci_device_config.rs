use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{
    MmioBus, MmioCompletion, MmioError, MmioRequest, MmioRequestId, MmioResponse, MmioRoute,
};
use rem6_virtio::{
    VirtioPciDeviceConfigAccess, VirtioPciDeviceConfigDevice, VirtioPciDeviceConfigSpec,
};

fn config_read(id: u64, address: u64, size: u64) -> MmioRequest {
    MmioRequest::read(
        MmioRequestId::new(id),
        Address::new(address),
        AccessSize::new(size).unwrap(),
    )
    .unwrap()
}

fn config_write(id: u64, address: u64, data: &[u8], mask: &[bool]) -> MmioRequest {
    MmioRequest::write(
        MmioRequestId::new(id),
        Address::new(address),
        data.to_vec(),
        ByteMask::from_bits(mask.to_vec()).unwrap(),
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
fn virtio_pci_device_config_records_serial_and_parallel_accesses() {
    let config = VirtioPciDeviceConfigDevice::new(
        VirtioPciDeviceConfigSpec::new(
            vec![0x11, 0x22, 0x33, 0x44],
            ByteMask::from_bits(vec![false, true, true, false]).unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(
        config.range(),
        AddressRange::new(Address::new(0), AccessSize::new(4).unwrap()).unwrap()
    );
    assert_eq!(config.bytes(), vec![0x11, 0x22, 0x33, 0x44]);

    let cpu = PartitionId::new(0);
    let virtio = PartitionId::new(1);
    let route = MmioRoute::new(cpu, virtio, 2, 1).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(config.range(), route, config.clone())
        .unwrap();
    let bus = Arc::new(bus);
    let completions = Arc::new(Mutex::new(Vec::new()));

    let serial_bus = Arc::clone(&bus);
    let serial_completed = Arc::clone(&completions);
    let mut serial_scheduler = PartitionedScheduler::new(2).unwrap();
    serial_scheduler
        .schedule_at(cpu, 4, move |context| {
            serial_bus
                .submit(context, config_read(1, 0, 4), {
                    let serial_completed = Arc::clone(&serial_completed);
                    move |completion| serial_completed.lock().unwrap().push(completion)
                })
                .unwrap();
            serial_bus
                .submit(
                    context,
                    config_write(2, 1, &[0xaa, 0xbb], &[true, false]),
                    {
                        let serial_completed = Arc::clone(&serial_completed);
                        move |completion| serial_completed.lock().unwrap().push(completion)
                    },
                )
                .unwrap();
        })
        .unwrap();
    serial_scheduler.run_until_idle();

    assert_eq!(config.bytes(), vec![0x11, 0xaa, 0x33, 0x44]);
    assert_eq!(
        config.accesses(),
        vec![
            VirtioPciDeviceConfigAccess::read(6, Address::new(0), vec![0x11, 0x22, 0x33, 0x44]),
            VirtioPciDeviceConfigAccess::write(
                6,
                Address::new(1),
                vec![0xaa, 0xbb],
                ByteMask::from_bits(vec![true, false]).unwrap(),
                vec![0x22, 0x33],
                vec![0xaa, 0x33],
            ),
        ]
    );
    let snapshot = config.snapshot();

    let parallel_bus = Arc::clone(&bus);
    let parallel_completed = Arc::clone(&completions);
    let mut parallel_scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    parallel_scheduler
        .schedule_parallel_at(cpu, 10, move |context| {
            parallel_bus
                .submit_parallel(
                    context,
                    config_write(3, 2, &[0xcc], &[true]),
                    move |completion| parallel_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();
    parallel_scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(config.bytes(), vec![0x11, 0xaa, 0xcc, 0x44]);
    assert_eq!(
        config.accesses(),
        vec![
            VirtioPciDeviceConfigAccess::read(6, Address::new(0), vec![0x11, 0x22, 0x33, 0x44]),
            VirtioPciDeviceConfigAccess::write(
                6,
                Address::new(1),
                vec![0xaa, 0xbb],
                ByteMask::from_bits(vec![true, false]).unwrap(),
                vec![0x22, 0x33],
                vec![0xaa, 0x33],
            ),
            VirtioPciDeviceConfigAccess::write(
                12,
                Address::new(2),
                vec![0xcc],
                ByteMask::from_bits(vec![true]).unwrap(),
                vec![0x33],
                vec![0xcc],
            ),
        ]
    );

    config.restore(&snapshot);
    assert_eq!(config.bytes(), vec![0x11, 0xaa, 0x33, 0x44]);
    assert_eq!(config.accesses(), snapshot.accesses());

    let completions = completions.lock().unwrap();
    assert_eq!(
        completion_for(&completions, MmioRequestId::new(1)),
        &Ok(MmioResponse::completed(
            MmioRequestId::new(1),
            Some(vec![0x11, 0x22, 0x33, 0x44]),
        ))
    );
    assert_eq!(
        completion_for(&completions, MmioRequestId::new(2)),
        &Ok(MmioResponse::completed(MmioRequestId::new(2), None))
    );
    assert_eq!(
        completion_for(&completions, MmioRequestId::new(3)),
        &Ok(MmioResponse::completed(MmioRequestId::new(3), None))
    );
}

#[test]
fn virtio_pci_device_config_rejects_invalid_layouts_and_bad_writes() {
    assert!(matches!(
        VirtioPciDeviceConfigSpec::new(Vec::new(), ByteMask::from_bits(vec![true]).unwrap()),
        Err(error) if error.to_string().contains("device config")
    ));
    assert!(matches!(
        VirtioPciDeviceConfigSpec::new(vec![0], ByteMask::from_bits(vec![true, false]).unwrap()),
        Err(error) if error.to_string().contains("writable mask")
    ));

    let config = VirtioPciDeviceConfigDevice::new(
        VirtioPciDeviceConfigSpec::new(
            vec![0x10, 0x20],
            ByteMask::from_bits(vec![false, true]).unwrap(),
        )
        .unwrap(),
    );

    assert!(matches!(
        config.read_local(Address::new(1), AccessSize::new(2).unwrap()),
        Err(MmioError::DeviceBoundaryCrossed { .. })
    ));
    assert!(matches!(
        config.write_local(
            Address::new(0),
            vec![0xff],
            ByteMask::from_bits(vec![true]).unwrap(),
        ),
        Err(MmioError::DeviceError { message, .. }) if message.contains("read-only")
    ));
    assert_eq!(
        config.write_local(
            Address::new(1),
            vec![0xaa],
            ByteMask::from_bits(vec![true, false]).unwrap(),
        ),
        Err(MmioError::ByteMaskSizeMismatch {
            request: MmioRequestId::new(0),
            expected: 1,
            actual: 2,
        })
    );
    assert_eq!(config.bytes(), vec![0x10, 0x20]);
    assert!(config.accesses().is_empty());
}

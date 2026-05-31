use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{
    MmioBus, MmioCompletion, MmioError, MmioRequest, MmioRequestId, MmioResponse, MmioRoute,
};
use rem6_virtio::{
    VirtioError, VirtioPciDeviceConfigAccess, VirtioPciDeviceConfigDevice,
    VirtioPciDeviceConfigSnapshot, VirtioPciDeviceConfigSpec,
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

#[test]
fn virtio_pci_device_config_snapshot_bytes_round_trip_and_restore() {
    let config = VirtioPciDeviceConfigDevice::new(
        VirtioPciDeviceConfigSpec::new(
            vec![0x11, 0x22, 0x33, 0x44],
            ByteMask::from_bits(vec![false, true, true, false]).unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(
        config.read_local(Address::new(0), AccessSize::new(4).unwrap()),
        Ok(vec![0x11, 0x22, 0x33, 0x44])
    );
    config
        .write_local(
            Address::new(1),
            vec![0xaa, 0xbb],
            ByteMask::from_bits(vec![true, false]).unwrap(),
        )
        .unwrap();
    let snapshot = config.snapshot();
    let payload = snapshot.to_bytes();

    assert_eq!(&payload[0..8], b"VIODCFG1");
    assert_eq!(u16::from_le_bytes(payload[8..10].try_into().unwrap()), 1);
    assert_eq!(u64::from_le_bytes(payload[10..18].try_into().unwrap()), 4);
    assert_eq!(&payload[18..22], &[0x11, 0xaa, 0x33, 0x44]);
    assert_eq!(u64::from_le_bytes(payload[22..30].try_into().unwrap()), 4);
    assert_eq!(&payload[30..34], &[0, 1, 1, 0]);
    assert_eq!(u64::from_le_bytes(payload[34..42].try_into().unwrap()), 2);

    let decoded = VirtioPciDeviceConfigSnapshot::from_bytes(&payload).unwrap();

    assert_eq!(decoded, snapshot);
    assert_eq!(decoded.bytes(), &[0x11, 0xaa, 0x33, 0x44]);
    assert_eq!(
        decoded.accesses(),
        vec![
            VirtioPciDeviceConfigAccess::read(0, Address::new(0), vec![0x11, 0x22, 0x33, 0x44]),
            VirtioPciDeviceConfigAccess::write(
                0,
                Address::new(1),
                vec![0xaa, 0xbb],
                ByteMask::from_bits(vec![true, false]).unwrap(),
                vec![0x22, 0x33],
                vec![0xaa, 0x33],
            ),
        ]
    );

    let restored = VirtioPciDeviceConfigDevice::new(
        VirtioPciDeviceConfigSpec::new(vec![0], ByteMask::from_bits(vec![true]).unwrap()).unwrap(),
    );
    restored.restore(&decoded);
    assert_eq!(restored.snapshot(), snapshot);
}

#[test]
fn virtio_pci_device_config_snapshot_bytes_reject_malformed_payloads() {
    let config = VirtioPciDeviceConfigDevice::new(
        VirtioPciDeviceConfigSpec::new(
            vec![0x11, 0x22, 0x33, 0x44],
            ByteMask::from_bits(vec![false, true, true, false]).unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(
        config.read_local(Address::new(0), AccessSize::new(4).unwrap()),
        Ok(vec![0x11, 0x22, 0x33, 0x44])
    );
    config
        .write_local(
            Address::new(1),
            vec![0xaa, 0xbb],
            ByteMask::from_bits(vec![true, false]).unwrap(),
        )
        .unwrap();
    let payload = config.snapshot().to_bytes();

    assert_eq!(
        VirtioPciDeviceConfigSnapshot::from_bytes(&payload[..payload.len() - 1]),
        Err(VirtioError::InvalidDeviceConfigSnapshot)
    );

    let mut invalid_magic = payload.clone();
    invalid_magic[0] ^= 0xff;
    assert_eq!(
        VirtioPciDeviceConfigSnapshot::from_bytes(&invalid_magic),
        Err(VirtioError::InvalidDeviceConfigSnapshot)
    );

    let mut invalid_version = payload.clone();
    invalid_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        VirtioPciDeviceConfigSnapshot::from_bytes(&invalid_version),
        Err(VirtioError::InvalidDeviceConfigSnapshot)
    );

    let mut empty_bytes = payload.clone();
    empty_bytes[10..18].copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(
        VirtioPciDeviceConfigSnapshot::from_bytes(&empty_bytes),
        Err(VirtioError::InvalidDeviceConfigSnapshot)
    );

    let mut mask_len_mismatch = payload.clone();
    mask_len_mismatch[22..30].copy_from_slice(&3_u64.to_le_bytes());
    assert_eq!(
        VirtioPciDeviceConfigSnapshot::from_bytes(&mask_len_mismatch),
        Err(VirtioError::InvalidDeviceConfigSnapshot)
    );

    let mut invalid_mask_bit = payload.clone();
    invalid_mask_bit[30] = 2;
    assert_eq!(
        VirtioPciDeviceConfigSnapshot::from_bytes(&invalid_mask_bit),
        Err(VirtioError::InvalidDeviceConfigSnapshot)
    );

    let mut invalid_access_count = payload.clone();
    invalid_access_count[34..42].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        VirtioPciDeviceConfigSnapshot::from_bytes(&invalid_access_count),
        Err(VirtioError::InvalidDeviceConfigSnapshot)
    );

    let mut invalid_access_kind = payload.clone();
    invalid_access_kind[42] = 9;
    assert_eq!(
        VirtioPciDeviceConfigSnapshot::from_bytes(&invalid_access_kind),
        Err(VirtioError::InvalidDeviceConfigSnapshot)
    );

    let mut invalid_read_range = payload.clone();
    invalid_read_range[51..59].copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(
        VirtioPciDeviceConfigSnapshot::from_bytes(&invalid_read_range),
        Err(VirtioError::InvalidDeviceConfigSnapshot)
    );

    let mut invalid_write_range = payload.clone();
    invalid_write_range[80..88].copy_from_slice(&3_u64.to_le_bytes());
    assert_eq!(
        VirtioPciDeviceConfigSnapshot::from_bytes(&invalid_write_range),
        Err(VirtioError::InvalidDeviceConfigSnapshot)
    );

    let mut invalid_write_mask_len = payload.clone();
    invalid_write_mask_len[98..106].copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(
        VirtioPciDeviceConfigSnapshot::from_bytes(&invalid_write_mask_len),
        Err(VirtioError::InvalidDeviceConfigSnapshot)
    );

    let mut trailing = payload.clone();
    trailing.push(0);
    assert_eq!(
        VirtioPciDeviceConfigSnapshot::from_bytes(&trailing),
        Err(VirtioError::InvalidDeviceConfigSnapshot)
    );
}

use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{
    MmioAccess, MmioBus, MmioCompletion, MmioError, MmioOperation, MmioRequest, MmioRequestId,
    MmioResponse, MmioRoute,
};
use rem6_virtio::{
    VirtioError, VirtioPciNotifyDevice, VirtioPciNotifySnapshot, VirtioQueueIndex,
    VirtioQueueNotification, VirtioQueueNotifySpec,
};

fn notify_write(id: u64, address: u64, value: u16) -> MmioRequest {
    MmioRequest::write(
        MmioRequestId::new(id),
        Address::new(address),
        value.to_le_bytes().to_vec(),
        ByteMask::from_bits(vec![true, true]).unwrap(),
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
fn virtio_pci_notify_records_serial_and_parallel_queue_notifications() {
    let notify = VirtioPciNotifyDevice::new(
        4,
        [
            VirtioQueueNotifySpec::new(VirtioQueueIndex::new(0).unwrap(), 0),
            VirtioQueueNotifySpec::new(VirtioQueueIndex::new(1).unwrap(), 3),
        ],
    )
    .unwrap();
    assert_eq!(
        notify.range(),
        AddressRange::new(Address::new(0), AccessSize::new(14).unwrap()).unwrap()
    );

    let cpu = PartitionId::new(0);
    let virtio = PartitionId::new(1);
    let route = MmioRoute::new(cpu, virtio, 2, 1).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(notify.range(), route, notify.clone())
        .unwrap();
    let bus = Arc::new(bus);
    let completions = Arc::new(Mutex::new(Vec::new()));

    let serial_bus = Arc::clone(&bus);
    let serial_completed = Arc::clone(&completions);
    let mut serial_scheduler = PartitionedScheduler::new(2).unwrap();
    serial_scheduler
        .schedule_at(cpu, 5, move |context| {
            serial_bus
                .submit(context, notify_write(1, 0, 0), move |completion| {
                    serial_completed.lock().unwrap().push(completion)
                })
                .unwrap();
        })
        .unwrap();
    serial_scheduler.run_until_idle();

    assert_eq!(
        notify.notifications(),
        vec![VirtioQueueNotification::new(
            7,
            VirtioQueueIndex::new(0).unwrap(),
            0,
            Address::new(0),
        )]
    );

    let snapshot = notify.snapshot();
    let parallel_bus = Arc::clone(&bus);
    let parallel_completed = Arc::clone(&completions);
    let mut parallel_scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    parallel_scheduler
        .schedule_parallel_at(cpu, 10, move |context| {
            parallel_bus
                .submit_parallel(context, notify_write(2, 12, 1), move |completion| {
                    parallel_completed.lock().unwrap().push(completion)
                })
                .unwrap();
        })
        .unwrap();
    parallel_scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        notify.notifications(),
        vec![
            VirtioQueueNotification::new(7, VirtioQueueIndex::new(0).unwrap(), 0, Address::new(0),),
            VirtioQueueNotification::new(
                12,
                VirtioQueueIndex::new(1).unwrap(),
                1,
                Address::new(12),
            ),
        ]
    );

    notify.restore(&snapshot);
    assert_eq!(
        notify.notifications(),
        vec![VirtioQueueNotification::new(
            7,
            VirtioQueueIndex::new(0).unwrap(),
            0,
            Address::new(0),
        )]
    );

    let completions = completions.lock().unwrap();
    assert_eq!(
        completion_for(&completions, MmioRequestId::new(1)),
        &Ok(MmioResponse::completed(MmioRequestId::new(1), None))
    );
    assert_eq!(
        completion_for(&completions, MmioRequestId::new(2)),
        &Ok(MmioResponse::completed(MmioRequestId::new(2), None))
    );
}

#[test]
fn virtio_pci_notify_rejects_invalid_layouts_and_bad_accesses() {
    assert!(matches!(
        VirtioPciNotifyDevice::new(
            6,
            [VirtioQueueNotifySpec::new(
                VirtioQueueIndex::new(0).unwrap(),
                0,
            )],
        ),
        Err(error) if error.to_string().contains("notify_off_multiplier")
    ));

    let notify = VirtioPciNotifyDevice::new(
        4,
        [
            VirtioQueueNotifySpec::new(VirtioQueueIndex::new(0).unwrap(), 0),
            VirtioQueueNotifySpec::new(VirtioQueueIndex::new(1).unwrap(), 3),
        ],
    )
    .unwrap();

    assert_eq!(
        notify.read_local(Address::new(0), AccessSize::new(2).unwrap()),
        Err(MmioError::AccessDenied {
            request: MmioRequestId::new(0),
            operation: MmioOperation::Read,
            access: MmioAccess::WriteOnly,
        })
    );

    let wrong_value = notify.write_local(
        Address::new(12),
        0_u16.to_le_bytes().to_vec(),
        ByteMask::from_bits(vec![true, true]).unwrap(),
        3,
    );
    assert!(matches!(
        wrong_value,
        Err(MmioError::DeviceError { message, .. }) if message.contains("does not match")
    ));
    assert!(notify.notifications().is_empty());

    let unmapped = notify.write_local(
        Address::new(8),
        1_u16.to_le_bytes().to_vec(),
        ByteMask::from_bits(vec![true, true]).unwrap(),
        4,
    );
    assert!(matches!(
        unmapped,
        Err(MmioError::DeviceError { message, .. }) if message.contains("no queue")
    ));
    assert!(notify.notifications().is_empty());
}

#[test]
fn virtio_pci_notify_snapshot_bytes_round_trip_and_restore() {
    let notify = VirtioPciNotifyDevice::new(
        4,
        [
            VirtioQueueNotifySpec::new(VirtioQueueIndex::new(0).unwrap(), 0),
            VirtioQueueNotifySpec::new(VirtioQueueIndex::new(1).unwrap(), 3),
        ],
    )
    .unwrap();
    notify
        .write_local(
            Address::new(0),
            0_u16.to_le_bytes().to_vec(),
            ByteMask::from_bits(vec![true, true]).unwrap(),
            7,
        )
        .unwrap();
    notify
        .write_local(
            Address::new(12),
            1_u16.to_le_bytes().to_vec(),
            ByteMask::from_bits(vec![true, true]).unwrap(),
            12,
        )
        .unwrap();
    let snapshot = notify.snapshot();
    let payload = snapshot.to_bytes();

    assert_eq!(&payload[0..8], b"VIONOTI1");
    assert_eq!(u16::from_le_bytes(payload[8..10].try_into().unwrap()), 1);
    assert_eq!(u64::from_le_bytes(payload[10..18].try_into().unwrap()), 2);
    assert_eq!(u64::from_le_bytes(payload[18..26].try_into().unwrap()), 7);
    assert_eq!(u16::from_le_bytes(payload[26..28].try_into().unwrap()), 0);
    assert_eq!(u16::from_le_bytes(payload[28..30].try_into().unwrap()), 0);
    assert_eq!(u64::from_le_bytes(payload[30..38].try_into().unwrap()), 0);
    assert_eq!(u64::from_le_bytes(payload[38..46].try_into().unwrap()), 12);
    assert_eq!(u16::from_le_bytes(payload[46..48].try_into().unwrap()), 1);
    assert_eq!(u16::from_le_bytes(payload[48..50].try_into().unwrap()), 1);
    assert_eq!(u64::from_le_bytes(payload[50..58].try_into().unwrap()), 12);

    let decoded = VirtioPciNotifySnapshot::from_bytes(&payload).unwrap();

    assert_eq!(decoded, snapshot);
    assert_eq!(
        decoded.notifications(),
        &[
            VirtioQueueNotification::new(7, VirtioQueueIndex::new(0).unwrap(), 0, Address::new(0)),
            VirtioQueueNotification::new(
                12,
                VirtioQueueIndex::new(1).unwrap(),
                1,
                Address::new(12),
            ),
        ]
    );

    let restored = VirtioPciNotifyDevice::new(
        4,
        [
            VirtioQueueNotifySpec::new(VirtioQueueIndex::new(0).unwrap(), 0),
            VirtioQueueNotifySpec::new(VirtioQueueIndex::new(1).unwrap(), 3),
        ],
    )
    .unwrap();
    restored.restore(&decoded);
    assert_eq!(restored.snapshot(), snapshot);
}

#[test]
fn virtio_pci_notify_snapshot_bytes_reject_malformed_payloads() {
    let notify = VirtioPciNotifyDevice::new(
        4,
        [VirtioQueueNotifySpec::new(
            VirtioQueueIndex::new(0).unwrap(),
            0,
        )],
    )
    .unwrap();
    notify
        .write_local(
            Address::new(0),
            0_u16.to_le_bytes().to_vec(),
            ByteMask::from_bits(vec![true, true]).unwrap(),
            3,
        )
        .unwrap();
    let payload = notify.snapshot().to_bytes();

    assert_eq!(
        VirtioPciNotifySnapshot::from_bytes(&payload[..payload.len() - 1]),
        Err(VirtioError::InvalidNotifySnapshot)
    );

    let mut invalid_magic = payload.clone();
    invalid_magic[0] ^= 0xff;
    assert_eq!(
        VirtioPciNotifySnapshot::from_bytes(&invalid_magic),
        Err(VirtioError::InvalidNotifySnapshot)
    );

    let mut invalid_version = payload.clone();
    invalid_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        VirtioPciNotifySnapshot::from_bytes(&invalid_version),
        Err(VirtioError::InvalidNotifySnapshot)
    );

    let mut invalid_count = payload.clone();
    invalid_count[10..18].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        VirtioPciNotifySnapshot::from_bytes(&invalid_count),
        Err(VirtioError::InvalidNotifySnapshot)
    );

    let mut trailing = payload.clone();
    trailing.push(0);
    assert_eq!(
        VirtioPciNotifySnapshot::from_bytes(&trailing),
        Err(VirtioError::InvalidNotifySnapshot)
    );
}

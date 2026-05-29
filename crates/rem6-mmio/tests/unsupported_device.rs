use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{
    MmioBus, MmioCompletion, MmioDevice, MmioError, MmioOperation, MmioRequest, MmioRequestId,
    MmioRoute, UnsupportedMmioAccess, UnsupportedMmioDevice,
};

fn range(start: u64, bytes: u64) -> AddressRange {
    AddressRange::new(Address::new(start), AccessSize::new(bytes).unwrap()).unwrap()
}

fn read_request(id: u64, address: u64, bytes: u64) -> MmioRequest {
    MmioRequest::read(
        MmioRequestId::new(id),
        Address::new(address),
        AccessSize::new(bytes).unwrap(),
    )
    .unwrap()
}

fn write_request(id: u64, address: u64, data: Vec<u8>) -> MmioRequest {
    MmioRequest::write(
        MmioRequestId::new(id),
        Address::new(address),
        data.clone(),
        ByteMask::full(AccessSize::new(data.len() as u64).unwrap()).unwrap(),
    )
    .unwrap()
}

#[test]
fn unsupported_mmio_device_records_serial_access_and_returns_typed_error() {
    let device = UnsupportedMmioDevice::new(
        "framebuffer",
        Address::new(0x1000_0000),
        AccessSize::new(0x10).unwrap(),
    )
    .unwrap();
    let request = write_request(7, 0x1000_0008, vec![0xaa, 0xbb, 0xcc, 0xdd]);
    let result = Arc::new(Mutex::new(None));
    let result_slot = Arc::clone(&result);
    let device_for_event = device.clone();
    let mut scheduler = PartitionedScheduler::new(1).unwrap();

    scheduler
        .schedule_at(PartitionId::new(0), 11, move |context| {
            *result_slot.lock().unwrap() = Some(device_for_event.respond(context, &request));
        })
        .unwrap();
    scheduler.run_until_idle();

    assert_eq!(
        result.lock().unwrap().clone().unwrap(),
        Err(MmioError::UnsupportedDeviceAccess {
            request: MmioRequestId::new(7),
            device: "framebuffer".to_string(),
            operation: MmioOperation::Write,
            address: Address::new(0x1000_0008),
            bytes: 4,
        })
    );
    assert_eq!(
        device.access_log(),
        vec![UnsupportedMmioAccess::new(
            11,
            "framebuffer".to_string(),
            MmioRequestId::new(7),
            MmioOperation::Write,
            range(0x1000_0008, 4),
            Some(vec![0xaa, 0xbb, 0xcc, 0xdd]),
        )]
    );
}

#[test]
fn unsupported_mmio_device_routes_parallel_bus_completion_without_panic() {
    let cpu = PartitionId::new(0);
    let device_partition = PartitionId::new(1);
    let route = MmioRoute::new(cpu, device_partition, 2, 3).unwrap();
    let device = UnsupportedMmioDevice::new(
        "legacy-vga",
        Address::new(0x2000_0000),
        AccessSize::new(0x20).unwrap(),
    )
    .unwrap();
    let monitor = device.clone();
    let mut bus = MmioBus::new();
    bus.insert_device(device.range(), route, device).unwrap();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 5, move |context| {
            bus.submit_parallel(
                context,
                read_request(9, 0x2000_0010, 8),
                move |completion| {
                    completed.lock().unwrap().push(completion);
                },
            )
            .unwrap();
        })
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[MmioCompletion::new(
            10,
            route,
            Err(MmioError::UnsupportedDeviceAccess {
                request: MmioRequestId::new(9),
                device: "legacy-vga".to_string(),
                operation: MmioOperation::Read,
                address: Address::new(0x2000_0010),
                bytes: 8,
            }),
        )]
    );
    assert_eq!(
        monitor.access_log(),
        vec![UnsupportedMmioAccess::new(
            7,
            "legacy-vga".to_string(),
            MmioRequestId::new(9),
            MmioOperation::Read,
            range(0x2000_0010, 8),
            None,
        )]
    );
}

#[test]
fn unsupported_mmio_device_rejects_empty_names_and_out_of_range_direct_access() {
    assert_eq!(
        UnsupportedMmioDevice::new(
            "",
            Address::new(0x3000_0000),
            AccessSize::new(0x10).unwrap(),
        ),
        Err(MmioError::InvalidDeviceName),
    );

    let device = UnsupportedMmioDevice::new(
        "reserved-rom",
        Address::new(0x3000_0000),
        AccessSize::new(0x10).unwrap(),
    )
    .unwrap();
    let result = Arc::new(Mutex::new(None));
    let result_slot = Arc::clone(&result);
    let mut scheduler = PartitionedScheduler::new(1).unwrap();

    scheduler
        .schedule_at(PartitionId::new(0), 3, move |context| {
            *result_slot.lock().unwrap() =
                Some(device.respond(context, &read_request(13, 0x3000_000c, 8)));
        })
        .unwrap();
    scheduler.run_until_idle();

    assert_eq!(
        result.lock().unwrap().clone().unwrap(),
        Err(MmioError::DeviceBoundaryCrossed {
            request: MmioRequestId::new(13),
            device_start: Address::new(0x3000_0000),
            device_end: Address::new(0x3000_0010),
            requested_start: Address::new(0x3000_000c),
            requested_end: Address::new(0x3000_0014),
        })
    );
}

use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerContext, Tick};
use rem6_memory::{AccessSize, Address, AddressRange};
use rem6_mmio::{
    MmioAccess, MmioBus, MmioCompletion, MmioDevice, MmioError, MmioRegisterBank, MmioRequest,
    MmioRequestId, MmioResponse, MmioRoute,
};

#[derive(Clone, Debug)]
struct RecordingBankDevice {
    bank: Arc<Mutex<MmioRegisterBank>>,
    deliveries: Arc<Mutex<Vec<(Tick, MmioRequest)>>>,
}

impl RecordingBankDevice {
    fn new(
        bank: Arc<Mutex<MmioRegisterBank>>,
        deliveries: Arc<Mutex<Vec<(Tick, MmioRequest)>>>,
    ) -> Self {
        Self { bank, deliveries }
    }
}

impl MmioDevice for RecordingBankDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.deliveries
            .lock()
            .unwrap()
            .push((context.now(), request.clone()));
        self.bank.lock().unwrap().respond(request)
    }
}

#[derive(Clone, Copy, Debug)]
struct NoopDevice;

impl MmioDevice for NoopDevice {
    fn respond(
        &self,
        _context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        Ok(MmioResponse::completed(request.id(), None))
    }
}

#[test]
fn mmio_bus_routes_requests_to_decoded_device_regions() {
    let cpu = PartitionId::new(0);
    let device_a = PartitionId::new(1);
    let device_b = PartitionId::new(2);
    let route_a = MmioRoute::new(cpu, device_a, 2, 1).unwrap();
    let route_b = MmioRoute::new(cpu, device_b, 4, 3).unwrap();
    let mut bank_a =
        MmioRegisterBank::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap();
    bank_a
        .insert_register(
            0x10,
            AccessSize::new(4).unwrap(),
            MmioAccess::ReadOnly,
            vec![0xaa, 0xbb, 0xcc, 0xdd],
        )
        .unwrap();
    let mut bank_b =
        MmioRegisterBank::new(Address::new(0x2000), AccessSize::new(0x100).unwrap()).unwrap();
    bank_b
        .insert_register(
            0x20,
            AccessSize::new(2).unwrap(),
            MmioAccess::ReadOnly,
            vec![0x12, 0x34],
        )
        .unwrap();
    let bank_a = Arc::new(Mutex::new(bank_a));
    let bank_b = Arc::new(Mutex::new(bank_b));
    let deliveries = Arc::new(Mutex::new(Vec::new()));
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut bus = MmioBus::new();

    bus.insert_device(
        AddressRange::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap(),
        route_a,
        RecordingBankDevice::new(Arc::clone(&bank_a), Arc::clone(&deliveries)),
    )
    .unwrap();
    bus.insert_device(
        AddressRange::new(Address::new(0x2000), AccessSize::new(0x100).unwrap()).unwrap(),
        route_b,
        RecordingBankDevice::new(Arc::clone(&bank_b), Arc::clone(&deliveries)),
    )
    .unwrap();

    let mut scheduler = PartitionedScheduler::new(3).unwrap();
    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 5, move |context| {
            let first_completed = Arc::clone(&completed);
            bus.submit(
                context,
                MmioRequest::read(
                    MmioRequestId::new(1),
                    Address::new(0x1010),
                    AccessSize::new(4).unwrap(),
                )
                .unwrap(),
                move |completion| first_completed.lock().unwrap().push(completion),
            )
            .unwrap();
            bus.submit(
                context,
                MmioRequest::read(
                    MmioRequestId::new(2),
                    Address::new(0x2020),
                    AccessSize::new(2).unwrap(),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 5);
    assert_eq!(summary.final_tick(), 12);
    assert_eq!(
        deliveries.lock().unwrap().as_slice(),
        &[
            (
                7,
                MmioRequest::read(
                    MmioRequestId::new(1),
                    Address::new(0x1010),
                    AccessSize::new(4).unwrap(),
                )
                .unwrap(),
            ),
            (
                9,
                MmioRequest::read(
                    MmioRequestId::new(2),
                    Address::new(0x2020),
                    AccessSize::new(2).unwrap(),
                )
                .unwrap(),
            ),
        ]
    );
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[
            MmioCompletion::new(
                8,
                route_a,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(1),
                    Some(vec![0xaa, 0xbb, 0xcc, 0xdd]),
                )),
            ),
            MmioCompletion::new(
                12,
                route_b,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(2),
                    Some(vec![0x12, 0x34]),
                )),
            ),
        ]
    );
}

#[test]
fn mmio_bus_routes_decoded_devices_on_parallel_scheduler() {
    let cpu = PartitionId::new(0);
    let device_a = PartitionId::new(1);
    let device_b = PartitionId::new(2);
    let route_a = MmioRoute::new(cpu, device_a, 2, 2).unwrap();
    let route_b = MmioRoute::new(cpu, device_b, 4, 2).unwrap();
    let mut bank_a =
        MmioRegisterBank::new(Address::new(0x3000), AccessSize::new(0x100).unwrap()).unwrap();
    bank_a
        .insert_register(
            0x08,
            AccessSize::new(4).unwrap(),
            MmioAccess::ReadOnly,
            vec![0x11, 0x22, 0x33, 0x44],
        )
        .unwrap();
    let mut bank_b =
        MmioRegisterBank::new(Address::new(0x4000), AccessSize::new(0x100).unwrap()).unwrap();
    bank_b
        .insert_register(
            0x18,
            AccessSize::new(2).unwrap(),
            MmioAccess::ReadOnly,
            vec![0x55, 0x66],
        )
        .unwrap();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut bus = MmioBus::new();

    bus.insert_device(
        AddressRange::new(Address::new(0x3000), AccessSize::new(0x100).unwrap()).unwrap(),
        route_a,
        Mutex::new(bank_a),
    )
    .unwrap();
    bus.insert_device(
        AddressRange::new(Address::new(0x4000), AccessSize::new(0x100).unwrap()).unwrap(),
        route_b,
        Mutex::new(bank_b),
    )
    .unwrap();

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 5, move |context| {
            let first_completed = Arc::clone(&completed);
            bus.submit_parallel(
                context,
                MmioRequest::read(
                    MmioRequestId::new(11),
                    Address::new(0x3008),
                    AccessSize::new(4).unwrap(),
                )
                .unwrap(),
                move |completion| first_completed.lock().unwrap().push(completion),
            )
            .unwrap();
            bus.submit_parallel(
                context,
                MmioRequest::read(
                    MmioRequestId::new(12),
                    Address::new(0x4018),
                    AccessSize::new(2).unwrap(),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 5);
    assert!(summary.final_tick() >= 11);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[
            MmioCompletion::new(
                9,
                route_a,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(11),
                    Some(vec![0x11, 0x22, 0x33, 0x44]),
                )),
            ),
            MmioCompletion::new(
                11,
                route_b,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(12),
                    Some(vec![0x55, 0x66]),
                )),
            ),
        ]
    );
}

#[test]
fn mmio_bus_rejects_overlapping_devices_and_boundary_crosses() {
    let cpu = PartitionId::new(0);
    let device = PartitionId::new(1);
    let route = MmioRoute::new(cpu, device, 2, 1).unwrap();
    let mut bus = MmioBus::new();

    bus.insert_device(
        AddressRange::new(Address::new(0x4000), AccessSize::new(0x100).unwrap()).unwrap(),
        route,
        NoopDevice,
    )
    .unwrap();

    assert_eq!(
        bus.insert_device(
            AddressRange::new(Address::new(0x4080), AccessSize::new(0x40).unwrap()).unwrap(),
            route,
            NoopDevice,
        ),
        Err(MmioError::OverlappingDeviceRegion {
            existing_start: Address::new(0x4000),
            existing_end: Address::new(0x4100),
            requested_start: Address::new(0x4080),
            requested_end: Address::new(0x40c0),
        })
    );

    let crossing = MmioRequest::read(
        MmioRequestId::new(3),
        Address::new(0x40f8),
        AccessSize::new(0x10).unwrap(),
    )
    .unwrap();
    assert_eq!(
        bus.route_for(&crossing),
        Err(MmioError::DeviceBoundaryCrossed {
            request: MmioRequestId::new(3),
            device_start: Address::new(0x4000),
            device_end: Address::new(0x4100),
            requested_start: Address::new(0x40f8),
            requested_end: Address::new(0x4108),
        })
    );

    let unmapped = MmioRequest::read(
        MmioRequestId::new(4),
        Address::new(0x5000),
        AccessSize::new(4).unwrap(),
    )
    .unwrap();
    assert_eq!(
        bus.route_for(&unmapped),
        Err(MmioError::UnmappedAddress {
            address: Address::new(0x5000),
        })
    );
}

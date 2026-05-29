use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{
    MmioAccess, MmioChannel, MmioCompletion, MmioDelivery, MmioError, MmioOperation,
    MmioRegisterBank, MmioRequest, MmioRequestId, MmioResponse, MmioRoute, MmioRouteLatency,
};

#[test]
fn mmio_register_bank_reads_and_applies_masked_writes() {
    let mut bank =
        MmioRegisterBank::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        0x10,
        AccessSize::new(4).unwrap(),
        MmioAccess::ReadWrite,
        vec![0x11, 0x22, 0x33, 0x44],
    )
    .unwrap();

    let read = MmioRequest::read(
        MmioRequestId::new(1),
        Address::new(0x1010),
        AccessSize::new(4).unwrap(),
    )
    .unwrap();
    assert_eq!(
        bank.respond(&read).unwrap(),
        MmioResponse::completed(read.id(), Some(vec![0x11, 0x22, 0x33, 0x44]))
    );

    let write = MmioRequest::write(
        MmioRequestId::new(2),
        Address::new(0x1011),
        vec![0xaa, 0xbb],
        ByteMask::from_bits(vec![true, false]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        bank.respond(&write).unwrap(),
        MmioResponse::completed(write.id(), None)
    );

    let read_after_write = MmioRequest::read(
        MmioRequestId::new(3),
        Address::new(0x1010),
        AccessSize::new(4).unwrap(),
    )
    .unwrap();
    assert_eq!(
        bank.respond(&read_after_write).unwrap(),
        MmioResponse::completed(read_after_write.id(), Some(vec![0x11, 0xaa, 0x33, 0x44]))
    );
}

#[test]
fn mmio_register_bank_rejects_overlaps_permissions_and_cross_registers() {
    let mut bank =
        MmioRegisterBank::new(Address::new(0x2000), AccessSize::new(0x20).unwrap()).unwrap();
    bank.insert_register(
        0x00,
        AccessSize::new(4).unwrap(),
        MmioAccess::ReadOnly,
        vec![0x01, 0x02, 0x03, 0x04],
    )
    .unwrap();

    assert_eq!(
        bank.insert_register(
            0x02,
            AccessSize::new(4).unwrap(),
            MmioAccess::ReadWrite,
            vec![0; 4],
        ),
        Err(MmioError::OverlappingRegister {
            existing_start: Address::new(0x2000),
            existing_end: Address::new(0x2004),
            requested_start: Address::new(0x2002),
            requested_end: Address::new(0x2006),
        })
    );

    let readonly_write = MmioRequest::write(
        MmioRequestId::new(4),
        Address::new(0x2000),
        vec![0xff],
        ByteMask::full(AccessSize::new(1).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        bank.respond(&readonly_write),
        Err(MmioError::AccessDenied {
            request: MmioRequestId::new(4),
            operation: MmioOperation::Write,
            access: MmioAccess::ReadOnly,
        })
    );

    let crossing_read = MmioRequest::read(
        MmioRequestId::new(5),
        Address::new(0x2002),
        AccessSize::new(4).unwrap(),
    )
    .unwrap();
    assert_eq!(
        bank.respond(&crossing_read),
        Err(MmioError::RegisterBoundaryCrossed {
            request: MmioRequestId::new(5),
            register_start: Address::new(0x2000),
            register_end: Address::new(0x2004),
            requested_start: Address::new(0x2002),
            requested_end: Address::new(0x2006),
        })
    );

    let unmapped = MmioRequest::read(
        MmioRequestId::new(6),
        Address::new(0x2010),
        AccessSize::new(4).unwrap(),
    )
    .unwrap();
    assert_eq!(
        bank.respond(&unmapped),
        Err(MmioError::UnmappedAddress {
            address: Address::new(0x2010),
        })
    );
}

#[test]
fn mmio_channel_routes_request_and_response_between_partitions() {
    let cpu = PartitionId::new(0);
    let device = PartitionId::new(1);
    let mut bank =
        MmioRegisterBank::new(Address::new(0x3000), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        0x18,
        AccessSize::new(4).unwrap(),
        MmioAccess::ReadWrite,
        vec![0xde, 0xad, 0xbe, 0xef],
    )
    .unwrap();
    let bank = Arc::new(Mutex::new(bank));
    let route = MmioRoute::new(cpu, device, 3, 2).unwrap();
    let channel = MmioChannel::new(route);
    let request = MmioRequest::read(
        MmioRequestId::new(7),
        Address::new(0x3018),
        AccessSize::new(4).unwrap(),
    )
    .unwrap();
    let deliveries = Arc::new(Mutex::new(Vec::new()));
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    let delivered = Arc::clone(&deliveries);
    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 5, move |context| {
            let bank = Arc::clone(&bank);
            channel
                .submit(
                    context,
                    request,
                    move |delivery, _context| {
                        delivered.lock().unwrap().push(delivery.clone());
                        bank.lock().unwrap().respond(delivery.request())
                    },
                    move |completion| completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 3);
    assert_eq!(summary.final_tick(), 10);
    assert_eq!(
        deliveries.lock().unwrap().as_slice(),
        &[MmioDelivery::new(
            8,
            route,
            MmioRequest::read(
                MmioRequestId::new(7),
                Address::new(0x3018),
                AccessSize::new(4).unwrap(),
            )
            .unwrap(),
        )]
    );
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[MmioCompletion::new(
            10,
            route,
            Ok(MmioResponse::completed(
                MmioRequestId::new(7),
                Some(vec![0xde, 0xad, 0xbe, 0xef]),
            ))
        )]
    );
}

#[test]
fn mmio_channel_routes_request_and_response_on_parallel_scheduler() {
    let cpu = PartitionId::new(0);
    let device = PartitionId::new(1);
    let mut bank =
        MmioRegisterBank::new(Address::new(0x3800), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        0x08,
        AccessSize::new(4).unwrap(),
        MmioAccess::ReadWrite,
        vec![0x10, 0x20, 0x30, 0x40],
    )
    .unwrap();
    let bank = Arc::new(Mutex::new(bank));
    let route = MmioRoute::new(cpu, device, 3, 2).unwrap();
    let channel = MmioChannel::new(route);
    let request = MmioRequest::read(
        MmioRequestId::new(17),
        Address::new(0x3808),
        AccessSize::new(4).unwrap(),
    )
    .unwrap();
    let deliveries = Arc::new(Mutex::new(Vec::new()));
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    let delivered = Arc::clone(&deliveries);
    let completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 5, move |context| {
            let bank = Arc::clone(&bank);
            channel
                .submit_parallel(
                    context,
                    request,
                    move |delivery, context| {
                        assert_eq!(context.partition(), device);
                        delivered.lock().unwrap().push(delivery.clone());
                        bank.lock().unwrap().respond(delivery.request())
                    },
                    move |completion| completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 3);
    assert_eq!(summary.final_tick(), 10);
    assert_eq!(
        deliveries.lock().unwrap().as_slice(),
        &[MmioDelivery::new(
            8,
            route,
            MmioRequest::read(
                MmioRequestId::new(17),
                Address::new(0x3808),
                AccessSize::new(4).unwrap(),
            )
            .unwrap(),
        )]
    );
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[MmioCompletion::new(
            10,
            route,
            Ok(MmioResponse::completed(
                MmioRequestId::new(17),
                Some(vec![0x10, 0x20, 0x30, 0x40]),
            ))
        )]
    );
}

#[test]
fn mmio_channel_rejects_invalid_latency_and_records_response_errors() {
    let cpu = PartitionId::new(0);
    let device = PartitionId::new(1);
    assert_eq!(
        MmioRoute::new(cpu, device, 0, 1),
        Err(MmioError::ZeroRouteLatency {
            latency: MmioRouteLatency::Request,
        })
    );

    let route = MmioRoute::new(cpu, device, 3, 2).unwrap();
    let channel = MmioChannel::new(route);
    let errors = channel.response_errors();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let request = MmioRequest::read(
        MmioRequestId::new(8),
        Address::new(0x4000),
        AccessSize::new(1).unwrap(),
    )
    .unwrap();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 3).unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 4, move |context| {
            channel
                .submit(
                    context,
                    request,
                    move |delivery, _context| {
                        Ok(MmioResponse::completed(delivery.request().id(), None))
                    },
                    move |completion| completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 2);
    assert_eq!(summary.final_tick(), 7);
    assert!(completions.lock().unwrap().is_empty());
    assert_eq!(
        errors.lock().unwrap().as_slice(),
        &[MmioError::Scheduler(
            SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
                source: device,
                target: cpu,
                source_tick: 7,
                delivery_tick: 9,
                minimum_delivery_tick: 10,
            }
        )]
    );
}

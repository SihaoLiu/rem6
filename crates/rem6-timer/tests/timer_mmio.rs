use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptEvent, InterruptEventKind, InterruptLineChannel, InterruptLineId,
    InterruptLinePort, InterruptRoute, InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{
    MmioAccess, MmioBus, MmioChannel, MmioCompletion, MmioError, MmioOperation, MmioRequest,
    MmioRequestId, MmioResponse, MmioRoute,
};
use rem6_timer::{
    ProgrammableTimer, TimerArm, TimerExpiry, TimerId, TimerMmioDevice, TIMER_MMIO_DEADLINE_OFFSET,
    TIMER_MMIO_REGISTER_BYTES, TIMER_MMIO_TIME_OFFSET,
};

fn le64(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn full_u64_mask() -> ByteMask {
    ByteMask::full(AccessSize::new(TIMER_MMIO_REGISTER_BYTES).unwrap()).unwrap()
}

fn build_timer(
    cpu: PartitionId,
    timer_partition: PartitionId,
    line: InterruptLineId,
    source: InterruptSourceId,
) -> (
    ProgrammableTimer,
    Arc<Mutex<InterruptController>>,
    TimerMmioDevice,
) {
    let route = InterruptRoute::new(line, InterruptTargetId::new(0), cpu);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    controller.lock().unwrap().register_route(route).unwrap();
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    let timer = ProgrammableTimer::new(TimerId::new(10), timer_partition, source, port);
    let device = TimerMmioDevice::new(timer.clone(), Address::new(0x5000));
    (timer, controller, device)
}

#[test]
fn timer_mmio_write_deadline_arms_timer_and_delivers_interrupt() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let source = InterruptSourceId::new(9);
    let line = InterruptLineId::new(30);
    let (timer, controller, device) = build_timer(cpu, timer_partition, line, source);
    let channel = MmioChannel::new(MmioRoute::new(cpu, timer_partition, 3, 1).unwrap());
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 2, move |context| {
            channel
                .submit(
                    context,
                    MmioRequest::write(
                        MmioRequestId::new(1),
                        Address::new(0x5000 + TIMER_MMIO_DEADLINE_OFFSET),
                        le64(10),
                        full_u64_mask(),
                    )
                    .unwrap(),
                    move |delivery, context| device.respond(context, delivery.request()),
                    move |completion| completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 5);
    assert_eq!(summary.final_tick(), 12);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[MmioCompletion::new(
            6,
            MmioRoute::new(cpu, timer_partition, 3, 1).unwrap(),
            Ok(MmioResponse::completed(MmioRequestId::new(1), None)),
        )]
    );
    assert_eq!(timer.snapshot().arms(), &[TimerArm::new(1, 5, 10)]);
    assert_eq!(timer.snapshot().expiries(), &[TimerExpiry::new(1, 10)]);
    assert_eq!(
        controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            12,
            line,
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn timer_mmio_bus_decodes_timer_region_and_programs_deadline() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let source = InterruptSourceId::new(12);
    let line = InterruptLineId::new(33);
    let (timer, controller, device) = build_timer(cpu, timer_partition, line, source);
    let route = MmioRoute::new(cpu, timer_partition, 3, 1).unwrap();
    let mut bus = MmioBus::new();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    bus.insert_device(
        AddressRange::new(Address::new(0x5000), AccessSize::new(0x100).unwrap()).unwrap(),
        route,
        device,
    )
    .unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 1, move |context| {
            bus.submit(
                context,
                MmioRequest::write(
                    MmioRequestId::new(7),
                    Address::new(0x5000 + TIMER_MMIO_DEADLINE_OFFSET),
                    le64(9),
                    full_u64_mask(),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 5);
    assert_eq!(summary.final_tick(), 11);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[MmioCompletion::new(
            5,
            route,
            Ok(MmioResponse::completed(MmioRequestId::new(7), None)),
        )]
    );
    assert_eq!(timer.snapshot().arms(), &[TimerArm::new(1, 4, 9)]);
    assert_eq!(timer.snapshot().expiries(), &[TimerExpiry::new(1, 9)]);
    assert_eq!(
        controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            11,
            line,
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn timer_parallel_mmio_bus_write_deadline_arms_timer_and_delivers_interrupt() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let source = InterruptSourceId::new(13);
    let line = InterruptLineId::new(34);
    let (timer, controller, device) = build_timer(cpu, timer_partition, line, source);
    let route = MmioRoute::new(cpu, timer_partition, 3, 2).unwrap();
    let mut bus = MmioBus::new();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    bus.insert_device(
        AddressRange::new(Address::new(0x5000), AccessSize::new(0x100).unwrap()).unwrap(),
        route,
        device,
    )
    .unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 2, move |context| {
            bus.submit_parallel(
                context,
                MmioRequest::write(
                    MmioRequestId::new(13),
                    Address::new(0x5000 + TIMER_MMIO_DEADLINE_OFFSET),
                    le64(10),
                    full_u64_mask(),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 5);
    assert!(summary.final_tick() >= 12);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[MmioCompletion::new(
            7,
            route,
            Ok(MmioResponse::completed(MmioRequestId::new(13), None)),
        )]
    );
    assert_eq!(timer.snapshot().arms(), &[TimerArm::new(1, 5, 10)]);
    assert_eq!(timer.snapshot().expiries(), &[TimerExpiry::new(1, 10)]);
    assert_eq!(
        controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            12,
            line,
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn timer_mmio_reads_current_tick_and_programmed_deadline() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let source = InterruptSourceId::new(10);
    let (_, _, device) = build_timer(cpu, timer_partition, InterruptLineId::new(31), source);
    let channel = MmioChannel::new(MmioRoute::new(cpu, timer_partition, 2, 1).unwrap());
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 4, move |context| {
            let deadline_write = MmioRequest::write(
                MmioRequestId::new(2),
                Address::new(0x5000 + TIMER_MMIO_DEADLINE_OFFSET),
                le64(20),
                full_u64_mask(),
            )
            .unwrap();
            let time_read = MmioRequest::read(
                MmioRequestId::new(3),
                Address::new(0x5000 + TIMER_MMIO_TIME_OFFSET),
                AccessSize::new(TIMER_MMIO_REGISTER_BYTES).unwrap(),
            )
            .unwrap();
            let deadline_read = MmioRequest::read(
                MmioRequestId::new(4),
                Address::new(0x5000 + TIMER_MMIO_DEADLINE_OFFSET),
                AccessSize::new(TIMER_MMIO_REGISTER_BYTES).unwrap(),
            )
            .unwrap();

            let first_device = device.clone();
            channel
                .submit(
                    context,
                    deadline_write,
                    move |delivery, context| first_device.respond(context, delivery.request()),
                    |_| {},
                )
                .unwrap();
            let second_device = device.clone();
            let completed_time = Arc::clone(&completed);
            channel
                .submit(
                    context,
                    time_read,
                    move |delivery, context| second_device.respond(context, delivery.request()),
                    move |completion| completed_time.lock().unwrap().push(completion),
                )
                .unwrap();
            channel
                .submit(
                    context,
                    deadline_read,
                    move |delivery, context| device.respond(context, delivery.request()),
                    move |completion| completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.final_tick(), 22);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[
            MmioCompletion::new(
                7,
                MmioRoute::new(cpu, timer_partition, 2, 1).unwrap(),
                Ok(MmioResponse::completed(
                    MmioRequestId::new(3),
                    Some(le64(6))
                )),
            ),
            MmioCompletion::new(
                7,
                MmioRoute::new(cpu, timer_partition, 2, 1).unwrap(),
                Ok(MmioResponse::completed(
                    MmioRequestId::new(4),
                    Some(le64(20))
                )),
            ),
        ]
    );
}

#[test]
fn timer_mmio_rejects_bad_width_and_readonly_time_writes() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let (_, _, device) = build_timer(
        cpu,
        timer_partition,
        InterruptLineId::new(32),
        InterruptSourceId::new(11),
    );
    let errors = Arc::new(Mutex::new(Vec::new()));
    let observed_errors = Arc::clone(&errors);
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(timer_partition, 8, move |context| {
            errors.lock().unwrap().push(
                device
                    .respond(
                        context,
                        &MmioRequest::read(
                            MmioRequestId::new(5),
                            Address::new(0x5000 + TIMER_MMIO_DEADLINE_OFFSET),
                            AccessSize::new(4).unwrap(),
                        )
                        .unwrap(),
                    )
                    .unwrap_err(),
            );
            errors.lock().unwrap().push(
                device
                    .respond(
                        context,
                        &MmioRequest::write(
                            MmioRequestId::new(6),
                            Address::new(0x5000 + TIMER_MMIO_TIME_OFFSET),
                            le64(12),
                            full_u64_mask(),
                        )
                        .unwrap(),
                    )
                    .unwrap_err(),
            );
        })
        .unwrap();

    scheduler.run_until_idle();

    assert_eq!(
        observed_errors.lock().unwrap().as_slice(),
        &[
            MmioError::AccessSizeMismatch {
                request: MmioRequestId::new(5),
                expected: TIMER_MMIO_REGISTER_BYTES,
                actual: 4,
            },
            MmioError::AccessDenied {
                request: MmioRequestId::new(6),
                operation: MmioOperation::Write,
                access: MmioAccess::ReadOnly,
            },
        ]
    );
}

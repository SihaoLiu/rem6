use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptEvent, InterruptEventKind, InterruptLineChannel, InterruptLineId,
    InterruptLinePort, InterruptRoute, InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{MmioError, MmioRequest, MmioRequestId, MmioResponse};
use rem6_timer::{
    Mc146818Rtc, Mc146818RtcMmioDevice, RtcDateTime, RtcEncoding, RtcError,
    RTC_DAY_OF_WEEK_REGISTER, RTC_MINUTES_ALARM_REGISTER, RTC_MMIO_ADDRESS_OFFSET,
    RTC_MMIO_DATA_OFFSET, RTC_MMIO_REGISTER_BYTES, RTC_SECONDS_REGISTER, RTC_STATUS_B_REGISTER,
    RTC_STATUS_C_REGISTER,
};

fn byte_size() -> AccessSize {
    AccessSize::new(RTC_MMIO_REGISTER_BYTES).unwrap()
}

fn byte_mask() -> ByteMask {
    ByteMask::full(byte_size()).unwrap()
}

fn byte_write(id: u64, address: Address, value: u8) -> MmioRequest {
    MmioRequest::write(MmioRequestId::new(id), address, vec![value], byte_mask()).unwrap()
}

fn byte_read(id: u64, address: Address) -> MmioRequest {
    MmioRequest::read(MmioRequestId::new(id), address, byte_size()).unwrap()
}

fn rtc_device(base: Address) -> Mc146818RtcMmioDevice {
    Mc146818RtcMmioDevice::new(
        base,
        Mc146818Rtc::new(
            RtcDateTime::new(2026, 5, 29, 1, 2, 3, 6).unwrap(),
            RtcEncoding::Bcd,
        )
        .unwrap(),
    )
}

fn interrupt_port(
    controller: &Arc<Mutex<InterruptController>>,
    line: InterruptLineId,
    target: InterruptTargetId,
    target_partition: PartitionId,
    latency: u64,
) -> InterruptLinePort {
    let route = InterruptRoute::new(line, target, target_partition);
    controller.lock().unwrap().register_route(route).unwrap();
    InterruptLinePort::new(
        InterruptLineChannel::new(route, latency).unwrap(),
        Arc::clone(controller),
    )
}

#[test]
fn mc146818_rtc_mmio_routes_cmos_address_and_data_ports() {
    let base = Address::new(0x70);
    let device = rtc_device(base);
    let snapshot_source = device.clone();
    let responses = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&responses);
    let mut scheduler = PartitionedScheduler::new(1).unwrap();

    scheduler
        .schedule_at(PartitionId::new(0), 4, move |context| {
            let address_port = Address::new(base.get() + RTC_MMIO_ADDRESS_OFFSET);
            let data_port = Address::new(base.get() + RTC_MMIO_DATA_OFFSET);

            device
                .respond(
                    context,
                    &byte_write(1, address_port, 0x80 | RTC_SECONDS_REGISTER),
                )
                .unwrap();
            responses
                .lock()
                .unwrap()
                .push(device.respond(context, &byte_read(2, address_port)));
            responses
                .lock()
                .unwrap()
                .push(device.respond(context, &byte_read(3, data_port)));

            device
                .respond(
                    context,
                    &byte_write(4, address_port, RTC_MINUTES_ALARM_REGISTER),
                )
                .unwrap();
            device
                .respond(context, &byte_write(5, data_port, 0x45))
                .unwrap();
            responses
                .lock()
                .unwrap()
                .push(device.respond(context, &byte_read(6, data_port)));

            device
                .respond(context, &byte_write(7, address_port, 0xa0))
                .unwrap();
            device
                .respond(context, &byte_write(8, data_port, 0x5a))
                .unwrap();
            responses
                .lock()
                .unwrap()
                .push(device.respond(context, &byte_read(9, address_port)));
            responses
                .lock()
                .unwrap()
                .push(device.respond(context, &byte_read(10, data_port)));
        })
        .unwrap();

    scheduler.run_until_idle();

    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[
            Ok(MmioResponse::completed(
                MmioRequestId::new(2),
                Some(vec![0x80 | RTC_SECONDS_REGISTER])
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(3),
                Some(vec![0x03])
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(6),
                Some(vec![0x45])
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(9),
                Some(vec![0xa0])
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(10),
                Some(vec![0x5a])
            )),
        ]
    );

    let snapshot = snapshot_source.snapshot();
    let restored = rtc_device(base);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
}

#[test]
fn mc146818_rtc_mmio_supports_parallel_responses() {
    let base = Address::new(0x70);
    let device = rtc_device(base);
    let responses = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&responses);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(1, 1).unwrap();

    scheduler
        .schedule_parallel_at(PartitionId::new(0), 8, move |context| {
            let address_port = Address::new(base.get() + RTC_MMIO_ADDRESS_OFFSET);
            let data_port = Address::new(base.get() + RTC_MMIO_DATA_OFFSET);

            device
                .respond_parallel(
                    context,
                    &byte_write(11, address_port, RTC_DAY_OF_WEEK_REGISTER),
                )
                .unwrap();
            responses
                .lock()
                .unwrap()
                .push(device.respond_parallel(context, &byte_read(12, data_port)));
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[Ok(MmioResponse::completed(
            MmioRequestId::new(12),
            Some(vec![0x06])
        ))]
    );
}

#[test]
fn mc146818_rtc_periodic_interrupt_pulses_line_and_reschedules() {
    let cpu = PartitionId::new(0);
    let rtc_partition = PartitionId::new(1);
    let base = Address::new(0x70);
    let line = InterruptLineId::new(41);
    let target = InterruptTargetId::new(0);
    let source = InterruptSourceId::new(61);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let port = interrupt_port(&controller, line, target, cpu, 2);
    let device = Mc146818RtcMmioDevice::with_periodic_interrupt(
        base,
        Mc146818Rtc::new(
            RtcDateTime::new(2026, 5, 29, 1, 2, 3, 6).unwrap(),
            RtcEncoding::Bcd,
        )
        .unwrap(),
        rtc_partition,
        source,
        port,
        4,
    )
    .unwrap();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(cpu, 1, move |context| {
            device.start_periodic_interrupts(context).unwrap();
        })
        .unwrap();
    for _ in 0..12 {
        scheduler.run_next_epoch();
        if controller.lock().unwrap().history().len() >= 4 {
            break;
        }
    }

    assert_eq!(
        controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(7, line, target, cpu, source, InterruptEventKind::Assert),
            InterruptEvent::routed(7, line, target, cpu, source, InterruptEventKind::Deassert),
            InterruptEvent::routed(11, line, target, cpu, source, InterruptEventKind::Assert),
            InterruptEvent::routed(11, line, target, cpu, source, InterruptEventKind::Deassert),
        ]
    );
}

#[test]
fn mc146818_rtc_periodic_interrupt_pulses_line_on_parallel_scheduler() {
    let cpu = PartitionId::new(0);
    let rtc_partition = PartitionId::new(1);
    let base = Address::new(0x70);
    let line = InterruptLineId::new(45);
    let target = InterruptTargetId::new(0);
    let source = InterruptSourceId::new(65);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let port = interrupt_port(&controller, line, target, cpu, 2);
    let device = Mc146818RtcMmioDevice::with_periodic_interrupt(
        base,
        Mc146818Rtc::new(
            RtcDateTime::new(2026, 5, 29, 1, 2, 3, 6).unwrap(),
            RtcEncoding::Bcd,
        )
        .unwrap(),
        rtc_partition,
        source,
        port,
        4,
    )
    .unwrap();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    scheduler
        .schedule_parallel_at(cpu, 1, move |context| {
            device.start_periodic_interrupts_parallel(context).unwrap();
        })
        .unwrap();
    for _ in 0..12 {
        scheduler.run_next_epoch_parallel().unwrap();
        if controller.lock().unwrap().history().len() >= 2 {
            break;
        }
    }

    assert_eq!(
        controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(7, line, target, cpu, source, InterruptEventKind::Assert),
            InterruptEvent::routed(7, line, target, cpu, source, InterruptEventKind::Deassert),
        ]
    );
}

#[test]
fn mc146818_rtc_periodic_interrupt_stops_after_pie_clear() {
    let cpu = PartitionId::new(0);
    let rtc_partition = PartitionId::new(1);
    let base = Address::new(0x70);
    let line = InterruptLineId::new(42);
    let target = InterruptTargetId::new(0);
    let source = InterruptSourceId::new(62);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let port = interrupt_port(&controller, line, target, cpu, 2);
    let device = Mc146818RtcMmioDevice::with_periodic_interrupt(
        base,
        Mc146818Rtc::new(
            RtcDateTime::new(2026, 5, 29, 1, 2, 3, 6).unwrap(),
            RtcEncoding::Bcd,
        )
        .unwrap(),
        rtc_partition,
        source,
        port,
        4,
    )
    .unwrap();
    let rtc = device.clone();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(cpu, 1, move |context| {
            device.start_periodic_interrupts(context).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_at(cpu, 6, move |context| {
            let address_port = Address::new(base.get() + RTC_MMIO_ADDRESS_OFFSET);
            let data_port = Address::new(base.get() + RTC_MMIO_DATA_OFFSET);
            rtc.respond(
                context,
                &byte_write(31, address_port, RTC_STATUS_B_REGISTER),
            )
            .unwrap();
            rtc.respond(context, &byte_write(32, data_port, 0x02))
                .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle();

    assert_eq!(
        controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(7, line, target, cpu, source, InterruptEventKind::Assert),
            InterruptEvent::routed(7, line, target, cpu, source, InterruptEventKind::Deassert),
        ]
    );
}

#[test]
fn mc146818_rtc_mmio_reports_typed_access_errors() {
    let base = Address::new(0x70);
    let device = rtc_device(base);
    let errors = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&errors);
    let mut scheduler = PartitionedScheduler::new(1).unwrap();

    scheduler
        .schedule_at(PartitionId::new(0), 12, move |context| {
            let address_port = Address::new(base.get() + RTC_MMIO_ADDRESS_OFFSET);
            let data_port = Address::new(base.get() + RTC_MMIO_DATA_OFFSET);

            errors.lock().unwrap().push(
                device
                    .respond(
                        context,
                        &MmioRequest::read(
                            MmioRequestId::new(21),
                            data_port,
                            AccessSize::new(2).unwrap(),
                        )
                        .unwrap(),
                    )
                    .unwrap_err(),
            );
            errors.lock().unwrap().push(
                device
                    .respond(
                        context,
                        &byte_read(22, Address::new(base.get() + RTC_MMIO_DATA_OFFSET + 1)),
                    )
                    .unwrap_err(),
            );
            device
                .respond(
                    context,
                    &byte_write(23, address_port, RTC_STATUS_C_REGISTER),
                )
                .unwrap();
            errors.lock().unwrap().push(
                device
                    .respond(context, &byte_write(24, data_port, 0xff))
                    .unwrap_err(),
            );
        })
        .unwrap();

    scheduler.run_until_idle();

    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[
            MmioError::AccessSizeMismatch {
                request: MmioRequestId::new(21),
                expected: RTC_MMIO_REGISTER_BYTES,
                actual: 2,
            },
            MmioError::UnmappedAddress {
                address: Address::new(base.get() + RTC_MMIO_DATA_OFFSET + 1),
            },
            MmioError::DeviceError {
                request: MmioRequestId::new(24),
                message: RtcError::ReadOnlyRegister {
                    register: RTC_STATUS_C_REGISTER,
                }
                .to_string(),
            },
        ]
    );
}

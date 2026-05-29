use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptEvent, InterruptEventKind, InterruptLineChannel, InterruptLineId,
    InterruptLinePort, InterruptRoute, InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{MmioError, MmioRequest, MmioRequestId, MmioResponse};
use rem6_timer::{
    Pl031Rtc, Pl031RtcMmioDevice, PL031_CONTROL_OFFSET, PL031_DATA_OFFSET, PL031_INT_CLEAR_OFFSET,
    PL031_INT_MASK_OFFSET, PL031_LOAD_OFFSET, PL031_MASKED_ISR_OFFSET, PL031_MATCH_OFFSET,
    PL031_MMIO_SIZE_BYTES, PL031_RAW_ISR_OFFSET, PL031_REGISTER_BYTES,
};

fn register_size() -> AccessSize {
    AccessSize::new(PL031_REGISTER_BYTES).unwrap()
}

fn register_mask() -> ByteMask {
    ByteMask::full(register_size()).unwrap()
}

fn le32(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn write_request(id: u64, base: Address, offset: u64, value: u32) -> MmioRequest {
    MmioRequest::write(
        MmioRequestId::new(id),
        Address::new(base.get() + offset),
        le32(value),
        register_mask(),
    )
    .unwrap()
}

fn read_request(id: u64, base: Address, offset: u64) -> MmioRequest {
    MmioRequest::read(
        MmioRequestId::new(id),
        Address::new(base.get() + offset),
        register_size(),
    )
    .unwrap()
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
fn pl031_rtc_tracks_elapsed_seconds_load_match_and_snapshot() {
    let mut rtc = Pl031Rtc::new(10, 5).unwrap();

    assert_eq!(rtc.read_data(0).unwrap(), 10);
    assert_eq!(rtc.read_data(14).unwrap(), 12);
    assert_eq!(rtc.read_register(PL031_CONTROL_OFFSET, 14).unwrap(), 1);
    assert_eq!(rtc.read_register(PL031_LOAD_OFFSET, 14).unwrap(), 10);

    rtc.write_register(PL031_LOAD_OFFSET, 100, 15).unwrap();
    assert_eq!(rtc.read_data(29).unwrap(), 102);
    assert_eq!(rtc.read_register(PL031_LOAD_OFFSET, 29).unwrap(), 100);

    rtc.write_register(PL031_MATCH_OFFSET, 105, 29).unwrap();
    assert_eq!(rtc.next_match_tick(29).unwrap(), 44);

    let snapshot = rtc.snapshot();
    assert_eq!(
        (
            snapshot.time_value(),
            snapshot.last_written_tick(),
            snapshot.load_value(),
            snapshot.match_value(),
            snapshot.raw_interrupt(),
            snapshot.interrupt_mask(),
            snapshot.pending_interrupt(),
            snapshot.ticks_per_second(),
            snapshot.generation(),
        ),
        (100, 15, 100, 105, false, false, false, 5, 2)
    );

    let mut restored = Pl031Rtc::new(0, 1).unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.read_data(44).unwrap(), 105);
}

#[test]
fn pl031_rtc_latches_masks_and_clears_match_interrupt_status() {
    let mut rtc = Pl031Rtc::new(20, 10).unwrap();
    rtc.write_register(PL031_MATCH_OFFSET, 22, 0).unwrap();
    rtc.write_register(PL031_INT_MASK_OFFSET, 1, 0).unwrap();

    assert_eq!(rtc.next_match_tick(0).unwrap(), 20);
    assert!(rtc.record_match(20).unwrap());
    assert_eq!(rtc.read_register(PL031_RAW_ISR_OFFSET, 20).unwrap(), 1);
    assert_eq!(rtc.read_register(PL031_MASKED_ISR_OFFSET, 20).unwrap(), 1);

    rtc.write_register(PL031_INT_CLEAR_OFFSET, 1, 21).unwrap();
    assert_eq!(rtc.read_register(PL031_RAW_ISR_OFFSET, 21).unwrap(), 0);
    assert_eq!(rtc.read_register(PL031_MASKED_ISR_OFFSET, 21).unwrap(), 0);

    rtc.write_register(PL031_INT_MASK_OFFSET, 0, 22).unwrap();
    assert!(!rtc.record_match(22).unwrap());
    assert_eq!(rtc.read_register(PL031_RAW_ISR_OFFSET, 22).unwrap(), 1);
    assert_eq!(rtc.read_register(PL031_MASKED_ISR_OFFSET, 22).unwrap(), 0);
}

#[test]
fn pl031_mmio_routes_registers_and_delivers_serial_interrupt_pulse() {
    let cpu = PartitionId::new(0);
    let rtc_partition = PartitionId::new(1);
    let base = Address::new(0x1c17_0000);
    let line = InterruptLineId::new(70);
    let target = InterruptTargetId::new(0);
    let source = InterruptSourceId::new(90);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let port = interrupt_port(&controller, line, target, cpu, 2);
    let device = Pl031RtcMmioDevice::with_interrupt(
        base,
        Pl031Rtc::new(10, 5).unwrap(),
        rtc_partition,
        source,
        port,
    )
    .unwrap();
    let monitor = device.clone();
    let responses = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&responses);
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(rtc_partition, 1, move |context| {
            device
                .respond(context, &write_request(1, base, PL031_MATCH_OFFSET, 12))
                .unwrap();
            device
                .respond(context, &write_request(2, base, PL031_INT_MASK_OFFSET, 1))
                .unwrap();
            observed
                .lock()
                .unwrap()
                .push(device.respond(context, &read_request(3, base, PL031_DATA_OFFSET)));
        })
        .unwrap();
    scheduler.run_until_idle();

    assert_eq!(
        responses.lock().unwrap().as_slice(),
        &[Ok(MmioResponse::completed(
            MmioRequestId::new(3),
            Some(le32(10)),
        ))]
    );
    assert_eq!(
        controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(13, line, target, cpu, source, InterruptEventKind::Assert),
            InterruptEvent::routed(13, line, target, cpu, source, InterruptEventKind::Deassert),
        ]
    );
    assert!(monitor.snapshot().rtc().raw_interrupt());
    assert!(monitor.snapshot().rtc().pending_interrupt());
}

#[test]
fn pl031_mmio_latches_match_status_without_interrupt_route() {
    let rtc_partition = PartitionId::new(1);
    let base = Address::new(0x1c17_1000);
    let device = Pl031RtcMmioDevice::new(base, Pl031Rtc::new(10, 5).unwrap());
    let monitor = device.clone();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(rtc_partition, 1, move |context| {
            device
                .respond(context, &write_request(20, base, PL031_MATCH_OFFSET, 12))
                .unwrap();
            device
                .respond(context, &write_request(21, base, PL031_INT_MASK_OFFSET, 1))
                .unwrap();
        })
        .unwrap();
    scheduler.run_until_idle();

    assert!(monitor.snapshot().rtc().raw_interrupt());
    assert!(monitor.snapshot().rtc().pending_interrupt());
}

#[test]
fn pl031_mmio_supports_parallel_access_and_typed_errors() {
    let cpu = PartitionId::new(0);
    let rtc_partition = PartitionId::new(1);
    let base = Address::new(0x1c18_0000);
    let line = InterruptLineId::new(71);
    let target = InterruptTargetId::new(0);
    let source = InterruptSourceId::new(91);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let port = interrupt_port(&controller, line, target, cpu, 2);
    let device = Pl031RtcMmioDevice::with_interrupt(
        base,
        Pl031Rtc::new(30, 4).unwrap(),
        rtc_partition,
        source,
        port,
    )
    .unwrap();
    assert_eq!(device.range_size_bytes(), PL031_MMIO_SIZE_BYTES);
    let responses = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&responses);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();

    scheduler
        .schedule_parallel_at(rtc_partition, 2, move |context| {
            device
                .respond_parallel(context, &write_request(4, base, PL031_LOAD_OFFSET, 40))
                .unwrap();
            device
                .respond_parallel(context, &write_request(5, base, PL031_MATCH_OFFSET, 42))
                .unwrap();
            device
                .respond_parallel(context, &write_request(6, base, PL031_INT_MASK_OFFSET, 1))
                .unwrap();
            responses
                .lock()
                .unwrap()
                .push(device.respond_parallel(context, &read_request(7, base, PL031_LOAD_OFFSET)));
            responses.lock().unwrap().push(
                device.respond_parallel(
                    context,
                    &MmioRequest::read(
                        MmioRequestId::new(8),
                        Address::new(base.get() + 0x80),
                        register_size(),
                    )
                    .unwrap(),
                ),
            );
        })
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[
            Ok(MmioResponse::completed(
                MmioRequestId::new(7),
                Some(le32(40)),
            )),
            Err(MmioError::DeviceError {
                request: MmioRequestId::new(8),
                message: "unknown PL031 RTC register offset 0x80".to_string(),
            }),
        ]
    );
    assert_eq!(
        controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(12, line, target, cpu, source, InterruptEventKind::Assert),
            InterruptEvent::routed(12, line, target, cpu, source, InterruptEventKind::Deassert),
        ]
    );
}

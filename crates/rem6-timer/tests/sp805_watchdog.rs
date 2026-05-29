use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptEvent, InterruptEventKind, InterruptLineChannel, InterruptLineId,
    InterruptLinePort, InterruptRoute, InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{MmioError, MmioRequest, MmioRequestId, MmioResponse};
use rem6_timer::{
    Sp805Watchdog, Sp805WatchdogMmioDevice, AMBA_CELL_ID0_OFFSET, AMBA_CELL_ID1_OFFSET,
    AMBA_CELL_ID2_OFFSET, AMBA_CELL_ID3_OFFSET, AMBA_PERIPHERAL_ID0_OFFSET,
    AMBA_PERIPHERAL_ID1_OFFSET, AMBA_PERIPHERAL_ID2_OFFSET, AMBA_PERIPHERAL_ID3_OFFSET,
    SP805_CONTROL_OFFSET, SP805_INT_CLEAR_OFFSET, SP805_ITOP_OFFSET, SP805_LOAD_OFFSET,
    SP805_LOCK_MAGIC, SP805_LOCK_OFFSET, SP805_MASKED_ISR_OFFSET, SP805_MMIO_SIZE_BYTES,
    SP805_RAW_ISR_OFFSET, SP805_REGISTER_BYTES, SP805_VALUE_OFFSET,
};

fn register_size() -> AccessSize {
    AccessSize::new(SP805_REGISTER_BYTES).unwrap()
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
fn sp805_core_counts_down_locks_and_records_reset_assertion() {
    let mut watchdog = Sp805Watchdog::new(1).unwrap();

    assert_eq!(
        watchdog.read_register(SP805_LOAD_OFFSET, 0).unwrap(),
        u32::MAX
    );
    assert_eq!(
        watchdog.read_register(SP805_VALUE_OFFSET, 0).unwrap(),
        u32::MAX
    );
    assert_eq!(watchdog.read_register(SP805_LOCK_OFFSET, 0).unwrap(), 1);

    watchdog.write_register(SP805_LOAD_OFFSET, 4, 10).unwrap();
    watchdog
        .write_register(SP805_CONTROL_OFFSET, 0x3, 10)
        .unwrap();
    assert_eq!(
        watchdog.read_register(SP805_CONTROL_OFFSET, 10).unwrap(),
        0x3
    );
    assert_eq!(watchdog.read_register(SP805_VALUE_OFFSET, 12).unwrap(), 2);
    assert_eq!(watchdog.next_timeout_tick(12).unwrap(), Some(14));

    let first_generation = watchdog.snapshot().generation();
    let first = watchdog
        .record_timeout(14, first_generation)
        .unwrap()
        .unwrap();
    assert!(first.interrupt_asserted());
    assert!(!first.reset_asserted());
    assert_eq!(watchdog.read_register(SP805_RAW_ISR_OFFSET, 14).unwrap(), 1);
    assert_eq!(
        watchdog.read_register(SP805_MASKED_ISR_OFFSET, 14).unwrap(),
        1
    );

    let second_generation = watchdog.snapshot().generation();
    let second = watchdog
        .record_timeout(18, second_generation)
        .unwrap()
        .unwrap();
    assert!(!second.interrupt_asserted());
    assert!(second.reset_asserted());
    assert_eq!(watchdog.snapshot().reset_assertions(), &[18]);

    watchdog.write_register(SP805_LOCK_OFFSET, 0, 19).unwrap();
    assert_eq!(watchdog.read_register(SP805_LOCK_OFFSET, 19).unwrap(), 0);
    watchdog.write_register(SP805_LOAD_OFFSET, 9, 19).unwrap();
    assert_eq!(watchdog.read_register(SP805_LOAD_OFFSET, 19).unwrap(), 4);

    watchdog
        .write_register(SP805_LOCK_OFFSET, SP805_LOCK_MAGIC, 20)
        .unwrap();
    watchdog
        .write_register(SP805_INT_CLEAR_OFFSET, 1, 20)
        .unwrap();
    watchdog.write_register(SP805_LOAD_OFFSET, 9, 20).unwrap();
    assert_eq!(watchdog.read_register(SP805_RAW_ISR_OFFSET, 20).unwrap(), 0);
    assert_eq!(watchdog.read_register(SP805_LOAD_OFFSET, 20).unwrap(), 9);
}

#[test]
fn sp805_zero_load_uses_minimum_clock_interval_without_immediate_requeue() {
    let mut watchdog = Sp805Watchdog::new(1).unwrap();

    watchdog
        .write_register(SP805_CONTROL_OFFSET, 0x1, 4)
        .unwrap();
    watchdog.write_register(SP805_LOAD_OFFSET, 0, 4).unwrap();

    assert_eq!(watchdog.read_register(SP805_VALUE_OFFSET, 4).unwrap(), 0);
    assert_eq!(watchdog.read_register(SP805_RAW_ISR_OFFSET, 4).unwrap(), 0);
    assert_eq!(watchdog.next_timeout_tick(4).unwrap(), Some(5));

    let generation = watchdog.snapshot().generation();
    let timeout = watchdog.record_timeout(5, generation).unwrap().unwrap();
    assert!(timeout.interrupt_asserted());
    assert_eq!(watchdog.next_timeout_tick(5).unwrap(), Some(6));
}

#[test]
fn sp805_mmio_exposes_primecell_id_and_delivers_serial_interrupt() {
    let cpu = PartitionId::new(0);
    let watchdog_partition = PartitionId::new(1);
    let base = Address::new(0x1c0f_0000);
    let line = InterruptLineId::new(90);
    let target = InterruptTargetId::new(0);
    let source = InterruptSourceId::new(120);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let port = interrupt_port(&controller, line, target, cpu, 1);
    let device = Sp805WatchdogMmioDevice::with_interrupt(
        base,
        Sp805Watchdog::new(1).unwrap(),
        watchdog_partition,
        source,
        port,
    )
    .unwrap();
    assert_eq!(device.range_size_bytes(), SP805_MMIO_SIZE_BYTES);
    let responses = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&responses);
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(watchdog_partition, 5, {
            let device = device.clone();
            let responses = Arc::clone(&responses);
            move |context| {
                device
                    .respond(context, &write_request(1, base, SP805_LOAD_OFFSET, 3))
                    .unwrap();
                device
                    .respond(context, &write_request(2, base, SP805_CONTROL_OFFSET, 1))
                    .unwrap();
                responses
                    .lock()
                    .unwrap()
                    .push(device.respond(context, &read_request(3, base, SP805_CONTROL_OFFSET)));
            }
        })
        .unwrap();
    scheduler
        .schedule_at(watchdog_partition, 7, {
            let device = device.clone();
            let responses = Arc::clone(&responses);
            move |context| {
                responses
                    .lock()
                    .unwrap()
                    .push(device.respond(context, &read_request(4, base, SP805_VALUE_OFFSET)));
            }
        })
        .unwrap();
    scheduler
        .schedule_at(watchdog_partition, 9, {
            let device = device.clone();
            move |context| {
                device
                    .respond(context, &write_request(5, base, SP805_CONTROL_OFFSET, 0))
                    .unwrap();
                device
                    .respond(context, &write_request(6, base, SP805_INT_CLEAR_OFFSET, 1))
                    .unwrap();
            }
        })
        .unwrap();
    scheduler.run_until_idle();

    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[
            Ok(MmioResponse::completed(
                MmioRequestId::new(3),
                Some(le32(1))
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(4),
                Some(le32(1))
            )),
        ]
    );
    assert_eq!(
        controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(9, line, target, cpu, source, InterruptEventKind::Assert),
            InterruptEvent::routed(10, line, target, cpu, source, InterruptEventKind::Deassert),
        ]
    );

    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let id_responses = Arc::new(Mutex::new(Vec::new()));
    let observed_ids = Arc::clone(&id_responses);
    scheduler
        .schedule_at(cpu, 20, move |context| {
            for (index, offset) in [
                AMBA_PERIPHERAL_ID0_OFFSET,
                AMBA_PERIPHERAL_ID1_OFFSET,
                AMBA_PERIPHERAL_ID2_OFFSET,
                AMBA_PERIPHERAL_ID3_OFFSET,
                AMBA_CELL_ID0_OFFSET,
                AMBA_CELL_ID1_OFFSET,
                AMBA_CELL_ID2_OFFSET,
                AMBA_CELL_ID3_OFFSET,
            ]
            .into_iter()
            .enumerate()
            {
                id_responses
                    .lock()
                    .unwrap()
                    .push(device.respond(context, &read_request(100 + index as u64, base, offset)));
            }
        })
        .unwrap();
    scheduler.run_until_idle();

    assert_eq!(
        observed_ids.lock().unwrap().as_slice(),
        &[
            Ok(MmioResponse::completed(
                MmioRequestId::new(100),
                Some(le32(0x05)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(101),
                Some(le32(0x18)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(102),
                Some(le32(0x14)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(103),
                Some(le32(0x00)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(104),
                Some(le32(0x0d)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(105),
                Some(le32(0xf0)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(106),
                Some(le32(0x05)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(107),
                Some(le32(0xb1)),
            )),
        ]
    );
}

#[test]
fn sp805_mmio_supports_parallel_access_and_typed_errors() {
    let partition = PartitionId::new(0);
    let base = Address::new(0x2b06_0000);
    let device = Sp805WatchdogMmioDevice::new(base, Sp805Watchdog::new(1).unwrap());
    let responses = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&responses);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(1, 1).unwrap();

    scheduler
        .schedule_parallel_at(partition, 3, move |context| {
            device
                .respond_parallel(context, &write_request(1, base, SP805_LOAD_OFFSET, 5))
                .unwrap();
            responses
                .lock()
                .unwrap()
                .push(device.respond_parallel(context, &read_request(2, base, SP805_LOAD_OFFSET)));
            responses.lock().unwrap().push(
                device.respond_parallel(
                    context,
                    &MmioRequest::read(
                        MmioRequestId::new(3),
                        Address::new(base.get() + 0x80),
                        register_size(),
                    )
                    .unwrap(),
                ),
            );
            responses.lock().unwrap().push(
                device.respond_parallel(
                    context,
                    &MmioRequest::read(
                        MmioRequestId::new(4),
                        Address::new(base.get() + SP805_VALUE_OFFSET),
                        AccessSize::new(8).unwrap(),
                    )
                    .unwrap(),
                ),
            );
            responses.lock().unwrap().push(
                device.respond_parallel(context, &write_request(5, base, SP805_ITOP_OFFSET, 1)),
            );
        })
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[
            Ok(MmioResponse::completed(
                MmioRequestId::new(2),
                Some(le32(5))
            )),
            Err(MmioError::DeviceError {
                request: MmioRequestId::new(3),
                message: "unknown SP805 watchdog register offset 0x80".to_string(),
            }),
            Err(MmioError::AccessSizeMismatch {
                request: MmioRequestId::new(4),
                expected: SP805_REGISTER_BYTES,
                actual: 8,
            }),
            Err(MmioError::DeviceError {
                request: MmioRequestId::new(5),
                message: "SP805 integration test harness is not supported".to_string(),
            }),
        ]
    );
}

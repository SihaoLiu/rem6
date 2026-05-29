use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptEvent, InterruptEventKind, InterruptLineChannel, InterruptLineId,
    InterruptLinePort, InterruptRoute, InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{MmioError, MmioRequest, MmioRequestId, MmioResponse};
use rem6_timer::{
    Sp804DualTimer, Sp804DualTimerMmioDevice, Sp804TimerControl, AMBA_CELL_ID0_OFFSET,
    AMBA_CELL_ID1_OFFSET, AMBA_CELL_ID2_OFFSET, AMBA_CELL_ID3_OFFSET, AMBA_PERIPHERAL_ID0_OFFSET,
    AMBA_PERIPHERAL_ID1_OFFSET, AMBA_PERIPHERAL_ID2_OFFSET, AMBA_PERIPHERAL_ID3_OFFSET,
    SP804_BGLOAD_OFFSET, SP804_CONTROL_OFFSET, SP804_CURRENT_OFFSET, SP804_INT_CLEAR_OFFSET,
    SP804_LOAD_OFFSET, SP804_MASKED_ISR_OFFSET, SP804_MMIO_SIZE_BYTES, SP804_RAW_ISR_OFFSET,
    SP804_REGISTER_BYTES, SP804_TIMER_WINDOW_BYTES,
};

fn register_size() -> AccessSize {
    AccessSize::new(SP804_REGISTER_BYTES).unwrap()
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
fn sp804_core_counts_down_one_shot_and_clears_interrupt() {
    let mut timers = Sp804DualTimer::new(2, 4).unwrap();
    let control = Sp804TimerControl::default()
        .with_interrupt_enabled(true)
        .with_enabled(true)
        .with_one_shot(true);

    timers
        .timer_mut(0)
        .unwrap()
        .write_register(SP804_LOAD_OFFSET, 3, 10)
        .unwrap();
    timers
        .timer_mut(0)
        .unwrap()
        .write_register(SP804_CONTROL_OFFSET, control.bits(), 10)
        .unwrap();

    assert_eq!(
        timers
            .timer(0)
            .unwrap()
            .read_register(SP804_CURRENT_OFFSET, 14)
            .unwrap(),
        1
    );
    assert_eq!(
        timers.timer(0).unwrap().next_zero_tick(14).unwrap(),
        Some(16)
    );
    assert!(timers.timer_mut(0).unwrap().record_zero(16).unwrap().0);
    assert_eq!(
        timers
            .timer(0)
            .unwrap()
            .read_register(SP804_RAW_ISR_OFFSET, 16)
            .unwrap(),
        1
    );
    assert_eq!(
        timers
            .timer(0)
            .unwrap()
            .read_register(SP804_MASKED_ISR_OFFSET, 16)
            .unwrap(),
        1
    );
    assert_eq!(
        timers
            .timer(0)
            .unwrap()
            .read_register(SP804_CURRENT_OFFSET, 18)
            .unwrap(),
        0
    );

    timers
        .timer_mut(0)
        .unwrap()
        .write_register(SP804_INT_CLEAR_OFFSET, 1, 19)
        .unwrap();
    assert_eq!(
        timers
            .timer(0)
            .unwrap()
            .read_register(SP804_RAW_ISR_OFFSET, 19)
            .unwrap(),
        0
    );
    assert_eq!(timers.timer(0).unwrap().next_zero_tick(19).unwrap(), None);
}

#[test]
fn sp804_core_periodic_reload_uses_background_load_and_prescale() {
    let mut timers = Sp804DualTimer::new(1, 8).unwrap();
    let control = Sp804TimerControl::default()
        .with_interrupt_enabled(true)
        .with_periodic(true)
        .with_enabled(true)
        .with_prescale(1)
        .unwrap();

    timers
        .timer_mut(1)
        .unwrap()
        .write_register(SP804_LOAD_OFFSET, 4, 0)
        .unwrap();
    timers
        .timer_mut(1)
        .unwrap()
        .write_register(SP804_BGLOAD_OFFSET, 2, 0)
        .unwrap();
    timers
        .timer_mut(1)
        .unwrap()
        .write_register(SP804_CONTROL_OFFSET, control.bits(), 0)
        .unwrap();

    assert_eq!(
        timers.timer(1).unwrap().next_zero_tick(0).unwrap(),
        Some(512)
    );
    let (_, next_generation) = timers.timer_mut(1).unwrap().record_zero(512).unwrap();
    assert_eq!(
        timers
            .timer(1)
            .unwrap()
            .read_register(SP804_CURRENT_OFFSET, 512)
            .unwrap(),
        2
    );
    assert_eq!(
        timers.timer(1).unwrap().next_zero_tick(512).unwrap(),
        Some(768)
    );
    assert_eq!(
        next_generation,
        Some(timers.timer(1).unwrap().snapshot().generation())
    );
}

#[test]
fn sp804_mmio_routes_dual_timer_windows_and_delivers_serial_interrupt() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let base = Address::new(0x1c11_0000);
    let line0 = InterruptLineId::new(80);
    let line1 = InterruptLineId::new(81);
    let target = InterruptTargetId::new(0);
    let source0 = InterruptSourceId::new(100);
    let source1 = InterruptSourceId::new(101);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let port0 = interrupt_port(&controller, line0, target, cpu, 2);
    let port1 = interrupt_port(&controller, line1, target, cpu, 2);
    let device = Sp804DualTimerMmioDevice::with_interrupts(
        base,
        Sp804DualTimer::new(1, 1).unwrap(),
        timer_partition,
        [(source0, port0), (source1, port1)],
    )
    .unwrap();
    let monitor = device.clone();
    let responses = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&responses);
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(timer_partition, 5, move |context| {
            let control = Sp804TimerControl::default()
                .with_interrupt_enabled(true)
                .with_enabled(true)
                .with_one_shot(true);
            device
                .respond(context, &write_request(1, base, SP804_LOAD_OFFSET, 3))
                .unwrap();
            device
                .respond(
                    context,
                    &write_request(2, base, SP804_CONTROL_OFFSET, control.bits()),
                )
                .unwrap();
            device
                .respond(
                    context,
                    &write_request(3, base, SP804_TIMER_WINDOW_BYTES + SP804_LOAD_OFFSET, 9),
                )
                .unwrap();
            observed.lock().unwrap().push(device.respond(
                context,
                &read_request(4, base, SP804_TIMER_WINDOW_BYTES + SP804_LOAD_OFFSET),
            ));
        })
        .unwrap();
    scheduler.run_until_idle();

    assert_eq!(
        responses.lock().unwrap().as_slice(),
        &[Ok(MmioResponse::completed(
            MmioRequestId::new(4),
            Some(le32(9)),
        ))]
    );
    assert_eq!(
        controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(10, line0, target, cpu, source0, InterruptEventKind::Assert),
            InterruptEvent::routed(
                10,
                line0,
                target,
                cpu,
                source0,
                InterruptEventKind::Deassert
            ),
        ]
    );
    assert!(monitor.snapshot().timer(0).unwrap().raw_interrupt());
    assert!(!monitor.snapshot().timer(1).unwrap().raw_interrupt());
}

#[test]
fn sp804_mmio_supports_parallel_access_and_typed_errors() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let base = Address::new(0x1c12_0000);
    let line0 = InterruptLineId::new(82);
    let line1 = InterruptLineId::new(83);
    let target = InterruptTargetId::new(0);
    let source0 = InterruptSourceId::new(102);
    let source1 = InterruptSourceId::new(103);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let port0 = interrupt_port(&controller, line0, target, cpu, 2);
    let port1 = interrupt_port(&controller, line1, target, cpu, 2);
    let device = Sp804DualTimerMmioDevice::with_interrupts(
        base,
        Sp804DualTimer::new(1, 1).unwrap(),
        timer_partition,
        [(source0, port0), (source1, port1)],
    )
    .unwrap();
    assert_eq!(device.range_size_bytes(), SP804_MMIO_SIZE_BYTES);
    let responses = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&responses);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();

    scheduler
        .schedule_parallel_at(timer_partition, 3, move |context| {
            let control = Sp804TimerControl::default()
                .with_interrupt_enabled(true)
                .with_enabled(true)
                .with_one_shot(true);
            device
                .respond_parallel(context, &write_request(10, base, SP804_LOAD_OFFSET, 2))
                .unwrap();
            device
                .respond_parallel(
                    context,
                    &write_request(11, base, SP804_CONTROL_OFFSET, control.bits()),
                )
                .unwrap();
            responses.lock().unwrap().push(
                device.respond_parallel(context, &read_request(12, base, SP804_CONTROL_OFFSET)),
            );
            responses.lock().unwrap().push(
                device.respond_parallel(
                    context,
                    &MmioRequest::read(
                        MmioRequestId::new(13),
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
                        MmioRequestId::new(14),
                        Address::new(base.get() + SP804_LOAD_OFFSET),
                        AccessSize::new(8).unwrap(),
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
                MmioRequestId::new(12),
                Some(le32(0xa1)),
            )),
            Err(MmioError::DeviceError {
                request: MmioRequestId::new(13),
                message: "unknown SP804 timer register offset 0x80".to_string(),
            }),
            Err(MmioError::AccessSizeMismatch {
                request: MmioRequestId::new(14),
                expected: SP804_REGISTER_BYTES,
                actual: 8,
            }),
        ]
    );
    assert_eq!(
        controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(7, line0, target, cpu, source0, InterruptEventKind::Assert),
            InterruptEvent::routed(7, line0, target, cpu, source0, InterruptEventKind::Deassert),
        ]
    );
}

#[test]
fn sp804_mmio_exposes_gem5_primecell_id_registers() {
    let timer_partition = PartitionId::new(0);
    let base = Address::new(0x1c11_0000);
    let device = Sp804DualTimerMmioDevice::new(base, Sp804DualTimer::new(1, 1).unwrap());
    let responses = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&responses);
    let mut scheduler = PartitionedScheduler::new(1).unwrap();

    scheduler
        .schedule_at(timer_partition, 1, move |context| {
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
                responses
                    .lock()
                    .unwrap()
                    .push(device.respond(context, &read_request(100 + index as u64, base, offset)));
            }
            device
                .respond(
                    context,
                    &write_request(200, base, AMBA_PERIPHERAL_ID0_OFFSET, 0xff),
                )
                .unwrap();
            responses.lock().unwrap().push(device.respond(
                context,
                &read_request(201, base, AMBA_PERIPHERAL_ID0_OFFSET),
            ));
        })
        .unwrap();
    scheduler.run_until_idle();

    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[
            Ok(MmioResponse::completed(
                MmioRequestId::new(100),
                Some(le32(0x04)),
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
            Ok(MmioResponse::completed(
                MmioRequestId::new(201),
                Some(le32(0x04)),
            )),
        ]
    );
}

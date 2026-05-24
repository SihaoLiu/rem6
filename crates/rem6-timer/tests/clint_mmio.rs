use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptEvent, InterruptEventKind, InterruptLineChannel, InterruptLineId,
    InterruptLinePort, InterruptRoute, InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{MmioRequest, MmioRequestId, MmioResponse};
use rem6_timer::{
    ClintHartConfig, ClintMmioDevice, CLINT_MSIP_BASE_OFFSET, CLINT_MSIP_REGISTER_BYTES,
    CLINT_MTIMECMP_BASE_OFFSET, CLINT_MTIMECMP_REGISTER_BYTES, CLINT_MTIME_OFFSET,
    CLINT_MTIME_REGISTER_BYTES,
};

fn le32(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn le64(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn full_mask(bytes: u64) -> ByteMask {
    ByteMask::full(AccessSize::new(bytes).unwrap()).unwrap()
}

fn interrupt_port(
    controller: &Arc<Mutex<InterruptController>>,
    line: InterruptLineId,
    target: InterruptTargetId,
    target_partition: PartitionId,
) -> InterruptLinePort {
    let route = InterruptRoute::new(line, target, target_partition);
    controller.lock().unwrap().register_route(route).unwrap();
    InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(controller),
    )
}

#[test]
fn clint_mmio_msip_write_asserts_software_interrupt() {
    let cpu = PartitionId::new(0);
    let clint_partition = PartitionId::new(1);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let software_line = InterruptLineId::new(70);
    let timer_line = InterruptLineId::new(71);
    let target = InterruptTargetId::new(0);
    let software_source = InterruptSourceId::new(40);
    let timer_source = InterruptSourceId::new(41);
    let software_port = interrupt_port(&controller, software_line, target, cpu);
    let timer_port = interrupt_port(&controller, timer_line, target, cpu);
    let device = ClintMmioDevice::new(
        Address::new(0x200_0000),
        [ClintHartConfig::new(
            0,
            software_port,
            software_source,
            timer_port,
            timer_source,
        )],
    )
    .unwrap();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let observed_completions = Arc::clone(&completions);
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(clint_partition, 5, move |context| {
            let response = device
                .respond(
                    context,
                    &MmioRequest::write(
                        MmioRequestId::new(1),
                        Address::new(0x200_0000 + CLINT_MSIP_BASE_OFFSET),
                        le32(1),
                        full_mask(CLINT_MSIP_REGISTER_BYTES),
                    )
                    .unwrap(),
                )
                .unwrap();
            completions.lock().unwrap().push(response);
        })
        .unwrap();

    scheduler.run_until_idle();

    assert_eq!(
        observed_completions.lock().unwrap().as_slice(),
        &[MmioResponse::completed(MmioRequestId::new(1), None)]
    );
    assert_eq!(
        controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            7,
            software_line,
            target,
            cpu,
            software_source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn clint_mmio_mtimecmp_write_schedules_timer_interrupt() {
    let cpu = PartitionId::new(0);
    let clint_partition = PartitionId::new(1);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let software_line = InterruptLineId::new(72);
    let timer_line = InterruptLineId::new(73);
    let target = InterruptTargetId::new(0);
    let software_source = InterruptSourceId::new(42);
    let timer_source = InterruptSourceId::new(43);
    let software_port = interrupt_port(&controller, software_line, target, cpu);
    let timer_port = interrupt_port(&controller, timer_line, target, cpu);
    let device = ClintMmioDevice::new(
        Address::new(0x200_0000),
        [ClintHartConfig::new(
            0,
            software_port,
            software_source,
            timer_port,
            timer_source,
        )],
    )
    .unwrap();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let observed_completions = Arc::clone(&completions);
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(clint_partition, 5, move |context| {
            let response = device
                .respond(
                    context,
                    &MmioRequest::write(
                        MmioRequestId::new(2),
                        Address::new(0x200_0000 + CLINT_MTIMECMP_BASE_OFFSET),
                        le64(10),
                        full_mask(CLINT_MTIMECMP_REGISTER_BYTES),
                    )
                    .unwrap(),
                )
                .unwrap();
            completions.lock().unwrap().push(response);
        })
        .unwrap();

    scheduler.run_until_idle();

    assert_eq!(
        observed_completions.lock().unwrap().as_slice(),
        &[MmioResponse::completed(MmioRequestId::new(2), None)]
    );
    assert_eq!(
        controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            12,
            timer_line,
            target,
            cpu,
            timer_source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn clint_mmio_mtimecmp_future_write_deasserts_timer_interrupt() {
    let cpu = PartitionId::new(0);
    let clint_partition = PartitionId::new(1);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let software_line = InterruptLineId::new(76);
    let timer_line = InterruptLineId::new(77);
    let target = InterruptTargetId::new(0);
    let software_source = InterruptSourceId::new(46);
    let timer_source = InterruptSourceId::new(47);
    let software_port = interrupt_port(&controller, software_line, target, cpu);
    let timer_port = interrupt_port(&controller, timer_line, target, cpu);
    let device = ClintMmioDevice::new(
        Address::new(0x200_0000),
        [ClintHartConfig::new(
            0,
            software_port,
            software_source,
            timer_port,
            timer_source,
        )],
    )
    .unwrap();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    let arm_device = device.clone();
    scheduler
        .schedule_at(clint_partition, 5, move |context| {
            arm_device
                .respond(
                    context,
                    &MmioRequest::write(
                        MmioRequestId::new(4),
                        Address::new(0x200_0000 + CLINT_MTIMECMP_BASE_OFFSET),
                        le64(6),
                        full_mask(CLINT_MTIMECMP_REGISTER_BYTES),
                    )
                    .unwrap(),
                )
                .unwrap();
        })
        .unwrap();

    scheduler
        .schedule_at(clint_partition, 9, move |context| {
            device
                .respond(
                    context,
                    &MmioRequest::write(
                        MmioRequestId::new(5),
                        Address::new(0x200_0000 + CLINT_MTIMECMP_BASE_OFFSET),
                        le64(20),
                        full_mask(CLINT_MTIMECMP_REGISTER_BYTES),
                    )
                    .unwrap(),
                )
                .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle();

    assert_eq!(
        controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(
                8,
                timer_line,
                target,
                cpu,
                timer_source,
                InterruptEventKind::Assert,
            ),
            InterruptEvent::routed(
                11,
                timer_line,
                target,
                cpu,
                timer_source,
                InterruptEventKind::Deassert,
            ),
            InterruptEvent::routed(
                22,
                timer_line,
                target,
                cpu,
                timer_source,
                InterruptEventKind::Assert,
            ),
        ]
    );
}

#[test]
fn clint_mmio_mtime_read_returns_scheduler_tick() {
    let cpu = PartitionId::new(0);
    let clint_partition = PartitionId::new(1);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let software_port = interrupt_port(
        &controller,
        InterruptLineId::new(74),
        InterruptTargetId::new(0),
        cpu,
    );
    let timer_port = interrupt_port(
        &controller,
        InterruptLineId::new(75),
        InterruptTargetId::new(0),
        cpu,
    );
    let device = ClintMmioDevice::new(
        Address::new(0x200_0000),
        [ClintHartConfig::new(
            0,
            software_port,
            InterruptSourceId::new(44),
            timer_port,
            InterruptSourceId::new(45),
        )],
    )
    .unwrap();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let observed_completions = Arc::clone(&completions);
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(clint_partition, 13, move |context| {
            let response = device
                .respond(
                    context,
                    &MmioRequest::read(
                        MmioRequestId::new(3),
                        Address::new(0x200_0000 + CLINT_MTIME_OFFSET),
                        AccessSize::new(CLINT_MTIME_REGISTER_BYTES).unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap();
            completions.lock().unwrap().push(response);
        })
        .unwrap();

    scheduler.run_until_idle();

    assert_eq!(
        observed_completions.lock().unwrap().as_slice(),
        &[MmioResponse::completed(
            MmioRequestId::new(3),
            Some(le64(13)),
        )]
    );
}

#[test]
fn clint_parallel_mtimecmp_write_schedules_timer_interrupt() {
    let cpu = PartitionId::new(0);
    let clint_partition = PartitionId::new(1);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let software_line = InterruptLineId::new(78);
    let timer_line = InterruptLineId::new(79);
    let target = InterruptTargetId::new(0);
    let software_source = InterruptSourceId::new(48);
    let timer_source = InterruptSourceId::new(49);
    let software_port = interrupt_port(&controller, software_line, target, cpu);
    let timer_port = interrupt_port(&controller, timer_line, target, cpu);
    let device = ClintMmioDevice::new(
        Address::new(0x200_0000),
        [ClintHartConfig::new(
            0,
            software_port,
            software_source,
            timer_port,
            timer_source,
        )],
    )
    .unwrap();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let observed_completions = Arc::clone(&completions);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    scheduler
        .schedule_parallel_at(clint_partition, 5, move |context| {
            let response = device
                .respond_parallel(
                    context,
                    &MmioRequest::write(
                        MmioRequestId::new(6),
                        Address::new(0x200_0000 + CLINT_MTIMECMP_BASE_OFFSET),
                        le64(10),
                        full_mask(CLINT_MTIMECMP_REGISTER_BYTES),
                    )
                    .unwrap(),
                )
                .unwrap();
            completions.lock().unwrap().push(response);
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        observed_completions.lock().unwrap().as_slice(),
        &[MmioResponse::completed(MmioRequestId::new(6), None)]
    );
    assert_eq!(
        controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            12,
            timer_line,
            target,
            cpu,
            timer_source,
            InterruptEventKind::Assert,
        )]
    );
}

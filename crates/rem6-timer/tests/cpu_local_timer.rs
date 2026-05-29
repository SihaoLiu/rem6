use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptEvent, InterruptEventKind, InterruptLineChannel, InterruptLineId,
    InterruptLinePort, InterruptRoute, InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{MmioRequest, MmioRequestId};
use rem6_timer::{
    CpuLocalTimerBank, CpuLocalTimerControl, CpuLocalTimerInterruptPorts, CpuLocalTimerMmioDevice,
    CpuLocalWatchdogControl, CPU_LOCAL_TIMER_CONTROL_OFFSET, CPU_LOCAL_TIMER_COUNTER_OFFSET,
    CPU_LOCAL_TIMER_INT_STATUS_OFFSET, CPU_LOCAL_TIMER_LOAD_OFFSET, CPU_LOCAL_TIMER_REGISTER_BYTES,
    CPU_LOCAL_WATCHDOG_CONTROL_OFFSET, CPU_LOCAL_WATCHDOG_COUNTER_OFFSET,
    CPU_LOCAL_WATCHDOG_DISABLE_OFFSET, CPU_LOCAL_WATCHDOG_INT_STATUS_OFFSET,
    CPU_LOCAL_WATCHDOG_LOAD_OFFSET, CPU_LOCAL_WATCHDOG_RESET_STATUS_OFFSET,
};

fn register_size() -> AccessSize {
    AccessSize::new(CPU_LOCAL_TIMER_REGISTER_BYTES).unwrap()
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

struct MmioFixture {
    cpu: PartitionId,
    base: Address,
    timer_line: InterruptLineId,
    timer_source: InterruptSourceId,
    target: InterruptTargetId,
    controller: Arc<Mutex<InterruptController>>,
    device: CpuLocalTimerMmioDevice,
}

fn mmio_fixture(base: u64, timer_line: u64, timer_source: u32) -> MmioFixture {
    let cpu = PartitionId::new(0);
    let base = Address::new(base);
    let timer_line = InterruptLineId::new(timer_line);
    let watchdog_line = InterruptLineId::new(timer_line.get() + 1);
    let timer_source = InterruptSourceId::new(timer_source);
    let watchdog_source = InterruptSourceId::new(timer_source.get() + 1);
    let target = InterruptTargetId::new(0);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let timer_port = interrupt_port(&controller, timer_line, target, cpu, 1);
    let watchdog_port = interrupt_port(&controller, watchdog_line, target, cpu, 1);
    let device = CpuLocalTimerMmioDevice::with_interrupts(
        base,
        CpuLocalTimerBank::new(1, 1).unwrap(),
        vec![CpuLocalTimerInterruptPorts::new(
            cpu,
            timer_source,
            timer_port,
            watchdog_source,
            watchdog_port,
        )],
    )
    .unwrap();

    MmioFixture {
        cpu,
        base,
        timer_line,
        timer_source,
        target,
        controller,
        device,
    }
}

#[test]
fn cpu_local_timer_core_counts_down_autoreloads_and_clears_timer_interrupt() {
    let mut bank = CpuLocalTimerBank::new(2, 2).unwrap();
    let control = CpuLocalTimerControl::new(0)
        .with_interrupt_enabled(true)
        .with_auto_reload(true)
        .with_enabled(true);

    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_TIMER_LOAD_OFFSET, 3, 10)
        .unwrap();
    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_TIMER_CONTROL_OFFSET, control.bits(), 10)
        .unwrap();

    assert_eq!(
        bank.cpu(0)
            .unwrap()
            .read_register(CPU_LOCAL_TIMER_COUNTER_OFFSET, 12)
            .unwrap(),
        2
    );
    assert_eq!(
        bank.cpu(0).unwrap().next_timer_zero_tick(12).unwrap(),
        Some(16)
    );
    let generation = bank.cpu(0).unwrap().snapshot().timer().generation();
    let outcome = bank
        .cpu_mut(0)
        .unwrap()
        .record_timer_zero(16, generation)
        .unwrap()
        .unwrap();
    assert!(outcome.interrupt_asserted());
    assert_eq!(outcome.next_generation(), Some(generation + 1));
    assert_eq!(
        bank.cpu(0)
            .unwrap()
            .read_register(CPU_LOCAL_TIMER_INT_STATUS_OFFSET, 16)
            .unwrap(),
        1
    );
    assert_eq!(
        bank.cpu(0).unwrap().next_timer_zero_tick(16).unwrap(),
        Some(22)
    );

    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_TIMER_INT_STATUS_OFFSET, 1, 17)
        .unwrap();
    assert_eq!(
        bank.cpu(0)
            .unwrap()
            .read_register(CPU_LOCAL_TIMER_INT_STATUS_OFFSET, 17)
            .unwrap(),
        0
    );
}

#[test]
fn cpu_local_timer_zero_load_autoreload_uses_minimum_decrement_tick() {
    let mut bank = CpuLocalTimerBank::new(1, 4).unwrap();
    let control = CpuLocalTimerControl::new(0)
        .with_auto_reload(true)
        .with_enabled(true);

    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_TIMER_LOAD_OFFSET, 0, 10)
        .unwrap();
    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_TIMER_CONTROL_OFFSET, control.bits(), 10)
        .unwrap();

    assert_eq!(
        bank.cpu(0).unwrap().next_timer_zero_tick(10).unwrap(),
        Some(14)
    );
    let generation = bank.cpu(0).unwrap().snapshot().timer().generation();
    let outcome = bank
        .cpu_mut(0)
        .unwrap()
        .record_timer_zero(14, generation)
        .unwrap()
        .unwrap();
    assert_eq!(outcome.next_generation(), Some(generation + 1));
    assert_eq!(
        bank.cpu(0).unwrap().next_timer_zero_tick(14).unwrap(),
        Some(18)
    );
}

#[test]
fn cpu_local_watchdog_records_reset_mode_without_fatal_and_requires_disable_sequence() {
    let mut bank = CpuLocalTimerBank::new(1, 1).unwrap();
    let watchdog_mode = CpuLocalWatchdogControl::new(0)
        .with_interrupt_enabled(true)
        .with_watchdog_mode(true)
        .with_enabled(true);

    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_WATCHDOG_LOAD_OFFSET, 2, 0)
        .unwrap();
    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_WATCHDOG_CONTROL_OFFSET, watchdog_mode.bits(), 0)
        .unwrap();

    assert_eq!(
        bank.cpu(0)
            .unwrap()
            .read_register(CPU_LOCAL_WATCHDOG_COUNTER_OFFSET, 1)
            .unwrap(),
        1
    );
    let generation = bank.cpu(0).unwrap().snapshot().watchdog().generation();
    let outcome = bank
        .cpu_mut(0)
        .unwrap()
        .record_watchdog_zero(2, generation)
        .unwrap()
        .unwrap();
    assert!(!outcome.interrupt_asserted());
    assert!(outcome.reset_asserted());
    assert_eq!(
        bank.cpu(0)
            .unwrap()
            .snapshot()
            .watchdog()
            .reset_assertions(),
        &[2]
    );
    assert_eq!(
        bank.cpu(0)
            .unwrap()
            .read_register(CPU_LOCAL_WATCHDOG_INT_STATUS_OFFSET, 2)
            .unwrap(),
        1
    );
    assert_eq!(
        bank.cpu(0)
            .unwrap()
            .read_register(CPU_LOCAL_WATCHDOG_RESET_STATUS_OFFSET, 2)
            .unwrap(),
        1
    );

    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_WATCHDOG_CONTROL_OFFSET, 0, 3)
        .unwrap();
    assert!(CpuLocalWatchdogControl::new(
        bank.cpu(0)
            .unwrap()
            .read_register(CPU_LOCAL_WATCHDOG_CONTROL_OFFSET, 3)
            .unwrap(),
    )
    .watchdog_mode());

    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_WATCHDOG_DISABLE_OFFSET, 0x1234_5678, 4)
        .unwrap();
    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_WATCHDOG_DISABLE_OFFSET, 0x8765_4321, 5)
        .unwrap();
    bank.cpu_mut(0)
        .unwrap()
        .write_register(CPU_LOCAL_WATCHDOG_CONTROL_OFFSET, 0, 6)
        .unwrap();
    assert!(!CpuLocalWatchdogControl::new(
        bank.cpu(0)
            .unwrap()
            .read_register(CPU_LOCAL_WATCHDOG_CONTROL_OFFSET, 6)
            .unwrap(),
    )
    .watchdog_mode());
}

#[test]
fn cpu_local_timer_mmio_selects_cpu_from_scheduler_partition_and_deasserts_on_clear() {
    let fixture = mmio_fixture(0x1c06_0000, 44, 88);
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let control = CpuLocalTimerControl::new(0)
        .with_interrupt_enabled(true)
        .with_enabled(true);
    let arm_device = fixture.device.clone();
    let clear_device = fixture.device.clone();
    let base = fixture.base;

    scheduler
        .schedule_at(fixture.cpu, 4, move |context| {
            arm_device
                .respond(
                    context,
                    &write_request(1, base, CPU_LOCAL_TIMER_LOAD_OFFSET, 2),
                )
                .unwrap();
            arm_device
                .respond(
                    context,
                    &write_request(2, base, CPU_LOCAL_TIMER_CONTROL_OFFSET, control.bits()),
                )
                .unwrap();
        })
        .unwrap();
    scheduler
        .schedule_at(fixture.cpu, 8, move |context| {
            clear_device
                .respond(
                    context,
                    &write_request(3, base, CPU_LOCAL_TIMER_INT_STATUS_OFFSET, 1),
                )
                .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle();

    assert_eq!(
        fixture.controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(
                7,
                fixture.timer_line,
                fixture.target,
                fixture.cpu,
                fixture.timer_source,
                InterruptEventKind::Assert,
            ),
            InterruptEvent::routed(
                9,
                fixture.timer_line,
                fixture.target,
                fixture.cpu,
                fixture.timer_source,
                InterruptEventKind::Deassert,
            ),
        ]
    );
    assert!(!fixture
        .device
        .snapshot()
        .cpu(0)
        .unwrap()
        .timer()
        .raw_interrupt());
}

#[test]
fn cpu_local_timer_mmio_uses_parallel_scheduler_path_for_timer_interrupts() {
    let fixture = mmio_fixture(0x1c06_1000, 46, 90);
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let control = CpuLocalTimerControl::new(0)
        .with_interrupt_enabled(true)
        .with_enabled(true);
    let arm_device = fixture.device.clone();
    let base = fixture.base;

    scheduler
        .schedule_parallel_at(fixture.cpu, 4, move |context| {
            arm_device
                .respond_parallel(
                    context,
                    &write_request(4, base, CPU_LOCAL_TIMER_LOAD_OFFSET, 2),
                )
                .unwrap();
            arm_device
                .respond_parallel(
                    context,
                    &write_request(5, base, CPU_LOCAL_TIMER_CONTROL_OFFSET, control.bits()),
                )
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.final_tick(), 7);
    assert_eq!(
        fixture.controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            7,
            fixture.timer_line,
            fixture.target,
            fixture.cpu,
            fixture.timer_source,
            InterruptEventKind::Assert,
        )]
    );
    assert!(fixture
        .device
        .snapshot()
        .cpu(0)
        .unwrap()
        .timer()
        .raw_interrupt());
}

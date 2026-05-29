use rem6_interrupt::{
    InterruptEvent, InterruptEventKind, InterruptLineId, InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{MmioCompletion, MmioRequest, MmioRequestId, MmioResponse, MmioRoute};
use rem6_platform::{
    PlatformBuilder, PlatformCpuLocalTimerConfig, PlatformCpuLocalTimerCpuConfig,
    PlatformCpuLocalTimerInterruptConfig, PlatformError,
};
use rem6_timer::{
    CpuLocalTimerControl, CPU_LOCAL_TIMER_CONTROL_OFFSET, CPU_LOCAL_TIMER_LOAD_OFFSET,
    CPU_LOCAL_TIMER_MMIO_SIZE_BYTES, CPU_LOCAL_TIMER_REGISTER_BYTES,
};

fn full_mask(bytes: u64) -> ByteMask {
    ByteMask::full(AccessSize::new(bytes).unwrap()).unwrap()
}

fn le32(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

#[test]
fn platform_builder_wires_cpu_local_timer_mmio_interrupts_and_retains_device() {
    let cpu0 = PartitionId::new(0);
    let cpu1 = PartitionId::new(1);
    let base = Address::new(0x1c06_0000);
    let timer_line0 = InterruptLineId::new(47);
    let watchdog_line0 = InterruptLineId::new(48);
    let timer_line1 = InterruptLineId::new(49);
    let watchdog_line1 = InterruptLineId::new(50);
    let timer_source0 = InterruptSourceId::new(90);
    let watchdog_source0 = InterruptSourceId::new(91);
    let timer_source1 = InterruptSourceId::new(92);
    let watchdog_source1 = InterruptSourceId::new(93);
    let target = InterruptTargetId::new(0);
    let route0 = MmioRoute::new(cpu0, cpu0, 1, 1).unwrap();
    let route1 = MmioRoute::new(cpu1, cpu1, 1, 1).unwrap();

    let platform = PlatformBuilder::new(2)
        .add_cpu_local_timer(PlatformCpuLocalTimerConfig {
            base,
            size: AccessSize::new(CPU_LOCAL_TIMER_MMIO_SIZE_BYTES).unwrap(),
            routes: vec![route0, route1],
            clock_tick: 1,
            cpus: vec![
                PlatformCpuLocalTimerCpuConfig {
                    partition: cpu0,
                    timer: PlatformCpuLocalTimerInterruptConfig {
                        line: timer_line0,
                        target,
                        source: timer_source0,
                        latency: 1,
                    },
                    watchdog: PlatformCpuLocalTimerInterruptConfig {
                        line: watchdog_line0,
                        target,
                        source: watchdog_source0,
                        latency: 1,
                    },
                },
                PlatformCpuLocalTimerCpuConfig {
                    partition: cpu1,
                    timer: PlatformCpuLocalTimerInterruptConfig {
                        line: timer_line1,
                        target,
                        source: timer_source1,
                        latency: 1,
                    },
                    watchdog: PlatformCpuLocalTimerInterruptConfig {
                        line: watchdog_line1,
                        target,
                        source: watchdog_source1,
                        latency: 1,
                    },
                },
            ],
        })
        .build()
        .unwrap();

    assert_eq!(
        platform
            .cpu_local_timers()
            .map(|(device_base, _)| device_base)
            .collect::<Vec<_>>(),
        vec![base]
    );
    let timer = platform.cpu_local_timer(base).unwrap().clone();
    let bus = platform.mmio_bus().clone();
    let controller = platform.interrupt_controller();
    let completions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(platform.partition_count()).unwrap();

    let completed = std::sync::Arc::clone(&completions);
    scheduler
        .schedule_at(cpu0, 4, {
            let bus = bus.clone();
            move |context| {
                let load_completed = std::sync::Arc::clone(&completed);
                bus.submit(
                    context,
                    MmioRequest::write(
                        MmioRequestId::new(90),
                        Address::new(base.get() + CPU_LOCAL_TIMER_LOAD_OFFSET),
                        le32(2),
                        full_mask(CPU_LOCAL_TIMER_REGISTER_BYTES),
                    )
                    .unwrap(),
                    move |completion| load_completed.lock().unwrap().push(completion),
                )
                .unwrap();

                let control = CpuLocalTimerControl::new(0)
                    .with_interrupt_enabled(true)
                    .with_enabled(true);
                bus.submit(
                    context,
                    MmioRequest::write(
                        MmioRequestId::new(91),
                        Address::new(base.get() + CPU_LOCAL_TIMER_CONTROL_OFFSET),
                        le32(control.bits()),
                        full_mask(CPU_LOCAL_TIMER_REGISTER_BYTES),
                    )
                    .unwrap(),
                    move |completion| completed.lock().unwrap().push(completion),
                )
                .unwrap();
            }
        })
        .unwrap();
    let completed = std::sync::Arc::clone(&completions);
    scheduler
        .schedule_at(cpu1, 5, move |context| {
            let load_completed = std::sync::Arc::clone(&completed);
            bus.submit(
                context,
                MmioRequest::write(
                    MmioRequestId::new(92),
                    Address::new(base.get() + CPU_LOCAL_TIMER_LOAD_OFFSET),
                    le32(2),
                    full_mask(CPU_LOCAL_TIMER_REGISTER_BYTES),
                )
                .unwrap(),
                move |completion| load_completed.lock().unwrap().push(completion),
            )
            .unwrap();

            let control = CpuLocalTimerControl::new(0)
                .with_interrupt_enabled(true)
                .with_enabled(true);
            bus.submit(
                context,
                MmioRequest::write(
                    MmioRequestId::new(93),
                    Address::new(base.get() + CPU_LOCAL_TIMER_CONTROL_OFFSET),
                    le32(control.bits()),
                    full_mask(CPU_LOCAL_TIMER_REGISTER_BYTES),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.final_tick(), 9);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[
            MmioCompletion::new(
                6,
                route0,
                Ok(MmioResponse::completed(MmioRequestId::new(90), None)),
            ),
            MmioCompletion::new(
                6,
                route0,
                Ok(MmioResponse::completed(MmioRequestId::new(91), None)),
            ),
            MmioCompletion::new(
                7,
                route1,
                Ok(MmioResponse::completed(MmioRequestId::new(92), None)),
            ),
            MmioCompletion::new(
                7,
                route1,
                Ok(MmioResponse::completed(MmioRequestId::new(93), None)),
            ),
        ]
    );
    assert_eq!(
        controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(
                8,
                timer_line0,
                target,
                cpu0,
                timer_source0,
                InterruptEventKind::Assert,
            ),
            InterruptEvent::routed(
                9,
                timer_line1,
                target,
                cpu1,
                timer_source1,
                InterruptEventKind::Assert,
            ),
        ]
    );
    assert!(timer.snapshot().cpu(0).unwrap().timer().raw_interrupt());
    assert!(timer.snapshot().cpu(1).unwrap().timer().raw_interrupt());
}

#[test]
fn platform_builder_rejects_cpu_local_timer_without_route_for_cpu_partition() {
    let cpu0 = PartitionId::new(0);
    let cpu1 = PartitionId::new(1);
    let base = Address::new(0x1c06_1000);
    let config = PlatformCpuLocalTimerConfig {
        base,
        size: AccessSize::new(CPU_LOCAL_TIMER_MMIO_SIZE_BYTES).unwrap(),
        routes: vec![MmioRoute::new(cpu0, cpu0, 1, 1).unwrap()],
        clock_tick: 1,
        cpus: vec![
            PlatformCpuLocalTimerCpuConfig {
                partition: cpu0,
                timer: PlatformCpuLocalTimerInterruptConfig {
                    line: InterruptLineId::new(51),
                    target: InterruptTargetId::new(0),
                    source: InterruptSourceId::new(94),
                    latency: 1,
                },
                watchdog: PlatformCpuLocalTimerInterruptConfig {
                    line: InterruptLineId::new(52),
                    target: InterruptTargetId::new(0),
                    source: InterruptSourceId::new(95),
                    latency: 1,
                },
            },
            PlatformCpuLocalTimerCpuConfig {
                partition: cpu1,
                timer: PlatformCpuLocalTimerInterruptConfig {
                    line: InterruptLineId::new(53),
                    target: InterruptTargetId::new(0),
                    source: InterruptSourceId::new(96),
                    latency: 1,
                },
                watchdog: PlatformCpuLocalTimerInterruptConfig {
                    line: InterruptLineId::new(54),
                    target: InterruptTargetId::new(0),
                    source: InterruptSourceId::new(97),
                    latency: 1,
                },
            },
        ],
    };

    let error = match PlatformBuilder::new(2).add_cpu_local_timer(config).build() {
        Err(error) => error,
        Ok(_) => panic!("CPU local timer route gap was accepted"),
    };
    assert_eq!(
        error,
        PlatformError::MissingCpuLocalTimerRoute {
            base,
            partition: cpu1,
        }
    );
}

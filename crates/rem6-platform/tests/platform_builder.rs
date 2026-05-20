use rem6_interrupt::{
    InterruptError, InterruptEvent, InterruptEventKind, InterruptLineId, InterruptSourceId,
    InterruptTargetId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{MmioCompletion, MmioError, MmioRequest, MmioRequestId, MmioResponse, MmioRoute};
use rem6_platform::{PlatformBuilder, PlatformError, PlatformTimerConfig, PlatformUartConfig};
use rem6_timer::{TimerArm, TimerExpiry, TimerId, TIMER_MMIO_DEADLINE_OFFSET};
use rem6_uart::{UartId, UartRxByte, UartTxByte, UART_MMIO_DATA_OFFSET};

fn full_mask(bytes: u64) -> ByteMask {
    ByteMask::full(AccessSize::new(bytes).unwrap()).unwrap()
}

fn le64(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

#[test]
fn platform_builder_wires_timer_uart_interrupts_and_mmio_bus() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let uart_partition = PartitionId::new(2);
    let timer_id = TimerId::new(1);
    let uart_id = UartId::new(2);
    let timer_line = InterruptLineId::new(20);
    let uart_line = InterruptLineId::new(21);
    let timer_source = InterruptSourceId::new(30);
    let uart_source = InterruptSourceId::new(31);

    let platform = PlatformBuilder::new(3)
        .add_timer(PlatformTimerConfig {
            id: timer_id,
            base: Address::new(0x5000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu, timer_partition, 2, 1).unwrap(),
            interrupt_line: timer_line,
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: timer_source,
            interrupt_latency: 2,
        })
        .add_uart(PlatformUartConfig {
            id: uart_id,
            base: Address::new(0x6000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu, uart_partition, 2, 1).unwrap(),
            interrupt_line: uart_line,
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: uart_source,
            interrupt_latency: 2,
        })
        .build()
        .unwrap();

    let timer = platform.timer(timer_id).unwrap().clone();
    let uart = platform.uart(uart_id).unwrap().clone();
    let controller = platform.interrupt_controller();
    let bus = platform.mmio_bus().clone();
    let completions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(platform.partition_count()).unwrap();

    let uart_input = uart.clone();
    scheduler
        .schedule_at(uart_partition, 2, move |context| {
            uart_input.inject_rx_after(context, 2, [b'R']).unwrap();
        })
        .unwrap();

    let completed = std::sync::Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 1, move |context| {
            let timer_completed = std::sync::Arc::clone(&completed);
            bus.submit(
                context,
                MmioRequest::write(
                    MmioRequestId::new(1),
                    Address::new(0x5000 + TIMER_MMIO_DEADLINE_OFFSET),
                    le64(7),
                    full_mask(8),
                )
                .unwrap(),
                move |completion| timer_completed.lock().unwrap().push(completion),
            )
            .unwrap();

            bus.submit(
                context,
                MmioRequest::write(
                    MmioRequestId::new(2),
                    Address::new(0x6000 + UART_MMIO_DATA_OFFSET),
                    vec![b'B'],
                    full_mask(1),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 10);
    assert_eq!(summary.final_tick(), 9);
    assert_eq!(timer.snapshot().arms(), &[TimerArm::new(1, 3, 7)]);
    assert_eq!(timer.snapshot().expiries(), &[TimerExpiry::new(1, 7)]);
    assert_eq!(uart.snapshot().rx_injected(), &[UartRxByte::new(4, b'R')]);
    assert_eq!(uart.snapshot().tx_bytes(), &[UartTxByte::new(3, b'B')]);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[
            MmioCompletion::new(
                4,
                MmioRoute::new(cpu, timer_partition, 2, 1).unwrap(),
                Ok(MmioResponse::completed(MmioRequestId::new(1), None)),
            ),
            MmioCompletion::new(
                4,
                MmioRoute::new(cpu, uart_partition, 2, 1).unwrap(),
                Ok(MmioResponse::completed(MmioRequestId::new(2), None)),
            ),
        ]
    );
    assert_eq!(
        controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(
                6,
                uart_line,
                InterruptTargetId::new(0),
                cpu,
                uart_source,
                InterruptEventKind::Assert,
            ),
            InterruptEvent::routed(
                9,
                timer_line,
                InterruptTargetId::new(0),
                cpu,
                timer_source,
                InterruptEventKind::Assert,
            ),
        ]
    );
}

#[test]
fn platform_builder_rejects_device_map_and_interrupt_conflicts() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let uart_partition = PartitionId::new(2);
    let shared_line = InterruptLineId::new(40);

    let overlap = PlatformBuilder::new(3)
        .add_timer(PlatformTimerConfig {
            id: TimerId::new(3),
            base: Address::new(0x8000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu, timer_partition, 2, 1).unwrap(),
            interrupt_line: InterruptLineId::new(41),
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: InterruptSourceId::new(50),
            interrupt_latency: 2,
        })
        .add_uart(PlatformUartConfig {
            id: UartId::new(4),
            base: Address::new(0x8080),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu, uart_partition, 2, 1).unwrap(),
            interrupt_line: InterruptLineId::new(42),
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: InterruptSourceId::new(51),
            interrupt_latency: 2,
        })
        .build();

    match overlap {
        Err(error) => assert_eq!(
            error,
            PlatformError::Mmio(MmioError::OverlappingDeviceRegion {
                existing_start: Address::new(0x8000),
                existing_end: Address::new(0x8100),
                requested_start: Address::new(0x8080),
                requested_end: Address::new(0x8180),
            }),
        ),
        Ok(_) => panic!("overlapping MMIO regions were accepted"),
    }

    let duplicate_line = PlatformBuilder::new(3)
        .add_timer(PlatformTimerConfig {
            id: TimerId::new(5),
            base: Address::new(0x9000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu, timer_partition, 2, 1).unwrap(),
            interrupt_line: shared_line,
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: InterruptSourceId::new(52),
            interrupt_latency: 2,
        })
        .add_uart(PlatformUartConfig {
            id: UartId::new(6),
            base: Address::new(0xa000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(cpu, uart_partition, 2, 1).unwrap(),
            interrupt_line: shared_line,
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: InterruptSourceId::new(53),
            interrupt_latency: 2,
        })
        .build();

    match duplicate_line {
        Err(error) => assert_eq!(
            error,
            PlatformError::Interrupt(InterruptError::DuplicateLine { line: shared_line }),
        ),
        Ok(_) => panic!("duplicate interrupt lines were accepted"),
    }
}

#[test]
fn platform_builder_rejects_empty_and_unknown_partitions() {
    let empty = PlatformBuilder::new(0).build();

    match empty {
        Err(error) => assert_eq!(error, PlatformError::NoPartitions),
        Ok(_) => panic!("empty platform was accepted"),
    }

    let target = PartitionId::new(3);
    let unknown_target = PlatformBuilder::new(3)
        .add_timer(PlatformTimerConfig {
            id: TimerId::new(7),
            base: Address::new(0xb000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(PartitionId::new(0), target, 2, 1).unwrap(),
            interrupt_line: InterruptLineId::new(60),
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: InterruptSourceId::new(70),
            interrupt_latency: 2,
        })
        .build();

    match unknown_target {
        Err(error) => assert_eq!(
            error,
            PlatformError::UnknownPartition {
                partition: target,
                partitions: 3,
            },
        ),
        Ok(_) => panic!("unknown target partition was accepted"),
    }

    let source = PartitionId::new(4);
    let unknown_source = PlatformBuilder::new(3)
        .add_uart(PlatformUartConfig {
            id: UartId::new(8),
            base: Address::new(0xc000),
            size: AccessSize::new(0x100).unwrap(),
            route: MmioRoute::new(source, PartitionId::new(2), 2, 1).unwrap(),
            interrupt_line: InterruptLineId::new(61),
            interrupt_target: InterruptTargetId::new(0),
            interrupt_source: InterruptSourceId::new(71),
            interrupt_latency: 2,
        })
        .build();

    match unknown_source {
        Err(error) => assert_eq!(
            error,
            PlatformError::UnknownPartition {
                partition: source,
                partitions: 3,
            },
        ),
        Ok(_) => panic!("unknown source partition was accepted"),
    }
}

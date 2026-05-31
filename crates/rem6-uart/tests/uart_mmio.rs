use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptError, InterruptEvent, InterruptEventKind, InterruptLineChannel,
    InterruptLineId, InterruptLinePort, InterruptRoute, InterruptSourceId, InterruptTargetId,
    PendingInterrupt,
};
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{
    MmioAccess, MmioBus, MmioCompletion, MmioError, MmioOperation, MmioRequest, MmioRequestId,
    MmioResponse, MmioRoute,
};
use rem6_uart::{
    UartId, UartInterruptError, UartMmioDevice, UartRxByte, UartTxByte, UART_MMIO_DATA_OFFSET,
    UART_MMIO_REGISTER_BYTES, UART_MMIO_STATUS_OFFSET, UART_STATUS_RX_READY, UART_STATUS_TX_READY,
};

fn byte_mask() -> ByteMask {
    ByteMask::full(AccessSize::new(UART_MMIO_REGISTER_BYTES).unwrap()).unwrap()
}

fn uart_range(base: Address) -> AddressRange {
    AddressRange::new(base, AccessSize::new(0x100).unwrap()).unwrap()
}

fn interrupt_uart(
    base: Address,
    cpu: PartitionId,
    line: InterruptLineId,
    source: InterruptSourceId,
) -> (UartMmioDevice, Arc<Mutex<InterruptController>>) {
    let route = InterruptRoute::new(line, InterruptTargetId::new(0), cpu);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    controller.lock().unwrap().register_route(route).unwrap();
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    (
        UartMmioDevice::with_interrupt(UartId::new(line.get()), base, source, port),
        controller,
    )
}

fn write_byte(uart: &UartMmioDevice, tick: u64, byte: u8) {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let base = uart.base();
    let uart = uart.clone();
    scheduler
        .schedule_at(PartitionId::new(0), tick, move |context| {
            uart.respond(
                context,
                &MmioRequest::write(
                    MmioRequestId::new(100 + tick),
                    Address::new(base.get() + UART_MMIO_DATA_OFFSET),
                    vec![byte],
                    byte_mask(),
                )
                .unwrap(),
            )
            .unwrap();
        })
        .unwrap();
    scheduler.run_until_idle();
}

fn read_byte(uart: &UartMmioDevice, tick: u64) -> u8 {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let base = uart.base();
    let uart = uart.clone();
    let read = Arc::new(Mutex::new(None));
    let read_result = Arc::clone(&read);
    scheduler
        .schedule_at(PartitionId::new(0), tick, move |context| {
            let response = uart
                .respond(
                    context,
                    &MmioRequest::read(
                        MmioRequestId::new(200 + tick),
                        Address::new(base.get() + UART_MMIO_DATA_OFFSET),
                        AccessSize::new(UART_MMIO_REGISTER_BYTES).unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap();
            *read_result.lock().unwrap() = Some(response.data().unwrap()[0]);
        })
        .unwrap();
    scheduler.run_until_idle();
    let byte = read.lock().unwrap().unwrap();
    byte
}

#[test]
fn uart_rx_injection_rejects_interrupt_before_state_change() {
    let cpu = PartitionId::new(0);
    let uart_partition = PartitionId::new(1);
    let base = Address::new(0x7600);
    let source = InterruptSourceId::new(21);
    let (uart, controller) = interrupt_uart(base, cpu, InterruptLineId::new(46), source);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 3).unwrap();

    let uart_input = uart.clone();
    scheduler
        .schedule_at(uart_partition, 5, move |context| {
            uart_input.inject_rx_after(context, 1, [b'Q']).unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 2);
    assert!(uart.snapshot().rx_pending().is_empty());
    assert!(uart.snapshot().rx_injected().is_empty());
    assert_eq!(
        uart.snapshot().interrupt_errors(),
        &[UartInterruptError::new(
            6,
            source,
            InterruptEventKind::Assert,
            InterruptError::Scheduler(SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
                source: uart_partition,
                target: cpu,
                source_tick: 6,
                delivery_tick: 8,
                minimum_delivery_tick: 9,
            }),
        )]
    );
    assert!(controller.lock().unwrap().history().is_empty());
}

#[test]
fn uart_parallel_rx_injection_rejects_interrupt_before_state_change() {
    let cpu = PartitionId::new(0);
    let uart_partition = PartitionId::new(1);
    let base = Address::new(0x7700);
    let source = InterruptSourceId::new(22);
    let (uart, controller) = interrupt_uart(base, cpu, InterruptLineId::new(47), source);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 3).unwrap();

    let uart_input = uart.clone();
    scheduler
        .schedule_parallel_at(uart_partition, 5, move |context| {
            uart_input
                .inject_rx_after_parallel(context, 1, [b'R'])
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 2);
    assert!(uart.snapshot().rx_pending().is_empty());
    assert!(uart.snapshot().rx_injected().is_empty());
    assert_eq!(
        uart.snapshot().interrupt_errors(),
        &[UartInterruptError::new(
            6,
            source,
            InterruptEventKind::Assert,
            InterruptError::Scheduler(SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
                source: uart_partition,
                target: cpu,
                source_tick: 6,
                delivery_tick: 8,
                minimum_delivery_tick: 9,
            }),
        )]
    );
    assert!(controller.lock().unwrap().history().is_empty());
}

#[test]
fn uart_mmio_read_rejects_deassert_before_consuming_last_rx_byte() {
    let cpu = PartitionId::new(0);
    let uart_partition = PartitionId::new(1);
    let base = Address::new(0x7a00);
    let source = InterruptSourceId::new(23);
    let (uart, controller) = interrupt_uart(base, cpu, InterruptLineId::new(48), source);
    uart.inject_rx([b'S']).unwrap();
    let observed_uart = uart.clone();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&errors);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 3).unwrap();

    scheduler
        .schedule_at(uart_partition, 5, move |context| {
            let error = uart
                .respond(
                    context,
                    &MmioRequest::read(
                        MmioRequestId::new(31),
                        Address::new(base.get() + UART_MMIO_DATA_OFFSET),
                        AccessSize::new(UART_MMIO_REGISTER_BYTES).unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap_err();
            captured.lock().unwrap().push(error);
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 1);
    assert_eq!(
        errors.lock().unwrap().as_slice(),
        &[MmioError::DeviceError {
            request: MmioRequestId::new(31),
            message: InterruptError::Scheduler(
                SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
                    source: uart_partition,
                    target: cpu,
                    source_tick: 5,
                    delivery_tick: 7,
                    minimum_delivery_tick: 8,
                }
            )
            .to_string(),
        }]
    );
    assert_eq!(observed_uart.snapshot().rx_pending(), b"S");
    assert!(observed_uart.snapshot().rx_consumed().is_empty());
    assert!(observed_uart.snapshot().interrupt_errors().is_empty());
    assert!(controller.lock().unwrap().history().is_empty());
}

#[test]
fn uart_parallel_mmio_read_rejects_deassert_before_consuming_last_rx_byte() {
    let cpu = PartitionId::new(0);
    let uart_partition = PartitionId::new(1);
    let base = Address::new(0x7b00);
    let source = InterruptSourceId::new(24);
    let (uart, controller) = interrupt_uart(base, cpu, InterruptLineId::new(49), source);
    uart.inject_rx([b'T']).unwrap();
    let observed_uart = uart.clone();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&errors);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 3).unwrap();

    scheduler
        .schedule_parallel_at(uart_partition, 5, move |context| {
            let error = uart
                .respond_parallel(
                    context,
                    &MmioRequest::read(
                        MmioRequestId::new(32),
                        Address::new(base.get() + UART_MMIO_DATA_OFFSET),
                        AccessSize::new(UART_MMIO_REGISTER_BYTES).unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap_err();
            captured.lock().unwrap().push(error);
        })
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 1);
    assert_eq!(
        errors.lock().unwrap().as_slice(),
        &[MmioError::DeviceError {
            request: MmioRequestId::new(32),
            message: InterruptError::Scheduler(
                SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
                    source: uart_partition,
                    target: cpu,
                    source_tick: 5,
                    delivery_tick: 7,
                    minimum_delivery_tick: 8,
                }
            )
            .to_string(),
        }]
    );
    assert_eq!(observed_uart.snapshot().rx_pending(), b"T");
    assert!(observed_uart.snapshot().rx_consumed().is_empty());
    assert!(observed_uart.snapshot().interrupt_errors().is_empty());
    assert!(controller.lock().unwrap().history().is_empty());
}

#[test]
fn uart_mmio_bus_records_transmitted_bytes_and_status() {
    let cpu = PartitionId::new(0);
    let uart_partition = PartitionId::new(1);
    let base = Address::new(0x6000);
    let route = MmioRoute::new(cpu, uart_partition, 2, 1).unwrap();
    let uart = UartMmioDevice::new(UartId::new(7), base);
    let mut bus = MmioBus::new();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    bus.insert_device(uart_range(base), route, uart.clone())
        .unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 3, move |context| {
            let first_completed = Arc::clone(&completed);
            bus.submit(
                context,
                MmioRequest::write(
                    MmioRequestId::new(1),
                    Address::new(base.get() + UART_MMIO_DATA_OFFSET),
                    vec![b'O'],
                    byte_mask(),
                )
                .unwrap(),
                move |completion| first_completed.lock().unwrap().push(completion),
            )
            .unwrap();

            let second_completed = Arc::clone(&completed);
            bus.submit(
                context,
                MmioRequest::write(
                    MmioRequestId::new(2),
                    Address::new(base.get() + UART_MMIO_DATA_OFFSET),
                    vec![b'K'],
                    byte_mask(),
                )
                .unwrap(),
                move |completion| second_completed.lock().unwrap().push(completion),
            )
            .unwrap();

            bus.submit(
                context,
                MmioRequest::read(
                    MmioRequestId::new(3),
                    Address::new(base.get() + UART_MMIO_STATUS_OFFSET),
                    AccessSize::new(UART_MMIO_REGISTER_BYTES).unwrap(),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 7);
    assert_eq!(summary.final_tick(), 6);
    assert_eq!(
        uart.snapshot().tx_bytes(),
        &[UartTxByte::new(5, b'O'), UartTxByte::new(5, b'K')]
    );
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[
            MmioCompletion::new(
                6,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(1), None))
            ),
            MmioCompletion::new(
                6,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(2), None))
            ),
            MmioCompletion::new(
                6,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(3),
                    Some(vec![UART_STATUS_TX_READY]),
                )),
            ),
        ]
    );
}

#[test]
fn uart_snapshot_restore_reinstates_tx_and_rx_state() {
    let base = Address::new(0x6400);
    let uart = UartMmioDevice::new(UartId::new(8), base);
    uart.inject_rx([b'A', b'B']).unwrap();
    write_byte(&uart, 3, b'O');
    let captured = uart.snapshot();

    write_byte(&uart, 4, b'X');
    assert_eq!(read_byte(&uart, 5), b'A');
    uart.inject_rx([b'C']).unwrap();
    assert_ne!(uart.snapshot(), captured);

    uart.restore(&captured);

    assert_eq!(uart.snapshot(), captured);
    assert_eq!(uart.snapshot().tx_bytes(), &[UartTxByte::new(3, b'O')]);
    assert_eq!(uart.snapshot().rx_pending(), b"AB");
    assert_eq!(read_byte(&uart, 6), b'A');
    assert_eq!(read_byte(&uart, 7), b'B');
    assert!(uart.snapshot().rx_pending().is_empty());
}

#[test]
fn uart_parallel_mmio_bus_records_transmitted_bytes_and_status() {
    let cpu = PartitionId::new(0);
    let uart_partition = PartitionId::new(1);
    let base = Address::new(0x6800);
    let route = MmioRoute::new(cpu, uart_partition, 2, 2).unwrap();
    let uart = UartMmioDevice::new(UartId::new(13), base);
    let mut bus = MmioBus::new();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    bus.insert_device(uart_range(base), route, uart.clone())
        .unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 3, move |context| {
            let write_completed = Arc::clone(&completed);
            bus.submit_parallel(
                context,
                MmioRequest::write(
                    MmioRequestId::new(14),
                    Address::new(base.get() + UART_MMIO_DATA_OFFSET),
                    vec![b'R'],
                    byte_mask(),
                )
                .unwrap(),
                move |completion| write_completed.lock().unwrap().push(completion),
            )
            .unwrap();
            bus.submit_parallel(
                context,
                MmioRequest::read(
                    MmioRequestId::new(15),
                    Address::new(base.get() + UART_MMIO_STATUS_OFFSET),
                    AccessSize::new(UART_MMIO_REGISTER_BYTES).unwrap(),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 5);
    assert!(summary.final_tick() >= 7);
    assert_eq!(uart.snapshot().tx_bytes(), &[UartTxByte::new(5, b'R')]);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[
            MmioCompletion::new(
                7,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(14), None)),
            ),
            MmioCompletion::new(
                7,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(15),
                    Some(vec![UART_STATUS_TX_READY]),
                )),
            ),
        ]
    );
}

#[test]
fn uart_direct_rx_injection_records_snapshot_history() {
    let base = Address::new(0x6f00);
    let uart = UartMmioDevice::new(UartId::new(7), base);

    uart.inject_rx([b'A', b'B']).unwrap();

    assert_eq!(
        uart.snapshot().rx_injected(),
        &[UartRxByte::new(0, b'A'), UartRxByte::new(0, b'B')]
    );
    assert_eq!(uart.snapshot().rx_pending(), b"AB");
    assert!(uart.snapshot().rx_consumed().is_empty());
}

#[test]
fn uart_mmio_bus_reads_injected_rx_bytes_in_order() {
    let cpu = PartitionId::new(0);
    let uart_partition = PartitionId::new(1);
    let base = Address::new(0x7000);
    let route = MmioRoute::new(cpu, uart_partition, 2, 1).unwrap();
    let uart = UartMmioDevice::new(UartId::new(8), base);
    uart.inject_rx([b'A', b'B']).unwrap();
    let mut bus = MmioBus::new();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    bus.insert_device(uart_range(base), route, uart.clone())
        .unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 1, move |context| {
            let requests = [
                MmioRequest::read(
                    MmioRequestId::new(4),
                    Address::new(base.get() + UART_MMIO_STATUS_OFFSET),
                    AccessSize::new(UART_MMIO_REGISTER_BYTES).unwrap(),
                )
                .unwrap(),
                MmioRequest::read(
                    MmioRequestId::new(5),
                    Address::new(base.get() + UART_MMIO_DATA_OFFSET),
                    AccessSize::new(UART_MMIO_REGISTER_BYTES).unwrap(),
                )
                .unwrap(),
                MmioRequest::read(
                    MmioRequestId::new(6),
                    Address::new(base.get() + UART_MMIO_DATA_OFFSET),
                    AccessSize::new(UART_MMIO_REGISTER_BYTES).unwrap(),
                )
                .unwrap(),
                MmioRequest::read(
                    MmioRequestId::new(7),
                    Address::new(base.get() + UART_MMIO_STATUS_OFFSET),
                    AccessSize::new(UART_MMIO_REGISTER_BYTES).unwrap(),
                )
                .unwrap(),
            ];

            for request in requests {
                let completed_request = Arc::clone(&completed);
                bus.submit(context, request, move |completion| {
                    completed_request.lock().unwrap().push(completion)
                })
                .unwrap();
            }
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 9);
    assert_eq!(summary.final_tick(), 4);
    assert_eq!(
        uart.snapshot().rx_consumed(),
        &[UartRxByte::new(3, b'A'), UartRxByte::new(3, b'B')]
    );
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(4),
                    Some(vec![UART_STATUS_RX_READY | UART_STATUS_TX_READY]),
                )),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(5),
                    Some(vec![b'A'])
                )),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(6),
                    Some(vec![b'B'])
                )),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(7),
                    Some(vec![UART_STATUS_TX_READY]),
                )),
            ),
        ]
    );
}

#[test]
fn uart_rx_injection_asserts_and_deasserts_interrupt_line() {
    let cpu = PartitionId::new(0);
    let uart_partition = PartitionId::new(1);
    let base = Address::new(0x7800);
    let line = InterruptLineId::new(44);
    let source = InterruptSourceId::new(15);
    let route = InterruptRoute::new(line, InterruptTargetId::new(0), cpu);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    controller.lock().unwrap().register_route(route).unwrap();
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    let uart = UartMmioDevice::with_interrupt(UartId::new(11), base, source, port.clone());
    let mmio_route = MmioRoute::new(cpu, uart_partition, 1, 1).unwrap();
    let mut bus = MmioBus::new();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    bus.insert_device(uart_range(base), mmio_route, uart.clone())
        .unwrap();

    let uart_input = uart.clone();
    scheduler
        .schedule_at(uart_partition, 2, move |context| {
            uart_input.inject_rx_after(context, 3, [b'Z']).unwrap();
        })
        .unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 8, move |context| {
            bus.submit(
                context,
                MmioRequest::read(
                    MmioRequestId::new(11),
                    Address::new(base.get() + UART_MMIO_DATA_OFFSET),
                    AccessSize::new(UART_MMIO_REGISTER_BYTES).unwrap(),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 7);
    assert_eq!(summary.final_tick(), 11);
    assert_eq!(uart.snapshot().rx_injected(), &[UartRxByte::new(5, b'Z')]);
    assert_eq!(uart.snapshot().rx_consumed(), &[UartRxByte::new(9, b'Z')]);
    assert_eq!(uart.snapshot().interrupt_errors(), &[]);
    assert!(port.delivery_errors().lock().unwrap().is_empty());
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[MmioCompletion::new(
            10,
            mmio_route,
            Ok(MmioResponse::completed(
                MmioRequestId::new(11),
                Some(vec![b'Z'])
            )),
        )]
    );
    assert_eq!(
        controller.lock().unwrap().history(),
        &[
            InterruptEvent::routed(
                7,
                line,
                InterruptTargetId::new(0),
                cpu,
                source,
                InterruptEventKind::Assert,
            ),
            InterruptEvent::routed(
                11,
                line,
                InterruptTargetId::new(0),
                cpu,
                source,
                InterruptEventKind::Deassert,
            ),
        ]
    );
}

#[test]
fn uart_parallel_rx_injection_asserts_interrupt_line() {
    let cpu = PartitionId::new(0);
    let uart_partition = PartitionId::new(1);
    let base = Address::new(0x7900);
    let line = InterruptLineId::new(45);
    let source = InterruptSourceId::new(16);
    let route = InterruptRoute::new(line, InterruptTargetId::new(0), cpu);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    controller.lock().unwrap().register_route(route).unwrap();
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    let uart = UartMmioDevice::with_interrupt(UartId::new(12), base, source, port.clone());
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    let uart_input = uart.clone();
    scheduler
        .schedule_parallel_at(uart_partition, 2, move |context| {
            uart_input
                .inject_rx_after_parallel(context, 3, [b'P'])
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 3);
    assert!(summary.final_tick() >= 7);
    assert_eq!(uart.snapshot().rx_injected(), &[UartRxByte::new(5, b'P')]);
    assert_eq!(uart.snapshot().rx_pending(), b"P");
    assert_eq!(uart.snapshot().interrupt_errors(), &[]);
    assert!(port.delivery_errors().lock().unwrap().is_empty());
    let controller = controller.lock().unwrap();
    assert_eq!(
        controller.pending(),
        vec![PendingInterrupt::routed(
            line,
            InterruptTargetId::new(0),
            cpu,
            source,
            7
        )]
    );
    assert_eq!(
        controller.history(),
        &[InterruptEvent::routed(
            7,
            line,
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn uart_mmio_rejects_bad_width_empty_rx_and_readonly_status_writes() {
    let base = Address::new(0x8000);
    let uart = UartMmioDevice::new(UartId::new(9), base);
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let errors = Arc::clone(&observed);

    scheduler
        .schedule_at(PartitionId::new(0), 10, move |context| {
            errors.lock().unwrap().push(
                uart.respond(
                    context,
                    &MmioRequest::read(
                        MmioRequestId::new(8),
                        Address::new(base.get() + UART_MMIO_DATA_OFFSET),
                        AccessSize::new(2).unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap_err(),
            );
            errors.lock().unwrap().push(
                uart.respond(
                    context,
                    &MmioRequest::read(
                        MmioRequestId::new(9),
                        Address::new(base.get() + UART_MMIO_DATA_OFFSET),
                        AccessSize::new(UART_MMIO_REGISTER_BYTES).unwrap(),
                    )
                    .unwrap(),
                )
                .unwrap_err(),
            );
            errors.lock().unwrap().push(
                uart.respond(
                    context,
                    &MmioRequest::write(
                        MmioRequestId::new(10),
                        Address::new(base.get() + UART_MMIO_STATUS_OFFSET),
                        vec![0xff],
                        byte_mask(),
                    )
                    .unwrap(),
                )
                .unwrap_err(),
            );
        })
        .unwrap();

    scheduler.run_until_idle();

    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[
            MmioError::AccessSizeMismatch {
                request: MmioRequestId::new(8),
                expected: UART_MMIO_REGISTER_BYTES,
                actual: 2,
            },
            MmioError::DeviceError {
                request: MmioRequestId::new(9),
                message: "UART receive queue is empty".to_string(),
            },
            MmioError::AccessDenied {
                request: MmioRequestId::new(10),
                operation: MmioOperation::Write,
                access: MmioAccess::ReadOnly,
            },
        ]
    );
}

use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{
    MmioAccess, MmioBus, MmioCompletion, MmioError, MmioOperation, MmioRequest, MmioRequestId,
    MmioResponse, MmioRoute,
};
use rem6_uart::{
    UartId, UartMmioDevice, UartRxByte, UartTxByte, UART_MMIO_DATA_OFFSET,
    UART_MMIO_REGISTER_BYTES, UART_MMIO_STATUS_OFFSET, UART_STATUS_RX_READY, UART_STATUS_TX_READY,
};

fn byte_mask() -> ByteMask {
    ByteMask::full(AccessSize::new(UART_MMIO_REGISTER_BYTES).unwrap()).unwrap()
}

fn uart_range(base: Address) -> AddressRange {
    AddressRange::new(base, AccessSize::new(0x100).unwrap()).unwrap()
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

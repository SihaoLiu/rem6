use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptEvent, InterruptEventKind, InterruptLineChannel, InterruptLineId,
    InterruptLinePort, InterruptRoute, InterruptSourceId, InterruptTargetId, PendingInterrupt,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{MmioError, MmioRequest, MmioRequestId, MmioResponse};
use rem6_uart::{
    Pl011UartMmioDevice, UartId, UartRxByte, UartTxByte, AMBA_CELL_ID0_OFFSET,
    AMBA_CELL_ID1_OFFSET, AMBA_CELL_ID2_OFFSET, AMBA_CELL_ID3_OFFSET, AMBA_PERIPHERAL_ID0_OFFSET,
    AMBA_PERIPHERAL_ID1_OFFSET, AMBA_PERIPHERAL_ID2_OFFSET, AMBA_PERIPHERAL_ID3_OFFSET,
    PL011_CONTROL_OFFSET, PL011_DATA_OFFSET, PL011_DMACR_OFFSET, PL011_FBRD_OFFSET, PL011_FLAG_CTS,
    PL011_FLAG_OFFSET, PL011_FLAG_RX_EMPTY, PL011_FLAG_RX_FULL, PL011_FLAG_TX_EMPTY,
    PL011_IFLS_OFFSET, PL011_IMSC_OFFSET, PL011_INTEGER_BRD_OFFSET, PL011_INT_CLEAR_OFFSET,
    PL011_INT_RX, PL011_INT_RX_TIMEOUT, PL011_INT_TX, PL011_LINE_CONTROL_OFFSET,
    PL011_MASKED_ISR_OFFSET, PL011_RAW_ISR_OFFSET, PL011_REGISTER_BYTES,
};

fn register_size() -> AccessSize {
    AccessSize::new(PL011_REGISTER_BYTES).unwrap()
}

fn register_mask() -> ByteMask {
    ByteMask::full(register_size()).unwrap()
}

fn le32(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn read_request(id: u64, base: Address, offset: u64) -> MmioRequest {
    MmioRequest::read(
        MmioRequestId::new(id),
        Address::new(base.get() + offset),
        register_size(),
    )
    .unwrap()
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

#[test]
fn pl011_uart_exposes_gem5_registers_primecell_id_and_fifo_flags() {
    let uart_partition = PartitionId::new(0);
    let base = Address::new(0x1c09_0000);
    let device = Pl011UartMmioDevice::new(UartId::new(31), base);
    let observed_device = device.clone();
    let responses = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&responses);
    let mut scheduler = PartitionedScheduler::new(1).unwrap();

    scheduler
        .schedule_at(uart_partition, 1, move |context| {
            responses
                .lock()
                .unwrap()
                .push(device.respond(context, &read_request(1, base, PL011_FLAG_OFFSET)));
            device
                .respond(
                    context,
                    &write_request(2, base, PL011_CONTROL_OFFSET, 0x101),
                )
                .unwrap();
            device
                .respond(
                    context,
                    &write_request(3, base, PL011_INTEGER_BRD_OFFSET, 3),
                )
                .unwrap();
            device
                .respond(context, &write_request(4, base, PL011_FBRD_OFFSET, 2))
                .unwrap();
            device
                .respond(
                    context,
                    &write_request(5, base, PL011_LINE_CONTROL_OFFSET, 0x70),
                )
                .unwrap();
            device
                .respond(context, &write_request(6, base, PL011_IFLS_OFFSET, 0x24))
                .unwrap();
            device
                .respond(context, &write_request(7, base, PL011_DMACR_OFFSET, 0))
                .unwrap();
            for (id, offset) in [
                (8, PL011_CONTROL_OFFSET),
                (9, PL011_INTEGER_BRD_OFFSET),
                (10, PL011_FBRD_OFFSET),
                (11, PL011_LINE_CONTROL_OFFSET),
                (12, PL011_IFLS_OFFSET),
                (13, PL011_DMACR_OFFSET),
            ] {
                responses
                    .lock()
                    .unwrap()
                    .push(device.respond(context, &read_request(id, base, offset)));
            }

            device
                .respond(
                    context,
                    &write_request(20, base, PL011_DATA_OFFSET, b'Z' as u32),
                )
                .unwrap();
            responses
                .lock()
                .unwrap()
                .push(device.respond(context, &read_request(21, base, PL011_RAW_ISR_OFFSET)));
            device
                .respond(
                    context,
                    &write_request(22, base, PL011_INT_CLEAR_OFFSET, PL011_INT_TX as u32),
                )
                .unwrap();
            responses
                .lock()
                .unwrap()
                .push(device.respond(context, &read_request(23, base, PL011_RAW_ISR_OFFSET)));

            device.inject_rx([b'A', b'B']).unwrap();
            for (id, offset) in [
                (30, PL011_RAW_ISR_OFFSET),
                (31, PL011_FLAG_OFFSET),
                (32, PL011_DATA_OFFSET),
                (33, PL011_RAW_ISR_OFFSET),
                (34, PL011_DATA_OFFSET),
                (35, PL011_RAW_ISR_OFFSET),
                (36, PL011_FLAG_OFFSET),
            ] {
                responses
                    .lock()
                    .unwrap()
                    .push(device.respond(context, &read_request(id, base, offset)));
            }

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

            responses
                .lock()
                .unwrap()
                .push(device.respond(context, &write_request(200, base, PL011_DMACR_OFFSET, 1)));
        })
        .unwrap();
    scheduler.run_until_idle();

    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[
            Ok(MmioResponse::completed(
                MmioRequestId::new(1),
                Some(le32(
                    (PL011_FLAG_CTS | PL011_FLAG_RX_EMPTY | PL011_FLAG_TX_EMPTY) as u32,
                )),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(8),
                Some(le32(0x101)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(9),
                Some(le32(3))
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(10),
                Some(le32(2))
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(11),
                Some(le32(0x70)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(12),
                Some(le32(0x24)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(13),
                Some(le32(0)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(21),
                Some(le32(PL011_INT_TX as u32)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(23),
                Some(le32(0)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(30),
                Some(le32((PL011_INT_RX | PL011_INT_RX_TIMEOUT) as u32)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(31),
                Some(le32(
                    (PL011_FLAG_CTS | PL011_FLAG_RX_FULL | PL011_FLAG_TX_EMPTY) as u32,
                )),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(32),
                Some(le32(b'A' as u32)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(33),
                Some(le32((PL011_INT_RX | PL011_INT_RX_TIMEOUT) as u32)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(34),
                Some(le32(b'B' as u32)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(35),
                Some(le32(0)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(36),
                Some(le32(
                    (PL011_FLAG_CTS | PL011_FLAG_RX_EMPTY | PL011_FLAG_TX_EMPTY) as u32,
                )),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(100),
                Some(le32(0x11)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(101),
                Some(le32(0x10)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(102),
                Some(le32(0x34)),
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
            Err(MmioError::DeviceError {
                request: MmioRequestId::new(200),
                message: "PL011 DMA is not supported".to_string(),
            }),
        ]
    );
    assert_eq!(
        observed_device.snapshot().tx_bytes(),
        &[UartTxByte::new(1, b'Z')]
    );
    assert_eq!(
        observed_device.snapshot().rx_consumed(),
        &[UartRxByte::new(1, b'A'), UartRxByte::new(1, b'B')]
    );
}

#[test]
fn pl011_uart_masks_rx_interrupts_and_routes_parallel_assertions() {
    let cpu = PartitionId::new(0);
    let uart_partition = PartitionId::new(1);
    let base = Address::new(0x1c09_1000);
    let line = InterruptLineId::new(81);
    let source = InterruptSourceId::new(181);
    let target = InterruptTargetId::new(0);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let port = interrupt_port(&controller, line, target, cpu, 2);
    let device = Pl011UartMmioDevice::with_interrupt(UartId::new(32), base, source, port.clone());
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    scheduler
        .schedule_parallel_at(uart_partition, 2, move |context| {
            device
                .respond_parallel(
                    context,
                    &write_request(
                        1,
                        base,
                        PL011_IMSC_OFFSET,
                        (PL011_INT_RX | PL011_INT_RX_TIMEOUT) as u32,
                    ),
                )
                .unwrap();
            device.inject_rx_after_parallel(context, 3, [b'Q']).unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 3);
    assert!(summary.final_tick() >= 7);
    assert!(port.delivery_errors().lock().unwrap().is_empty());
    let controller = controller.lock().unwrap();
    assert_eq!(
        controller.pending(),
        vec![PendingInterrupt::routed(line, target, cpu, source, 7)]
    );
    assert_eq!(
        controller.history(),
        &[InterruptEvent::routed(
            7,
            line,
            target,
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn pl011_uart_read_data_deasserts_serial_rx_interrupt_after_last_byte() {
    let cpu = PartitionId::new(0);
    let base = Address::new(0x1c09_2000);
    let line = InterruptLineId::new(82);
    let source = InterruptSourceId::new(182);
    let target = InterruptTargetId::new(0);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let port = interrupt_port(&controller, line, target, cpu, 1);
    let device = Pl011UartMmioDevice::with_interrupt(UartId::new(33), base, source, port.clone());
    let reader = device.clone();
    let responses = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&responses);
    let mut scheduler = PartitionedScheduler::new(1).unwrap();

    scheduler
        .schedule_at(cpu, 1, move |context| {
            device
                .respond(
                    context,
                    &write_request(
                        1,
                        base,
                        PL011_IMSC_OFFSET,
                        (PL011_INT_RX | PL011_INT_RX_TIMEOUT) as u32,
                    ),
                )
                .unwrap();
            device.inject_rx_after(context, 1, [b'R']).unwrap();
        })
        .unwrap();

    scheduler
        .schedule_at(cpu, 4, move |context| {
            responses
                .lock()
                .unwrap()
                .push(reader.respond(context, &read_request(2, base, PL011_DATA_OFFSET)));
            responses
                .lock()
                .unwrap()
                .push(reader.respond(context, &read_request(3, base, PL011_MASKED_ISR_OFFSET)));
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 5);
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[
            Ok(MmioResponse::completed(
                MmioRequestId::new(2),
                Some(le32(b'R' as u32)),
            )),
            Ok(MmioResponse::completed(
                MmioRequestId::new(3),
                Some(le32(0))
            )),
        ]
    );
    assert!(port.delivery_errors().lock().unwrap().is_empty());
    let controller = controller.lock().unwrap();
    assert!(controller.pending().is_empty());
    assert_eq!(
        controller.history(),
        &[
            InterruptEvent::routed(3, line, target, cpu, source, InterruptEventKind::Assert,),
            InterruptEvent::routed(5, line, target, cpu, source, InterruptEventKind::Deassert,),
        ]
    );
}

use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{
    MmioAccess, MmioBus, MmioCompletion, MmioError, MmioOperation, MmioRequest, MmioRequestId,
    MmioResponse, MmioRoute,
};
use rem6_net::{
    EthernetPacket, SinicDataDescriptor, SinicDoneStatus, SinicFifoDevice, SinicInterrupts,
    SinicMmioDevice, SinicRegisterBlock, SinicRegisterOffset, SinicRegisterParams,
    SINIC_MMIO_VIRTUAL_STRIDE,
};

fn packet(bytes: &[u8]) -> EthernetPacket {
    EthernetPacket::new(bytes.to_vec()).unwrap()
}

fn le32(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn le64(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn full_mask(bytes: u64) -> ByteMask {
    ByteMask::full(AccessSize::new(bytes).unwrap()).unwrap()
}

fn read_request(id: u64, base: Address, offset: u64, bytes: u64) -> MmioRequest {
    MmioRequest::read(
        MmioRequestId::new(id),
        Address::new(base.get() + offset),
        AccessSize::new(bytes).unwrap(),
    )
    .unwrap()
}

fn write_request(id: u64, base: Address, offset: u64, data: Vec<u8>) -> MmioRequest {
    MmioRequest::write(
        MmioRequestId::new(id),
        Address::new(base.get() + offset),
        data.clone(),
        full_mask(data.len() as u64),
    )
    .unwrap()
}

fn direct_response(
    device: &SinicMmioDevice,
    tick: u64,
    request: MmioRequest,
) -> Result<MmioResponse, MmioError> {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let device = device.clone();
    let result = Arc::new(Mutex::new(None));
    let result_slot = Arc::clone(&result);
    scheduler
        .schedule_at(PartitionId::new(0), tick, move |context| {
            *result_slot.lock().unwrap() = Some(device.respond(context, &request));
        })
        .unwrap();
    scheduler.run_until_idle();
    let response = result.lock().unwrap().clone().unwrap();
    response
}

#[test]
fn sinic_mmio_reads_writes_registers_and_clears_interrupt_status() {
    let base = Address::new(0x9000);
    let device = SinicMmioDevice::new(
        base,
        SinicFifoDevice::new(
            SinicRegisterParams::default()
                .with_fifo_limits(32, 64, 4, 8, 16, 24)
                .with_hardware_address(0x0012_3456_789a),
        )
        .unwrap(),
    );

    assert_eq!(device.base(), base);
    assert_eq!(device.range_size_bytes(), SINIC_MMIO_VIRTUAL_STRIDE);
    assert_eq!(
        direct_response(
            &device,
            1,
            read_request(1, base, SinicRegisterOffset::RX_FIFO_SIZE.addr() as u64, 4),
        )
        .unwrap(),
        MmioResponse::completed(MmioRequestId::new(1), Some(le32(32)))
    );
    assert_eq!(
        direct_response(
            &device,
            1,
            read_request(2, base, SinicRegisterOffset::HW_ADDR.addr() as u64, 8),
        )
        .unwrap(),
        MmioResponse::completed(MmioRequestId::new(2), Some(le64(0x0012_3456_789a)))
    );

    let config = SinicRegisterBlock::CONFIG_INT_EN | SinicRegisterBlock::CONFIG_RX_EN;
    direct_response(
        &device,
        2,
        write_request(
            3,
            base,
            SinicRegisterOffset::CONFIG.addr() as u64,
            le32(config),
        ),
    )
    .unwrap();
    direct_response(
        &device,
        2,
        write_request(
            4,
            base,
            SinicRegisterOffset::INTR_MASK.addr() as u64,
            le32(SinicInterrupts::SOFT.bits()),
        ),
    )
    .unwrap();
    direct_response(
        &device,
        3,
        write_request(
            5,
            base,
            SinicRegisterOffset::COMMAND.addr() as u64,
            le32(0x2),
        ),
    )
    .unwrap();

    assert_eq!(
        direct_response(
            &device,
            4,
            read_request(6, base, SinicRegisterOffset::INTR_STATUS.addr() as u64, 4),
        )
        .unwrap(),
        MmioResponse::completed(
            MmioRequestId::new(6),
            Some(le32(SinicInterrupts::SOFT.bits()))
        )
    );
    assert_eq!(
        direct_response(
            &device,
            5,
            read_request(7, base, SinicRegisterOffset::INTR_STATUS.addr() as u64, 4),
        )
        .unwrap(),
        MmioResponse::completed(MmioRequestId::new(7), Some(le32(0)))
    );

    assert_eq!(
        direct_response(
            &device,
            6,
            read_request(8, base, SinicRegisterOffset::COMMAND.addr() as u64, 4),
        ),
        Err(MmioError::AccessDenied {
            request: MmioRequestId::new(8),
            operation: MmioOperation::Read,
            access: MmioAccess::WriteOnly,
        })
    );
    assert_eq!(
        direct_response(
            &device,
            7,
            write_request(9, base, SinicRegisterOffset::RX_DONE.addr() as u64, le64(1)),
        ),
        Err(MmioError::AccessDenied {
            request: MmioRequestId::new(9),
            operation: MmioOperation::Write,
            access: MmioAccess::ReadOnly,
        })
    );
    assert_eq!(
        direct_response(
            &device,
            7,
            read_request(10, base, SinicRegisterOffset::CONFIG.addr() as u64, 8),
        ),
        Err(MmioError::AccessSizeMismatch {
            request: MmioRequestId::new(10),
            expected: 4,
            actual: 8,
        })
    );
}

#[test]
fn sinic_mmio_rx_and_tx_data_registers_drive_typed_dma_state() {
    let base = Address::new(0xa000);
    let device = SinicMmioDevice::new(
        base,
        SinicFifoDevice::new(
            SinicRegisterParams::default()
                .with_zero_copy(true)
                .with_rx_copy_limits(8, 4, 2)
                .with_fifo_limits(16, 8, 8, 2, 12, 12)
                .with_tx_max_copy(4)
                .with_interrupt_mask(
                    SinicInterrupts::RX_DMA
                        | SinicInterrupts::RX_EMPTY
                        | SinicInterrupts::TX_DMA
                        | SinicInterrupts::TX_FULL,
                ),
        )
        .unwrap(),
    );
    direct_response(
        &device,
        1,
        write_request(
            1,
            base,
            SinicRegisterOffset::CONFIG.addr() as u64,
            le32(
                SinicRegisterBlock::CONFIG_INT_EN
                    | SinicRegisterBlock::CONFIG_RX_EN
                    | SinicRegisterBlock::CONFIG_TX_EN
                    | SinicRegisterBlock::CONFIG_ZERO_COPY,
            ),
        ),
    )
    .unwrap();
    device
        .receive_from_wire(packet(&[1, 2, 3, 4, 5, 6]), 2, 0)
        .unwrap();

    direct_response(
        &device,
        3,
        write_request(
            2,
            base,
            SinicRegisterOffset::RX_DATA.addr() as u64,
            le64(SinicDataDescriptor::new(0x1000, 8).unwrap().bits()),
        ),
    )
    .unwrap();
    let rx = device.complete_rx_dma_copy(4, 0).unwrap();
    assert_eq!(rx.copied_bytes(), 2);
    assert_eq!(rx.remaining_packet_bytes(), 4);
    assert_eq!(
        direct_response(
            &device,
            5,
            read_request(3, base, SinicRegisterOffset::RX_DONE.addr() as u64, 8),
        )
        .unwrap(),
        MmioResponse::completed(
            MmioRequestId::new(3),
            Some(le64(
                SinicDoneStatus::new()
                    .with_packets(1)
                    .with_complete(true)
                    .with_more(true)
                    .with_not_high(true)
                    .with_copy_len(4)
                    .unwrap()
                    .bits()
            )),
        )
    );

    direct_response(
        &device,
        6,
        write_request(
            4,
            base,
            SinicRegisterOffset::TX_DATA.addr() as u64,
            le64(
                SinicDataDescriptor::new(0x2000, 3)
                    .unwrap()
                    .with_more(true)
                    .bits(),
            ),
        ),
    )
    .unwrap();
    device.complete_tx_dma_copy(&[7, 8, 9], 7, 0).unwrap();
    direct_response(
        &device,
        8,
        write_request(
            5,
            base,
            SinicRegisterOffset::TX_DATA.addr() as u64,
            le64(SinicDataDescriptor::new(0x3000, 2).unwrap().bits()),
        ),
    )
    .unwrap();
    device.complete_tx_dma_copy(&[10, 11], 9, 0).unwrap();

    assert_eq!(device.snapshot().tx_packet_count(), 1);
    assert_eq!(
        direct_response(
            &device,
            10,
            read_request(6, base, SinicRegisterOffset::TX_DONE.addr() as u64, 8),
        )
        .unwrap(),
        MmioResponse::completed(
            MmioRequestId::new(6),
            Some(le64(
                SinicDoneStatus::new()
                    .with_packets(1)
                    .with_complete(true)
                    .with_full(true)
                    .with_copy_len(2)
                    .unwrap()
                    .bits()
            )),
        )
    );
}

#[test]
fn sinic_mmio_participates_in_parallel_mmio_bus_routing() {
    let cpu = PartitionId::new(0);
    let nic = PartitionId::new(1);
    let base = Address::new(0xb000);
    let route = MmioRoute::new(cpu, nic, 2, 2).unwrap();
    let device = SinicMmioDevice::new(
        base,
        SinicFifoDevice::new(
            SinicRegisterParams::default().with_hardware_address(0x00aa_bbcc_ddee),
        )
        .unwrap(),
    );
    let mut bus = MmioBus::new();
    bus.insert_device(device.range(), route, device.clone())
        .unwrap();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 3, move |context| {
            bus.submit_parallel(
                context,
                MmioRequest::read(
                    MmioRequestId::new(1),
                    Address::new(base.get() + SinicRegisterOffset::HW_ADDR.addr() as u64),
                    AccessSize::new(8).unwrap(),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.final_tick(), 8);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[MmioCompletion::new(
            7,
            route,
            Ok(MmioResponse::completed(
                MmioRequestId::new(1),
                Some(le64(0x00aa_bbcc_ddee)),
            )),
        )]
    );
}

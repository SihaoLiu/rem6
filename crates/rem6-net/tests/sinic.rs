use rem6_net::{
    EthernetInterfaceRegistry, EthernetPacket, SinicDataDescriptor, SinicDoneStatus, SinicError,
    SinicFifoDevice, SinicInterrupts, SinicQueueKind, SinicRegisterBlock, SinicRegisterOffset,
    SinicRegisterParams, SinicRxStatus,
};

fn packet(bytes: &[u8]) -> EthernetPacket {
    EthernetPacket::new(bytes.to_vec()).unwrap()
}

#[test]
fn sinic_register_info_matches_typed_layout_and_access_policy() {
    assert_eq!(SinicRegisterOffset::CONFIG.addr(), 0x00);
    assert_eq!(SinicRegisterOffset::COMMAND.addr(), 0x04);
    assert_eq!(SinicRegisterOffset::INTR_STATUS.addr(), 0x08);
    assert_eq!(SinicRegisterOffset::RX_DATA.addr(), 0x40);
    assert_eq!(SinicRegisterOffset::TX_DONE.addr(), 0x60);
    assert_eq!(SinicRegisterOffset::RX_STATUS.addr(), 0x78);
    assert_eq!(SinicRegisterOffset::SIZE, 0x80);

    let config = SinicRegisterOffset::info(0x00).unwrap();
    assert_eq!(config.name(), "Config");
    assert_eq!(config.bytes(), 4);
    assert!(config.can_read());
    assert!(config.can_write());

    let command = SinicRegisterOffset::info(0x04).unwrap();
    assert_eq!(command.name(), "Command");
    assert!(!command.can_read());
    assert!(command.can_write());

    let rx_data = SinicRegisterOffset::info(0x40).unwrap();
    assert_eq!(rx_data.name(), "RxData");
    assert_eq!(rx_data.bytes(), 8);
    assert!(rx_data.can_read());
    assert!(rx_data.can_write());

    let rx_done = SinicRegisterOffset::info(0x48).unwrap();
    assert_eq!(rx_done.name(), "RxDone");
    assert!(rx_done.can_read());
    assert!(!rx_done.can_write());

    assert!(SinicRegisterOffset::info(0x44).is_none());
    assert!(SinicRegisterOffset::info(SinicRegisterOffset::SIZE).is_none());
}

#[test]
fn sinic_descriptors_and_status_words_preserve_gem5_bit_positions() {
    let rx = SinicDataDescriptor::new(0x00ab_cdef_0123, 0x01234)
        .unwrap()
        .with_no_delay(true)
        .with_virtual_address(true);
    assert_eq!(rx.bits(), 0x3012_34ab_cdef_0123);
    let decoded_rx = SinicDataDescriptor::from_bits(rx.bits());
    assert_eq!(decoded_rx.address(), 0x00ab_cdef_0123);
    assert_eq!(decoded_rx.byte_len(), 0x01234);
    assert!(decoded_rx.no_delay());
    assert!(decoded_rx.virtual_address());

    let tx = SinicDataDescriptor::new(0x0000_1234_5678, 0x0abcd)
        .unwrap()
        .with_more(true)
        .with_checksum(true)
        .with_virtual_address(true);
    assert_eq!(tx.bits(), 0xd0ab_cd00_1234_5678);
    let decoded_tx = SinicDataDescriptor::from_bits(tx.bits());
    assert_eq!(decoded_tx.address(), 0x0000_1234_5678);
    assert_eq!(decoded_tx.byte_len(), 0x0abcd);
    assert!(decoded_tx.more());
    assert!(decoded_tx.checksum());
    assert!(decoded_tx.virtual_address());

    assert!(matches!(
        SinicDataDescriptor::new(1_u64 << 40, 1),
        Err(SinicError::DescriptorAddressTooWide {
            address: 0x0100_0000_0000,
        })
    ));
    assert!(matches!(
        SinicDataDescriptor::new(0, 1_u32 << 20),
        Err(SinicError::DescriptorLengthTooWide { len: 0x10_0000 })
    ));

    let rx_done = SinicDoneStatus::new()
        .with_packets(7)
        .with_busy(true)
        .with_complete(true)
        .with_more(true)
        .with_empty(true)
        .with_high(true)
        .with_not_high(true)
        .with_ip_packet(true)
        .with_tcp_error(true)
        .with_copy_len(0x4567)
        .unwrap();
    assert_eq!(rx_done.bits(), 0x0000_0007_fe10_4567);
    assert_eq!(SinicDoneStatus::from_bits(rx_done.bits()).packets(), 7);
    assert_eq!(
        SinicDoneStatus::from_bits(rx_done.bits()).copy_len(),
        0x4567
    );

    let rx_status = SinicRxStatus::new()
        .with_dirty(3)
        .with_mapped(4)
        .with_busy(5)
        .with_head(0x1234);
    assert_eq!(rx_status.bits(), 0x0003_0004_0005_1234);
}

#[test]
fn sinic_register_block_validates_reset_parameters_and_snapshots() {
    assert!(matches!(
        SinicRegisterBlock::new(
            SinicRegisterParams::default()
                .with_zero_copy(true)
                .with_delay_copy(true)
        ),
        Err(SinicError::IncompatibleCopyModes)
    ));
    assert!(matches!(
        SinicRegisterBlock::new(SinicRegisterParams::default().with_rx_copy_limits(32, 64, 8)),
        Err(SinicError::RxMaxCopyBelowZeroCopyMark {
            rx_max_copy: 32,
            zero_copy_mark: 64,
        })
    ));
    assert!(matches!(
        SinicRegisterBlock::new(SinicRegisterParams::default().with_rx_copy_limits(128, 64, 64)),
        Err(SinicError::ZeroCopySizeNotBelowMark {
            zero_copy_size: 64,
            zero_copy_mark: 64,
        })
    ));

    let params = SinicRegisterParams::default()
        .with_virtual_count(4)
        .with_fifo_limits(4096, 2048, 64, 128, 512, 1024)
        .with_hardware_address(0x0012_3456_789a);
    let mut regs = SinicRegisterBlock::new(params).unwrap();
    assert_eq!(regs.virtual_count(), 4);
    assert_eq!(regs.hardware_address(), 0x0012_3456_789a);
    assert_eq!(regs.rx_fifo_size(), 4096);
    assert_eq!(regs.tx_fifo_high(), 1024);

    regs.change_interrupt_mask(SinicInterrupts::SOFT | SinicInterrupts::RX_PACKET, 10)
        .unwrap();
    regs.change_config(regs.config_bits() | SinicRegisterBlock::CONFIG_INT_EN, 10)
        .unwrap();
    regs.post_interrupt(SinicInterrupts::RX_PACKET, 12, 5)
        .unwrap();
    let snapshot = regs.snapshot();

    regs.clear_interrupts(SinicInterrupts::RX_PACKET).unwrap();
    assert_eq!(regs.interrupt_status().bits(), 0);

    regs.restore(&snapshot).unwrap();
    assert_eq!(
        regs.interrupt_status().bits(),
        SinicInterrupts::RX_PACKET.bits()
    );
    assert_eq!(regs.pending_interrupt_tick(), Some(17));
}

#[test]
fn sinic_register_block_checkpoint_payload_round_trips_snapshot() {
    let params = SinicRegisterParams::default()
        .with_virtual_count(3)
        .with_fifo_limits(4096, 2048, 64, 128, 512, 1024)
        .with_hardware_address(0x0012_3456_789a);
    let mut regs = SinicRegisterBlock::new(params).unwrap();
    regs.change_interrupt_mask(SinicInterrupts::SOFT | SinicInterrupts::RX_PACKET, 10)
        .unwrap();
    regs.change_config(regs.config_bits() | SinicRegisterBlock::CONFIG_INT_EN, 10)
        .unwrap();
    regs.post_interrupt(SinicInterrupts::RX_PACKET, 12, 5)
        .unwrap();

    let snapshot = regs.snapshot();
    let payload = snapshot.encode_checkpoint_payload();
    let decoded =
        rem6_net::SinicRegisterBlockSnapshot::decode_checkpoint_payload(&payload).unwrap();
    assert_eq!(decoded, snapshot);

    let mut restored = SinicRegisterBlock::new(SinicRegisterParams::default()).unwrap();
    restored.restore_checkpoint_payload(&payload).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
}

#[test]
fn sinic_register_block_checkpoint_payload_rejects_bad_payload_without_mutation() {
    let mut regs = SinicRegisterBlock::new(
        SinicRegisterParams::default().with_interrupt_mask(SinicInterrupts::SOFT),
    )
    .unwrap();
    regs.change_config(regs.config_bits() | SinicRegisterBlock::CONFIG_INT_EN, 1)
        .unwrap();
    regs.post_interrupt(SinicInterrupts::SOFT, 2, 4).unwrap();
    let before = regs.snapshot();
    let mut payload = before.encode_checkpoint_payload();
    payload.truncate(payload.len() - 1);

    assert!(matches!(
        regs.restore_checkpoint_payload(&payload),
        Err(SinicError::InvalidSnapshotPayload { .. })
    ));
    assert_eq!(regs.snapshot(), before);
}

#[test]
fn sinic_fifo_device_checkpoint_payload_round_trips_snapshot() {
    let params = SinicRegisterParams::default()
        .with_zero_copy(true)
        .with_fifo_limits(48, 48, 4, 4, 16, 16)
        .with_interrupt_mask(
            SinicInterrupts::RX_PACKET
                | SinicInterrupts::RX_DMA
                | SinicInterrupts::TX_DMA
                | SinicInterrupts::TX_FULL,
        );
    let mut device = SinicFifoDevice::new(params).unwrap();
    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_INT_EN
                | SinicRegisterBlock::CONFIG_RX_EN
                | SinicRegisterBlock::CONFIG_TX_EN
                | SinicRegisterBlock::CONFIG_ZERO_COPY,
            1,
        )
        .unwrap();
    device
        .receive_from_wire(
            packet(&[1, 2, 3, 4, 5, 6, 7, 8])
                .with_wire_length_bytes(12)
                .unwrap(),
            2,
            3,
        )
        .unwrap();
    device
        .begin_rx_dma_copy(SinicDataDescriptor::new(0x1000, 4).unwrap())
        .unwrap()
        .expect("pending receive DMA copy");
    device
        .complete_rx_dma_copy(3, 4)
        .expect("complete first receive DMA copy");
    device
        .begin_rx_dma_copy(SinicDataDescriptor::new(0x2000, 4).unwrap())
        .unwrap()
        .expect("pending second receive DMA copy");
    device
        .begin_tx_dma_copy(SinicDataDescriptor::new(0x3000, 3).unwrap().with_more(true))
        .unwrap();
    device.complete_tx_dma_copy(&[9, 8, 7], 4, 5).unwrap();
    device
        .begin_tx_dma_copy(SinicDataDescriptor::new(0x4000, 2).unwrap())
        .unwrap();

    let snapshot = device.snapshot();
    let payload = snapshot.encode_checkpoint_payload();
    let decoded = rem6_net::SinicFifoDeviceSnapshot::decode_checkpoint_payload(&payload).unwrap();
    assert_eq!(decoded, snapshot);

    let mut restored = SinicFifoDevice::new(SinicRegisterParams::default()).unwrap();
    restored.restore_checkpoint_payload(&payload).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
}

#[test]
fn sinic_fifo_device_checkpoint_payload_rejects_bad_payload_without_mutation() {
    let params = SinicRegisterParams::default()
        .with_fifo_limits(16, 16, 4, 4, 8, 8)
        .with_interrupt_mask(SinicInterrupts::RX_PACKET);
    let mut device = SinicFifoDevice::new(params).unwrap();
    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_INT_EN | SinicRegisterBlock::CONFIG_RX_EN,
            1,
        )
        .unwrap();
    device
        .receive_from_wire(packet(&[1, 2, 3, 4]), 2, 3)
        .unwrap();
    let before = device.snapshot();
    let mut payload = before.encode_checkpoint_payload();
    payload.extend_from_slice(&[0xaa, 0xbb]);

    assert!(matches!(
        device.restore_checkpoint_payload(&payload),
        Err(SinicError::InvalidSnapshotPayload { .. })
    ));
    assert_eq!(device.snapshot(), before);
}

#[test]
fn sinic_interrupts_are_masked_delayed_and_cleared_as_typed_events() {
    let mut regs = SinicRegisterBlock::new(
        SinicRegisterParams::default().with_interrupt_mask(SinicInterrupts::SOFT),
    )
    .unwrap();

    let disabled = regs.post_interrupt(SinicInterrupts::SOFT, 4, 10).unwrap();
    assert_eq!(disabled.status_bits().bits(), SinicInterrupts::SOFT.bits());
    assert_eq!(disabled.masked_bits().bits(), SinicInterrupts::SOFT.bits());
    assert_eq!(disabled.scheduled_tick(), None);

    let enable_record = regs
        .change_config(regs.config_bits() | SinicRegisterBlock::CONFIG_INT_EN, 5)
        .unwrap()
        .unwrap();
    assert_eq!(enable_record.scheduled_tick(), Some(5));
    assert_eq!(regs.pending_interrupt_tick(), Some(5));

    let delayed = regs
        .change_interrupt_mask(SinicInterrupts::RX_PACKET, 6)
        .unwrap();
    assert!(delayed.is_none());
    regs.clear_interrupts(SinicInterrupts::SOFT).unwrap();
    let delayed = regs
        .post_interrupt(SinicInterrupts::RX_PACKET, 7, 11)
        .unwrap();
    assert_eq!(
        delayed.masked_bits().bits(),
        SinicInterrupts::RX_PACKET.bits()
    );
    assert_eq!(delayed.scheduled_tick(), Some(18));
    assert_eq!(regs.pending_interrupt_tick(), Some(18));

    regs.clear_interrupts(SinicInterrupts::RX_PACKET).unwrap();
    assert_eq!(regs.pending_interrupt_tick(), None);

    assert!(matches!(
        regs.post_interrupt(SinicInterrupts::from_bits_truncate(0x0200), 8, 1),
        Err(SinicError::ReservedInterruptBits {
            bits: 0x0200,
            reserved_bits: 0x0200,
        })
    ));
}

#[test]
fn sinic_fifo_watermark_latches_gate_high_and_low_interrupts() {
    let mut regs = SinicRegisterBlock::new(SinicRegisterParams::default().with_interrupt_mask(
        SinicInterrupts::RX_HIGH | SinicInterrupts::RX_EMPTY | SinicInterrupts::TX_LOW,
    ))
    .unwrap();
    regs.change_config(regs.config_bits() | SinicRegisterBlock::CONFIG_INT_EN, 1)
        .unwrap();

    let rx_high = regs.post_interrupt(SinicInterrupts::RX_HIGH, 2, 5).unwrap();
    assert_eq!(rx_high.masked_bits().bits(), 0);
    assert_eq!(rx_high.scheduled_tick(), None);

    let rx_empty = regs.record_rx_empty(3, 5).unwrap();
    assert_eq!(
        rx_empty.masked_bits().bits(),
        SinicInterrupts::RX_EMPTY.bits()
    );
    assert_eq!(rx_empty.scheduled_tick(), Some(3));
    let rx_high = regs.post_interrupt(SinicInterrupts::RX_HIGH, 4, 5).unwrap();
    assert_eq!(
        rx_high.masked_bits().bits(),
        (SinicInterrupts::RX_EMPTY | SinicInterrupts::RX_HIGH).bits()
    );
    assert_eq!(rx_high.scheduled_tick(), Some(4));

    regs.clear_interrupts(SinicInterrupts::RX_EMPTY | SinicInterrupts::RX_HIGH)
        .unwrap();
    let tx_low = regs.post_interrupt(SinicInterrupts::TX_LOW, 5, 5).unwrap();
    assert_eq!(tx_low.masked_bits().bits(), 0);
    assert_eq!(tx_low.scheduled_tick(), None);

    regs.record_tx_full(6, 5).unwrap();
    let tx_low = regs.post_interrupt(SinicInterrupts::TX_LOW, 7, 5).unwrap();
    assert_eq!(tx_low.masked_bits().bits(), SinicInterrupts::TX_LOW.bits());
    assert_eq!(tx_low.scheduled_tick(), Some(7));
}

#[test]
fn sinic_fifo_device_drops_rx_when_disabled_and_posts_packet_interrupts() {
    let params = SinicRegisterParams::default()
        .with_fifo_limits(12, 12, 4, 4, 4, 8)
        .with_interrupt_mask(
            SinicInterrupts::RX_PACKET | SinicInterrupts::RX_HIGH | SinicInterrupts::RX_EMPTY,
        );
    let mut device = SinicFifoDevice::new(params).unwrap();

    let disabled = device.receive_from_wire(packet(&[1, 2, 3]), 10, 5).unwrap();
    assert!(!disabled.queued());
    assert_eq!(
        disabled
            .interrupt_record()
            .map(|record| record.masked_bits().bits()),
        None
    );
    assert_eq!(device.rx_packet_count(), 0);

    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_INT_EN | SinicRegisterBlock::CONFIG_RX_EN,
            11,
        )
        .unwrap();
    let first = device
        .receive_from_wire(packet(&[1, 2, 3, 4]), 12, 5)
        .unwrap();
    assert!(first.queued());
    assert_eq!(first.rx_packet_count(), 1);
    assert_eq!(
        first
            .interrupt_record()
            .expect("rx packet interrupt")
            .masked_bits()
            .bits(),
        SinicInterrupts::RX_PACKET.bits()
    );
    assert_eq!(device.rx_done_status().packets(), 1);
    assert_eq!(device.rx_done_status().copy_len(), 0);

    device.mark_rx_empty(13, 5).unwrap();
    let second = device.receive_from_wire(packet(&[5]), 14, 5).unwrap();
    assert!(second.queued());
    assert_eq!(second.rx_packet_count(), 2);
    assert_eq!(
        second
            .interrupt_record()
            .expect("rx high and packet interrupt")
            .masked_bits()
            .bits(),
        (SinicInterrupts::RX_PACKET | SinicInterrupts::RX_HIGH | SinicInterrupts::RX_EMPTY).bits()
    );

    let popped = device.pop_rx_packet(15, 5).unwrap().unwrap();
    assert_eq!(popped.packet().payload(), &[1, 2, 3, 4]);
    assert_eq!(device.rx_packet_count(), 1);
    assert_eq!(device.rx_done_status().packets(), 1);
    device.pop_rx_packet(16, 5).unwrap().unwrap();
    assert_eq!(device.rx_packet_count(), 0);
    assert!(device.rx_done_status().bits() & (1 << 28) != 0);
}

#[test]
fn sinic_fifo_device_rejects_rx_overflow_without_mutation() {
    let params = SinicRegisterParams::default()
        .with_fifo_limits(4, 12, 1, 4, 8, 8)
        .with_interrupt_mask(SinicInterrupts::RX_PACKET);
    let mut device = SinicFifoDevice::new(params).unwrap();
    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_INT_EN | SinicRegisterBlock::CONFIG_RX_EN,
            1,
        )
        .unwrap();
    device
        .receive_from_wire(packet(&[1, 2, 3, 4]), 2, 0)
        .unwrap();

    assert!(matches!(
        device.receive_from_wire(packet(&[5]), 3, 0),
        Err(SinicError::PacketQueueCapacityExceeded {
            queue: SinicQueueKind::Receive,
            capacity_bytes: 4,
            occupied_bytes: 4,
            packet_bytes: 1,
        })
    ));
    assert_eq!(device.rx_packet_count(), 1);
    assert_eq!(device.rx_occupied_bytes(), 4);
}

#[test]
fn sinic_fifo_device_transmits_only_when_peer_accepts_and_posts_watermarks() {
    let params = SinicRegisterParams::default()
        .with_fifo_limits(12, 8, 4, 4, 8, 8)
        .with_interrupt_mask(SinicInterrupts::TX_PACKET | SinicInterrupts::TX_LOW);
    let mut device = SinicFifoDevice::new(params).unwrap();
    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_INT_EN | SinicRegisterBlock::CONFIG_TX_EN,
            1,
        )
        .unwrap();
    let mut registry = EthernetInterfaceRegistry::new();
    let nic = registry.register("sinic").unwrap();
    let peer = registry.register("peer").unwrap();
    registry.bind_pair(nic, peer).unwrap();

    device
        .enqueue_tx_packet(packet(&[0xaa, 0xbb, 0xcc]), 2, 0)
        .unwrap();
    assert_eq!(device.tx_packet_count(), 1);
    assert_eq!(device.tx_done_status().packets(), 1);
    registry.set_busy(peer, true).unwrap();
    assert!(matches!(
        device.transmit_one(&mut registry, nic, 3, 0),
        Err(SinicError::EthernetPeerBusy { interface }) if interface == nic
    ));
    assert_eq!(device.tx_packet_count(), 1);

    registry.set_busy(peer, false).unwrap();
    device.mark_tx_full(4, 0).unwrap();
    let transmitted = device
        .transmit_one(&mut registry, nic, 5, 0)
        .unwrap()
        .unwrap();
    assert_eq!(transmitted.send_record().peer(), Some(peer));
    assert_eq!(
        transmitted.send_record().packet().payload(),
        &[0xaa, 0xbb, 0xcc]
    );
    assert_eq!(
        transmitted
            .interrupt_record()
            .expect("tx packet and tx low interrupt")
            .masked_bits()
            .bits(),
        (SinicInterrupts::TX_PACKET | SinicInterrupts::TX_LOW).bits()
    );
    assert_eq!(device.tx_packet_count(), 0);
    assert_eq!(registry.receive_count(peer).unwrap(), 1);
}

#[test]
fn sinic_fifo_device_records_tx_full_and_restores_snapshot() {
    let params = SinicRegisterParams::default()
        .with_fifo_limits(12, 6, 4, 2, 8, 8)
        .with_tx_max_copy(2)
        .with_interrupt_mask(SinicInterrupts::TX_FULL);
    let mut device = SinicFifoDevice::new(params).unwrap();
    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_INT_EN | SinicRegisterBlock::CONFIG_TX_EN,
            1,
        )
        .unwrap();

    let first = device.enqueue_tx_packet(packet(&[1, 2, 3]), 2, 0).unwrap();
    assert!(first.interrupt_record().is_none());
    let snapshot = device.snapshot();
    let second = device.enqueue_tx_packet(packet(&[4, 5]), 3, 0).unwrap();
    assert_eq!(second.tx_packet_count(), 2);
    assert_eq!(
        second
            .interrupt_record()
            .expect("tx full interrupt")
            .masked_bits()
            .bits(),
        SinicInterrupts::TX_FULL.bits()
    );
    assert_eq!(device.tx_occupied_bytes(), 5);

    device.restore(&snapshot).unwrap();
    assert_eq!(device.tx_packet_count(), 1);
    assert_eq!(device.tx_occupied_bytes(), 3);
    assert_eq!(device.tx_done_status().packets(), 1);
}

#[test]
fn sinic_dma_rx_copy_records_partial_more_then_packet_completion() {
    let params = SinicRegisterParams::default()
        .with_zero_copy(true)
        .with_rx_copy_limits(8, 4, 2)
        .with_fifo_limits(16, 16, 8, 4, 12, 12)
        .with_interrupt_mask(SinicInterrupts::RX_DMA | SinicInterrupts::RX_EMPTY);
    let mut device = SinicFifoDevice::new(params).unwrap();
    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_INT_EN
                | SinicRegisterBlock::CONFIG_RX_EN
                | SinicRegisterBlock::CONFIG_ZERO_COPY,
            1,
        )
        .unwrap();
    device
        .receive_from_wire(packet(&[10, 11, 12, 13, 14, 15]), 2, 0)
        .unwrap();

    let limited = device
        .begin_rx_dma_copy(SinicDataDescriptor::new(0x1000, 8).unwrap())
        .unwrap()
        .expect("queued receive packet");
    assert_eq!(limited.guest_address(), 0x1000);
    assert_eq!(limited.copy_len(), 2);
    assert!(limited.zero_limited());
    assert_eq!(limited.packet_offset(), 0);

    let partial = device.complete_rx_dma_copy(3, 7).unwrap();
    assert_eq!(partial.copied_bytes(), 2);
    assert_eq!(partial.remaining_packet_bytes(), 4);
    assert_eq!(partial.rx_packet_count(), 1);
    assert_eq!(
        partial.done_status().bits(),
        SinicDoneStatus::new()
            .with_complete(true)
            .with_more(true)
            .with_copy_len(4)
            .unwrap()
            .bits()
    );
    assert_eq!(
        partial
            .interrupt_record()
            .expect("rx dma interrupt")
            .masked_bits()
            .bits(),
        SinicInterrupts::RX_DMA.bits()
    );

    let final_plan = device
        .begin_rx_dma_copy(
            SinicDataDescriptor::new(0x2000, 8)
                .unwrap()
                .with_no_delay(true),
        )
        .unwrap()
        .expect("partial receive packet remains queued");
    assert_eq!(final_plan.copy_len(), 4);
    assert!(!final_plan.zero_limited());
    assert_eq!(final_plan.packet_offset(), 2);

    let complete = device.complete_rx_dma_copy(4, 7).unwrap();
    assert_eq!(complete.copied_bytes(), 4);
    assert_eq!(complete.remaining_packet_bytes(), 0);
    assert_eq!(complete.rx_packet_count(), 0);
    assert_eq!(
        complete.done_status().bits(),
        SinicDoneStatus::new()
            .with_complete(true)
            .with_copy_len(4)
            .unwrap()
            .bits()
    );
    assert_eq!(
        complete
            .interrupt_record()
            .expect("rx dma and empty interrupt")
            .masked_bits()
            .bits(),
        (SinicInterrupts::RX_DMA | SinicInterrupts::RX_EMPTY).bits()
    );
}

#[test]
fn sinic_dma_rx_completion_reports_ipv4_tcp_checksum_status() {
    let mut device = SinicFifoDevice::new(
        SinicRegisterParams::default()
            .with_fifo_limits(128, 128, 16, 16, 96, 96)
            .with_interrupt_mask(SinicInterrupts::RX_DMA),
    )
    .unwrap();
    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_INT_EN | SinicRegisterBlock::CONFIG_RX_EN,
            1,
        )
        .unwrap();
    let frame = ipv4_tcp_frame(&[0xde, 0xad, 0xbe, 0xef]);
    device.receive_from_wire(packet(&frame), 2, 0).unwrap();

    device
        .begin_rx_dma_copy(SinicDataDescriptor::new(0x3000, frame.len() as u32).unwrap())
        .unwrap()
        .unwrap();
    let complete = device.complete_rx_dma_copy(3, 0).unwrap();

    assert_eq!(
        complete.done_status().bits(),
        SinicDoneStatus::new()
            .with_complete(true)
            .with_ip_packet(true)
            .with_tcp_packet(true)
            .with_copy_len(frame.len() as u32)
            .unwrap()
            .bits()
    );
}

#[test]
fn sinic_dma_rx_completion_reports_ipv4_udp_checksum_errors() {
    let mut device = SinicFifoDevice::new(
        SinicRegisterParams::default()
            .with_fifo_limits(128, 128, 16, 16, 96, 96)
            .with_interrupt_mask(SinicInterrupts::RX_DMA),
    )
    .unwrap();
    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_INT_EN | SinicRegisterBlock::CONFIG_RX_EN,
            1,
        )
        .unwrap();
    let mut frame = ipv4_udp_frame(&[0xca, 0xfe, 0xba, 0xbe]);
    frame[IPV4_UDP_CHECKSUM_OFFSET] ^= 0x01;
    device.receive_from_wire(packet(&frame), 2, 0).unwrap();

    device
        .begin_rx_dma_copy(SinicDataDescriptor::new(0x3000, frame.len() as u32).unwrap())
        .unwrap()
        .unwrap();
    let complete = device.complete_rx_dma_copy(3, 0).unwrap();

    assert_eq!(
        complete.done_status().bits(),
        SinicDoneStatus::new()
            .with_complete(true)
            .with_ip_packet(true)
            .with_udp_packet(true)
            .with_udp_error(true)
            .with_copy_len(frame.len() as u32)
            .unwrap()
            .bits()
    );
}

#[test]
fn sinic_dma_tx_copy_accumulates_fragments_and_posts_dma_interrupts() {
    let params = SinicRegisterParams::default()
        .with_fifo_limits(16, 6, 4, 2, 12, 12)
        .with_tx_max_copy(2)
        .with_interrupt_mask(SinicInterrupts::TX_DMA | SinicInterrupts::TX_FULL);
    let mut device = SinicFifoDevice::new(params).unwrap();
    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_INT_EN | SinicRegisterBlock::CONFIG_TX_EN,
            1,
        )
        .unwrap();

    let first = device
        .begin_tx_dma_copy(SinicDataDescriptor::new(0x3000, 3).unwrap().with_more(true))
        .unwrap();
    assert_eq!(first.copy_len(), 3);
    assert!(first.more_fragment());
    let partial = device.complete_tx_dma_copy(&[1, 2, 3], 2, 5).unwrap();
    assert!(!partial.packet_complete());
    assert_eq!(partial.assembled_bytes(), 3);
    assert_eq!(partial.tx_packet_count(), 0);
    assert_eq!(
        partial
            .interrupt_record()
            .expect("partial tx dma interrupt")
            .masked_bits()
            .bits(),
        SinicInterrupts::TX_DMA.bits()
    );

    let second = device
        .begin_tx_dma_copy(SinicDataDescriptor::new(0x4000, 2).unwrap())
        .unwrap();
    assert_eq!(second.packet_offset(), 3);
    let complete = device.complete_tx_dma_copy(&[4, 5], 3, 5).unwrap();
    assert!(complete.packet_complete());
    assert_eq!(complete.assembled_bytes(), 5);
    assert_eq!(complete.tx_packet_count(), 1);
    assert_eq!(
        complete
            .interrupt_record()
            .expect("tx dma and full interrupt")
            .masked_bits()
            .bits(),
        (SinicInterrupts::TX_DMA | SinicInterrupts::TX_FULL).bits()
    );

    let mut registry = EthernetInterfaceRegistry::new();
    let nic = registry.register("sinic").unwrap();
    let peer = registry.register("peer").unwrap();
    registry.bind_pair(nic, peer).unwrap();
    let sent = device
        .transmit_one(&mut registry, nic, 4, 0)
        .unwrap()
        .unwrap();
    assert_eq!(sent.send_record().packet().payload(), &[1, 2, 3, 4, 5]);
}

#[test]
fn sinic_dma_tx_checksum_descriptor_fills_ipv4_tcp_checksum() {
    let mut device = SinicFifoDevice::new(
        SinicRegisterParams::default()
            .with_fifo_limits(128, 128, 16, 16, 96, 96)
            .with_interrupt_mask(SinicInterrupts::TX_DMA),
    )
    .unwrap();
    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_INT_EN | SinicRegisterBlock::CONFIG_TX_EN,
            1,
        )
        .unwrap();
    let frame = ipv4_tcp_frame_with_blank_checksums(&[0xde, 0xad, 0xbe, 0xef]);

    device
        .begin_tx_dma_copy(
            SinicDataDescriptor::new(0x3000, frame.len() as u32)
                .unwrap()
                .with_checksum(true),
        )
        .unwrap();
    let complete = device.complete_tx_dma_copy(&frame, 2, 0).unwrap();
    assert!(complete.packet_complete());

    let sent = transmit_single(&mut device);
    let payload = sent.send_record().packet().payload();
    assert_ne!(u16_at(payload, IPV4_HEADER_CHECKSUM_OFFSET), 0);
    assert_ne!(u16_at(payload, IPV4_TCP_CHECKSUM_OFFSET), 0);
    assert_eq!(
        internet_checksum(&payload[ETHERNET_HEADER_LEN..ETHERNET_HEADER_LEN + IPV4_HEADER_LEN]),
        0
    );
    assert_eq!(ipv4_transport_checksum(payload), 0);
}

#[test]
fn sinic_dma_tx_checksum_descriptor_fills_ipv4_udp_checksum() {
    let mut device = SinicFifoDevice::new(
        SinicRegisterParams::default()
            .with_fifo_limits(128, 128, 16, 16, 96, 96)
            .with_interrupt_mask(SinicInterrupts::TX_DMA),
    )
    .unwrap();
    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_INT_EN | SinicRegisterBlock::CONFIG_TX_EN,
            1,
        )
        .unwrap();
    let frame = ipv4_udp_frame_with_blank_checksums(&[0xca, 0xfe, 0xba, 0xbe]);

    device
        .begin_tx_dma_copy(
            SinicDataDescriptor::new(0x3000, frame.len() as u32)
                .unwrap()
                .with_checksum(true),
        )
        .unwrap();
    let complete = device.complete_tx_dma_copy(&frame, 2, 0).unwrap();
    assert!(complete.packet_complete());

    let sent = transmit_single(&mut device);
    let payload = sent.send_record().packet().payload();
    assert_ne!(u16_at(payload, IPV4_HEADER_CHECKSUM_OFFSET), 0);
    assert_ne!(u16_at(payload, IPV4_UDP_CHECKSUM_OFFSET), 0);
    assert_eq!(
        internet_checksum(&payload[ETHERNET_HEADER_LEN..ETHERNET_HEADER_LEN + IPV4_HEADER_LEN]),
        0
    );
    assert_eq!(ipv4_transport_checksum(payload), 0);
}

#[test]
fn sinic_dma_errors_preserve_pending_state_and_restore_partial_tx_packet() {
    let params = SinicRegisterParams::default()
        .with_fifo_limits(16, 16, 4, 4, 12, 12)
        .with_interrupt_mask(SinicInterrupts::TX_DMA);
    let mut device = SinicFifoDevice::new(params).unwrap();

    device
        .begin_tx_dma_copy(SinicDataDescriptor::new(0x5000, 3).unwrap())
        .unwrap();
    assert!(matches!(
        device.begin_tx_dma_copy(SinicDataDescriptor::new(0x6000, 1).unwrap()),
        Err(SinicError::DmaCopyAlreadyPending {
            direction: rem6_net::SinicDmaDirection::Transmit,
        })
    ));
    assert!(matches!(
        device.complete_tx_dma_copy(&[1, 2], 1, 0),
        Err(SinicError::DmaCompletionLengthMismatch {
            direction: rem6_net::SinicDmaDirection::Transmit,
            expected_bytes: 3,
            actual_bytes: 2,
        })
    ));
    let complete = device.complete_tx_dma_copy(&[1, 2, 3], 2, 0).unwrap();
    assert!(complete.packet_complete());
    assert_eq!(complete.done_status().copy_len(), 3);

    let mut snapshot_device = SinicFifoDevice::new(params).unwrap();
    snapshot_device
        .begin_tx_dma_copy(SinicDataDescriptor::new(0x7000, 2).unwrap().with_more(true))
        .unwrap();
    snapshot_device.complete_tx_dma_copy(&[7, 8], 3, 0).unwrap();
    let snapshot = snapshot_device.snapshot();
    snapshot_device
        .begin_tx_dma_copy(SinicDataDescriptor::new(0x8000, 1).unwrap())
        .unwrap();
    snapshot_device.complete_tx_dma_copy(&[1], 4, 0).unwrap();
    assert_eq!(snapshot_device.tx_packet_count(), 1);

    snapshot_device.restore(&snapshot).unwrap();
    assert_eq!(snapshot_device.tx_packet_count(), 0);
    snapshot_device
        .begin_tx_dma_copy(SinicDataDescriptor::new(0x9000, 1).unwrap())
        .unwrap();
    snapshot_device.complete_tx_dma_copy(&[9], 5, 0).unwrap();

    let mut registry = EthernetInterfaceRegistry::new();
    let nic = registry.register("sinic").unwrap();
    let peer = registry.register("peer").unwrap();
    registry.bind_pair(nic, peer).unwrap();
    let sent = snapshot_device
        .transmit_one(&mut registry, nic, 6, 0)
        .unwrap()
        .unwrap();
    assert_eq!(sent.send_record().packet().payload(), &[7, 8, 9]);
}

const ETHERNET_HEADER_LEN: usize = 14;
const IPV4_HEADER_LEN: usize = 20;
const IPV4_PROTOCOL_OFFSET: usize = ETHERNET_HEADER_LEN + 9;
const IPV4_HEADER_CHECKSUM_OFFSET: usize = ETHERNET_HEADER_LEN + 10;
const IPV4_TCP_CHECKSUM_OFFSET: usize = ETHERNET_HEADER_LEN + IPV4_HEADER_LEN + 16;
const IPV4_UDP_CHECKSUM_OFFSET: usize = ETHERNET_HEADER_LEN + IPV4_HEADER_LEN + 6;

fn transmit_single(device: &mut SinicFifoDevice) -> rem6_net::SinicTransmitRecord {
    let mut registry = EthernetInterfaceRegistry::new();
    let nic = registry.register("sinic").unwrap();
    let peer = registry.register("peer").unwrap();
    registry.bind_pair(nic, peer).unwrap();
    device
        .transmit_one(&mut registry, nic, 4, 0)
        .unwrap()
        .unwrap()
}

fn ipv4_tcp_frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = ipv4_tcp_frame_with_blank_checksums(payload);
    fill_ipv4_tcp_checksums(&mut frame);
    frame
}

fn ipv4_udp_frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = ipv4_udp_frame_with_blank_checksums(payload);
    fill_ipv4_udp_checksums(&mut frame);
    frame
}

fn ipv4_tcp_frame_with_blank_checksums(payload: &[u8]) -> Vec<u8> {
    let tcp_len = 20 + payload.len();
    let total_len = IPV4_HEADER_LEN + tcp_len;
    let mut frame = ipv4_frame_header(total_len, 6);
    frame.extend_from_slice(&[
        0x12, 0x34, 0x56, 0x78, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x50, 0x18, 0x20,
        0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
    frame.extend_from_slice(payload);
    frame
}

fn ipv4_udp_frame_with_blank_checksums(payload: &[u8]) -> Vec<u8> {
    let udp_len = 8 + payload.len();
    let total_len = IPV4_HEADER_LEN + udp_len;
    let mut frame = ipv4_frame_header(total_len, 17);
    frame.extend_from_slice(&[
        0x12,
        0x34,
        0x56,
        0x78,
        (udp_len >> 8) as u8,
        udp_len as u8,
        0x00,
        0x00,
    ]);
    frame.extend_from_slice(payload);
    frame
}

fn ipv4_frame_header(total_len: usize, protocol: u8) -> Vec<u8> {
    let mut frame = vec![
        0x00,
        0x11,
        0x22,
        0x33,
        0x44,
        0x55,
        0x66,
        0x77,
        0x88,
        0x99,
        0xaa,
        0xbb,
        0x08,
        0x00,
        0x45,
        0x00,
        (total_len >> 8) as u8,
        total_len as u8,
        0x12,
        0x34,
        0x40,
        0x00,
        0x40,
        protocol,
        0x00,
        0x00,
        192,
        0,
        2,
        1,
        198,
        51,
        100,
        2,
    ];
    let ip_sum =
        internet_checksum(&frame[ETHERNET_HEADER_LEN..ETHERNET_HEADER_LEN + IPV4_HEADER_LEN]);
    write_u16(&mut frame, IPV4_HEADER_CHECKSUM_OFFSET, ip_sum);
    frame
}

fn fill_ipv4_tcp_checksums(frame: &mut [u8]) {
    let tcp_sum = ipv4_transport_checksum(frame);
    write_u16(frame, IPV4_TCP_CHECKSUM_OFFSET, tcp_sum);
}

fn fill_ipv4_udp_checksums(frame: &mut [u8]) {
    let udp_sum = ipv4_transport_checksum(frame);
    write_u16(frame, IPV4_UDP_CHECKSUM_OFFSET, udp_sum);
}

fn ipv4_transport_checksum(frame: &[u8]) -> u16 {
    let ip_start = ETHERNET_HEADER_LEN;
    let ihl = usize::from(frame[ip_start] & 0x0f) * 4;
    let total_len = usize::from(u16_at(frame, ip_start + 2));
    let protocol = frame[IPV4_PROTOCOL_OFFSET];
    let transport_start = ip_start + ihl;
    let transport_len = total_len - ihl;
    let mut checksum_bytes = Vec::new();
    checksum_bytes.extend_from_slice(&frame[ip_start + 12..ip_start + 20]);
    checksum_bytes.push(0);
    checksum_bytes.push(protocol);
    checksum_bytes.push((transport_len >> 8) as u8);
    checksum_bytes.push(transport_len as u8);
    checksum_bytes.extend_from_slice(&frame[transport_start..transport_start + transport_len]);
    internet_checksum(&checksum_bytes)
}

fn internet_checksum(bytes: &[u8]) -> u16 {
    let mut sum = 0_u32;
    let mut chunks = bytes.chunks_exact(2);
    for chunk in &mut chunks {
        sum += u32::from(u16::from_be_bytes([chunk[0], chunk[1]]));
    }
    if let [last] = chunks.remainder() {
        sum += u32::from(*last) << 8;
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    let [high, low] = value.to_be_bytes();
    bytes[offset] = high;
    bytes[offset + 1] = low;
}

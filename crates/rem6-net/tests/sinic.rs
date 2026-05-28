use rem6_net::{
    SinicDataDescriptor, SinicDoneStatus, SinicError, SinicInterrupts, SinicRegisterBlock,
    SinicRegisterOffset, SinicRegisterParams, SinicRxStatus,
};

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

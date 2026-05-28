use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryRequestId, MemoryTargetId,
    PartitionedMemoryStore,
};
use rem6_net::{
    EthernetInterfaceRegistry, EthernetPacket, SinicDataDescriptor, SinicDmaMemoryBackend,
    SinicError, SinicFifoDevice, SinicInterrupts, SinicRegisterBlock, SinicRegisterParams,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(4).unwrap()
}

fn packet(bytes: &[u8]) -> EthernetPacket {
    EthernetPacket::new(bytes.to_vec()).unwrap()
}

fn mapped_memory() -> (PartitionedMemoryStore, MemoryTargetId) {
    let target = MemoryTargetId::new(7);
    let mut memory = PartitionedMemoryStore::new();
    memory.add_partition(target, layout()).unwrap();
    memory
        .map_region(
            target,
            Address::new(0x1000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap();
    for (line, bytes) in [
        (0x1000, vec![0xa0, 0xa1, 0xa2, 0xa3]),
        (0x1004, vec![0xb0, 0xb1, 0xb2, 0xb3]),
        (0x2000, vec![10, 11, 12, 13]),
        (0x2004, vec![14, 15, 16, 17]),
    ] {
        memory
            .insert_line(target, Address::new(line), bytes)
            .unwrap();
    }
    (memory, target)
}

#[test]
fn sinic_memory_backend_moves_descriptor_bytes_across_cache_lines() {
    let (mut memory, target) = mapped_memory();
    let mut backend = SinicDmaMemoryBackend::new(AgentId::new(3), layout());
    let mut device = SinicFifoDevice::new(
        SinicRegisterParams::default()
            .with_fifo_limits(32, 32, 4, 4, 24, 24)
            .with_interrupt_mask(SinicInterrupts::RX_DMA | SinicInterrupts::TX_DMA),
    )
    .unwrap();
    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_RX_EN | SinicRegisterBlock::CONFIG_TX_EN,
            0,
        )
        .unwrap();

    device
        .receive_from_wire(packet(&[1, 2, 3, 4, 5, 6]), 1, 0)
        .unwrap();
    device
        .begin_rx_dma_copy(SinicDataDescriptor::new(0x1002, 6).unwrap())
        .unwrap();
    let rx = backend
        .complete_rx_dma_copy_to_memory(&mut device, &mut memory, 2, 0)
        .unwrap();

    assert_eq!(rx.completion().copied_bytes(), 6);
    assert_eq!(rx.completion().remaining_packet_bytes(), 0);
    assert_eq!(rx.completion().rx_packet_count(), 0);
    assert_eq!(
        rx.transactions()
            .iter()
            .map(|transaction| {
                (
                    transaction.request_id(),
                    transaction.target(),
                    transaction.address(),
                    transaction.byte_len(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (
                MemoryRequestId::new(AgentId::new(3), 0),
                target,
                Address::new(0x1002),
                2
            ),
            (
                MemoryRequestId::new(AgentId::new(3), 1),
                target,
                Address::new(0x1004),
                4
            ),
        ]
    );
    assert_eq!(
        memory.line_data(target, Address::new(0x1000)).unwrap(),
        vec![0xa0, 0xa1, 1, 2]
    );
    assert_eq!(
        memory.line_data(target, Address::new(0x1004)).unwrap(),
        vec![3, 4, 5, 6]
    );

    device
        .begin_tx_dma_copy(SinicDataDescriptor::new(0x2003, 5).unwrap())
        .unwrap();
    let tx = backend
        .complete_tx_dma_copy_from_memory(&mut device, &mut memory, 3, 0)
        .unwrap();

    assert!(tx.completion().packet_complete());
    assert_eq!(tx.completion().assembled_bytes(), 5);
    assert_eq!(
        tx.transactions()
            .iter()
            .map(|transaction| {
                (
                    transaction.request_id(),
                    transaction.target(),
                    transaction.address(),
                    transaction.byte_len(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (
                MemoryRequestId::new(AgentId::new(3), 2),
                target,
                Address::new(0x2003),
                1
            ),
            (
                MemoryRequestId::new(AgentId::new(3), 3),
                target,
                Address::new(0x2004),
                4
            ),
        ]
    );

    let mut registry = EthernetInterfaceRegistry::new();
    let nic = registry.register("sinic").unwrap();
    let peer = registry.register("peer").unwrap();
    registry.bind_pair(nic, peer).unwrap();
    let sent = device
        .transmit_one(&mut registry, nic, 4, 0)
        .unwrap()
        .unwrap();
    assert_eq!(sent.send_record().packet().payload(), &[13, 14, 15, 16, 17]);
    assert_eq!(backend.next_sequence(), 4);
}

#[test]
fn sinic_memory_backend_preserves_pending_dma_after_memory_error() {
    let (mut memory, target) = mapped_memory();
    let mut backend = SinicDmaMemoryBackend::new(AgentId::new(4), layout());
    let mut device = SinicFifoDevice::new(
        SinicRegisterParams::default()
            .with_fifo_limits(32, 32, 4, 4, 24, 24)
            .with_interrupt_mask(SinicInterrupts::RX_DMA | SinicInterrupts::TX_DMA),
    )
    .unwrap();
    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_RX_EN | SinicRegisterBlock::CONFIG_TX_EN,
            0,
        )
        .unwrap();

    device
        .receive_from_wire(packet(&[31, 32, 33, 34]), 1, 0)
        .unwrap();
    device
        .begin_rx_dma_copy(SinicDataDescriptor::new(0x1008, 4).unwrap())
        .unwrap();
    assert!(matches!(
        backend.complete_rx_dma_copy_to_memory(&mut device, &mut memory, 2, 0),
        Err(SinicError::Memory {
            source: MemoryError::UnmappedLine { line }
        }) if line == Address::new(0x1008)
    ));
    memory
        .insert_line(target, Address::new(0x1008), vec![0, 0, 0, 0])
        .unwrap();
    let rx = backend
        .complete_rx_dma_copy_to_memory(&mut device, &mut memory, 3, 0)
        .unwrap();
    assert_eq!(rx.completion().copied_bytes(), 4);
    assert_eq!(
        memory.line_data(target, Address::new(0x1008)).unwrap(),
        vec![31, 32, 33, 34]
    );

    device
        .begin_tx_dma_copy(SinicDataDescriptor::new(0x100c, 4).unwrap())
        .unwrap();
    assert!(matches!(
        backend.complete_tx_dma_copy_from_memory(&mut device, &mut memory, 4, 0),
        Err(SinicError::Memory {
            source: MemoryError::UnmappedLine { line }
        }) if line == Address::new(0x100c)
    ));
    memory
        .insert_line(target, Address::new(0x100c), vec![41, 42, 43, 44])
        .unwrap();
    let tx = backend
        .complete_tx_dma_copy_from_memory(&mut device, &mut memory, 5, 0)
        .unwrap();
    assert!(tx.completion().packet_complete());

    let mut registry = EthernetInterfaceRegistry::new();
    let nic = registry.register("sinic").unwrap();
    let peer = registry.register("peer").unwrap();
    registry.bind_pair(nic, peer).unwrap();
    let sent = device
        .transmit_one(&mut registry, nic, 6, 0)
        .unwrap()
        .unwrap();
    assert_eq!(sent.send_record().packet().payload(), &[41, 42, 43, 44]);
    assert_eq!(backend.next_sequence(), 4);
}

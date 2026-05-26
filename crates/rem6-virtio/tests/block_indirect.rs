use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId, PartitionedMemoryStore,
};
use rem6_virtio::{
    VirtioBlockConfigSpec, VirtioBlockDevice, VirtioBlockMemoryBackend, VirtioGuestMemory,
    VirtioQueueIndex, VirtioSplitQueue, VirtioSplitUsedElement, VIRTIO_BLOCK_SECTOR_SIZE,
    VIRTIO_BLOCK_S_OK, VIRTIO_BLOCK_T_IN, VIRTIO_SPLIT_DESC_F_INDIRECT, VIRTIO_SPLIT_DESC_F_NEXT,
    VIRTIO_SPLIT_DESC_F_WRITE,
};

fn queue(index: u16) -> VirtioQueueIndex {
    VirtioQueueIndex::new(index).unwrap()
}

fn header(raw_type: u32, sector: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(raw_type.to_le_bytes());
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend(sector.to_le_bytes());
    bytes
}

fn sector(byte: u8) -> Vec<u8> {
    vec![byte; VIRTIO_BLOCK_SECTOR_SIZE as usize]
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn guest_store() -> PartitionedMemoryStore {
    let target = MemoryTargetId::new(2);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x1000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    for line in (0x1000..0x2000).step_by(64) {
        store
            .insert_line(target, Address::new(line), vec![0; 64])
            .unwrap();
    }
    store
}

fn write_guest(store: &mut PartitionedMemoryStore, address: u64, bytes: &[u8], sequence: u64) {
    let mut cursor = 0;
    while cursor < bytes.len() {
        let current = address + cursor as u64;
        let line_remaining = 64 - (current % 64) as usize;
        let count = line_remaining.min(bytes.len() - cursor);
        let request = MemoryRequest::write(
            request_id(sequence + cursor as u64),
            Address::new(current),
            AccessSize::new(count as u64).unwrap(),
            bytes[cursor..cursor + count].to_vec(),
            ByteMask::from_bits(vec![true; count]).unwrap(),
            layout(),
        )
        .unwrap();
        store.respond(&request).unwrap();
        cursor += count;
    }
}

fn read_guest(
    store: &mut PartitionedMemoryStore,
    address: u64,
    bytes: usize,
    sequence: u64,
) -> Vec<u8> {
    let mut data = Vec::new();
    let mut cursor = 0;
    while cursor < bytes {
        let current = address + cursor as u64;
        let line_remaining = 64 - (current % 64) as usize;
        let count = line_remaining.min(bytes - cursor);
        let request = MemoryRequest::read_shared(
            request_id(sequence + cursor as u64),
            Address::new(current),
            AccessSize::new(count as u64).unwrap(),
            layout(),
        )
        .unwrap();
        let outcome = store.respond(&request).unwrap();
        data.extend_from_slice(outcome.response().unwrap().data().unwrap());
        cursor += count;
    }
    data
}

fn descriptor(address: u64, length: u32, flags: u16, next: u16) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(address.to_le_bytes());
    bytes.extend(length.to_le_bytes());
    bytes.extend(flags.to_le_bytes());
    bytes.extend(next.to_le_bytes());
    bytes
}

#[test]
fn virtio_split_queue_consumes_indirect_block_read_chain_from_guest_memory() {
    let mut store = guest_store();
    write_guest(
        &mut store,
        0x1030,
        &descriptor(0x1600, 48, VIRTIO_SPLIT_DESC_F_INDIRECT, 0),
        1,
    );
    write_guest(&mut store, 0x1102, &1_u16.to_le_bytes(), 2);
    write_guest(&mut store, 0x1104, &3_u16.to_le_bytes(), 3);
    write_guest(
        &mut store,
        0x1600,
        &descriptor(0x1200, 16, VIRTIO_SPLIT_DESC_F_NEXT, 1),
        4,
    );
    write_guest(
        &mut store,
        0x1610,
        &descriptor(
            0x1300,
            VIRTIO_BLOCK_SECTOR_SIZE as u32,
            VIRTIO_SPLIT_DESC_F_NEXT | VIRTIO_SPLIT_DESC_F_WRITE,
            2,
        ),
        5,
    );
    write_guest(
        &mut store,
        0x1620,
        &descriptor(0x1500, 1, VIRTIO_SPLIT_DESC_F_WRITE, 0),
        6,
    );
    write_guest(&mut store, 0x1200, &header(VIRTIO_BLOCK_T_IN, 0), 7);

    let backend = VirtioBlockMemoryBackend::from_bytes(sector(0x5a)).unwrap();
    let device = VirtioBlockDevice::new(VirtioBlockConfigSpec::new(1), backend).unwrap();
    let mut split_queue = VirtioSplitQueue::new(
        4,
        Address::new(0x1000),
        Address::new(0x1100),
        Address::new(0x1800),
        0,
    )
    .unwrap();
    {
        let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(9));
        let decoded = split_queue
            .consume_available_block(&mut guest, queue(2))
            .unwrap()
            .unwrap();

        assert_eq!(decoded.request().id().get(), 3);
        assert_eq!(decoded.request().queue(), queue(2));
        assert_eq!(decoded.writable_data_bytes(), VIRTIO_BLOCK_SECTOR_SIZE);
        assert_eq!(decoded.used_length(), 529);

        let completion = device.execute_at(55, decoded.request().clone()).unwrap();
        let writeback = split_queue
            .complete_block_request(&mut guest, &decoded, &completion)
            .unwrap();

        assert_eq!(
            writeback.used_element(),
            VirtioSplitUsedElement::new(3, 529)
        );
    }

    assert_eq!(read_guest(&mut store, 0x1300, 512, 100), sector(0x5a));
    assert_eq!(
        read_guest(&mut store, 0x1500, 1, 200),
        vec![VIRTIO_BLOCK_S_OK]
    );
    assert_eq!(
        read_guest(&mut store, 0x1804, 8, 300),
        VirtioSplitUsedElement::new(3, 529).to_le_bytes()
    );
    assert_eq!(read_guest(&mut store, 0x1802, 2, 400), 1_u16.to_le_bytes());
}

#[test]
fn virtio_split_queue_ignores_writable_flag_on_indirect_table_descriptor() {
    let mut store = guest_store();
    write_guest(
        &mut store,
        0x1030,
        &descriptor(
            0x1600,
            48,
            VIRTIO_SPLIT_DESC_F_INDIRECT | VIRTIO_SPLIT_DESC_F_WRITE,
            0,
        ),
        1,
    );
    write_guest(&mut store, 0x1102, &1_u16.to_le_bytes(), 2);
    write_guest(&mut store, 0x1104, &3_u16.to_le_bytes(), 3);
    write_guest(
        &mut store,
        0x1600,
        &descriptor(0x1200, 16, VIRTIO_SPLIT_DESC_F_NEXT, 1),
        4,
    );
    write_guest(
        &mut store,
        0x1610,
        &descriptor(
            0x1300,
            VIRTIO_BLOCK_SECTOR_SIZE as u32,
            VIRTIO_SPLIT_DESC_F_NEXT | VIRTIO_SPLIT_DESC_F_WRITE,
            2,
        ),
        5,
    );
    write_guest(
        &mut store,
        0x1620,
        &descriptor(0x1500, 1, VIRTIO_SPLIT_DESC_F_WRITE, 0),
        6,
    );
    write_guest(&mut store, 0x1200, &header(VIRTIO_BLOCK_T_IN, 0), 7);

    let mut split_queue = VirtioSplitQueue::new(
        4,
        Address::new(0x1000),
        Address::new(0x1100),
        Address::new(0x1800),
        0,
    )
    .unwrap();
    {
        let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(9));
        let decoded = split_queue
            .consume_available_block(&mut guest, queue(2))
            .unwrap()
            .unwrap();

        assert_eq!(decoded.request().id().get(), 3);
        assert_eq!(decoded.writable_data_bytes(), VIRTIO_BLOCK_SECTOR_SIZE);

        let backend = VirtioBlockMemoryBackend::from_bytes(sector(0xa5)).unwrap();
        let device = VirtioBlockDevice::new(VirtioBlockConfigSpec::new(1), backend).unwrap();
        let completion = device.execute_at(61, decoded.request().clone()).unwrap();
        let writeback = split_queue
            .complete_block_request(&mut guest, &decoded, &completion)
            .unwrap();

        assert_eq!(
            writeback.used_element(),
            VirtioSplitUsedElement::new(3, 529)
        );
    }

    assert_eq!(split_queue.last_available_index(), 1);
    assert_eq!(read_guest(&mut store, 0x1300, 512, 100), sector(0xa5));
    assert_eq!(
        read_guest(&mut store, 0x1500, 1, 200),
        vec![VIRTIO_BLOCK_S_OK]
    );
    assert_eq!(
        read_guest(&mut store, 0x1804, 8, 300),
        VirtioSplitUsedElement::new(3, 529).to_le_bytes()
    );
}

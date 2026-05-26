use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId, PartitionedMemoryStore,
};
use rem6_virtio::{
    VirtioBlockConfigSpec, VirtioBlockDecodedRequest, VirtioBlockDevice, VirtioBlockMemoryBackend,
    VirtioBlockRequestId, VirtioBlockRequestKind, VirtioGuestMemory, VirtioPciIsrDevice,
    VirtioPciIsrEvent, VirtioPciIsrEventKind, VirtioPciIsrStatus, VirtioQueueIndex,
    VirtioSplitDescriptor, VirtioSplitDescriptorChain, VirtioSplitQueue, VirtioSplitUsedElement,
    VirtioSplitUsedRing, VIRTIO_BLOCK_SECTOR_SIZE, VIRTIO_BLOCK_S_OK, VIRTIO_BLOCK_T_FLUSH,
    VIRTIO_BLOCK_T_GET_ID, VIRTIO_BLOCK_T_IN, VIRTIO_BLOCK_T_OUT, VIRTIO_SPLIT_DESC_F_NEXT,
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

fn decoded(chain: VirtioSplitDescriptorChain) -> VirtioBlockDecodedRequest {
    chain.decode_block_request(queue(3)).unwrap()
}

#[test]
fn virtio_split_queue_writes_block_completion_to_guest_memory() {
    let mut store = guest_store();
    write_guest(
        &mut store,
        0x1000,
        &descriptor(0x1200, 16, VIRTIO_SPLIT_DESC_F_NEXT, 1),
        1,
    );
    write_guest(
        &mut store,
        0x1010,
        &descriptor(
            0x1300,
            VIRTIO_BLOCK_SECTOR_SIZE as u32,
            VIRTIO_SPLIT_DESC_F_NEXT | VIRTIO_SPLIT_DESC_F_WRITE,
            2,
        ),
        2,
    );
    write_guest(
        &mut store,
        0x1020,
        &descriptor(0x1500, 1, VIRTIO_SPLIT_DESC_F_WRITE, 0),
        3,
    );
    write_guest(&mut store, 0x1102, &1_u16.to_le_bytes(), 4);
    write_guest(&mut store, 0x1104, &0_u16.to_le_bytes(), 5);
    write_guest(&mut store, 0x1200, &header(VIRTIO_BLOCK_T_IN, 0), 6);
    write_guest(&mut store, 0x1802, &0xfffe_u16.to_le_bytes(), 7);

    let backend = VirtioBlockMemoryBackend::from_bytes(sector(0x7d)).unwrap();
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
        let completion = device.execute_at(55, decoded.request().clone()).unwrap();
        let writeback = split_queue
            .complete_block_request(&mut guest, &decoded, &completion)
            .unwrap();

        assert_eq!(writeback.used_slot(), 2);
        assert_eq!(
            writeback.used_element(),
            VirtioSplitUsedElement::new(0, 529)
        );
    }

    assert_eq!(read_guest(&mut store, 0x1300, 512, 100), sector(0x7d));
    assert_eq!(
        read_guest(&mut store, 0x1500, 1, 200),
        vec![VIRTIO_BLOCK_S_OK]
    );
    assert_eq!(
        read_guest(&mut store, 0x1814, 8, 300),
        VirtioSplitUsedElement::new(0, 529).to_le_bytes()
    );
    assert_eq!(
        read_guest(&mut store, 0x1802, 2, 400),
        0xffff_u16.to_le_bytes()
    );
}

#[test]
fn virtio_split_queue_raises_isr_after_guest_completion_writeback() {
    let mut store = guest_store();
    write_guest(
        &mut store,
        0x1000,
        &descriptor(0x1200, 16, VIRTIO_SPLIT_DESC_F_NEXT, 1),
        1,
    );
    write_guest(
        &mut store,
        0x1010,
        &descriptor(
            0x1300,
            VIRTIO_BLOCK_SECTOR_SIZE as u32,
            VIRTIO_SPLIT_DESC_F_NEXT | VIRTIO_SPLIT_DESC_F_WRITE,
            2,
        ),
        2,
    );
    write_guest(
        &mut store,
        0x1020,
        &descriptor(0x1500, 1, VIRTIO_SPLIT_DESC_F_WRITE, 0),
        3,
    );
    write_guest(&mut store, 0x1102, &1_u16.to_le_bytes(), 4);
    write_guest(&mut store, 0x1104, &0_u16.to_le_bytes(), 5);
    write_guest(&mut store, 0x1200, &header(VIRTIO_BLOCK_T_IN, 0), 6);

    let backend = VirtioBlockMemoryBackend::from_bytes(sector(0x44)).unwrap();
    let device = VirtioBlockDevice::new(VirtioBlockConfigSpec::new(1), backend).unwrap();
    let isr = VirtioPciIsrDevice::new();
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
        let completion = device.execute_at(77, decoded.request().clone()).unwrap();
        split_queue
            .complete_block_request_and_raise_isr(&mut guest, &decoded, &completion, &isr)
            .unwrap();
    }

    assert_eq!(read_guest(&mut store, 0x1300, 512, 100), sector(0x44));
    assert_eq!(isr.status(), VirtioPciIsrStatus::queue_interrupt());
    assert_eq!(
        isr.events(),
        vec![VirtioPciIsrEvent::new(
            77,
            VirtioPciIsrEventKind::QueueInterrupt,
            VirtioPciIsrStatus::empty(),
            VirtioPciIsrStatus::queue_interrupt(),
        )]
    );
}

#[test]
fn virtio_split_queue_walks_available_block_request_from_guest_memory() {
    let mut store = guest_store();
    write_guest(
        &mut store,
        0x1000,
        &descriptor(0x1200, 16, VIRTIO_SPLIT_DESC_F_NEXT, 1),
        1,
    );
    write_guest(
        &mut store,
        0x1010,
        &descriptor(
            0x1300,
            VIRTIO_BLOCK_SECTOR_SIZE as u32,
            VIRTIO_SPLIT_DESC_F_NEXT,
            2,
        ),
        2,
    );
    write_guest(
        &mut store,
        0x1020,
        &descriptor(0x1500, 1, VIRTIO_SPLIT_DESC_F_WRITE, 0),
        3,
    );
    write_guest(&mut store, 0x1102, &1_u16.to_le_bytes(), 4);
    write_guest(&mut store, 0x1104, &0_u16.to_le_bytes(), 5);
    write_guest(&mut store, 0x1200, &header(VIRTIO_BLOCK_T_OUT, 3), 6);
    write_guest(&mut store, 0x1300, &sector(0x6a), 7);

    let mut split_queue = VirtioSplitQueue::new(
        4,
        Address::new(0x1000),
        Address::new(0x1100),
        Address::new(0x1800),
        0,
    )
    .unwrap();
    let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(9));
    let decoded = split_queue
        .consume_available_block(&mut guest, queue(2))
        .unwrap()
        .unwrap();

    assert_eq!(split_queue.last_available_index(), 1);
    assert_eq!(decoded.request().id(), VirtioBlockRequestId::new(0));
    assert_eq!(decoded.request().queue(), queue(2));
    assert_eq!(decoded.request().sector(), 3);
    assert_eq!(decoded.status_descriptor(), 2);
    assert_eq!(
        decoded.request().kind(),
        &VirtioBlockRequestKind::Write { data: sector(0x6a) }
    );
    assert!(split_queue
        .consume_available_block(&mut guest, queue(2))
        .unwrap()
        .is_none());
}

#[test]
fn virtio_split_descriptor_chain_decodes_block_requests() {
    let write = decoded(
        VirtioSplitDescriptorChain::new(
            0,
            vec![
                VirtioSplitDescriptor::device_readable(0, header(VIRTIO_BLOCK_T_OUT, 7), Some(1)),
                VirtioSplitDescriptor::device_readable(1, sector(0xab), Some(2)),
                VirtioSplitDescriptor::device_writable(2, 1, None),
            ],
        )
        .unwrap(),
    );
    assert_eq!(write.request().id(), VirtioBlockRequestId::new(0));
    assert_eq!(write.request().queue(), queue(3));
    assert_eq!(write.request().sector(), 7);
    assert_eq!(write.status_descriptor(), 2);
    assert_eq!(write.writable_data_bytes(), 0);
    assert_eq!(
        write.request().kind(),
        &VirtioBlockRequestKind::Write { data: sector(0xab) }
    );

    let read = decoded(
        VirtioSplitDescriptorChain::new(
            4,
            vec![
                VirtioSplitDescriptor::device_readable(4, header(VIRTIO_BLOCK_T_IN, 9), Some(5)),
                VirtioSplitDescriptor::device_writable(5, VIRTIO_BLOCK_SECTOR_SIZE as u32, Some(6)),
                VirtioSplitDescriptor::device_writable(6, 1, None),
            ],
        )
        .unwrap(),
    );
    assert_eq!(read.request().id(), VirtioBlockRequestId::new(4));
    assert_eq!(read.request().sector(), 9);
    assert_eq!(read.status_descriptor(), 6);
    assert_eq!(read.writable_data_bytes(), VIRTIO_BLOCK_SECTOR_SIZE);
    assert_eq!(
        read.request().kind(),
        &VirtioBlockRequestKind::Read {
            bytes: VIRTIO_BLOCK_SECTOR_SIZE
        }
    );

    let flush = decoded(
        VirtioSplitDescriptorChain::new(
            8,
            vec![
                VirtioSplitDescriptor::device_readable(8, header(VIRTIO_BLOCK_T_FLUSH, 0), Some(9)),
                VirtioSplitDescriptor::device_writable(9, 1, None),
            ],
        )
        .unwrap(),
    );
    assert_eq!(flush.request().kind(), &VirtioBlockRequestKind::Flush);
    assert_eq!(flush.status_descriptor(), 9);
    assert_eq!(flush.writable_data_bytes(), 0);

    let get_id = decoded(
        VirtioSplitDescriptorChain::new(
            11,
            vec![
                VirtioSplitDescriptor::device_readable(
                    11,
                    header(VIRTIO_BLOCK_T_GET_ID, 0),
                    Some(12),
                ),
                VirtioSplitDescriptor::device_writable(12, 20, Some(13)),
                VirtioSplitDescriptor::device_writable(13, 1, None),
            ],
        )
        .unwrap(),
    );
    assert_eq!(get_id.request().kind(), &VirtioBlockRequestKind::GetId);
    assert_eq!(get_id.status_descriptor(), 13);
    assert_eq!(get_id.writable_data_bytes(), 20);
}

#[test]
fn virtio_split_descriptor_chain_rejects_bad_block_shapes() {
    assert!(matches!(
        VirtioSplitDescriptorChain::new(
            0,
            vec![
                VirtioSplitDescriptor::device_readable(0, header(VIRTIO_BLOCK_T_IN, 0), Some(1)),
                VirtioSplitDescriptor::device_writable(1, 1, Some(0)),
            ],
        ),
        Err(error) if error.to_string().contains("loop")
    ));
    assert!(matches!(
        VirtioSplitDescriptorChain::new(
            0,
            vec![VirtioSplitDescriptor::device_readable(0, vec![0; 8], None)],
        )
        .unwrap()
        .decode_block_request(queue(0)),
        Err(error) if error.to_string().contains("header")
    ));
    assert!(matches!(
        VirtioSplitDescriptorChain::new(
            0,
            vec![VirtioSplitDescriptor::device_readable(
                0,
                header(VIRTIO_BLOCK_T_IN, 0),
                None,
            )],
        )
        .unwrap()
        .decode_block_request(queue(0)),
        Err(error) if error.to_string().contains("status")
    ));
    assert!(matches!(
        VirtioSplitDescriptorChain::new(
            0,
            vec![
                VirtioSplitDescriptor::device_readable(0, header(VIRTIO_BLOCK_T_IN, 0), Some(1)),
                VirtioSplitDescriptor::device_readable(1, sector(0x11), Some(2)),
                VirtioSplitDescriptor::device_writable(2, 1, None),
            ],
        )
        .unwrap()
        .decode_block_request(queue(0)),
        Err(error) if error.to_string().contains("writable")
    ));
    assert!(matches!(
        VirtioSplitDescriptorChain::new(
            0,
            vec![
                VirtioSplitDescriptor::device_readable(0, header(VIRTIO_BLOCK_T_OUT, 0), Some(1)),
                VirtioSplitDescriptor::device_writable(1, VIRTIO_BLOCK_SECTOR_SIZE as u32, Some(2)),
                VirtioSplitDescriptor::device_writable(2, 1, None),
            ],
        )
        .unwrap()
        .decode_block_request(queue(0)),
        Err(error) if error.to_string().contains("readable")
    ));
    assert!(matches!(
        VirtioSplitDescriptorChain::new(
            0,
            vec![
                VirtioSplitDescriptor::device_readable(0, header(VIRTIO_BLOCK_T_GET_ID, 0), Some(1)),
                VirtioSplitDescriptor::device_writable(1, 19, Some(2)),
                VirtioSplitDescriptor::device_writable(2, 1, None),
            ],
        )
        .unwrap()
        .decode_block_request(queue(0)),
        Err(error) if error.to_string().contains("device id")
    ));
}

#[test]
fn virtio_split_used_ring_records_block_completion_writeback() {
    let data = sector(0x44);
    let backend = VirtioBlockMemoryBackend::from_bytes(data.clone()).unwrap();
    let device = VirtioBlockDevice::new(VirtioBlockConfigSpec::new(1), backend.clone()).unwrap();
    let decoded = decoded(
        VirtioSplitDescriptorChain::new(
            14,
            vec![
                VirtioSplitDescriptor::device_readable(14, header(VIRTIO_BLOCK_T_IN, 0), Some(15)),
                VirtioSplitDescriptor::device_writable(15, 300, Some(16)),
                VirtioSplitDescriptor::device_writable(16, 212, Some(17)),
                VirtioSplitDescriptor::device_writable(17, 1, None),
            ],
        )
        .unwrap(),
    );
    let completion = device.execute_at(41, decoded.request().clone()).unwrap();

    let mut used_ring = VirtioSplitUsedRing::new(4, 0xfffe).unwrap();
    let writeback = used_ring
        .complete_block_request(&decoded, &completion)
        .unwrap();

    assert_eq!(writeback.status_write().descriptor(), 17);
    assert_eq!(writeback.status_write().offset(), 0);
    assert_eq!(writeback.status_write().bytes(), &[VIRTIO_BLOCK_S_OK]);
    assert_eq!(writeback.data_writes().len(), 2);
    assert_eq!(writeback.data_writes()[0].descriptor(), 15);
    assert_eq!(writeback.data_writes()[0].bytes(), &data[..300]);
    assert_eq!(writeback.data_writes()[1].descriptor(), 16);
    assert_eq!(writeback.data_writes()[1].bytes(), &data[300..]);
    assert_eq!(writeback.used_slot(), 2);
    assert_eq!(writeback.used_index(), 0xffff);
    assert_eq!(
        writeback.used_element(),
        VirtioSplitUsedElement::new(14, 529)
    );
    assert_eq!(used_ring.index(), 0xffff);
    assert_eq!(
        used_ring.entry(2),
        Some(&VirtioSplitUsedElement::new(14, 529))
    );
    assert_eq!(
        writeback.used_element().to_le_bytes(),
        [14, 0, 0, 0, 17, 2, 0, 0]
    );
}

use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId, PartitionedMemoryStore,
};
use rem6_virtio::{
    VirtioError, VirtioGuestMemory, VirtioPciIsrDevice, VirtioPciIsrEventKind, VirtioQueueIndex,
    VirtioRngByteSource, VirtioRngDevice, VirtioRngRequest, VirtioRngRequestId,
    VirtioSplitDescriptor, VirtioSplitDescriptorChain, VirtioSplitQueue, VirtioSplitUsedElement,
    VirtioSplitUsedRing, VIRTIO_F_VERSION_1_PAGE_BITS, VIRTIO_RNG_DEVICE_ID,
    VIRTIO_SPLIT_DESC_F_NEXT, VIRTIO_SPLIT_DESC_F_WRITE,
};

fn queue(index: u16) -> VirtioQueueIndex {
    VirtioQueueIndex::new(index).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(11), sequence)
}

fn guest_store() -> PartitionedMemoryStore {
    let target = MemoryTargetId::new(3);
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
fn virtio_rng_device_reports_gem5_device_id_and_reproducible_entropy() {
    assert_eq!(VIRTIO_RNG_DEVICE_ID, 4);
    let device =
        VirtioRngDevice::new(VirtioRngByteSource::repeating(vec![0x10, 0x20, 0x30]).unwrap());

    assert_eq!(
        device.feature_pages(),
        vec![(1, VIRTIO_F_VERSION_1_PAGE_BITS)]
    );
    assert_eq!(device.config_size(), 0);

    let request = VirtioRngRequest::new(VirtioRngRequestId::new(7), queue(0), 5).unwrap();
    let completion = device.execute_at(42, request).unwrap();

    assert_eq!(completion.request(), VirtioRngRequestId::new(7));
    assert_eq!(completion.queue(), queue(0));
    assert_eq!(completion.tick(), 42);
    assert_eq!(completion.bytes(), &[0x10, 0x20, 0x30, 0x10, 0x20]);
    assert_eq!(device.completions(), vec![completion]);
}

#[test]
fn virtio_rng_device_rejects_request_above_vec_limit_before_allocation() {
    let device = VirtioRngDevice::new(VirtioRngByteSource::repeating(vec![0x10]).unwrap());
    let request =
        VirtioRngRequest::new(VirtioRngRequestId::new(8), queue(0), isize::MAX as u64 + 1).unwrap();

    assert!(matches!(
        device.execute_at(43, request),
        Err(VirtioError::VirtioRngPayloadLengthOverflow)
    ));
    assert!(device.completions().is_empty());
}

#[test]
fn virtio_rng_descriptor_chain_decodes_writable_buffers_and_used_length() {
    let chain = VirtioSplitDescriptorChain::new(
        0,
        [
            VirtioSplitDescriptor::device_writable(0, 3, Some(1)),
            VirtioSplitDescriptor::device_writable(1, 2, None),
        ],
    )
    .unwrap();
    let decoded = chain.decode_rng_request(queue(0)).unwrap();
    let device = VirtioRngDevice::new(VirtioRngByteSource::repeating(vec![1, 2, 3, 4, 5]).unwrap());
    let completion = device.execute_at(9, decoded.request().clone()).unwrap();
    let mut used_ring = VirtioSplitUsedRing::new(4, 2).unwrap();
    let writeback = used_ring
        .complete_rng_request(&decoded, &completion)
        .unwrap();

    assert_eq!(decoded.request().bytes(), 5);
    assert_eq!(decoded.used_length(), 5);
    assert_eq!(writeback.data_writes().len(), 2);
    assert_eq!(writeback.data_writes()[0].descriptor(), 0);
    assert_eq!(writeback.data_writes()[0].bytes(), &[1, 2, 3]);
    assert_eq!(writeback.data_writes()[1].descriptor(), 1);
    assert_eq!(writeback.data_writes()[1].bytes(), &[4, 5]);
    assert_eq!(writeback.used_slot(), 2);
    assert_eq!(writeback.used_index(), 3);
    assert_eq!(writeback.used_element(), VirtioSplitUsedElement::new(0, 5));
}

#[test]
fn virtio_rng_rejects_gem5_panic_paths_as_typed_errors() {
    assert!(VirtioRngByteSource::repeating(Vec::new()).is_err());

    let readable_chain = VirtioSplitDescriptorChain::new(
        0,
        [VirtioSplitDescriptor::device_readable(0, vec![0xaa], None)],
    )
    .unwrap();
    assert!(readable_chain.decode_rng_request(queue(0)).is_err());

    let empty_chain =
        VirtioSplitDescriptorChain::new(0, [VirtioSplitDescriptor::device_writable(0, 0, None)])
            .unwrap();
    assert!(empty_chain.decode_rng_request(queue(0)).is_err());
}

#[test]
fn virtio_split_queue_completes_rng_request_to_guest_memory_and_raises_isr() {
    let mut store = guest_store();
    write_guest(
        &mut store,
        0x1000,
        &descriptor(
            0x1300,
            3,
            VIRTIO_SPLIT_DESC_F_NEXT | VIRTIO_SPLIT_DESC_F_WRITE,
            1,
        ),
        1,
    );
    write_guest(
        &mut store,
        0x1010,
        &descriptor(0x13fd, 5, VIRTIO_SPLIT_DESC_F_WRITE, 0),
        2,
    );
    write_guest(&mut store, 0x1102, &1_u16.to_le_bytes(), 3);
    write_guest(&mut store, 0x1104, &0_u16.to_le_bytes(), 4);
    write_guest(&mut store, 0x1802, &7_u16.to_le_bytes(), 5);

    let device = VirtioRngDevice::new(
        VirtioRngByteSource::repeating(vec![0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7])
            .unwrap(),
    );
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
        let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(12));
        let decoded = split_queue
            .consume_available_rng(&mut guest, queue(0))
            .unwrap()
            .unwrap();
        let completion = device.execute_at(77, decoded.request().clone()).unwrap();
        let writeback = split_queue
            .complete_rng_request_and_raise_isr(&mut guest, &decoded, &completion, &isr)
            .unwrap();

        assert_eq!(writeback.used_slot(), 3);
        assert_eq!(writeback.used_index(), 8);
        assert_eq!(writeback.used_element(), VirtioSplitUsedElement::new(0, 8));
    }

    assert_eq!(
        read_guest(&mut store, 0x1300, 3, 100),
        vec![0xa0, 0xa1, 0xa2]
    );
    assert_eq!(
        read_guest(&mut store, 0x13fd, 5, 200),
        vec![0xa3, 0xa4, 0xa5, 0xa6, 0xa7]
    );
    assert_eq!(
        read_guest(&mut store, 0x181c, 8, 300),
        VirtioSplitUsedElement::new(0, 8).to_le_bytes()
    );
    assert_eq!(read_guest(&mut store, 0x1802, 2, 400), 8_u16.to_le_bytes());

    let events = isr.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind(), VirtioPciIsrEventKind::QueueInterrupt);
}

#[test]
fn virtio_split_queue_rejects_rng_writeback_before_guest_state_mutates() {
    let mut store = guest_store();
    write_guest(
        &mut store,
        0x1000,
        &descriptor(
            0x1300,
            3,
            VIRTIO_SPLIT_DESC_F_NEXT | VIRTIO_SPLIT_DESC_F_WRITE,
            1,
        ),
        1,
    );
    write_guest(
        &mut store,
        0x1010,
        &descriptor(0x13fd, 5, VIRTIO_SPLIT_DESC_F_WRITE, 0),
        2,
    );
    write_guest(&mut store, 0x1102, &1_u16.to_le_bytes(), 3);
    write_guest(&mut store, 0x1104, &0_u16.to_le_bytes(), 4);
    write_guest(&mut store, 0x1ffe, &0_u16.to_le_bytes(), 5);

    let device = VirtioRngDevice::new(
        VirtioRngByteSource::repeating(vec![0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7])
            .unwrap(),
    );
    let mut split_queue = VirtioSplitQueue::new(
        4,
        Address::new(0x1000),
        Address::new(0x1100),
        Address::new(0x1ffc),
        0,
    )
    .unwrap();
    {
        let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(12));
        let decoded = split_queue
            .consume_available_rng(&mut guest, queue(0))
            .unwrap()
            .unwrap();
        let completion = device.execute_at(78, decoded.request().clone()).unwrap();

        assert!(split_queue
            .complete_rng_request(&mut guest, &decoded, &completion)
            .is_err());
    }

    assert_eq!(read_guest(&mut store, 0x1300, 3, 100), vec![0; 3]);
    assert_eq!(read_guest(&mut store, 0x13fd, 5, 200), vec![0; 5]);
    assert_eq!(read_guest(&mut store, 0x1ffe, 2, 300), vec![0; 2]);
}

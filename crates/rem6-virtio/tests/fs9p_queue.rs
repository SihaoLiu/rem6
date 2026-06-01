use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId, PartitionedMemoryStore,
};
use rem6_virtio::{
    Virtio9pCompletion, Virtio9pRequestId, VirtioError, VirtioGuestMemory, VirtioPciIsrDevice,
    VirtioPciIsrEventKind, VirtioQueueIndex, VirtioSplitDescriptor, VirtioSplitDescriptorChain,
    VirtioSplitQueue, VirtioSplitUsedElement, VirtioSplitUsedRing, VIRTIO_9P_REQUEST_QUEUE_INDEX,
    VIRTIO_SPLIT_DESC_F_NEXT, VIRTIO_SPLIT_DESC_F_WRITE,
};

fn queue(index: u16) -> VirtioQueueIndex {
    VirtioQueueIndex::new(index).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(17), sequence)
}

fn guest_store() -> PartitionedMemoryStore {
    let target = MemoryTargetId::new(4);
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

fn p9_message(message_type: u8, tag: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend((7_u32 + payload.len() as u32).to_le_bytes());
    bytes.push(message_type);
    bytes.extend(tag.to_le_bytes());
    bytes.extend(payload);
    bytes
}

#[test]
fn virtio_9p_decodes_request_header_payload_and_reply_writeback() {
    let mut message = p9_message(100, 0x1234, b"msize=8192");
    let tail = message.split_off(9);
    let chain = VirtioSplitDescriptorChain::new(
        5,
        [
            VirtioSplitDescriptor::device_readable(5, message, Some(6)),
            VirtioSplitDescriptor::device_readable(6, tail, Some(7)),
            VirtioSplitDescriptor::device_writable(7, 4, Some(8)),
            VirtioSplitDescriptor::device_writable(8, 16, None),
        ],
    )
    .unwrap();

    let decoded = chain
        .decode_9p_request(queue(VIRTIO_9P_REQUEST_QUEUE_INDEX))
        .unwrap();

    assert_eq!(decoded.request().id(), Virtio9pRequestId::new(5));
    assert_eq!(
        decoded.request().queue(),
        queue(VIRTIO_9P_REQUEST_QUEUE_INDEX)
    );
    assert_eq!(decoded.request().message_type(), 100);
    assert_eq!(decoded.request().tag(), 0x1234);
    assert_eq!(decoded.request().payload(), b"msize=8192");
    assert_eq!(decoded.writable_data_bytes(), 20);

    let completion = Virtio9pCompletion::new(
        decoded.request().id(),
        decoded.request().queue(),
        44,
        101,
        decoded.request().tag(),
        b"ok".to_vec(),
    )
    .unwrap();
    let mut used_ring = VirtioSplitUsedRing::new(8, 3).unwrap();
    let writeback = used_ring
        .complete_9p_request(&decoded, &completion)
        .unwrap();

    assert_eq!(writeback.data_writes().len(), 2);
    assert_eq!(writeback.data_writes()[0].descriptor(), 7);
    assert_eq!(writeback.data_writes()[0].bytes(), b"\x09\x00\x00\x00");
    assert_eq!(writeback.data_writes()[1].descriptor(), 8);
    assert_eq!(writeback.data_writes()[1].bytes(), b"e\x34\x12ok");
    assert_eq!(writeback.used_slot(), 3);
    assert_eq!(writeback.used_index(), 4);
    assert_eq!(writeback.used_element(), VirtioSplitUsedElement::new(5, 9));
}

#[test]
fn virtio_9p_rejects_malformed_descriptor_chains_as_typed_errors() {
    let short_header = VirtioSplitDescriptorChain::new(
        0,
        [
            VirtioSplitDescriptor::device_readable(0, b"\x01\x00".to_vec(), Some(1)),
            VirtioSplitDescriptor::device_writable(1, 16, None),
        ],
    )
    .unwrap();
    assert!(matches!(
        short_header.decode_9p_request(queue(0)),
        Err(VirtioError::ShortVirtio9pHeader { bytes: 2 })
    ));

    let bad_length = VirtioSplitDescriptorChain::new(
        0,
        [
            VirtioSplitDescriptor::device_readable(
                0,
                p9_message(100, 1, b"abc")[..9].to_vec(),
                Some(1),
            ),
            VirtioSplitDescriptor::device_writable(1, 16, None),
        ],
    )
    .unwrap();
    assert!(matches!(
        bad_length.decode_9p_request(queue(0)),
        Err(VirtioError::InvalidVirtio9pMessageLength {
            declared: 10,
            actual: 9
        })
    ));

    let missing_reply = VirtioSplitDescriptorChain::new(
        0,
        [VirtioSplitDescriptor::device_readable(
            0,
            p9_message(100, 1, b"abc"),
            None,
        )],
    )
    .unwrap();
    assert!(matches!(
        missing_reply.decode_9p_request(queue(0)),
        Err(VirtioError::MissingVirtio9pWritableDescriptor)
    ));

    let readable_reply = VirtioSplitDescriptorChain::new(
        0,
        [
            VirtioSplitDescriptor::device_readable(0, p9_message(100, 1, b"abc"), Some(1)),
            VirtioSplitDescriptor::device_readable(1, b"wrong".to_vec(), None),
        ],
    )
    .unwrap();
    assert!(matches!(
        readable_reply.decode_9p_request(queue(0)),
        Err(VirtioError::InvalidVirtio9pWritableDescriptor { index: 1 })
    ));
}

#[test]
fn virtio_split_queue_completes_9p_reply_to_guest_memory_and_raises_isr() {
    let mut store = guest_store();
    write_guest(&mut store, 0x1400, &p9_message(104, 0x77, b"attach"), 1);
    write_guest(
        &mut store,
        0x1000,
        &descriptor(0x1400, 13, VIRTIO_SPLIT_DESC_F_NEXT, 1),
        2,
    );
    write_guest(
        &mut store,
        0x1010,
        &descriptor(
            0x1500,
            5,
            VIRTIO_SPLIT_DESC_F_NEXT | VIRTIO_SPLIT_DESC_F_WRITE,
            2,
        ),
        3,
    );
    write_guest(
        &mut store,
        0x1020,
        &descriptor(0x153f, 8, VIRTIO_SPLIT_DESC_F_WRITE, 0),
        4,
    );
    write_guest(&mut store, 0x1102, &1_u16.to_le_bytes(), 5);
    write_guest(&mut store, 0x1104, &0_u16.to_le_bytes(), 6);
    write_guest(&mut store, 0x1802, &6_u16.to_le_bytes(), 7);

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
        let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(18));
        let decoded = split_queue
            .consume_available_9p(&mut guest, queue(0))
            .unwrap()
            .unwrap();
        let completion = Virtio9pCompletion::new(
            decoded.request().id(),
            decoded.request().queue(),
            91,
            105,
            decoded.request().tag(),
            b"reply".to_vec(),
        )
        .unwrap();
        let writeback = split_queue
            .complete_9p_request_and_raise_isr(&mut guest, &decoded, &completion, &isr)
            .unwrap();

        assert_eq!(writeback.used_slot(), 2);
        assert_eq!(writeback.used_index(), 7);
        assert_eq!(writeback.used_element(), VirtioSplitUsedElement::new(0, 12));
    }

    assert_eq!(read_guest(&mut store, 0x1500, 5, 100), b"\x0c\x00\x00\x00i");
    assert_eq!(read_guest(&mut store, 0x153f, 7, 200), b"\x77\x00reply");
    assert_eq!(
        read_guest(&mut store, 0x1814, 8, 300),
        VirtioSplitUsedElement::new(0, 12).to_le_bytes()
    );
    assert_eq!(read_guest(&mut store, 0x1802, 2, 400), 7_u16.to_le_bytes());

    let events = isr.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind(), VirtioPciIsrEventKind::QueueInterrupt);
}

#[test]
fn virtio_split_queue_rejects_9p_writeback_before_guest_state_mutates() {
    let mut store = guest_store();
    write_guest(&mut store, 0x1400, &p9_message(104, 0x77, b"attach"), 1);
    write_guest(
        &mut store,
        0x1000,
        &descriptor(0x1400, 13, VIRTIO_SPLIT_DESC_F_NEXT, 1),
        2,
    );
    write_guest(
        &mut store,
        0x1010,
        &descriptor(
            0x1500,
            4,
            VIRTIO_SPLIT_DESC_F_NEXT | VIRTIO_SPLIT_DESC_F_WRITE,
            2,
        ),
        3,
    );
    write_guest(
        &mut store,
        0x1020,
        &descriptor(0x2500, 8, VIRTIO_SPLIT_DESC_F_WRITE, 0),
        4,
    );
    write_guest(&mut store, 0x1102, &1_u16.to_le_bytes(), 5);
    write_guest(&mut store, 0x1104, &0_u16.to_le_bytes(), 6);
    write_guest(&mut store, 0x1802, &6_u16.to_le_bytes(), 7);

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
        let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(18));
        let decoded = split_queue
            .consume_available_9p(&mut guest, queue(0))
            .unwrap()
            .unwrap();
        let completion = Virtio9pCompletion::new(
            decoded.request().id(),
            decoded.request().queue(),
            91,
            105,
            decoded.request().tag(),
            b"reply".to_vec(),
        )
        .unwrap();
        let error = split_queue
            .complete_9p_request_and_raise_isr(&mut guest, &decoded, &completion, &isr)
            .unwrap_err();

        assert!(matches!(
            error,
            VirtioError::PciTransportRuntimeConfig { .. }
        ));
    }

    assert_eq!(read_guest(&mut store, 0x1500, 4, 100), [0; 4]);
    assert_eq!(read_guest(&mut store, 0x1802, 2, 200), 6_u16.to_le_bytes());
    assert_eq!(read_guest(&mut store, 0x1810, 8, 300), [0; 8]);
    assert!(isr.events().is_empty());
}

#[test]
fn virtio_split_queue_keeps_9p_available_cursor_on_decode_error() {
    let mut store = guest_store();
    write_guest(&mut store, 0x1400, &p9_message(100, 1, b"abc"), 1);
    write_guest(&mut store, 0x1000, &descriptor(0x1400, 10, 0, 0), 2);
    write_guest(&mut store, 0x1102, &1_u16.to_le_bytes(), 3);
    write_guest(&mut store, 0x1104, &0_u16.to_le_bytes(), 4);

    let mut split_queue = VirtioSplitQueue::new(
        4,
        Address::new(0x1000),
        Address::new(0x1100),
        Address::new(0x1800),
        0,
    )
    .unwrap();
    let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(18));
    let error = split_queue
        .consume_available_9p(&mut guest, queue(0))
        .unwrap_err();

    assert!(matches!(
        error,
        VirtioError::MissingVirtio9pWritableDescriptor
    ));
    assert_eq!(split_queue.last_available_index(), 0);
}

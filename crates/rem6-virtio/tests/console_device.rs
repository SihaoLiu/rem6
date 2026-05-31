use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId, PartitionedMemoryStore,
};
use rem6_virtio::{
    VirtioConsoleConfig, VirtioConsoleDevice, VirtioError, VirtioGuestMemory, VirtioPciIsrDevice,
    VirtioPciIsrEventKind, VirtioQueueIndex, VirtioSplitDescriptor, VirtioSplitDescriptorChain,
    VirtioSplitQueue, VirtioSplitUsedElement, VirtioSplitUsedRing, VIRTIO_CONSOLE_CONFIG_SIZE,
    VIRTIO_CONSOLE_DEVICE_ID, VIRTIO_CONSOLE_F_SIZE, VIRTIO_SPLIT_DESC_F_NEXT,
    VIRTIO_SPLIT_DESC_F_WRITE,
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
fn virtio_console_reports_gem5_identity_size_feature_and_default_config() {
    let device = VirtioConsoleDevice::new();

    assert_eq!(VIRTIO_CONSOLE_DEVICE_ID, 3);
    assert_eq!(VIRTIO_CONSOLE_CONFIG_SIZE, 4);
    assert_eq!(VIRTIO_CONSOLE_F_SIZE, 1);
    assert_eq!(device.feature_pages(), vec![(0, VIRTIO_CONSOLE_F_SIZE)]);
    assert_eq!(device.config_size(), VIRTIO_CONSOLE_CONFIG_SIZE);
    assert_eq!(device.config(), VirtioConsoleConfig::new(80, 24).unwrap());
    assert_eq!(device.config_bytes(), [80, 0, 24, 0]);
}

#[test]
fn virtio_console_transmit_chain_copies_guest_bytes_to_terminal_and_uses_zero_length() {
    let device = VirtioConsoleDevice::new();
    let chain = VirtioSplitDescriptorChain::new(
        5,
        [
            VirtioSplitDescriptor::device_readable(5, b"hel".to_vec(), Some(6)),
            VirtioSplitDescriptor::device_readable(6, b"lo".to_vec(), None),
        ],
    )
    .unwrap();
    let decoded = chain.decode_console_transmit_request(queue(1)).unwrap();

    assert_eq!(decoded.request().queue(), queue(1));
    assert_eq!(decoded.request().data(), b"hello");

    let completion = device.transmit_at(33, decoded.request().clone()).unwrap();
    let mut used_ring = VirtioSplitUsedRing::new(8, 4).unwrap();
    let writeback = used_ring
        .complete_console_transmit(&decoded, &completion)
        .unwrap();

    assert_eq!(device.guest_output(), b"hello");
    assert_eq!(completion.tick(), 33);
    assert_eq!(completion.used_length(), 0);
    assert!(writeback.data_writes().is_empty());
    assert_eq!(writeback.used_slot(), 4);
    assert_eq!(writeback.used_index(), 5);
    assert_eq!(writeback.used_element(), VirtioSplitUsedElement::new(5, 0));
}

#[test]
fn virtio_console_receive_chain_scatters_host_bytes_and_reports_actual_length() {
    let device = VirtioConsoleDevice::new();
    device.push_host_input(b"abcdef".to_vec());
    let chain = VirtioSplitDescriptorChain::new(
        2,
        [
            VirtioSplitDescriptor::device_writable(2, 3, Some(3)),
            VirtioSplitDescriptor::device_writable(3, 4, None),
        ],
    )
    .unwrap();
    let decoded = chain.decode_console_receive_request(queue(0)).unwrap();

    assert_eq!(decoded.request().queue(), queue(0));
    assert_eq!(decoded.request().capacity(), 7);

    let completion = device.receive_at(44, decoded.request().clone()).unwrap();
    let mut used_ring = VirtioSplitUsedRing::new(8, 7).unwrap();
    let writeback = used_ring
        .complete_console_receive(&decoded, &completion)
        .unwrap();

    assert_eq!(completion.bytes(), b"abcdef");
    assert_eq!(completion.used_length(), 6);
    assert!(device.pending_host_input().is_empty());
    assert_eq!(writeback.data_writes().len(), 2);
    assert_eq!(writeback.data_writes()[0].descriptor(), 2);
    assert_eq!(writeback.data_writes()[0].bytes(), b"abc");
    assert_eq!(writeback.data_writes()[1].descriptor(), 3);
    assert_eq!(writeback.data_writes()[1].bytes(), b"def");
    assert_eq!(writeback.used_slot(), 7);
    assert_eq!(writeback.used_index(), 8);
    assert_eq!(writeback.used_element(), VirtioSplitUsedElement::new(2, 6));
}

#[test]
fn virtio_console_rejects_gem5_panic_paths_as_typed_errors() {
    assert!(matches!(
        VirtioConsoleConfig::new(0, 24),
        Err(VirtioError::InvalidConsoleSize { cols: 0, rows: 24 })
    ));

    let receive_readable = VirtioSplitDescriptorChain::new(
        0,
        [VirtioSplitDescriptor::device_readable(
            0,
            b"x".to_vec(),
            None,
        )],
    )
    .unwrap();
    assert!(matches!(
        receive_readable.decode_console_receive_request(queue(0)),
        Err(VirtioError::InvalidVirtioConsoleReceiveDescriptor { index: 0 })
    ));

    let receive_empty =
        VirtioSplitDescriptorChain::new(0, [VirtioSplitDescriptor::device_writable(0, 0, None)])
            .unwrap();
    assert!(matches!(
        receive_empty.decode_console_receive_request(queue(0)),
        Err(VirtioError::MissingVirtioConsoleReceiveDescriptor)
    ));

    let transmit_writable =
        VirtioSplitDescriptorChain::new(4, [VirtioSplitDescriptor::device_writable(4, 2, None)])
            .unwrap();
    assert!(matches!(
        transmit_writable.decode_console_transmit_request(queue(1)),
        Err(VirtioError::InvalidVirtioConsoleTransmitDescriptor { index: 4 })
    ));
}

#[test]
fn virtio_split_queue_completes_console_receive_to_guest_memory_and_raises_isr() {
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
        &descriptor(0x13fd, 3, VIRTIO_SPLIT_DESC_F_WRITE, 0),
        2,
    );
    write_guest(&mut store, 0x1102, &1_u16.to_le_bytes(), 3);
    write_guest(&mut store, 0x1104, &0_u16.to_le_bytes(), 4);
    write_guest(&mut store, 0x1802, &9_u16.to_le_bytes(), 5);

    let device = VirtioConsoleDevice::new();
    device.push_host_input(b"abcde".to_vec());
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
            .consume_available_console_receive(&mut guest, queue(0))
            .unwrap()
            .unwrap();
        let completion = device.receive_at(88, decoded.request().clone()).unwrap();
        let writeback = split_queue
            .complete_console_receive_and_raise_isr(&mut guest, &decoded, &completion, &isr)
            .unwrap();

        assert_eq!(writeback.used_slot(), 1);
        assert_eq!(writeback.used_index(), 10);
        assert_eq!(writeback.used_element(), VirtioSplitUsedElement::new(0, 5));
    }

    assert_eq!(read_guest(&mut store, 0x1300, 3, 100), b"abc");
    assert_eq!(read_guest(&mut store, 0x13fd, 2, 200), b"de");
    assert_eq!(
        read_guest(&mut store, 0x180c, 8, 300),
        VirtioSplitUsedElement::new(0, 5).to_le_bytes()
    );
    assert_eq!(read_guest(&mut store, 0x1802, 2, 400), 10_u16.to_le_bytes());

    let events = isr.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind(), VirtioPciIsrEventKind::QueueInterrupt);
}

#[test]
fn virtio_split_queue_completes_console_transmit_from_guest_memory_and_raises_isr() {
    let mut store = guest_store();
    write_guest(&mut store, 0x1400, b"he", 1);
    write_guest(&mut store, 0x143e, b"llo", 2);
    write_guest(
        &mut store,
        0x1000,
        &descriptor(0x1400, 2, VIRTIO_SPLIT_DESC_F_NEXT, 1),
        3,
    );
    write_guest(&mut store, 0x1010, &descriptor(0x143e, 3, 0, 0), 4);
    write_guest(&mut store, 0x1102, &1_u16.to_le_bytes(), 5);
    write_guest(&mut store, 0x1104, &0_u16.to_le_bytes(), 6);
    write_guest(&mut store, 0x1802, &2_u16.to_le_bytes(), 7);

    let device = VirtioConsoleDevice::new();
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
            .consume_available_console_transmit(&mut guest, queue(1))
            .unwrap()
            .unwrap();
        let completion = device.transmit_at(99, decoded.request().clone()).unwrap();
        let writeback = split_queue
            .complete_console_transmit_and_raise_isr(&mut guest, &decoded, &completion, &isr)
            .unwrap();

        assert!(writeback.data_writes().is_empty());
        assert_eq!(writeback.used_slot(), 2);
        assert_eq!(writeback.used_index(), 3);
        assert_eq!(writeback.used_element(), VirtioSplitUsedElement::new(0, 0));
    }

    assert_eq!(device.guest_output(), b"hello");
    assert_eq!(
        read_guest(&mut store, 0x1814, 8, 300),
        VirtioSplitUsedElement::new(0, 0).to_le_bytes()
    );
    assert_eq!(read_guest(&mut store, 0x1802, 2, 400), 3_u16.to_le_bytes());

    let events = isr.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind(), VirtioPciIsrEventKind::QueueInterrupt);
}

#[test]
fn virtio_split_queue_keeps_console_available_cursor_on_decode_error() {
    let mut store = guest_store();
    write_guest(&mut store, 0x1400, b"x", 1);
    write_guest(&mut store, 0x1000, &descriptor(0x1400, 1, 0, 0), 2);
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
    let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(12));
    let error = split_queue
        .consume_available_console_receive(&mut guest, queue(0))
        .unwrap_err();

    assert!(matches!(
        error,
        VirtioError::InvalidVirtioConsoleReceiveDescriptor { index: 0 }
    ));
    assert_eq!(split_queue.last_available_index(), 0);
}

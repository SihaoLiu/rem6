use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptEvent, InterruptEventKind, InterruptLineId, InterruptRoute,
    InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId, PartitionedMemoryStore,
};
use rem6_pci::{
    PciBarIndex, PciClassCode, PciConfigOffset, PciDeviceIdentity, PciEndpointConfig,
    PciFunctionAddress, PciInterruptPin, PciLegacyInterruptMapper, PciLegacyInterruptPolicy,
    PciLegacyInterruptPort, PciMsiCapabilitySpec, PciMsiPort, PciMsiRoute, PciMsixCapabilitySpec,
    PciMsixPort, PciMsixRoute,
};
use rem6_virtio::{
    VirtioBlockConfigSpec, VirtioBlockDecodedRequest, VirtioBlockDevice,
    VirtioBlockIntxCompletionTarget, VirtioBlockMemoryBackend, VirtioBlockMsiCompletionTarget,
    VirtioBlockMsixCompletionTarget, VirtioBlockRequestId, VirtioBlockRequestKind,
    VirtioGuestMemory, VirtioPciIsrDevice, VirtioPciIsrEvent, VirtioPciIsrEventKind,
    VirtioPciIsrStatus, VirtioQueueIndex, VirtioSplitDescriptor, VirtioSplitDescriptorChain,
    VirtioSplitQueue, VirtioSplitUsedElement, VirtioSplitUsedRing, VIRTIO_BLOCK_SECTOR_SIZE,
    VIRTIO_BLOCK_S_OK, VIRTIO_BLOCK_T_FLUSH, VIRTIO_BLOCK_T_GET_ID, VIRTIO_BLOCK_T_IN,
    VIRTIO_BLOCK_T_OUT, VIRTIO_SPLIT_AVAIL_F_NO_INTERRUPT, VIRTIO_SPLIT_DESC_F_NEXT,
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

fn write_guest_read_queue(store: &mut PartitionedMemoryStore, sector: u64, sequence: u64) {
    write_guest(
        store,
        0x1000,
        &descriptor(0x1200, 16, VIRTIO_SPLIT_DESC_F_NEXT, 1),
        sequence,
    );
    write_guest(
        store,
        0x1010,
        &descriptor(
            0x1300,
            VIRTIO_BLOCK_SECTOR_SIZE as u32,
            VIRTIO_SPLIT_DESC_F_NEXT | VIRTIO_SPLIT_DESC_F_WRITE,
            2,
        ),
        sequence + 1,
    );
    write_guest(
        store,
        0x1020,
        &descriptor(0x1500, 1, VIRTIO_SPLIT_DESC_F_WRITE, 0),
        sequence + 2,
    );
    write_guest(store, 0x1102, &1_u16.to_le_bytes(), sequence + 3);
    write_guest(store, 0x1104, &0_u16.to_le_bytes(), sequence + 4);
    write_guest(
        store,
        0x1200,
        &header(VIRTIO_BLOCK_T_IN, sector),
        sequence + 5,
    );
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

fn function(device: u8) -> PciFunctionAddress {
    PciFunctionAddress::new(0, device, 0).unwrap()
}

fn intx_port(
    target_partition: PartitionId,
    signal_latency: u64,
) -> (Arc<Mutex<InterruptController>>, PciLegacyInterruptPort) {
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let route = PciLegacyInterruptMapper::new(
        InterruptLineId::new(40),
        4,
        PciLegacyInterruptPolicy::DevicePinModulo,
    )
    .unwrap()
    .route(
        function(3),
        PciInterruptPin::IntA,
        InterruptTargetId::new(0),
        target_partition,
        signal_latency,
    )
    .unwrap();
    controller
        .lock()
        .unwrap()
        .register_route(route.interrupt_route())
        .unwrap();
    let port = PciLegacyInterruptPort::new(route, Arc::clone(&controller)).unwrap();
    (controller, port)
}

fn virtio_block_endpoint() -> PciEndpointConfig {
    PciEndpointConfig::new(
        function(3),
        PciDeviceIdentity::new(0x1af4, 0x1001),
        PciClassCode::new(0x01, 0x00, 0x00, 0x00),
    )
}

fn program_enabled_msi(endpoint: &mut PciEndpointConfig) {
    endpoint
        .install_msi_capability(
            PciMsiCapabilitySpec::new(PciConfigOffset::new(0x50).unwrap(), 4, true, true).unwrap(),
        )
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0x52).unwrap(),
            &0x0021_u16.to_le_bytes(),
        )
        .unwrap();
    endpoint
        .write_u32(PciConfigOffset::new(0x54).unwrap(), 0xfee0_0123)
        .unwrap();
    endpoint
        .write_u32(PciConfigOffset::new(0x58).unwrap(), 0x0000_0001)
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0x5c).unwrap(),
            &0x0040_u16.to_le_bytes(),
        )
        .unwrap();
}

fn msi_port(
    target_partition: PartitionId,
    signal_latency: u64,
    vector: u8,
    line: InterruptLineId,
) -> (
    Arc<Mutex<InterruptController>>,
    PciEndpointConfig,
    PciMsiPort,
) {
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let mut endpoint = virtio_block_endpoint();
    program_enabled_msi(&mut endpoint);
    let interrupt_route = InterruptRoute::new(line, InterruptTargetId::new(0), target_partition);
    controller
        .lock()
        .unwrap()
        .register_route(interrupt_route)
        .unwrap();
    let route = PciMsiRoute::new(
        endpoint.function(),
        vector,
        endpoint.msi_message(vector).unwrap().unwrap(),
        interrupt_route,
        signal_latency,
    )
    .unwrap();
    let port = PciMsiPort::new(route, Arc::clone(&controller)).unwrap();
    (controller, endpoint, port)
}

fn program_enabled_msix(endpoint: &mut PciEndpointConfig, vector: u16, data: u16) {
    endpoint
        .install_msix_capability(
            PciMsixCapabilitySpec::new(
                PciConfigOffset::new(0x70).unwrap(),
                4,
                PciBarIndex::new(2).unwrap(),
                Address::new(0x100),
                PciBarIndex::new(2).unwrap(),
                Address::new(0x180),
            )
            .unwrap(),
        )
        .unwrap();
    let offset = 0x100 + u64::from(vector) * 16;
    endpoint
        .write_msix_region(Address::new(offset), &0xfee0_0123_u32.to_le_bytes())
        .unwrap();
    endpoint
        .write_msix_region(Address::new(offset + 4), &0x0000_0001_u32.to_le_bytes())
        .unwrap();
    endpoint
        .write_msix_region(Address::new(offset + 8), &u32::from(data).to_le_bytes())
        .unwrap();
    endpoint
        .write_msix_region(Address::new(offset + 12), &0_u32.to_le_bytes())
        .unwrap();
    endpoint
        .write_config(
            PciConfigOffset::new(0x72).unwrap(),
            &0x8000_u16.to_le_bytes(),
        )
        .unwrap();
}

fn msix_port(
    target_partition: PartitionId,
    signal_latency: u64,
    vector: u16,
    line: InterruptLineId,
) -> (
    Arc<Mutex<InterruptController>>,
    PciEndpointConfig,
    PciMsixPort,
) {
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let mut endpoint = virtio_block_endpoint();
    program_enabled_msix(&mut endpoint, vector, 0x0080 + vector);
    let interrupt_route = InterruptRoute::new(line, InterruptTargetId::new(0), target_partition);
    controller
        .lock()
        .unwrap()
        .register_route(interrupt_route)
        .unwrap();
    let route = PciMsixRoute::new(
        endpoint.function(),
        vector,
        endpoint.msix_message(vector).unwrap().unwrap(),
        interrupt_route,
        signal_latency,
    )
    .unwrap();
    let port = PciMsixPort::new(route, Arc::clone(&controller)).unwrap();
    (controller, endpoint, port)
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
fn virtio_split_queue_suppresses_isr_when_available_ring_requests_no_interrupt() {
    let mut store = guest_store();
    write_guest_read_queue(&mut store, 0, 1);
    write_guest(
        &mut store,
        0x1100,
        &VIRTIO_SPLIT_AVAIL_F_NO_INTERRUPT.to_le_bytes(),
        7,
    );

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
        let completion = device.execute_at(57, decoded.request().clone()).unwrap();
        let writeback = split_queue
            .complete_block_request_and_raise_isr(&mut guest, &decoded, &completion, &isr)
            .unwrap();

        assert_eq!(
            writeback.used_element(),
            VirtioSplitUsedElement::new(0, 529)
        );
    }

    assert_eq!(read_guest(&mut store, 0x1300, 512, 100), sector(0x44));
    assert_eq!(
        read_guest(&mut store, 0x1500, 1, 200),
        vec![VIRTIO_BLOCK_S_OK]
    );
    assert_eq!(
        read_guest(&mut store, 0x1804, 8, 300),
        VirtioSplitUsedElement::new(0, 529).to_le_bytes()
    );
    assert_eq!(isr.status(), VirtioPciIsrStatus::empty());
}

#[test]
fn virtio_split_queue_uses_event_index_for_isr_suppression() {
    let mut store = guest_store();
    write_guest_read_queue(&mut store, 0, 1);
    write_guest(&mut store, 0x110c, &1_u16.to_le_bytes(), 7);

    let backend = VirtioBlockMemoryBackend::from_bytes(sector(0x64)).unwrap();
    let device = VirtioBlockDevice::new(VirtioBlockConfigSpec::new(1), backend).unwrap();
    let isr = VirtioPciIsrDevice::new();
    let mut split_queue = VirtioSplitQueue::new(
        4,
        Address::new(0x1000),
        Address::new(0x1100),
        Address::new(0x1800),
        0,
    )
    .unwrap()
    .with_event_index(true);
    {
        let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(9));
        let decoded = split_queue
            .consume_available_block(&mut guest, queue(2))
            .unwrap()
            .unwrap();
        let completion = device.execute_at(58, decoded.request().clone()).unwrap();
        let writeback = split_queue
            .complete_block_request_and_raise_isr(&mut guest, &decoded, &completion, &isr)
            .unwrap();

        assert_eq!(
            writeback.used_element(),
            VirtioSplitUsedElement::new(0, 529)
        );
    }

    assert_eq!(read_guest(&mut store, 0x1300, 512, 100), sector(0x64));
    assert_eq!(
        read_guest(&mut store, 0x1500, 1, 200),
        vec![VIRTIO_BLOCK_S_OK]
    );
    assert_eq!(
        read_guest(&mut store, 0x1804, 8, 300),
        VirtioSplitUsedElement::new(0, 529).to_le_bytes()
    );
    assert_eq!(isr.status(), VirtioPciIsrStatus::empty());
}

#[test]
fn virtio_split_queue_posts_serial_intx_after_completion_writeback() {
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let source = InterruptSourceId::new(90);
    let (controller, port) = intx_port(cpu, 5);
    let line = port.line();
    let store = Arc::new(Mutex::new(guest_store()));
    {
        let mut store = store.lock().unwrap();
        write_guest_read_queue(&mut store, 0, 1);
    }

    let backend = VirtioBlockMemoryBackend::from_bytes(sector(0x33)).unwrap();
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
    let event_store = Arc::clone(&store);
    let event_isr = isr.clone();
    let event_port = port.clone();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();
    scheduler
        .schedule_at(pci, 77, move |context| {
            let mut store = event_store.lock().unwrap();
            let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(9));
            let decoded = split_queue
                .consume_available_block(&mut guest, queue(2))
                .unwrap()
                .unwrap();
            let completion = device.execute(context, decoded.request().clone()).unwrap();
            split_queue
                .complete_block_request_and_post_intx(
                    context,
                    &mut guest,
                    &decoded,
                    &completion,
                    VirtioBlockIntxCompletionTarget::new(&event_isr, &event_port, source),
                )
                .unwrap();
        })
        .unwrap();
    scheduler.run_until_idle();

    assert_eq!(
        read_guest(&mut store.lock().unwrap(), 0x1300, 512, 100),
        sector(0x33)
    );
    assert_eq!(isr.status(), VirtioPciIsrStatus::queue_interrupt());
    assert_eq!(
        controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            82,
            line,
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn virtio_split_queue_posts_parallel_intx_after_completion_writeback() {
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let source = InterruptSourceId::new(91);
    let (controller, port) = intx_port(cpu, 6);
    assert_eq!(
        port.interrupt_route(),
        InterruptRoute::new(port.line(), InterruptTargetId::new(0), cpu)
    );
    let store = Arc::new(Mutex::new(guest_store()));
    {
        let mut store = store.lock().unwrap();
        write_guest_read_queue(&mut store, 0, 11);
    }

    let backend = VirtioBlockMemoryBackend::from_bytes(sector(0x22)).unwrap();
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
    let event_store = Arc::clone(&store);
    let event_isr = isr.clone();
    let event_port = port.clone();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    scheduler
        .schedule_parallel_at(pci, 81, move |context| {
            let mut store = event_store.lock().unwrap();
            let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(9));
            let decoded = split_queue
                .consume_available_block(&mut guest, queue(2))
                .unwrap()
                .unwrap();
            let completion = device
                .execute_parallel(context, decoded.request().clone())
                .unwrap();
            split_queue
                .complete_block_request_and_post_intx_parallel(
                    context,
                    &mut guest,
                    &decoded,
                    &completion,
                    VirtioBlockIntxCompletionTarget::new(&event_isr, &event_port, source),
                )
                .unwrap();
        })
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        read_guest(&mut store.lock().unwrap(), 0x1300, 512, 200),
        sector(0x22)
    );
    assert_eq!(isr.status(), VirtioPciIsrStatus::queue_interrupt());
    assert_eq!(
        controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            87,
            port.line(),
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn virtio_split_queue_suppresses_parallel_msi_when_available_ring_requests_no_interrupt() {
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let source = InterruptSourceId::new(97);
    let (controller, endpoint, port) = msi_port(cpu, 5, 2, InterruptLineId::new(55));
    let store = Arc::new(Mutex::new(guest_store()));
    {
        let mut store = store.lock().unwrap();
        write_guest_read_queue(&mut store, 0, 71);
        write_guest(
            &mut store,
            0x1100,
            &VIRTIO_SPLIT_AVAIL_F_NO_INTERRUPT.to_le_bytes(),
            77,
        );
    }

    let backend = VirtioBlockMemoryBackend::from_bytes(sector(0xbb)).unwrap();
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
    let event_store = Arc::clone(&store);
    let event_isr = isr.clone();
    let event_port = port.clone();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    scheduler
        .schedule_parallel_at(pci, 93, move |context| {
            let mut store = event_store.lock().unwrap();
            let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(9));
            let decoded = split_queue
                .consume_available_block(&mut guest, queue(2))
                .unwrap()
                .unwrap();
            let completion = device
                .execute_parallel(context, decoded.request().clone())
                .unwrap();
            let outcome = split_queue
                .complete_block_request_and_post_msi_parallel(
                    context,
                    &mut guest,
                    &decoded,
                    &completion,
                    VirtioBlockMsiCompletionTarget::new(&event_isr, &endpoint, &event_port, source),
                )
                .unwrap();

            assert_eq!(outcome.interrupt_delivery(), None);
        })
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        read_guest(&mut store.lock().unwrap(), 0x1300, 512, 600),
        sector(0xbb)
    );
    assert_eq!(isr.status(), VirtioPciIsrStatus::empty());
    assert!(controller.lock().unwrap().history().is_empty());
}

#[test]
fn virtio_split_queue_posts_serial_msi_after_completion_writeback() {
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let source = InterruptSourceId::new(92);
    let (controller, endpoint, port) = msi_port(cpu, 7, 0, InterruptLineId::new(50));
    let line = port.route().interrupt_route().line();
    let store = Arc::new(Mutex::new(guest_store()));
    {
        let mut store = store.lock().unwrap();
        write_guest_read_queue(&mut store, 0, 21);
    }

    let backend = VirtioBlockMemoryBackend::from_bytes(sector(0x66)).unwrap();
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
    let event_store = Arc::clone(&store);
    let event_isr = isr.clone();
    let event_port = port.clone();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();
    scheduler
        .schedule_at(pci, 79, move |context| {
            let mut store = event_store.lock().unwrap();
            let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(9));
            let decoded = split_queue
                .consume_available_block(&mut guest, queue(2))
                .unwrap()
                .unwrap();
            let completion = device.execute(context, decoded.request().clone()).unwrap();
            split_queue
                .complete_block_request_and_post_msi(
                    context,
                    &mut guest,
                    &decoded,
                    &completion,
                    VirtioBlockMsiCompletionTarget::new(&event_isr, &endpoint, &event_port, source),
                )
                .unwrap();
        })
        .unwrap();
    scheduler.run_until_idle();

    assert_eq!(
        read_guest(&mut store.lock().unwrap(), 0x1300, 512, 300),
        sector(0x66)
    );
    assert_eq!(isr.status(), VirtioPciIsrStatus::queue_interrupt());
    assert_eq!(
        controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            86,
            line,
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn virtio_split_queue_posts_parallel_msi_after_completion_writeback() {
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let source = InterruptSourceId::new(93);
    let (controller, endpoint, port) = msi_port(cpu, 5, 1, InterruptLineId::new(51));
    let line = port.route().interrupt_route().line();
    let store = Arc::new(Mutex::new(guest_store()));
    {
        let mut store = store.lock().unwrap();
        write_guest_read_queue(&mut store, 0, 31);
    }

    let backend = VirtioBlockMemoryBackend::from_bytes(sector(0x77)).unwrap();
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
    let event_store = Arc::clone(&store);
    let event_isr = isr.clone();
    let event_port = port.clone();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    scheduler
        .schedule_parallel_at(pci, 83, move |context| {
            let mut store = event_store.lock().unwrap();
            let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(9));
            let decoded = split_queue
                .consume_available_block(&mut guest, queue(2))
                .unwrap()
                .unwrap();
            let completion = device
                .execute_parallel(context, decoded.request().clone())
                .unwrap();
            split_queue
                .complete_block_request_and_post_msi_parallel(
                    context,
                    &mut guest,
                    &decoded,
                    &completion,
                    VirtioBlockMsiCompletionTarget::new(&event_isr, &endpoint, &event_port, source),
                )
                .unwrap();
        })
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        read_guest(&mut store.lock().unwrap(), 0x1300, 512, 400),
        sector(0x77)
    );
    assert_eq!(isr.status(), VirtioPciIsrStatus::queue_interrupt());
    assert_eq!(
        controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            88,
            line,
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn virtio_split_queue_posts_serial_msix_after_completion_writeback() {
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let source = InterruptSourceId::new(94);
    let (controller, mut endpoint, port) = msix_port(cpu, 8, 2, InterruptLineId::new(52));
    let line = port.route().interrupt_route().line();
    let store = Arc::new(Mutex::new(guest_store()));
    {
        let mut store = store.lock().unwrap();
        write_guest_read_queue(&mut store, 0, 41);
    }

    let backend = VirtioBlockMemoryBackend::from_bytes(sector(0x88)).unwrap();
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
    let event_store = Arc::clone(&store);
    let event_isr = isr.clone();
    let event_port = port.clone();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();
    scheduler
        .schedule_at(pci, 85, move |context| {
            let mut store = event_store.lock().unwrap();
            let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(9));
            let decoded = split_queue
                .consume_available_block(&mut guest, queue(2))
                .unwrap()
                .unwrap();
            let completion = device.execute(context, decoded.request().clone()).unwrap();
            split_queue
                .complete_block_request_and_post_msix(
                    context,
                    &mut guest,
                    &decoded,
                    &completion,
                    VirtioBlockMsixCompletionTarget::new(
                        &event_isr,
                        &mut endpoint,
                        &event_port,
                        source,
                    ),
                )
                .unwrap();
        })
        .unwrap();
    scheduler.run_until_idle();

    assert_eq!(
        read_guest(&mut store.lock().unwrap(), 0x1300, 512, 500),
        sector(0x88)
    );
    assert_eq!(isr.status(), VirtioPciIsrStatus::queue_interrupt());
    assert_eq!(
        controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            93,
            line,
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn virtio_split_queue_posts_parallel_msix_after_completion_writeback() {
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let source = InterruptSourceId::new(95);
    let (controller, mut endpoint, port) = msix_port(cpu, 9, 3, InterruptLineId::new(53));
    let line = port.route().interrupt_route().line();
    let store = Arc::new(Mutex::new(guest_store()));
    {
        let mut store = store.lock().unwrap();
        write_guest_read_queue(&mut store, 0, 51);
    }

    let backend = VirtioBlockMemoryBackend::from_bytes(sector(0x99)).unwrap();
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
    let event_store = Arc::clone(&store);
    let event_isr = isr.clone();
    let event_port = port.clone();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    scheduler
        .schedule_parallel_at(pci, 89, move |context| {
            let mut store = event_store.lock().unwrap();
            let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(9));
            let decoded = split_queue
                .consume_available_block(&mut guest, queue(2))
                .unwrap()
                .unwrap();
            let completion = device
                .execute_parallel(context, decoded.request().clone())
                .unwrap();
            split_queue
                .complete_block_request_and_post_msix_parallel(
                    context,
                    &mut guest,
                    &decoded,
                    &completion,
                    VirtioBlockMsixCompletionTarget::new(
                        &event_isr,
                        &mut endpoint,
                        &event_port,
                        source,
                    ),
                )
                .unwrap();
        })
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        read_guest(&mut store.lock().unwrap(), 0x1300, 512, 600),
        sector(0x99)
    );
    assert_eq!(isr.status(), VirtioPciIsrStatus::queue_interrupt());
    assert_eq!(
        controller.lock().unwrap().history(),
        &[InterruptEvent::routed(
            98,
            line,
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn virtio_split_queue_records_masked_parallel_msix_as_pending_without_losing_writeback() {
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let source = InterruptSourceId::new(96);
    let (controller, mut endpoint, port) = msix_port(cpu, 4, 1, InterruptLineId::new(54));
    endpoint
        .write_msix_region(Address::new(0x11c), &1_u32.to_le_bytes())
        .unwrap();
    let store = Arc::new(Mutex::new(guest_store()));
    {
        let mut store = store.lock().unwrap();
        write_guest_read_queue(&mut store, 0, 61);
    }

    let backend = VirtioBlockMemoryBackend::from_bytes(sector(0xaa)).unwrap();
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
    let event_store = Arc::clone(&store);
    let event_isr = isr.clone();
    let event_port = port.clone();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    scheduler
        .schedule_parallel_at(pci, 91, move |context| {
            let mut store = event_store.lock().unwrap();
            let mut guest = VirtioGuestMemory::new(&mut store, layout(), AgentId::new(9));
            let decoded = split_queue
                .consume_available_block(&mut guest, queue(2))
                .unwrap()
                .unwrap();
            let completion = device
                .execute_parallel(context, decoded.request().clone())
                .unwrap();
            let outcome = split_queue
                .complete_block_request_and_post_msix_parallel(
                    context,
                    &mut guest,
                    &decoded,
                    &completion,
                    VirtioBlockMsixCompletionTarget::new(
                        &event_isr,
                        &mut endpoint,
                        &event_port,
                        source,
                    ),
                )
                .unwrap();
            assert_eq!(outcome.writeback().used_index(), 1);
            assert!(outcome.interrupt_delivery().is_none());
            assert_eq!(
                endpoint
                    .read_msix_region(Address::new(0x180), AccessSize::new(8).unwrap())
                    .unwrap(),
                0b10_u64.to_le_bytes().to_vec()
            );
        })
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        read_guest(&mut store.lock().unwrap(), 0x1300, 512, 700),
        sector(0xaa)
    );
    assert_eq!(isr.status(), VirtioPciIsrStatus::queue_interrupt());
    assert!(controller.lock().unwrap().history().is_empty());
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

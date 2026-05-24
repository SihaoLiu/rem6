use rem6_cache::{CacheWriteQueue, CacheWriteQueueConfig, CacheWriteQueueError};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryOperation, MemoryRequest,
    MemoryRequestId,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(11), sequence)
}

fn dirty_writeback(sequence: u64, line: u64, value: u8) -> MemoryRequest {
    MemoryRequest::writeback_dirty(
        request_id(sequence),
        Address::new(line),
        vec![value; 64],
        layout(),
    )
    .unwrap()
}

fn clean_writeback(sequence: u64, line: u64, value: u8) -> MemoryRequest {
    MemoryRequest::writeback_clean(
        request_id(sequence),
        Address::new(line),
        vec![value; 64],
        layout(),
    )
    .unwrap()
}

fn clean_evict(sequence: u64, line: u64) -> MemoryRequest {
    MemoryRequest::clean_evict(request_id(sequence), Address::new(line), layout()).unwrap()
}

fn uncacheable_write(sequence: u64, address: u64, data: Vec<u8>, mask: Vec<bool>) -> MemoryRequest {
    MemoryRequest::write(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(data.len() as u64).unwrap(),
        data,
        ByteMask::from_bits(mask).unwrap(),
        layout(),
    )
    .unwrap()
}

#[test]
fn cache_write_queue_orders_ready_entries_applies_reserve_and_restores_state() {
    let config = CacheWriteQueueConfig::new(2, 1).unwrap();
    let mut queue = CacheWriteQueue::new(config.clone());

    let dirty = queue
        .enqueue_writeback(dirty_writeback(0, 0x1000, 0xaa), false, 20)
        .unwrap();
    assert_eq!(dirty.handle().index(), 0);
    assert_eq!(queue.allocated_count(), 1);
    assert_eq!(queue.next_ready_tick(), Some(20));
    assert!(!queue.is_full());

    let evict = queue
        .enqueue_writeback(clean_evict(1, 0x2000), true, 10)
        .unwrap();
    assert_eq!(queue.allocated_count(), 2);
    assert!(queue.is_full());
    assert!(!queue.is_reserve_full());
    assert_eq!(queue.ready_handles(9), Vec::new());
    assert_eq!(queue.ready_handles(10), vec![evict.handle()]);
    assert_eq!(
        queue.ready_handles(20),
        vec![evict.handle(), dirty.handle()]
    );

    assert_eq!(
        queue.enqueue_writeback(clean_writeback(2, 0x3000, 0xbb), false, 5),
        Err(CacheWriteQueueError::EntrySlotsFull {
            entries: 2,
            reserve: 1
        })
    );

    let reserve = queue
        .enqueue_reserved_writeback(clean_writeback(2, 0x3000, 0xbb), false, 5)
        .unwrap();
    assert_eq!(queue.allocated_count(), 3);
    assert!(queue.is_reserve_full());
    assert_eq!(
        queue.enqueue_reserved_writeback(dirty_writeback(3, 0x4000, 0xcc), false, 6),
        Err(CacheWriteQueueError::ReserveSlotsFull { total_entries: 3 })
    );

    let snapshot = queue.snapshot();
    queue.delay_until(evict.handle(), 30).unwrap();
    assert_eq!(
        queue.ready_handles(20),
        vec![reserve.handle(), dirty.handle()]
    );

    queue.restore(&snapshot).unwrap();
    assert_eq!(queue.snapshot(), snapshot);
    assert_eq!(
        queue.ready_handles(20),
        vec![reserve.handle(), evict.handle(), dirty.handle()]
    );

    let issued = queue.issue_next(20).unwrap().unwrap();
    assert_eq!(issued.handle(), reserve.handle());
    assert_eq!(
        issued.request().operation(),
        MemoryOperation::WritebackClean
    );
    assert_eq!(queue.allocated_count(), 2);
    assert!(!queue.is_reserve_full());
}

#[test]
fn cache_write_queue_matches_conflicts_and_satisfies_functional_reads() {
    let mut queue = CacheWriteQueue::new(CacheWriteQueueConfig::new(4, 0).unwrap());
    let dirty = MemoryRequest::writeback_dirty(
        request_id(0),
        Address::new(0x1000),
        (0_u8..64).collect(),
        layout(),
    )
    .unwrap();
    let dirty = queue.enqueue_writeback(dirty, false, 10).unwrap();

    assert_eq!(
        queue.find_match(Address::new(0x1000), false, true),
        Some(dirty.handle())
    );
    assert_eq!(
        queue.pending_conflict(Address::new(0x1000), false),
        Some(dirty.handle())
    );
    assert_eq!(
        queue
            .satisfy_read(Address::new(0x1010), AccessSize::new(4).unwrap(), false)
            .unwrap(),
        Some(vec![16, 17, 18, 19])
    );
    assert_eq!(
        queue
            .satisfy_read(Address::new(0x1010), AccessSize::new(4).unwrap(), true)
            .unwrap(),
        None
    );

    let clean = queue
        .enqueue_writeback(clean_evict(1, 0x3000), false, 11)
        .unwrap();
    assert_eq!(
        queue
            .satisfy_read(Address::new(0x3000), AccessSize::new(4).unwrap(), false)
            .unwrap(),
        None
    );
    assert_eq!(
        queue.pending_conflict(Address::new(0x3000), false),
        Some(clean.handle())
    );

    let uncached = queue
        .enqueue_uncacheable_write(
            uncacheable_write(
                2,
                0x2020,
                vec![0xde, 0xad, 0xbe, 0xef],
                vec![true, false, true, true],
            ),
            false,
            12,
        )
        .unwrap();
    assert_eq!(queue.find_match(Address::new(0x2000), false, true), None);
    assert_eq!(
        queue.find_match(Address::new(0x2000), false, false),
        Some(uncached.handle())
    );
    assert_eq!(
        queue
            .satisfy_read(Address::new(0x2020), AccessSize::new(1).unwrap(), false)
            .unwrap(),
        Some(vec![0xde])
    );
    assert_eq!(
        queue
            .satisfy_read(Address::new(0x2021), AccessSize::new(1).unwrap(), false)
            .unwrap(),
        None
    );
}

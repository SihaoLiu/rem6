use rem6_cache::{
    CacheCleanReplacementPolicy, CacheReplacementPolicyConfig, CacheReplacementPolicyKind,
    CacheReplacementVictim, CacheWriteQueue, CacheWriteQueueConfig, CacheWriteQueueEntryKind,
    CacheWriteQueueError, ReplacementSet,
};
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

fn replacement_victim_way() -> usize {
    let mut replacement = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Lru, 2).unwrap(),
    );
    replacement.reset(0).unwrap();
    replacement.reset(1).unwrap();
    replacement.touch(1).unwrap();
    replacement.victim([0, 1]).unwrap().way()
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
    queue
        .enqueue_uncacheable_write(
            uncacheable_write(3, 0x1012, vec![0xde, 0xad], vec![true, true]),
            false,
            13,
        )
        .unwrap();
    assert_eq!(
        queue
            .satisfy_read(Address::new(0x1010), AccessSize::new(4).unwrap(), false)
            .unwrap(),
        Some(vec![16, 17, 0xde, 0xad])
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

#[test]
fn cache_write_queue_enqueues_replacement_victims_from_replacement_decisions() {
    let mut replacement = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Lru, 2).unwrap(),
    );
    replacement.reset(0).unwrap();
    replacement.reset(1).unwrap();
    replacement.touch(1).unwrap();
    let decision = replacement.victim([0, 1]).unwrap();
    assert_eq!(decision.way(), 0);

    let mut queue = CacheWriteQueue::new(CacheWriteQueueConfig::new(4, 0).unwrap());
    let dirty = CacheReplacementVictim::dirty(
        decision.way(),
        Address::new(0x4000),
        (0_u8..64).collect(),
        layout(),
        false,
    );
    let dirty_update = queue
        .enqueue_replacement_writeback(
            &decision,
            dirty,
            request_id(20),
            30,
            CacheCleanReplacementPolicy::CleanEvict,
        )
        .unwrap()
        .unwrap();
    assert_eq!(dirty_update.line(), Address::new(0x4000));

    let clean = CacheReplacementVictim::clean(
        decision.way(),
        Address::new(0x5000),
        vec![0x5a; 64],
        layout(),
        true,
    );
    let clean_update = queue
        .enqueue_replacement_writeback(
            &decision,
            clean,
            request_id(21),
            10,
            CacheCleanReplacementPolicy::CleanEvict,
        )
        .unwrap()
        .unwrap();
    assert_eq!(queue.allocated_count(), 2);

    let invalid =
        CacheReplacementVictim::invalid(decision.way(), Address::new(0x6000), layout(), false);
    assert_eq!(
        queue
            .enqueue_replacement_writeback(
                &decision,
                invalid,
                request_id(22),
                5,
                CacheCleanReplacementPolicy::WritebackClean,
            )
            .unwrap(),
        None
    );
    assert_eq!(queue.allocated_count(), 2);

    let clean_issue = queue.issue_next(30).unwrap().unwrap();
    assert_eq!(clean_issue.handle(), clean_update.handle());
    assert_eq!(clean_issue.kind(), CacheWriteQueueEntryKind::CleanEvict);
    assert_eq!(
        clean_issue.request().operation(),
        MemoryOperation::CleanEvict
    );
    assert!(clean_issue.secure());

    let dirty_issue = queue.issue_next(30).unwrap().unwrap();
    assert_eq!(dirty_issue.handle(), dirty_update.handle());
    assert_eq!(dirty_issue.kind(), CacheWriteQueueEntryKind::WritebackDirty);
    assert_eq!(
        dirty_issue.request().data().unwrap().get(4..8),
        Some(&[4, 5, 6, 7][..])
    );
}

#[test]
fn cache_write_queue_replacement_writeback_policy_and_way_mismatch_are_explicit() {
    let mut replacement = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Fifo, 2).unwrap(),
    );
    replacement.reset(0).unwrap();
    replacement.reset(1).unwrap();
    let decision = replacement.victim([0, 1]).unwrap();
    let mut queue = CacheWriteQueue::new(CacheWriteQueueConfig::new(2, 0).unwrap());

    let wrong_way =
        CacheReplacementVictim::dirty(1, Address::new(0x7000), vec![0x77; 64], layout(), false);
    assert_eq!(
        queue.enqueue_replacement_writeback(
            &decision,
            wrong_way,
            request_id(30),
            1,
            CacheCleanReplacementPolicy::CleanEvict,
        ),
        Err(CacheWriteQueueError::ReplacementVictimWayMismatch {
            decision_way: decision.way(),
            victim_way: 1,
        })
    );

    let clean = CacheReplacementVictim::clean(
        decision.way(),
        Address::new(0x8000),
        vec![0x88; 64],
        layout(),
        false,
    );
    queue
        .enqueue_replacement_writeback(
            &decision,
            clean,
            request_id(31),
            1,
            CacheCleanReplacementPolicy::WritebackClean,
        )
        .unwrap()
        .unwrap();
    let issued = queue.issue_next(1).unwrap().unwrap();
    assert_eq!(issued.kind(), CacheWriteQueueEntryKind::WritebackClean);
    assert_eq!(issued.request().data().unwrap(), &[0x88; 64]);

    assert_eq!(replacement_victim_way(), 0);
}

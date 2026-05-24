use rem6_cache::{
    QueuedPrefetchConfig, QueuedPrefetchDemandAccess, QueuedPrefetchFullPolicy,
    QueuedPrefetchRedundantLine, QueuedPrefetchThrottle, QueuedPrefetchThrottleConfig,
    QueuedPrefetchThrottleError, QueuedPrefetcher, StridePrefetchAccess, StridePrefetcher,
    StridePrefetcherConfig,
};
use rem6_memory::{Address, AgentId};

fn access(agent: u32, pc: u64, address: u64) -> StridePrefetchAccess {
    StridePrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

#[test]
fn stride_prefetcher_trains_per_requestor_context_and_snapshots_state() {
    let config = StridePrefetcherConfig::new(64, 4, 2, 2, 0, true).unwrap();
    let mut prefetcher = StridePrefetcher::new(config.clone());

    assert_eq!(prefetcher.observe(access(1, 0x80, 0x1000)).unwrap(), &[]);
    assert_eq!(prefetcher.observe(access(1, 0x80, 0x1040)).unwrap(), &[]);

    let candidates = prefetcher.observe(access(1, 0x80, 0x1080)).unwrap();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x10c0), Address::new(0x1100)]
    );
    assert_eq!(candidates[0].stride(), 64);
    assert_eq!(candidates[0].degree_index(), 1);
    assert_eq!(prefetcher.context_count(), 1);
    assert_eq!(prefetcher.entry_count(AgentId::new(1)), 1);

    assert_eq!(prefetcher.observe(access(2, 0x80, 0x2000)).unwrap(), &[]);
    assert_eq!(prefetcher.context_count(), 2);
    assert_eq!(prefetcher.entry_count(AgentId::new(2)), 1);

    let snapshot = prefetcher.snapshot();
    let mut restored = StridePrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let restored_candidates = restored.observe(access(1, 0x80, 0x10c0)).unwrap();
    assert_eq!(
        restored_candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x1100), Address::new(0x1140)]
    );
}

#[test]
fn queued_prefetcher_delays_deduplicates_and_snapshots_candidates() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 2, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();

    let queue_config = QueuedPrefetchConfig::new(4, 3, 1, true).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config.clone());
    assert_eq!(queue.enqueue_candidates(10, &candidates).unwrap(), 2);
    assert_eq!(queue.enqueue_candidates(11, &candidates).unwrap(), 0);
    assert_eq!(queue.pending_count(), 2);
    assert!(queue.issue_ready(12).is_empty());

    let first = queue.issue_ready(13);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].address(), Address::new(0x10c0));
    assert_eq!(first[0].context(), AgentId::new(1));
    assert_eq!(first[0].pc(), 0x80);
    assert_eq!(first[0].ready_tick(), 13);
    assert_eq!(queue.pending_count(), 1);

    let snapshot = queue.snapshot();
    let mut restored = QueuedPrefetcher::new(queue_config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let second = restored.issue_ready(13);
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].address(), Address::new(0x1100));
    assert_eq!(second[0].degree_index(), 2);
    assert_eq!(restored.pending_count(), 0);
}

#[test]
fn queued_prefetcher_squashes_same_line_demand_accesses() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 2, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();

    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 2, true, 64).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config.clone());
    assert_eq!(queue.enqueue_candidates(10, &candidates).unwrap(), 2);

    let demand = QueuedPrefetchDemandAccess::new(Address::new(0x10d8), false);
    assert_eq!(queue.squash_demand_access(demand), 1);
    assert_eq!(queue.pending_count(), 1);
    assert_eq!(
        queue.snapshot().pending()[0].address(),
        Address::new(0x1100)
    );

    let snapshot = queue.snapshot();
    let mut restored = QueuedPrefetcher::new(queue_config);
    restored.restore(&snapshot).unwrap();

    let issued = restored.issue_ready(13);
    assert_eq!(issued.len(), 1);
    assert_eq!(issued[0].address(), Address::new(0x1100));
    assert_eq!(restored.pending_count(), 0);
}

#[test]
fn queued_prefetcher_filters_candidates_already_in_cache_resources() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 2, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();

    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 2, true, 64).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);
    let result = queue
        .enqueue_candidates_filtered(
            10,
            &candidates,
            &[
                QueuedPrefetchRedundantLine::in_cache(Address::new(0x10f8), false),
                QueuedPrefetchRedundantLine::in_miss_queue(Address::new(0x1108), true),
            ],
        )
        .unwrap();

    assert_eq!(result.accepted(), 1);
    assert_eq!(result.dropped_redundant(), 1);
    assert_eq!(queue.pending_count(), 1);
    assert_eq!(
        queue.snapshot().pending()[0].address(),
        Address::new(0x1100)
    );

    let issued = queue.issue_ready(13);
    assert_eq!(issued.len(), 1);
    assert_eq!(issued[0].address(), Address::new(0x1100));
}

#[test]
fn queued_prefetcher_updates_duplicate_candidate_priority() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 3, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let initial_candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();

    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);
    assert_eq!(
        queue.enqueue_candidates(10, &initial_candidates).unwrap(),
        3
    );
    let old_priority = queue
        .snapshot()
        .pending()
        .iter()
        .find(|entry| entry.address() == Address::new(0x1100))
        .unwrap()
        .priority();

    let next_candidates = stride.observe(access(1, 0x80, 0x10c0)).unwrap().to_vec();
    assert_eq!(next_candidates[0].address(), Address::new(0x1100));
    assert_eq!(next_candidates[1].address(), Address::new(0x1140));
    assert_eq!(next_candidates[2].address(), Address::new(0x1180));
    let result = queue
        .enqueue_candidates_filtered(11, &next_candidates, &[])
        .unwrap();

    assert_eq!(result.accepted(), 1);
    assert_eq!(result.duplicate_hits(), 2);
    assert_eq!(result.updated_priorities(), 2);
    assert_eq!(queue.pending_count(), 4);

    let snapshot = queue.snapshot();
    let updated = snapshot
        .pending()
        .iter()
        .find(|entry| entry.address() == Address::new(0x1100))
        .unwrap();
    assert!(updated.priority() > old_priority);
    assert!(snapshot
        .pending()
        .iter()
        .any(|entry| entry.address() == Address::new(0x1180)));
}

#[test]
fn queued_prefetcher_can_evict_oldest_lowest_priority_entry_when_full() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 2, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let initial_candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();

    let queue_config = QueuedPrefetchConfig::with_line_size(2, 3, 4, true, 64)
        .unwrap()
        .with_full_policy(QueuedPrefetchFullPolicy::EvictOldestLowestPriority);
    let mut queue = QueuedPrefetcher::new(queue_config);
    assert_eq!(
        queue.enqueue_candidates(10, &initial_candidates).unwrap(),
        2
    );

    let next_candidates = stride.observe(access(1, 0x80, 0x10c0)).unwrap().to_vec();
    assert_eq!(next_candidates[1].address(), Address::new(0x1140));
    let result = queue
        .enqueue_candidates_filtered(11, &next_candidates[1..], &[])
        .unwrap();

    assert_eq!(result.accepted(), 1);
    assert_eq!(result.evicted_full(), 1);
    assert_eq!(queue.pending_count(), 2);
    assert_eq!(
        queue
            .snapshot()
            .pending()
            .iter()
            .map(|entry| entry.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x10c0), Address::new(0x1140)]
    );
}

#[test]
fn queued_prefetcher_exposes_next_ready_tick_for_scheduler_planning() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 2, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();

    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 1, true, 64).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config.clone());
    assert_eq!(queue.next_ready_tick(), None);
    assert_eq!(queue.enqueue_candidates(10, &candidates).unwrap(), 2);
    assert_eq!(queue.next_ready_tick(), Some(13));
    assert!(queue.issue_ready(12).is_empty());
    assert_eq!(queue.next_ready_tick(), Some(13));

    assert_eq!(queue.issue_ready(13).len(), 1);
    assert_eq!(queue.next_ready_tick(), Some(13));

    let snapshot = queue.snapshot();
    let mut restored = QueuedPrefetcher::new(queue_config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.next_ready_tick(), Some(13));

    assert_eq!(restored.issue_ready(13).len(), 1);
    assert_eq!(restored.next_ready_tick(), None);
}

#[test]
fn queued_prefetcher_applies_accuracy_throttle_before_candidate_enqueue() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 5, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();
    assert_eq!(candidates.len(), 5);

    let mut throttle = QueuedPrefetchThrottle::new(QueuedPrefetchThrottleConfig::new(60).unwrap());
    throttle.record_issued(10).unwrap();
    throttle.record_useful(5).unwrap();

    let queue_config = QueuedPrefetchConfig::with_line_size(8, 3, 8, true, 64).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);
    let result = queue
        .enqueue_candidates_throttled(10, &candidates, &[], &throttle)
        .unwrap();

    assert_eq!(result.accepted(), 3);
    assert_eq!(result.dropped_throttled(), 2);
    assert_eq!(queue.pending_count(), 3);
    assert_eq!(
        queue
            .snapshot()
            .pending()
            .iter()
            .map(|entry| entry.address())
            .collect::<Vec<_>>(),
        vec![
            Address::new(0x10c0),
            Address::new(0x1100),
            Address::new(0x1140)
        ]
    );
}

#[test]
fn queued_prefetch_throttle_uses_accuracy_and_snapshots_counters() {
    let config = QueuedPrefetchThrottleConfig::new(60).unwrap();
    let mut throttle = QueuedPrefetchThrottle::new(config.clone());

    assert_eq!(throttle.max_permitted(5), 5);

    throttle.record_issued(10).unwrap();
    throttle.record_useful(5).unwrap();
    assert_eq!(throttle.max_permitted(5), 3);
    assert_eq!(throttle.issued_prefetches(), 10);
    assert_eq!(throttle.useful_prefetches(), 5);

    let snapshot = throttle.snapshot();
    let mut restored = QueuedPrefetchThrottle::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.max_permitted(5), 3);

    let mut always_keeps_one =
        QueuedPrefetchThrottle::new(QueuedPrefetchThrottleConfig::new(100).unwrap());
    assert_eq!(always_keeps_one.max_permitted(4), 4);
    always_keeps_one.record_issued(4).unwrap();
    assert_eq!(always_keeps_one.max_permitted(4), 1);
}

#[test]
fn queued_prefetch_throttle_rejects_useful_counts_above_issued() {
    let mut throttle = QueuedPrefetchThrottle::new(QueuedPrefetchThrottleConfig::new(60).unwrap());

    assert_eq!(
        throttle.record_useful(1).unwrap_err(),
        QueuedPrefetchThrottleError::UsefulExceedsIssued {
            issued_prefetches: 0,
            useful_prefetches: 1,
        }
    );
    assert_eq!(throttle.max_permitted(4), 4);

    throttle.record_issued(2).unwrap();
    throttle.record_useful(2).unwrap();
    assert_eq!(
        throttle.record_useful(1).unwrap_err(),
        QueuedPrefetchThrottleError::UsefulExceedsIssued {
            issued_prefetches: 2,
            useful_prefetches: 3,
        }
    );
    assert_eq!(throttle.useful_prefetches(), 2);
    assert_eq!(throttle.max_permitted(4), 4);
}

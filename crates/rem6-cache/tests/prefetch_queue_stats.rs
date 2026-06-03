use rem6_cache::{
    QueuedPrefetchConfig, QueuedPrefetchDemandAccess, QueuedPrefetchFullPolicy,
    QueuedPrefetchRedundantLine, QueuedPrefetcher, StridePrefetchAccess, StridePrefetcher,
    StridePrefetcherConfig, TaggedPrefetchAccess, TaggedPrefetcher, TaggedPrefetcherConfig,
};
use rem6_memory::{Address, AgentId};

fn access(agent: u32, pc: u64, address: u64) -> StridePrefetchAccess {
    StridePrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

fn tagged_access(agent: u32, pc: u64, address: u64) -> TaggedPrefetchAccess {
    TaggedPrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

#[test]
fn queued_prefetcher_records_resource_stats_in_snapshots() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 3, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();
    assert_eq!(candidates.len(), 3);

    let queue_config = QueuedPrefetchConfig::with_line_size(2, 3, 4, true, 64)
        .unwrap()
        .with_full_policy(QueuedPrefetchFullPolicy::EvictOldestLowestPriority);
    let mut queue = QueuedPrefetcher::new(queue_config.clone());

    assert_eq!(
        queue
            .enqueue_candidates_filtered(10, &candidates[..2], &[])
            .unwrap()
            .accepted(),
        2
    );
    assert_eq!(
        queue
            .enqueue_candidates_filtered(11, &candidates[..2], &[])
            .unwrap()
            .duplicate_hits(),
        2
    );
    assert_eq!(
        queue
            .enqueue_candidates_filtered(
                12,
                &candidates[2..],
                &[QueuedPrefetchRedundantLine::in_cache(
                    Address::new(0x1140),
                    false,
                )],
            )
            .unwrap()
            .dropped_redundant(),
        1
    );
    assert_eq!(
        queue
            .enqueue_candidates_filtered(
                12,
                &candidates[2..],
                &[QueuedPrefetchRedundantLine::in_miss_queue(
                    Address::new(0x1140),
                    false,
                )],
            )
            .unwrap()
            .dropped_redundant(),
        1
    );
    assert_eq!(
        queue
            .enqueue_candidates_filtered(13, &candidates[2..], &[])
            .unwrap()
            .evicted_full(),
        1
    );
    assert_eq!(
        queue.squash_demand_access(QueuedPrefetchDemandAccess::new(Address::new(0x1140), false)),
        1
    );

    let stats = queue.stats();
    assert_eq!(stats.identified_prefetches(), 7);
    assert_eq!(stats.buffer_hits(), 2);
    assert_eq!(stats.in_cache_drops(), 2);
    assert_eq!(stats.removed_by_full_queue(), 1);
    assert_eq!(stats.removed_by_demand(), 1);
    assert_eq!(stats.span_page_prefetches(), 0);
    assert_eq!(stats.useful_span_page_prefetches(), 0);

    let snapshot = queue.snapshot();
    let mut restored = QueuedPrefetcher::new(queue_config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.stats(), stats);

    let mut tagged = TaggedPrefetcher::new(TaggedPrefetcherConfig::new(64, 2).unwrap());
    let page_crossing = tagged
        .observe(tagged_access(4, 0x90, 0x0fc0))
        .unwrap()
        .to_vec();
    assert_eq!(
        page_crossing
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x1000), Address::new(0x1040)]
    );

    let page_queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64)
        .unwrap()
        .with_page_size(4096)
        .unwrap();
    let mut page_queue = QueuedPrefetcher::new(page_queue_config);
    let page_result = page_queue
        .enqueue_candidates_filtered(20, &page_crossing, &[])
        .unwrap();
    assert_eq!(page_result.accepted(), 0);
    assert_eq!(page_result.dropped_page_crossing(), 2);
    assert_eq!(page_queue.stats().identified_prefetches(), 0);
    assert_eq!(page_queue.stats().span_page_prefetches(), 2);
    assert_eq!(page_queue.stats().useful_span_page_prefetches(), 0);
}

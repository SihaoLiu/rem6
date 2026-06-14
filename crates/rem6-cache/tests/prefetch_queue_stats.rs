use rem6_cache::{
    PrefetchCandidate, QueuedPrefetchConfig, QueuedPrefetchDemandAccess, QueuedPrefetchFullPolicy,
    QueuedPrefetchRedundantLine, QueuedPrefetchSourceStatus, QueuedPrefetchThrottle,
    QueuedPrefetchThrottleConfig, QueuedPrefetcher, StridePrefetchAccess, StridePrefetcher,
    StridePrefetcherConfig, TaggedPrefetchAccess, TaggedPrefetcher, TaggedPrefetcherConfig,
};
use rem6_memory::{Address, AgentId};

fn access(agent: u32, pc: u64, address: u64) -> StridePrefetchAccess {
    StridePrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

fn tagged_access(agent: u32, pc: u64, address: u64) -> TaggedPrefetchAccess {
    TaggedPrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

#[derive(Clone, Debug)]
struct QueueCandidate {
    address: Address,
    source_address: Address,
    context: AgentId,
    secure: bool,
    degree_index: u32,
}

impl QueueCandidate {
    const fn new(address: u64, source_address: u64, context: u32, secure: bool) -> Self {
        Self {
            address: Address::new(address),
            source_address: Address::new(source_address),
            context: AgentId::new(context),
            secure,
            degree_index: 1,
        }
    }
}

impl PrefetchCandidate for QueueCandidate {
    fn address(&self) -> Address {
        self.address
    }

    fn source_address(&self) -> Address {
        self.source_address
    }

    fn context(&self) -> AgentId {
        self.context
    }

    fn pc(&self) -> u64 {
        0x100
    }

    fn secure(&self) -> bool {
        self.secure
    }

    fn stride(&self) -> i64 {
        64
    }

    fn degree_index(&self) -> u32 {
        self.degree_index
    }
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

#[test]
fn queued_prefetcher_records_queue_stats_for_drop_paths() {
    let queue_config = QueuedPrefetchConfig::with_line_size(1, 3, 4, true, 64)
        .unwrap()
        .with_full_policy(QueuedPrefetchFullPolicy::EvictOldestLowestPriority);
    let mut queue = QueuedPrefetcher::new(queue_config);
    let first = QueueCandidate::new(0x1040, 0x1000, 1, false);
    let second = QueueCandidate::new(0x1080, 0x1000, 1, false);

    assert_eq!(
        queue
            .enqueue_candidates_filtered(10, &[first], &[])
            .unwrap()
            .accepted(),
        1
    );
    assert_eq!(
        queue
            .enqueue_candidates_filtered(11, &[second], &[])
            .unwrap()
            .evicted_full(),
        1
    );
    assert_eq!(queue.stats().prefetch_queue().enqueued(), 2);
    assert_eq!(queue.stats().prefetch_queue().dropped(), 1);

    assert_eq!(
        queue.squash_demand_access(QueuedPrefetchDemandAccess::new(Address::new(0x1080), false)),
        1
    );
    assert_eq!(queue.stats().prefetch_queue().dropped(), 2);

    let mut tagged = TaggedPrefetcher::new(TaggedPrefetcherConfig::new(64, 2).unwrap());
    let page_crossing = tagged
        .observe(tagged_access(4, 0x90, 0x0fc0))
        .unwrap()
        .to_vec();
    let page_queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64)
        .unwrap()
        .with_page_size(4096)
        .unwrap();
    let mut page_queue = QueuedPrefetcher::new(page_queue_config);
    assert_eq!(
        page_queue
            .enqueue_candidates_filtered(20, &page_crossing, &[])
            .unwrap()
            .dropped_page_crossing(),
        2
    );
    assert_eq!(page_queue.stats().translation_queue().enqueued(), 0);
    assert_eq!(page_queue.stats().translation_queue().dropped(), 2);
}

#[test]
fn queued_prefetcher_filters_duplicates_across_requestors_by_line_and_secure_bit() {
    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);
    let first = QueueCandidate::new(0x1040, 0x1000, 1, false);
    let same_line_other_requestor = QueueCandidate::new(0x1040, 0x1000, 2, false);
    let same_line_secure = QueueCandidate::new(0x1040, 0x1000, 2, true);

    assert_eq!(
        queue
            .enqueue_candidates_filtered(10, &[first], &[])
            .unwrap()
            .accepted(),
        1
    );
    let duplicate = queue
        .enqueue_candidates_filtered(11, &[same_line_other_requestor], &[])
        .unwrap();
    assert_eq!(duplicate.accepted(), 0);
    assert_eq!(duplicate.duplicate_hits(), 1);
    assert_eq!(queue.pending_count(), 1);
    assert_eq!(queue.stats().buffer_hits(), 1);
    assert_eq!(queue.stats().identified_prefetches(), 2);

    let secure_result = queue
        .enqueue_candidates_filtered(12, &[same_line_secure], &[])
        .unwrap();
    assert_eq!(secure_result.accepted(), 1);
    assert_eq!(secure_result.duplicate_hits(), 0);
    assert_eq!(queue.pending_count(), 2);
}

#[test]
fn queued_prefetcher_records_useful_span_page_candidates_from_prefetched_source() {
    let mut tagged = TaggedPrefetcher::new(TaggedPrefetcherConfig::new(64, 2).unwrap());
    let page_crossing = tagged
        .observe(tagged_access(4, 0x90, 0x0fc0))
        .unwrap()
        .to_vec();

    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64)
        .unwrap()
        .with_page_size(4096)
        .unwrap();
    let mut demand_source_queue = QueuedPrefetcher::new(queue_config.clone());
    let demand_result = demand_source_queue
        .enqueue_candidates_filtered_with_source(
            20,
            &page_crossing,
            &[],
            QueuedPrefetchSourceStatus::demand(),
        )
        .unwrap();
    assert_eq!(demand_result.dropped_page_crossing(), 2);
    assert_eq!(demand_source_queue.stats().span_page_prefetches(), 2);
    assert_eq!(demand_source_queue.stats().useful_span_page_prefetches(), 0);

    let mut prefetched_source_queue = QueuedPrefetcher::new(queue_config);
    let prefetched_result = prefetched_source_queue
        .enqueue_candidates_filtered_with_source(
            20,
            &page_crossing,
            &[],
            QueuedPrefetchSourceStatus::prefetched(),
        )
        .unwrap();
    assert_eq!(prefetched_result.dropped_page_crossing(), 2);
    assert_eq!(prefetched_source_queue.stats().span_page_prefetches(), 2);
    assert_eq!(
        prefetched_source_queue
            .stats()
            .useful_span_page_prefetches(),
        2
    );

    let snapshot = prefetched_source_queue.snapshot();
    let mut restored = QueuedPrefetcher::new(snapshot.config().clone());
    restored.restore(&snapshot).unwrap();
    assert_eq!(
        restored.stats().useful_span_page_prefetches(),
        prefetched_source_queue
            .stats()
            .useful_span_page_prefetches()
    );

    let throttle = QueuedPrefetchThrottle::new(QueuedPrefetchThrottleConfig::new(0).unwrap());
    let mut throttled_source_queue = QueuedPrefetcher::new(restored.config().clone());
    let throttled_result = throttled_source_queue
        .enqueue_candidates_throttled_with_source(
            20,
            &page_crossing,
            &[],
            &throttle,
            QueuedPrefetchSourceStatus::prefetched(),
        )
        .unwrap();
    assert_eq!(throttled_result.dropped_page_crossing(), 2);
    assert_eq!(
        throttled_source_queue.stats().useful_span_page_prefetches(),
        2
    );
}

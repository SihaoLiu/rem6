use rem6_cache::{
    QueuedPrefetchConfig, QueuedPrefetcher, StridePrefetchAccess, StridePrefetcher,
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

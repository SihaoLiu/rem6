use rem6_cache::{
    MultiQueuedPrefetcher, MultiQueuedPrefetcherError, QueuedPrefetchConfig, QueuedPrefetcher,
    QueuedPrefetcherError, StridePrefetchAccess, StridePrefetchCandidate, StridePrefetcher,
    StridePrefetcherConfig,
};
use rem6_memory::{Address, AgentId};

fn access(agent: u32, pc: u64, address: u64) -> StridePrefetchAccess {
    StridePrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

fn candidates(agent: u32, pc: u64, base: u64) -> Vec<StridePrefetchCandidate> {
    let config = StridePrefetcherConfig::new(64, 4, 2, 2, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(config);
    assert!(stride.observe(access(agent, pc, base)).unwrap().is_empty());
    assert!(stride
        .observe(access(agent, pc, base + 0x40))
        .unwrap()
        .is_empty());
    stride
        .observe(access(agent, pc, base + 0x80))
        .unwrap()
        .to_vec()
}

fn source_queue(
    agent: u32,
    pc: u64,
    base: u64,
    latency: u64,
    max_issue_per_tick: usize,
    count: usize,
) -> QueuedPrefetcher {
    let queue_config =
        QueuedPrefetchConfig::with_line_size(2, latency, max_issue_per_tick, true, 64).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);
    let candidates = candidates(agent, pc, base);
    assert_eq!(
        queue.enqueue_candidates(10, &candidates[..count]).unwrap(),
        count
    );
    queue
}

fn empty_source_queue(latency: u64, max_issue_per_tick: usize) -> QueuedPrefetcher {
    let queue_config =
        QueuedPrefetchConfig::with_line_size(2, latency, max_issue_per_tick, true, 64).unwrap();
    QueuedPrefetcher::new(queue_config)
}

#[test]
fn multi_queued_prefetcher_round_robins_ready_sources() {
    assert_eq!(
        MultiQueuedPrefetcher::new(Vec::new()).unwrap_err(),
        MultiQueuedPrefetcherError::NoSources
    );

    let source0 = source_queue(0, 0x80, 0x1000, 5, 1, 1);
    let source1 = source_queue(1, 0x90, 0x2000, 3, 2, 2);
    let source2 = source_queue(2, 0xa0, 0x3000, 3, 1, 1);
    let mut multi = MultiQueuedPrefetcher::new(vec![source0, source1, source2]).unwrap();

    assert_eq!(multi.source_count(), 3);
    assert_eq!(multi.next_ready_tick(), Some(13));
    assert_eq!(multi.issue_ready(12), None);
    assert_eq!(multi.snapshot().last_chosen_source(), 1);

    let first = multi.issue_ready(13).unwrap();
    assert_eq!(first.source_index(), 2);
    assert_eq!(first.address(), Address::new(0x30c0));
    assert_eq!(multi.next_ready_tick(), Some(13));
    assert_eq!(multi.snapshot().last_chosen_source(), 2);

    let second = multi.issue_ready(13).unwrap();
    assert_eq!(second.source_index(), 1);
    assert_eq!(second.address(), Address::new(0x20c0));
    assert_eq!(multi.snapshot().last_chosen_source(), 0);

    let third = multi.issue_ready(13).unwrap();
    assert_eq!(third.source_index(), 1);
    assert_eq!(third.address(), Address::new(0x2100));
    assert_eq!(multi.snapshot().last_chosen_source(), 1);
    assert_eq!(multi.next_ready_tick(), Some(15));

    let fourth = multi.issue_ready(15).unwrap();
    assert_eq!(fourth.source_index(), 0);
    assert_eq!(fourth.address(), Address::new(0x10c0));
    assert_eq!(multi.snapshot().last_chosen_source(), 2);
    assert_eq!(multi.next_ready_tick(), None);
}

#[test]
fn multi_queued_prefetcher_restores_sources_and_round_robin_state() {
    let source0 = source_queue(0, 0x80, 0x1000, 5, 1, 1);
    let source1 = source_queue(1, 0x90, 0x2000, 3, 2, 2);
    let source2 = source_queue(2, 0xa0, 0x3000, 3, 1, 1);
    let mut multi = MultiQueuedPrefetcher::new(vec![source0, source1, source2]).unwrap();

    let first = multi.issue_ready(13).unwrap();
    assert_eq!(first.source_index(), 1);
    assert_eq!(first.address(), Address::new(0x20c0));

    let snapshot = multi.snapshot();
    assert_eq!(snapshot.last_chosen_source(), 1);
    assert_eq!(snapshot.sources().len(), 3);
    assert_eq!(snapshot.sources()[0].pending().len(), 1);
    assert_eq!(snapshot.sources()[1].pending().len(), 1);
    assert_eq!(snapshot.sources()[2].pending().len(), 1);

    let mut restored = MultiQueuedPrefetcher::new(vec![
        empty_source_queue(5, 1),
        empty_source_queue(3, 2),
        empty_source_queue(3, 1),
    ])
    .unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let second = restored.issue_ready(13).unwrap();
    assert_eq!(second.source_index(), 2);
    assert_eq!(second.address(), Address::new(0x30c0));

    let third = restored.issue_ready(13).unwrap();
    assert_eq!(third.source_index(), 1);
    assert_eq!(third.address(), Address::new(0x2100));
}

#[test]
fn multi_queued_prefetcher_restore_rejects_source_count_mismatch() {
    let snapshot = MultiQueuedPrefetcher::new(vec![
        source_queue(0, 0x80, 0x1000, 5, 1, 1),
        source_queue(1, 0x90, 0x2000, 3, 2, 2),
    ])
    .unwrap()
    .snapshot();
    let mut restored = MultiQueuedPrefetcher::new(vec![empty_source_queue(5, 1)]).unwrap();
    let before = restored.snapshot();

    assert_eq!(
        restored.restore(&snapshot),
        Err(MultiQueuedPrefetcherError::SnapshotSourceCountMismatch {
            expected: 1,
            actual: 2,
        })
    );
    assert_eq!(restored.snapshot(), before);
}

#[test]
fn multi_queued_prefetcher_restore_rejects_child_config_mismatch_without_mutation() {
    let snapshot = MultiQueuedPrefetcher::new(vec![
        source_queue(0, 0x80, 0x1000, 5, 1, 1),
        source_queue(1, 0x90, 0x2000, 3, 2, 2),
    ])
    .unwrap()
    .snapshot();
    let mut restored = MultiQueuedPrefetcher::new(vec![
        empty_source_queue(5, 1),
        source_queue(9, 0x98, 0x4000, 9, 2, 1),
    ])
    .unwrap();
    let before = restored.snapshot();
    let expected_config = restored.source(1).unwrap().config().clone();

    assert_eq!(
        restored.restore(&snapshot),
        Err(MultiQueuedPrefetcherError::SnapshotSourceRestore {
            source_index: 1,
            source: QueuedPrefetcherError::SnapshotConfigMismatch {
                expected: expected_config,
                actual: snapshot.sources()[1].config().clone(),
            },
        })
    );
    assert_eq!(restored.snapshot(), before);
}

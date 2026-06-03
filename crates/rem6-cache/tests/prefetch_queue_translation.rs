use rem6_cache::{
    PrefetchCandidate, QueuedPrefetchConfig, QueuedPrefetchFullPolicy, QueuedPrefetchRedundantLine,
    QueuedPrefetchThrottle, QueuedPrefetchThrottleConfig, QueuedPrefetchTranslationOutcome,
    QueuedPrefetcher, QueuedPrefetcherError, TaggedPrefetchAccess, TaggedPrefetcher,
    TaggedPrefetcherConfig,
};
use rem6_memory::{
    Address, AgentId, TranslationAccessKind, TranslationFault, TranslationFaultKind,
    TranslationRequestId, TranslationResolution,
};

fn tagged_access(agent: u32, pc: u64, address: u64) -> TaggedPrefetchAccess {
    TaggedPrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

fn page_crossing_candidates() -> Vec<rem6_cache::TaggedPrefetchCandidate> {
    let mut tagged = TaggedPrefetcher::new(TaggedPrefetcherConfig::new(64, 2).unwrap());
    tagged
        .observe(tagged_access(4, 0x90, 0x0fc0))
        .unwrap()
        .to_vec()
}

#[derive(Clone, Debug)]
struct TestCandidate {
    address: Address,
    source_address: Address,
    context: AgentId,
    secure: bool,
    degree_index: u32,
}

impl TestCandidate {
    const fn new(address: u64, source_address: u64, context: u32, degree_index: u32) -> Self {
        Self {
            address: Address::new(address),
            source_address: Address::new(source_address),
            context: AgentId::new(context),
            secure: false,
            degree_index,
        }
    }
}

impl PrefetchCandidate for TestCandidate {
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
fn queued_prefetcher_defers_page_crossing_candidates_until_translation_completes() {
    let candidates = page_crossing_candidates();
    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64)
        .unwrap()
        .with_page_size(4096)
        .unwrap()
        .with_missing_translation_capacity(2)
        .unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config.clone());

    let result = queue
        .enqueue_candidates_filtered(20, &candidates[..1], &[])
        .unwrap();
    assert_eq!(result.accepted(), 0);
    assert_eq!(result.pending_translations(), 1);
    assert_eq!(result.dropped_page_crossing(), 0);
    assert_eq!(queue.missing_translation_count(), 1);
    assert_eq!(queue.pending_count(), 0);
    assert_eq!(queue.stats().span_page_prefetches(), 1);
    assert_eq!(queue.stats().identified_prefetches(), 1);

    let snapshot = queue.snapshot();
    assert_eq!(snapshot.missing_translations().len(), 1);
    assert_eq!(
        snapshot.missing_translations()[0].virtual_address(),
        Address::new(0x1000)
    );
    assert_eq!(
        snapshot.missing_translations()[0].source_address(),
        Address::new(0x0fc0)
    );
    assert_eq!(snapshot.missing_translations()[0].source_tick(), 20);
    assert_eq!(
        snapshot.missing_translations()[0].context(),
        AgentId::new(4)
    );
    assert!(!snapshot.missing_translations()[0].ongoing_translation());

    let mut restored = QueuedPrefetcher::new(queue_config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let duplicate = restored
        .enqueue_candidates_filtered(21, &candidates[..1], &[])
        .unwrap();
    assert_eq!(duplicate.pending_translations(), 0);
    assert_eq!(duplicate.duplicate_hits(), 1);
    assert_eq!(restored.missing_translation_count(), 1);

    let started = restored.process_missing_translations(1).unwrap();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].request().virtual_address(), Address::new(0x1000));
    assert_eq!(
        started[0].request().access(),
        TranslationAccessKind::Prefetch
    );
    assert_eq!(started[0].request().id().agent(), AgentId::new(4));
    assert_eq!(started[0].source_address(), Address::new(0x0fc0));
    assert!(restored.process_missing_translations(1).unwrap().is_empty());
    assert!(restored.snapshot().missing_translations()[0].ongoing_translation());

    let outcome = restored
        .complete_translation(
            30,
            started[0].request().id(),
            TranslationResolution::mapped(Address::new(0x3008)),
            &[],
        )
        .unwrap();
    assert_eq!(outcome, QueuedPrefetchTranslationOutcome::Queued);
    assert_eq!(restored.missing_translation_count(), 0);
    assert_eq!(restored.pending_count(), 1);
    assert_eq!(restored.next_ready_tick(), Some(33));

    let issues = restored.issue_ready(33);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].address(), Address::new(0x3000));
    assert_eq!(issues[0].context(), AgentId::new(4));
    assert_eq!(issues[0].source_tick(), 20);
    assert_eq!(issues[0].ready_tick(), 33);
}

#[test]
fn queued_prefetcher_rejects_completion_for_not_started_translation() {
    let candidates = page_crossing_candidates();
    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64)
        .unwrap()
        .with_page_size(4096)
        .unwrap()
        .with_missing_translation_capacity(2)
        .unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);

    assert_eq!(
        queue
            .enqueue_candidates_filtered(20, &candidates[..1], &[])
            .unwrap()
            .pending_translations(),
        1
    );
    let snapshot = queue.snapshot();
    let missing = &snapshot.missing_translations()[0];
    assert!(!missing.ongoing_translation());
    let request = TranslationRequestId::new(missing.context(), missing.order());

    assert_eq!(
        queue.complete_translation(
            30,
            request,
            TranslationResolution::mapped(Address::new(0x3008)),
            &[],
        ),
        Err(QueuedPrefetcherError::TranslationNotStarted { request })
    );
    assert_eq!(queue.snapshot(), snapshot);
}

#[test]
fn queued_prefetcher_drops_failed_or_redundant_translations() {
    let candidates = page_crossing_candidates();
    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64)
        .unwrap()
        .with_page_size(4096)
        .unwrap()
        .with_missing_translation_capacity(4)
        .unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);

    let result = queue
        .enqueue_candidates_filtered(20, &candidates, &[])
        .unwrap();
    assert_eq!(result.pending_translations(), 2);
    assert_eq!(queue.missing_translation_count(), 2);

    let started = queue.process_missing_translations(4).unwrap();
    assert_eq!(started.len(), 2);

    let failed = queue
        .complete_translation(
            30,
            started[0].request().id(),
            TranslationResolution::fault(TranslationFault::new(
                Address::new(0x1000),
                TranslationFaultKind::PageFault,
            )),
            &[],
        )
        .unwrap();
    assert_eq!(failed, QueuedPrefetchTranslationOutcome::TranslationFailed);
    assert_eq!(queue.missing_translation_count(), 1);
    assert_eq!(queue.pending_count(), 0);

    let redundant = queue
        .complete_translation(
            31,
            started[1].request().id(),
            TranslationResolution::mapped(Address::new(0x4000)),
            &[QueuedPrefetchRedundantLine::in_cache(
                Address::new(0x4008),
                false,
            )],
        )
        .unwrap();
    assert_eq!(redundant, QueuedPrefetchTranslationOutcome::Redundant);
    assert_eq!(queue.missing_translation_count(), 0);
    assert_eq!(queue.pending_count(), 0);
    assert_eq!(queue.stats().in_cache_drops(), 1);
}

#[test]
fn queued_prefetcher_filters_page_crossing_duplicates_after_translation_completion() {
    let first = TestCandidate::new(0x1000, 0x0fc0, 7, 1);
    let same_line_other_requestor = TestCandidate::new(0x1000, 0x0fc0, 8, 1);
    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64)
        .unwrap()
        .with_page_size(4096)
        .unwrap()
        .with_missing_translation_capacity(4)
        .unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config.clone());

    assert_eq!(
        queue
            .enqueue_candidates_filtered(20, std::slice::from_ref(&first), &[])
            .unwrap()
            .pending_translations(),
        1
    );
    let duplicate_in_translation_queue = queue
        .enqueue_candidates_filtered(21, std::slice::from_ref(&same_line_other_requestor), &[])
        .unwrap();
    assert_eq!(duplicate_in_translation_queue.pending_translations(), 0);
    assert_eq!(duplicate_in_translation_queue.duplicate_hits(), 1);
    assert_eq!(queue.missing_translation_count(), 1);

    let started = queue.process_missing_translations(1).unwrap();
    assert_eq!(started.len(), 1);
    assert_eq!(
        queue
            .complete_translation(
                30,
                started[0].request().id(),
                TranslationResolution::mapped(Address::new(0x3008)),
                &[],
            )
            .unwrap(),
        QueuedPrefetchTranslationOutcome::Queued
    );
    assert_eq!(queue.pending_count(), 1);
    assert_eq!(queue.missing_translation_count(), 0);

    let duplicate_in_prefetch_queue = queue
        .enqueue_candidates_filtered(31, std::slice::from_ref(&same_line_other_requestor), &[])
        .unwrap();
    assert_eq!(duplicate_in_prefetch_queue.pending_translations(), 0);
    assert_eq!(duplicate_in_prefetch_queue.duplicate_hits(), 1);
    assert_eq!(queue.pending_count(), 1);
    assert_eq!(queue.missing_translation_count(), 0);

    let snapshot = queue.snapshot();
    let mut restored = QueuedPrefetcher::new(queue_config);
    restored.restore(&snapshot).unwrap();
    let restored_duplicate = restored
        .enqueue_candidates_filtered(32, &[same_line_other_requestor], &[])
        .unwrap();
    assert_eq!(restored_duplicate.pending_translations(), 0);
    assert_eq!(restored_duplicate.duplicate_hits(), 1);
    assert_eq!(restored.pending_count(), 1);
    assert_eq!(restored.missing_translation_count(), 0);
}

#[test]
fn queued_prefetcher_processes_missing_translations_with_width_and_full_policy() {
    let candidates = page_crossing_candidates();
    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64)
        .unwrap()
        .with_page_size(4096)
        .unwrap()
        .with_missing_translation_capacity(4)
        .unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);

    assert_eq!(
        queue
            .enqueue_candidates_filtered(20, &candidates, &[])
            .unwrap()
            .pending_translations(),
        2
    );
    let first = queue.process_missing_translations(1).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].request().virtual_address(), Address::new(0x1000));
    assert!(queue.snapshot().missing_translations()[0].ongoing_translation());
    assert!(!queue.snapshot().missing_translations()[1].ongoing_translation());

    let second = queue.process_missing_translations(4).unwrap();
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].request().virtual_address(), Address::new(0x1040));

    let evicting_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64)
        .unwrap()
        .with_page_size(4096)
        .unwrap()
        .with_missing_translation_capacity(1)
        .unwrap()
        .with_full_policy(QueuedPrefetchFullPolicy::EvictOldestLowestPriority);
    let mut evicting_queue = QueuedPrefetcher::new(evicting_config);
    assert_eq!(
        evicting_queue
            .enqueue_candidates_filtered(20, &candidates[1..], &[])
            .unwrap()
            .pending_translations(),
        1
    );
    assert_eq!(
        evicting_queue
            .enqueue_candidates_filtered(21, &candidates[..1], &[])
            .unwrap()
            .evicted_full(),
        1
    );

    let remaining = evicting_queue.process_missing_translations(4).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(
        remaining[0].request().virtual_address(),
        Address::new(0x1000)
    );
    assert_eq!(evicting_queue.stats().removed_by_full_queue(), 1);
}

#[test]
fn queued_prefetcher_throttles_missing_translation_inserts() {
    let candidates = page_crossing_candidates();
    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64)
        .unwrap()
        .with_page_size(4096)
        .unwrap()
        .with_missing_translation_capacity(4)
        .unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);
    let mut throttle = QueuedPrefetchThrottle::new(QueuedPrefetchThrottleConfig::new(100).unwrap());
    throttle.record_issued(1).unwrap();

    let result = queue
        .enqueue_candidates_throttled(20, &candidates, &[], &throttle)
        .unwrap();
    assert_eq!(result.pending_translations(), 1);
    assert_eq!(result.dropped_throttled(), 1);
    assert_eq!(queue.missing_translation_count(), 1);
}

#[test]
fn queued_prefetcher_legacy_enqueue_counts_missing_translation_accepts() {
    let candidates = page_crossing_candidates();
    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64)
        .unwrap()
        .with_page_size(4096)
        .unwrap()
        .with_missing_translation_capacity(4)
        .unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);

    assert_eq!(queue.enqueue_candidates(20, &candidates).unwrap(), 2);
    assert_eq!(queue.missing_translation_count(), 2);
}

#[test]
fn queued_prefetcher_does_not_evict_ongoing_missing_translation() {
    let candidates = page_crossing_candidates();
    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64)
        .unwrap()
        .with_page_size(4096)
        .unwrap()
        .with_missing_translation_capacity(1)
        .unwrap()
        .with_full_policy(QueuedPrefetchFullPolicy::EvictOldestLowestPriority);
    let mut queue = QueuedPrefetcher::new(queue_config);

    assert_eq!(
        queue
            .enqueue_candidates_filtered(20, &candidates[..1], &[])
            .unwrap()
            .pending_translations(),
        1
    );
    let started = queue.process_missing_translations(1).unwrap();
    assert_eq!(started.len(), 1);
    assert_eq!(
        queue.enqueue_candidates_filtered(21, &candidates[1..], &[]),
        Err(QueuedPrefetcherError::QueueFull { capacity: 1 })
    );

    let outcome = queue
        .complete_translation(
            30,
            started[0].request().id(),
            TranslationResolution::mapped(Address::new(0x5000)),
            &[],
        )
        .unwrap();
    assert_eq!(outcome, QueuedPrefetchTranslationOutcome::Queued);
    assert_eq!(queue.missing_translation_count(), 0);
}

#[test]
fn queued_prefetcher_process_missing_translation_error_is_atomic() {
    let candidates = [
        TestCandidate::new(0x1000, 0x0fc0, 7, 1),
        TestCandidate::new(u64::MAX, 0x0fc0, 7, 2),
    ];
    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64)
        .unwrap()
        .with_page_size(4096)
        .unwrap()
        .with_missing_translation_capacity(4)
        .unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);
    assert_eq!(
        queue
            .enqueue_candidates_filtered(20, &candidates, &[])
            .unwrap()
            .pending_translations(),
        2
    );

    assert!(matches!(
        queue.process_missing_translations(4),
        Err(QueuedPrefetcherError::TranslationRequestAddressOverflow { .. })
    ));
    assert!(queue
        .snapshot()
        .missing_translations()
        .iter()
        .all(|entry| !entry.ongoing_translation()));
}

#[test]
fn queued_prefetcher_starts_high_priority_missing_translation_first() {
    let older_low_priority = TestCandidate::new(0x1000, 0x0fc0, 7, 4);
    let newer_high_priority = TestCandidate::new(0x2000, 0x1fc0, 7, 1);
    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64)
        .unwrap()
        .with_page_size(4096)
        .unwrap()
        .with_missing_translation_capacity(4)
        .unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);
    assert_eq!(
        queue
            .enqueue_candidates_filtered(20, &[older_low_priority], &[])
            .unwrap()
            .pending_translations(),
        1
    );
    assert_eq!(
        queue
            .enqueue_candidates_filtered(21, &[newer_high_priority], &[])
            .unwrap()
            .pending_translations(),
        1
    );

    let started = queue.process_missing_translations(1).unwrap();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].request().virtual_address(), Address::new(0x2000));
}

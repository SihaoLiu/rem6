use rem6_cache::{
    FetchDirectedCacheLookup, FetchDirectedPrefetchQueueEntrySnapshot, FetchDirectedPrefetcher,
    FetchDirectedPrefetcherConfig, FetchDirectedPrefetcherError, FetchDirectedTarget,
    FetchDirectedTranslation, FetchDirectedTranslationEntrySnapshot,
    FetchDirectedTranslationOutcome,
};
use rem6_memory::{Address, AgentId};

const FDP_PREFETCH_QUEUE_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<FetchDirectedPrefetchQueueEntrySnapshot>() + 1;
const FDP_TRANSLATION_QUEUE_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<FetchDirectedTranslationEntrySnapshot>() + 1;

fn fetch_target(agent: u32, target_id: u64, start: u64, end: u64) -> FetchDirectedTarget {
    FetchDirectedTarget::new(
        AgentId::new(agent),
        target_id,
        Address::new(start),
        Address::new(end),
        false,
    )
    .unwrap()
}

#[test]
fn fetch_directed_prefetcher_config_rejects_vector_lengths_above_host_limit() {
    assert!(matches!(
        FetchDirectedPrefetcherConfig::new(
            64,
            1,
            FDP_PREFETCH_QUEUE_BYTE_OVERFLOW_LENGTH,
            4,
            true,
            true,
            true,
        ),
        Err(FetchDirectedPrefetcherError::VectorLengthTooLarge {
            field: "prefetch queue entries",
            length: FDP_PREFETCH_QUEUE_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
    assert!(matches!(
        FetchDirectedPrefetcherConfig::new(
            64,
            1,
            4,
            FDP_TRANSLATION_QUEUE_BYTE_OVERFLOW_LENGTH,
            true,
            true,
            true,
        ),
        Err(FetchDirectedPrefetcherError::VectorLengthTooLarge {
            field: "translation queue entries",
            length: FDP_TRANSLATION_QUEUE_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
}

#[test]
fn fetch_directed_prefetcher_queues_translated_ftq_blocks_and_restores_state() {
    let config = FetchDirectedPrefetcherConfig::new(64, 3, 4, 4, true, true, true).unwrap();
    let mut prefetcher = FetchDirectedPrefetcher::new(config.clone());

    let inserted = prefetcher
        .notify_fetch_target_insert(fetch_target(2, 11, 0x1003, 0x10bf))
        .unwrap();
    assert_eq!(inserted.identified(), 3);
    assert_eq!(inserted.translation_queue_inserts(), 3);
    assert_eq!(prefetcher.translation_queue_len(), 3);

    assert_eq!(
        prefetcher
            .complete_translation(
                10,
                11,
                Address::new(0x1000),
                Ok(FetchDirectedTranslation::new(
                    Address::new(0x8000),
                    false,
                    FetchDirectedCacheLookup::Miss,
                )),
            )
            .unwrap(),
        FetchDirectedTranslationOutcome::Queued
    );
    assert_eq!(prefetcher.prefetch_queue_len(), 1);
    assert_eq!(prefetcher.next_prefetch_ready_tick(), Some(13));

    let duplicate = prefetcher
        .notify_fetch_target_insert(fetch_target(2, 12, 0x1000, 0x1040))
        .unwrap();
    assert_eq!(duplicate.already_in_prefetch_queue(), 1);
    assert_eq!(duplicate.already_in_translation_queue(), 1);
    assert_eq!(duplicate.identified(), 0);

    let snapshot = prefetcher.snapshot();
    let mut restored = FetchDirectedPrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    assert_eq!(restored.issue_ready(12), None);
    let issued = restored.issue_ready(13).unwrap();
    assert_eq!(issued.address(), Address::new(0x8000));
    assert_eq!(issued.virtual_block(), Address::new(0x1000));
    assert_eq!(issued.fetch_target_id(), 11);
    assert_eq!(issued.context(), AgentId::new(2));
    assert_eq!(issued.ready_tick(), 13);
    assert!(issued.marked_as_prefetch());
    assert!(!issued.secure());
    assert_eq!(restored.prefetch_queue_len(), 0);
}

#[test]
fn fetch_directed_prefetcher_squashes_snoops_fails_and_tracks_capacity() {
    let config = FetchDirectedPrefetcherConfig::new(64, 1, 1, 2, true, true, true).unwrap();
    let mut prefetcher = FetchDirectedPrefetcher::new(config);

    prefetcher
        .notify_fetch_target_insert(fetch_target(3, 21, 0x2000, 0x2040))
        .unwrap();
    let removed = prefetcher.notify_fetch_target_remove(21);
    assert_eq!(removed.translation_queue_canceled(), 2);
    assert_eq!(removed.prefetch_queue_removed(), 0);

    for address in [0x2000, 0x2040] {
        assert_eq!(
            prefetcher
                .complete_translation(
                    20,
                    21,
                    Address::new(address),
                    Ok(FetchDirectedTranslation::new(
                        Address::new(address + 0x4000),
                        false,
                        FetchDirectedCacheLookup::Miss,
                    )),
                )
                .unwrap(),
            FetchDirectedTranslationOutcome::Canceled
        );
    }
    assert_eq!(prefetcher.translation_queue_len(), 0);
    assert_eq!(prefetcher.stats().prefetches_squashed(), 2);

    prefetcher
        .notify_fetch_target_insert(fetch_target(3, 22, 0x3000, 0x3040))
        .unwrap();
    assert_eq!(
        prefetcher
            .complete_translation(
                30,
                22,
                Address::new(0x3000),
                Ok(FetchDirectedTranslation::new(
                    Address::new(0x9000),
                    false,
                    FetchDirectedCacheLookup::Hit,
                )),
            )
            .unwrap(),
        FetchDirectedTranslationOutcome::Redundant
    );
    assert_eq!(
        prefetcher
            .complete_translation(
                30,
                22,
                Address::new(0x3040),
                Ok(FetchDirectedTranslation::new(
                    Address::new(0x9040),
                    false,
                    FetchDirectedCacheLookup::Miss,
                )),
            )
            .unwrap(),
        FetchDirectedTranslationOutcome::Queued
    );

    prefetcher
        .notify_fetch_target_insert(fetch_target(3, 23, 0x3080, 0x3080))
        .unwrap();
    assert_eq!(
        prefetcher
            .complete_translation(
                31,
                23,
                Address::new(0x3080),
                Ok(FetchDirectedTranslation::new(
                    Address::new(0x9080),
                    false,
                    FetchDirectedCacheLookup::Miss,
                )),
            )
            .unwrap(),
        FetchDirectedTranslationOutcome::PrefetchQueueFull
    );

    prefetcher
        .notify_fetch_target_insert(fetch_target(3, 24, 0x30c0, 0x30c0))
        .unwrap();
    assert_eq!(
        prefetcher
            .complete_translation(32, 24, Address::new(0x30c0), Err(()))
            .unwrap(),
        FetchDirectedTranslationOutcome::TranslationFailed
    );

    let stats = prefetcher.stats();
    assert_eq!(stats.prefetches_in_cache(), 1);
    assert_eq!(stats.prefetch_queue_drops(), 1);
    assert_eq!(stats.translation_failures(), 1);
    assert_eq!(prefetcher.next_prefetch_ready_tick(), Some(31));
}

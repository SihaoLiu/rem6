use rem6_cache::{
    PifCompactorEntrySnapshot, PifHistoryEntrySnapshot, PifIndexEntrySnapshot, PifPrefetchAccess,
    PifPrefetchCandidate, PifPrefetcher, PifPrefetcherConfig, PifPrefetcherError,
};
use rem6_memory::{Address, AgentId};

const PIF_SPATIAL_WINDOW_BYTE_OVERFLOW_BLOCKS: usize =
    isize::MAX as usize / std::mem::size_of::<PifPrefetchCandidate>() + 1;
const PIF_TEMPORAL_COMPACTOR_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<PifCompactorEntrySnapshot>() + 1;
const PIF_HISTORY_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<PifHistoryEntrySnapshot>() + 1;
const PIF_INDEX_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<PifIndexEntrySnapshot>() + 1;
const U64_BYTE_OVERFLOW_LENGTH: usize = isize::MAX as usize / std::mem::size_of::<u64>() + 1;

fn pif_access(agent: u32, pc: u64, secure: bool) -> PifPrefetchAccess {
    PifPrefetchAccess::new(AgentId::new(agent), Address::new(pc), secure)
}

#[test]
fn pif_prefetcher_config_rejects_vector_lengths_above_host_limit() {
    assert!(matches!(
        PifPrefetcherConfig::new(64, PIF_SPATIAL_WINDOW_BYTE_OVERFLOW_BLOCKS, 0, 2, 2, 4, 4,),
        Err(PifPrefetcherError::VectorLengthTooLarge {
            field: "spatial window blocks",
            length: PIF_SPATIAL_WINDOW_BYTE_OVERFLOW_BLOCKS,
            ..
        })
    ));
    assert!(matches!(
        PifPrefetcherConfig::new(
            64,
            2,
            4,
            PIF_TEMPORAL_COMPACTOR_BYTE_OVERFLOW_LENGTH,
            2,
            4,
            4,
        ),
        Err(PifPrefetcherError::VectorLengthTooLarge {
            field: "temporal compactor entries",
            length: PIF_TEMPORAL_COMPACTOR_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
    assert_eq!(
        PifPrefetcherConfig::new(64, 2, 4, 2, U64_BYTE_OVERFLOW_LENGTH, 4, 4),
        Err(PifPrefetcherError::VectorLengthTooLarge {
            field: "stream address buffer entries",
            length: U64_BYTE_OVERFLOW_LENGTH,
            maximum: isize::MAX as usize / std::mem::size_of::<u64>(),
        })
    );
    assert!(matches!(
        PifPrefetcherConfig::new(64, 2, 4, 2, 2, PIF_HISTORY_BYTE_OVERFLOW_LENGTH, 4),
        Err(PifPrefetcherError::VectorLengthTooLarge {
            field: "history buffer entries",
            length: PIF_HISTORY_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
    assert!(matches!(
        PifPrefetcherConfig::new(64, 2, 4, 2, 2, 4, PIF_INDEX_BYTE_OVERFLOW_LENGTH),
        Err(PifPrefetcherError::VectorLengthTooLarge {
            field: "index entries",
            length: PIF_INDEX_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
}

#[test]
fn pif_prefetcher_indexes_retired_regions_advances_sab_and_restores_state() {
    let config = PifPrefetcherConfig::new(64, 2, 4, 2, 2, 4, 4).unwrap();
    let mut prefetcher = PifPrefetcher::new(config.clone());

    for pc in [0x1000, 0x1040, 0x1080, 0x2000, 0x2040, 0x2080, 0x3000] {
        prefetcher.observe_retired_instruction(Address::new(pc));
    }

    assert_eq!(prefetcher.history_entry_count(), 2);
    assert_eq!(
        prefetcher.history_triggers(),
        vec![Address::new(0x1000), Address::new(0x2000)]
    );
    assert!(prefetcher.index_contains(Address::new(0x1000), false));
    assert!(prefetcher.index_contains(Address::new(0x2000), false));

    let first = prefetcher.observe(pif_access(7, 0x1000, false)).to_vec();
    assert_eq!(
        first
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x1040), Address::new(0x1080)]
    );
    assert_eq!(first[0].source_address(), Address::new(0x1000));
    assert_eq!(first[0].context(), AgentId::new(7));
    assert_eq!(first[0].pc(), 0x1000);
    assert_eq!(first[0].block_offset(), 1);
    assert_eq!(first[0].degree_index(), 1);
    assert_eq!(first[1].block_offset(), 2);
    assert_eq!(prefetcher.stream_address_buffer_count(), 1);

    let snapshot = prefetcher.snapshot();
    let mut restored = PifPrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let second = restored.observe(pif_access(7, 0x1040, false)).to_vec();
    assert_eq!(
        second
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x2040), Address::new(0x2080)]
    );
    assert_eq!(second[0].source_address(), Address::new(0x1040));
    assert_eq!(second[0].trigger(), Address::new(0x2000));
    assert_eq!(second[1].block_offset(), 2);

    assert!(restored.observe(pif_access(7, 0x1000, true)).is_empty());
}

#[test]
fn pif_prefetcher_applies_history_index_and_stream_buffer_capacity() {
    let config = PifPrefetcherConfig::new(64, 1, 3, 1, 1, 1, 1).unwrap();
    let mut prefetcher = PifPrefetcher::new(config);

    for pc in [0x4000, 0x4040, 0x5000, 0x5040, 0x6000] {
        prefetcher.observe_retired_instruction(Address::new(pc));
    }

    assert_eq!(prefetcher.history_entry_count(), 1);
    assert_eq!(prefetcher.index_entry_count(), 1);
    assert_eq!(prefetcher.history_triggers(), vec![Address::new(0x5000)]);
    assert!(!prefetcher.index_contains(Address::new(0x4000), false));
    assert!(prefetcher.index_contains(Address::new(0x5000), false));

    assert!(prefetcher.observe(pif_access(8, 0x4000, false)).is_empty());
    let candidates = prefetcher.observe(pif_access(8, 0x5000, false)).to_vec();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x5040)]
    );
    assert_eq!(prefetcher.stream_address_buffer_count(), 1);

    prefetcher.observe(pif_access(8, 0x5000, false));
    assert_eq!(prefetcher.stream_address_buffer_count(), 1);
}

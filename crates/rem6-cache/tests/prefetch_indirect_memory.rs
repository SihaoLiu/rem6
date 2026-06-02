use rem6_cache::{
    IndirectMemoryPatternDetectorEntrySnapshot, IndirectMemoryPrefetchAccess,
    IndirectMemoryPrefetchEntrySnapshot, IndirectMemoryPrefetchKind, IndirectMemoryPrefetcher,
    IndirectMemoryPrefetcherConfig, IndirectMemoryPrefetcherError,
};
use rem6_memory::{Address, AgentId};

const IMP_PREFETCH_TABLE_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<IndirectMemoryPrefetchEntrySnapshot>() + 1;
const IMP_PATTERN_DETECTOR_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<IndirectMemoryPatternDetectorEntrySnapshot>() + 1;
const IMP_ADDRESS_ARRAY_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<Vec<Option<Address>>>() + 1;

fn imp_access(
    agent: u32,
    pc: u64,
    address: u64,
    secure: bool,
    cache_miss: bool,
) -> IndirectMemoryPrefetchAccess {
    IndirectMemoryPrefetchAccess::new(
        AgentId::new(agent),
        pc,
        Address::new(address),
        secure,
        cache_miss,
    )
}

fn imp_index_access(agent: u32, pc: u64, address: u64, index: i64) -> IndirectMemoryPrefetchAccess {
    imp_access(agent, pc, address, false, false)
        .with_read_index(8, index)
        .unwrap()
}

fn imp_index_access_with_lookahead(
    agent: u32,
    pc: u64,
    address: u64,
    index: i64,
    lookahead: [i64; 3],
) -> IndirectMemoryPrefetchAccess {
    imp_access(agent, pc, address, false, false)
        .with_read_index_lookahead(8, index, lookahead)
        .unwrap()
}

#[test]
fn indirect_memory_prefetcher_config_rejects_vector_lengths_above_host_limit() {
    assert!(matches!(
        IndirectMemoryPrefetcherConfig::new(
            IMP_PREFETCH_TABLE_BYTE_OVERFLOW_LENGTH,
            2,
            2,
            vec![2],
            4,
            2,
            1,
            100,
            2,
        ),
        Err(IndirectMemoryPrefetcherError::VectorLengthTooLarge {
            field: "prefetch table entries",
            length: IMP_PREFETCH_TABLE_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
    assert!(matches!(
        IndirectMemoryPrefetcherConfig::new(
            4,
            IMP_PATTERN_DETECTOR_BYTE_OVERFLOW_LENGTH,
            2,
            vec![2],
            4,
            2,
            1,
            100,
            2,
        ),
        Err(IndirectMemoryPrefetcherError::VectorLengthTooLarge {
            field: "pattern detector entries",
            length: IMP_PATTERN_DETECTOR_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
    assert!(matches!(
        IndirectMemoryPrefetcherConfig::new(
            4,
            2,
            IMP_ADDRESS_ARRAY_BYTE_OVERFLOW_LENGTH,
            vec![2],
            4,
            2,
            1,
            100,
            2,
        ),
        Err(IndirectMemoryPrefetcherError::VectorLengthTooLarge {
            field: "address array length",
            length: IMP_ADDRESS_ARRAY_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
}

#[test]
fn indirect_memory_prefetcher_detects_indirect_pattern_and_restores_state() {
    let config = IndirectMemoryPrefetcherConfig::new(4, 2, 2, vec![2], 4, 2, 1, 100, 2).unwrap();
    let mut prefetcher = IndirectMemoryPrefetcher::new(config.clone());

    assert!(prefetcher
        .observe(imp_access(8, 0xaaa, 0x1000, false, false))
        .unwrap()
        .is_empty());
    assert!(prefetcher
        .observe(imp_index_access(8, 0xaaa, 0x1100, 4))
        .unwrap()
        .is_empty());
    assert_eq!(prefetcher.prefetch_table_entry_count(), 1);
    assert_eq!(prefetcher.pattern_detector_entry_count(), 1);
    assert_eq!(prefetcher.tracking_pattern_key(), Some((0xaaa, false)));

    assert!(prefetcher
        .observe(imp_access(8, 0xbbb, 0x8010, false, true))
        .unwrap()
        .is_empty());
    assert!(prefetcher
        .observe(imp_index_access(8, 0xaaa, 0x1200, 5))
        .unwrap()
        .is_empty());
    assert!(prefetcher
        .observe(imp_access(8, 0xbbb, 0x8014, false, true))
        .unwrap()
        .is_empty());
    assert_eq!(
        prefetcher.indirect_mapping(0xaaa, false),
        Some((Address::new(0x8000), 2))
    );
    assert_eq!(prefetcher.pattern_detector_entry_count(), 0);

    assert!(prefetcher
        .observe(imp_index_access(8, 0xaaa, 0x1300, 6))
        .unwrap()
        .is_empty());
    for _ in 0..3 {
        assert!(prefetcher
            .observe(imp_access(8, 0xbbb, 0x8018, false, false))
            .unwrap()
            .is_empty());
    }
    assert_eq!(prefetcher.indirect_counter(0xaaa, false), Some(3));

    let snapshot = prefetcher.snapshot();
    let mut restored = IndirectMemoryPrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let candidates = restored
        .observe(imp_index_access_with_lookahead(
            8,
            0xaaa,
            0x1400,
            7,
            [8, 9, 10],
        ))
        .unwrap()
        .to_vec();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![
            Address::new(0x8020),
            Address::new(0x8024),
            Address::new(0x8028)
        ]
    );
    assert!(candidates
        .iter()
        .all(|candidate| candidate.kind() == IndirectMemoryPrefetchKind::Indirect));
    assert_eq!(candidates[0].source_address(), Address::new(0x1400));
    assert_eq!(candidates[0].context(), AgentId::new(8));
    assert_eq!(candidates[0].pc(), 0xaaa);
    assert_eq!(candidates[0].base_address(), Address::new(0x8000));
    assert_eq!(candidates[0].index(), 8);
    assert_eq!(candidates[0].shift(), 2);
    assert_eq!(candidates[0].indirect_counter(), 3);
    assert_eq!(candidates[2].index(), 10);
    assert_eq!(candidates[2].degree_index(), 3);
    assert_eq!(restored.last_candidates(), candidates.as_slice());
}

#[test]
fn indirect_memory_prefetcher_without_lookahead_does_not_duplicate_current_index() {
    let config = IndirectMemoryPrefetcherConfig::new(4, 2, 2, vec![2], 4, 2, 1, 100, 2).unwrap();
    let mut prefetcher = IndirectMemoryPrefetcher::new(config);

    prefetcher
        .observe(imp_access(8, 0xaaa, 0x1000, false, false))
        .unwrap();
    prefetcher
        .observe(imp_index_access(8, 0xaaa, 0x1100, 4))
        .unwrap();
    prefetcher
        .observe(imp_access(8, 0xbbb, 0x8010, false, true))
        .unwrap();
    prefetcher
        .observe(imp_index_access(8, 0xaaa, 0x1200, 5))
        .unwrap();
    prefetcher
        .observe(imp_access(8, 0xbbb, 0x8014, false, true))
        .unwrap();
    prefetcher
        .observe(imp_index_access(8, 0xaaa, 0x1300, 6))
        .unwrap();
    for _ in 0..3 {
        prefetcher
            .observe(imp_access(8, 0xbbb, 0x8018, false, false))
            .unwrap();
    }

    let candidates = prefetcher
        .observe(imp_index_access(8, 0xaaa, 0x1400, 7))
        .unwrap()
        .to_vec();

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].address(), Address::new(0x801c));
    assert_eq!(candidates[0].index(), 7);
    assert_eq!(candidates[0].degree_index(), 1);
}

#[test]
fn indirect_memory_prefetcher_streams_and_applies_lru_capacity() {
    let config = IndirectMemoryPrefetcherConfig::new(1, 1, 1, vec![2], 8, 2, 1, 2, 2).unwrap();
    let mut prefetcher = IndirectMemoryPrefetcher::new(config);

    assert!(prefetcher
        .observe(imp_access(9, 0xc00, 0x2000, false, false))
        .unwrap()
        .is_empty());
    assert!(prefetcher
        .observe(imp_access(9, 0xc00, 0x2040, false, false))
        .unwrap()
        .is_empty());

    let stream_candidates = prefetcher
        .observe(imp_access(9, 0xc00, 0x2080, false, false))
        .unwrap()
        .to_vec();
    assert_eq!(
        stream_candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x20c0), Address::new(0x2100)]
    );
    assert!(stream_candidates
        .iter()
        .all(|candidate| candidate.kind() == IndirectMemoryPrefetchKind::Stream));
    assert_eq!(stream_candidates[1].stream_delta(), 0x40);
    assert_eq!(stream_candidates[1].degree_index(), 2);

    prefetcher
        .observe(imp_access(9, 0xd00, 0x3000, false, false))
        .unwrap();
    assert_eq!(prefetcher.prefetch_table_entry_count(), 1);
    assert!(!prefetcher.prefetch_table_contains(0xc00, false));
    assert!(prefetcher.prefetch_table_contains(0xd00, false));

    prefetcher
        .observe(imp_index_access(9, 0xd00, 0x3040, 1))
        .unwrap();
    assert_eq!(prefetcher.pattern_detector_entry_count(), 1);
    prefetcher
        .observe(imp_access(9, 0xe00, 0x4000, false, false))
        .unwrap();
    assert_eq!(prefetcher.pattern_detector_entry_count(), 0);
    assert_eq!(prefetcher.tracking_pattern_key(), None);
}

use rem6_cache::{
    StemsCacheResidency, StemsGenerationEntrySnapshot, StemsPatternSequenceEntrySnapshot,
    StemsPrefetchAccess, StemsPrefetchCandidate, StemsPrefetcher, StemsPrefetcherConfig,
    StemsPrefetcherError, StemsRegionMissOrderBufferEntrySnapshot, StemsSequenceEntrySnapshot,
};
use rem6_memory::{Address, AgentId};

const STEMS_SEQUENCE_SLOT_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<StemsSequenceEntrySnapshot>() + 1;
const STEMS_SEQUENCE_SLOT_POWER_OF_TWO_OVERFLOW_LENGTH: usize = 1usize << (usize::BITS - 4);
const _: () = assert!(
    STEMS_SEQUENCE_SLOT_POWER_OF_TWO_OVERFLOW_LENGTH > STEMS_SEQUENCE_SLOT_BYTE_OVERFLOW_LENGTH
);
const STEMS_RECONSTRUCTION_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<StemsPrefetchCandidate>() + 1;
const STEMS_ACTIVE_GENERATION_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<StemsGenerationEntrySnapshot>() + 1;
const STEMS_PATTERN_SEQUENCE_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<StemsPatternSequenceEntrySnapshot>() + 1;
const STEMS_RMOB_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<StemsRegionMissOrderBufferEntrySnapshot>() + 1;

fn stems_access(
    agent: u32,
    pc: u64,
    address: u64,
    secure: bool,
    cache_miss: bool,
) -> StemsPrefetchAccess {
    StemsPrefetchAccess::new(
        AgentId::new(agent),
        pc,
        Address::new(address),
        Address::new(address),
        secure,
        cache_miss,
    )
}

fn residency(addresses: &[u64]) -> StemsCacheResidency {
    addresses
        .iter()
        .fold(StemsCacheResidency::new(), |resident, address| {
            resident.with_cache_line(Address::new(*address), false)
        })
}

#[test]
fn stems_prefetcher_config_rejects_vector_lengths_above_host_limit() {
    assert!(matches!(
        StemsPrefetcherConfig::new(
            1,
            STEMS_SEQUENCE_SLOT_POWER_OF_TWO_OVERFLOW_LENGTH as u64,
            8,
            4,
            4,
            8,
            false,
        ),
        Err(StemsPrefetcherError::VectorLengthTooLarge {
            field: "sequence slots",
            length: STEMS_SEQUENCE_SLOT_POWER_OF_TWO_OVERFLOW_LENGTH,
            ..
        })
    ));
    assert!(matches!(
        StemsPrefetcherConfig::new(
            64,
            256,
            STEMS_RECONSTRUCTION_BYTE_OVERFLOW_LENGTH,
            4,
            4,
            8,
            false,
        ),
        Err(StemsPrefetcherError::VectorLengthTooLarge {
            field: "reconstruction entries",
            length: STEMS_RECONSTRUCTION_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
    assert!(matches!(
        StemsPrefetcherConfig::new(
            64,
            256,
            8,
            STEMS_ACTIVE_GENERATION_BYTE_OVERFLOW_LENGTH,
            4,
            8,
            false,
        ),
        Err(StemsPrefetcherError::VectorLengthTooLarge {
            field: "active generation entries",
            length: STEMS_ACTIVE_GENERATION_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
    assert!(matches!(
        StemsPrefetcherConfig::new(
            64,
            256,
            8,
            4,
            STEMS_PATTERN_SEQUENCE_BYTE_OVERFLOW_LENGTH,
            8,
            false,
        ),
        Err(StemsPrefetcherError::VectorLengthTooLarge {
            field: "pattern sequence entries",
            length: STEMS_PATTERN_SEQUENCE_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
    assert!(matches!(
        StemsPrefetcherConfig::new(64, 256, 8, 4, 4, STEMS_RMOB_BYTE_OVERFLOW_LENGTH, false,),
        Err(StemsPrefetcherError::VectorLengthTooLarge {
            field: "RMOB entries",
            length: STEMS_RMOB_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
}

#[test]
fn stems_prefetcher_commits_generation_reconstructs_sequence_and_restores_state() {
    let config = StemsPrefetcherConfig::new(64, 256, 8, 4, 4, 8, false).unwrap();
    let mut prefetcher = StemsPrefetcher::new(config.clone());

    assert_eq!(
        prefetcher
            .observe(
                stems_access(6, 0xabc, 0x1000, false, true),
                &residency(&[0x1000])
            )
            .unwrap()
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x1000)]
    );

    for address in [0x1040, 0x1080, 0x1040, 0x1080] {
        assert!(prefetcher
            .observe(
                stems_access(6, 0xabc, address, false, false),
                &residency(&[0x1000, 0x1040, 0x1080])
            )
            .unwrap()
            .is_empty());
    }

    assert_eq!(prefetcher.active_generation_count(), 1);
    assert_eq!(prefetcher.pattern_sequence_count(), 0);

    prefetcher
        .observe(
            stems_access(6, 0xdef, 0x2000, false, false),
            &StemsCacheResidency::new(),
        )
        .unwrap();
    assert_eq!(prefetcher.active_generation_count(), 1);
    assert_eq!(prefetcher.pattern_sequence_count(), 1);

    let snapshot = prefetcher.snapshot();
    let mut restored = StemsPrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let candidates = restored
        .observe(
            stems_access(6, 0xabc, 0x1000, false, true),
            &residency(&[0x2000]),
        )
        .unwrap()
        .to_vec();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![
            Address::new(0x1000),
            Address::new(0x1040),
            Address::new(0x1080),
            Address::new(0x2000)
        ]
    );
    assert_eq!(candidates[1].source_address(), Address::new(0x1000));
    assert_eq!(candidates[1].context(), AgentId::new(6));
    assert_eq!(candidates[1].pc(), 0xabc);
    assert_eq!(candidates[1].region_address(), 0x10);
    assert_eq!(candidates[1].spatial_offset(), 1);
    assert_eq!(candidates[1].reconstruction_index(), 1);
    assert_eq!(candidates[1].degree_index(), 2);
    assert_eq!(restored.last_candidates(), candidates.as_slice());

    let secure_candidates = restored
        .observe(
            stems_access(6, 0xabc, 0x1000, true, true),
            &StemsCacheResidency::new(),
        )
        .unwrap()
        .to_vec();
    assert_eq!(
        secure_candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x1000)]
    );
}

#[test]
fn stems_prefetcher_filters_duplicate_rmob_entries_and_applies_capacity() {
    let config = StemsPrefetcherConfig::new(64, 256, 4, 2, 2, 2, false).unwrap();
    let mut prefetcher = StemsPrefetcher::new(config);

    prefetcher
        .observe(
            stems_access(7, 0xaaa, 0x3000, false, true),
            &residency(&[0x3000]),
        )
        .unwrap();
    assert_eq!(
        prefetcher
            .rmob_entries()
            .iter()
            .map(|entry| (entry.region_address(), entry.pst_address(), entry.delta()))
            .collect::<Vec<_>>(),
        vec![(0x30, 0xaaa00, 0)]
    );

    prefetcher
        .observe(
            stems_access(7, 0xbbb, 0x4000, false, true),
            &StemsCacheResidency::new(),
        )
        .unwrap();
    prefetcher
        .observe(
            stems_access(7, 0xaaa, 0x3000, false, true),
            &StemsCacheResidency::new(),
        )
        .unwrap();
    assert_eq!(
        prefetcher
            .rmob_entries()
            .iter()
            .map(|entry| entry.region_address())
            .collect::<Vec<_>>(),
        vec![0x30, 0x40]
    );

    prefetcher
        .observe(
            stems_access(7, 0xccc, 0x5000, false, true),
            &StemsCacheResidency::new(),
        )
        .unwrap();
    assert_eq!(
        prefetcher
            .rmob_entries()
            .iter()
            .map(|entry| entry.region_address())
            .collect::<Vec<_>>(),
        vec![0x40, 0x50]
    );
    assert_eq!(prefetcher.active_generation_count(), 1);
    assert!(prefetcher
        .active_generation_regions()
        .contains(&(0x50, false)));
}

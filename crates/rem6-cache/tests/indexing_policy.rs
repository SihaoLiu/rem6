use rem6_cache::{
    CacheIndexingLocation, CacheIndexingPolicyConfig, CacheIndexingPolicyError,
    CacheIndexingPolicyKind,
};
use rem6_memory::{Address, CacheLineLayout};

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn locations(pairs: &[(usize, usize)]) -> Vec<CacheIndexingLocation> {
    pairs
        .iter()
        .map(|(set, way)| CacheIndexingLocation::new(*set, *way))
        .collect()
}

#[test]
fn cache_indexing_set_associative_matches_gem5_identity_mapping() {
    let config = CacheIndexingPolicyConfig::new(
        CacheIndexingPolicyKind::SetAssociative,
        line_layout(),
        8,
        4,
    )
    .unwrap();

    assert_eq!(config.kind(), CacheIndexingPolicyKind::SetAssociative);
    assert_eq!(config.extract_tag(Address::new(0x80)), 1);
    assert_eq!(
        config.candidate_locations(Address::new(0x83)),
        locations(&[(0, 0), (0, 1), (0, 2), (0, 3)])
    );

    for location in config.candidate_locations(Address::new(0x80)) {
        assert_eq!(
            config.regenerate_address(1, location).unwrap(),
            Address::new(0x80)
        );
    }
}

#[test]
fn cache_indexing_skewed_associative_matches_gem5_way_hashes() {
    let config = CacheIndexingPolicyConfig::new(
        CacheIndexingPolicyKind::SkewedAssociative,
        line_layout(),
        8,
        4,
    )
    .unwrap();

    assert_eq!(config.kind(), CacheIndexingPolicyKind::SkewedAssociative);
    assert_eq!(config.extract_tag(Address::new(0x80)), 1);
    assert_eq!(
        config.candidate_locations(Address::new(0x80)),
        locations(&[(5, 0), (4, 1), (3, 2), (2, 3)])
    );
    assert_eq!(
        config.candidate_locations(Address::new(0xa0)),
        locations(&[(4, 0), (7, 1), (2, 2), (1, 3)])
    );

    for location in config.candidate_locations(Address::new(0x80)) {
        assert_eq!(
            config.regenerate_address(1, location).unwrap(),
            Address::new(0x80)
        );
    }
}

#[test]
fn cache_indexing_rejects_gem5_invalid_indexing_shapes() {
    assert_eq!(
        CacheIndexingPolicyConfig::new(
            CacheIndexingPolicyKind::SetAssociative,
            line_layout(),
            0,
            4
        ),
        Err(CacheIndexingPolicyError::ZeroSets)
    );
    assert_eq!(
        CacheIndexingPolicyConfig::new(
            CacheIndexingPolicyKind::SetAssociative,
            line_layout(),
            8,
            0
        ),
        Err(CacheIndexingPolicyError::ZeroWays)
    );
    assert_eq!(
        CacheIndexingPolicyConfig::new(
            CacheIndexingPolicyKind::SetAssociative,
            line_layout(),
            6,
            4
        ),
        Err(CacheIndexingPolicyError::SetsNotPowerOfTwo { sets: 6 })
    );
    assert_eq!(
        CacheIndexingPolicyConfig::new(
            CacheIndexingPolicyKind::SkewedAssociative,
            line_layout(),
            2,
            4,
        ),
        Err(CacheIndexingPolicyError::SkewedAssociativeTooFewSets { sets: 2 })
    );
    assert_eq!(
        CacheIndexingPolicyConfig::new(
            CacheIndexingPolicyKind::SkewedAssociative,
            CacheLineLayout::new(1).unwrap(),
            1_usize << 33,
            4,
        ),
        Err(
            CacheIndexingPolicyError::SkewedAssociativeAddressBitsTooWide {
                set_shift: 0,
                set_bits: 33,
            }
        )
    );
}

#[test]
fn cache_indexing_rejects_unknown_regeneration_locations() {
    let config = CacheIndexingPolicyConfig::new(
        CacheIndexingPolicyKind::SetAssociative,
        line_layout(),
        8,
        4,
    )
    .unwrap();

    assert_eq!(
        config.regenerate_address(0, CacheIndexingLocation::new(8, 0)),
        Err(CacheIndexingPolicyError::UnknownSet { set: 8, sets: 8 })
    );
    assert_eq!(
        config.regenerate_address(0, CacheIndexingLocation::new(0, 4)),
        Err(CacheIndexingPolicyError::UnknownWay { way: 4, ways: 4 })
    );
}

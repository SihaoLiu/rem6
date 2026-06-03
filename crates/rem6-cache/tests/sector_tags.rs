use rem6_cache::{
    CacheIndexingLocation, CacheIndexingPolicyError, CacheIndexingPolicyKind,
    CacheReplacementPolicyKind, CacheSectorTagEntrySnapshot, CacheSectorTagSetSnapshot,
    CacheSectorTags, CacheSectorTagsConfig, CacheSectorTagsError, CacheSectorTagsSnapshot,
};
use rem6_memory::{Address, CacheLineLayout};
use std::collections::BTreeMap;

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn config(sets: usize, ways: usize) -> CacheSectorTagsConfig {
    CacheSectorTagsConfig::new(
        CacheReplacementPolicyKind::Lru,
        line_layout(),
        sets,
        ways,
        4,
    )
    .unwrap()
}

fn sector_address(config: &CacheSectorTagsConfig, sector_index: u64) -> Address {
    Address::new(sector_index * config.sector_layout().bytes())
}

fn candidate_sector_bases(
    tags: &CacheSectorTags,
    target: Address,
) -> Vec<(CacheIndexingLocation, Address)> {
    let snapshot = tags.snapshot();
    tags.config()
        .indexing_config()
        .candidate_locations(target)
        .into_iter()
        .filter_map(|location| {
            snapshot.sets()[location.set()].sectors()[location.way()]
                .sector_base()
                .map(|sector_base| (location, sector_base))
        })
        .collect()
}

#[test]
fn sector_tags_reject_invalid_gem5_sector_shapes() {
    assert_eq!(
        CacheSectorTagsConfig::new(CacheReplacementPolicyKind::Lru, line_layout(), 2, 2, 0,),
        Err(CacheSectorTagsError::ZeroBlocksPerSector)
    );
    assert_eq!(
        CacheSectorTagsConfig::new(CacheReplacementPolicyKind::Lru, line_layout(), 2, 2, 3,),
        Err(CacheSectorTagsError::BlocksPerSectorNotPowerOfTwo { blocks: 3 })
    );
    assert_eq!(
        CacheSectorTagsConfig::new(
            CacheReplacementPolicyKind::Lru,
            CacheLineLayout::new(2).unwrap(),
            2,
            2,
            4,
        ),
        Err(CacheSectorTagsError::LineSizeTooSmall { bytes: 2 })
    );
    assert_eq!(
        CacheSectorTagsConfig::new(CacheReplacementPolicyKind::Lru, line_layout(), 3, 2, 4,),
        Err(CacheSectorTagsError::IndexingPolicyConfig {
            source: CacheIndexingPolicyError::SetsNotPowerOfTwo { sets: 3 },
        })
    );
}

#[test]
fn sector_tags_use_sector_entry_size_for_offsets_and_indexing() {
    let config = config(8, 4);

    assert_eq!(config.line_layout(), line_layout());
    assert_eq!(config.sector_layout(), CacheLineLayout::new(64).unwrap());
    assert_eq!(
        config.sector_base(Address::new(0x1027)),
        Address::new(0x1000)
    );
    assert_eq!(config.sector_offset(Address::new(0x1000)), 0);
    assert_eq!(config.sector_offset(Address::new(0x1010)), 1);
    assert_eq!(config.sector_offset(Address::new(0x1020)), 2);
    assert_eq!(config.sector_offset(Address::new(0x1030)), 3);
    assert_eq!(config.sector_offset(Address::new(0x1040)), 0);
    assert_eq!(
        config
            .indexing_config()
            .candidate_locations(Address::new(0x1010)),
        config
            .indexing_config()
            .candidate_locations(Address::new(0x1000))
    );
}

#[test]
fn sector_tags_insert_and_find_multiple_subblocks_in_one_sector() {
    let mut tags = CacheSectorTags::new(config(2, 2));

    let first = tags.insert(Address::new(0x1000)).unwrap();
    let second = tags.insert(Address::new(0x1010)).unwrap();

    assert!(first.new_sector());
    assert!(!second.new_sector());
    assert_eq!(first.sector_base(), Address::new(0x1000));
    assert_eq!(second.sector_base(), Address::new(0x1000));
    assert_eq!(first.set(), second.set());
    assert_eq!(first.way(), second.way());
    assert_eq!(first.offset(), 0);
    assert_eq!(second.offset(), 1);
    assert_eq!(first.evicted_lines(), &[]);
    assert_eq!(second.evicted_lines(), &[]);
    assert_eq!(tags.valid_sector_count(), 1);
    assert_eq!(tags.valid_line_count(), 2);

    let hit = tags.find(Address::new(0x1017)).unwrap();
    assert_eq!(hit.line(), Address::new(0x1010));
    assert_eq!(hit.sector_base(), Address::new(0x1000));
    assert_eq!(hit.offset(), 1);
    assert_eq!(tags.find(Address::new(0x1020)), None);
    assert_eq!(
        tags.sector_lines(first.set(), first.way()).unwrap(),
        vec![
            Some(Address::new(0x1000)),
            Some(Address::new(0x1010)),
            None,
            None,
        ]
    );
}

#[test]
fn sector_tags_invalidate_only_clears_sector_after_last_subblock() {
    let mut tags = CacheSectorTags::new(config(2, 2));
    let first = tags.insert(Address::new(0x1000)).unwrap();
    tags.insert(Address::new(0x1010)).unwrap();

    let partial = tags.invalidate(Address::new(0x1000)).unwrap().unwrap();
    assert!(partial.sector_still_valid());
    assert_eq!(tags.valid_sector_count(), 1);
    assert_eq!(tags.valid_line_count(), 1);
    assert_eq!(tags.find(Address::new(0x1000)), None);
    assert!(tags.find(Address::new(0x1010)).is_some());

    let final_invalidate = tags.invalidate(Address::new(0x1010)).unwrap().unwrap();
    assert!(!final_invalidate.sector_still_valid());
    assert_eq!(tags.valid_sector_count(), 0);
    assert_eq!(tags.valid_line_count(), 0);
    assert_eq!(tags.invalidate(Address::new(0x1010)).unwrap(), None);
    assert_eq!(
        tags.sector_lines(first.set(), first.way()).unwrap(),
        vec![None, None, None, None]
    );
}

#[test]
fn sector_tags_evict_all_valid_subblocks_when_replacing_a_sector() {
    let mut tags = CacheSectorTags::new(config(1, 2));
    let sector_a = tags.insert(Address::new(0x0000)).unwrap();
    tags.insert(Address::new(0x0010)).unwrap();
    let sector_b = tags.insert(Address::new(0x0040)).unwrap();
    let same_sector_b = tags.insert(Address::new(0x0050)).unwrap();

    assert_eq!(sector_b.set(), same_sector_b.set());
    assert_eq!(sector_b.way(), same_sector_b.way());
    assert_eq!(same_sector_b.evicted_lines(), &[]);

    let replacement = tags.insert(Address::new(0x0080)).unwrap();

    assert_eq!(
        replacement.evicted_lines(),
        &[Address::new(0x0000), Address::new(0x0010)]
    );
    assert_eq!(replacement.set(), sector_a.set());
    assert_eq!(replacement.way(), sector_a.way());
    assert_eq!(tags.find(Address::new(0x0000)), None);
    assert_eq!(tags.find(Address::new(0x0010)), None);
    assert!(tags.find(Address::new(0x0040)).is_some());
    assert!(tags.find(Address::new(0x0050)).is_some());
    assert!(tags.find(Address::new(0x0080)).is_some());
}

#[test]
fn sector_tags_snapshot_restore_preserves_sector_and_policy_state() {
    let tag_config = config(1, 2);
    let mut tags = CacheSectorTags::new(tag_config.clone());
    tags.insert(Address::new(0x0000)).unwrap();
    tags.insert(Address::new(0x0010)).unwrap();
    tags.insert(Address::new(0x0040)).unwrap();
    let snapshot = tags.snapshot();

    let expected = tags.insert(Address::new(0x0080)).unwrap();
    assert_eq!(
        expected.evicted_lines(),
        &[Address::new(0x0000), Address::new(0x0010)]
    );

    tags.restore(&snapshot).unwrap();
    assert_eq!(tags.snapshot(), snapshot);
    let restored = tags.insert(Address::new(0x0080)).unwrap();
    assert_eq!(restored.evicted_lines(), expected.evicted_lines());

    let incompatible_config = config(2, 2);
    let mut incompatible = CacheSectorTags::new(incompatible_config.clone());
    assert_eq!(
        incompatible.restore(&snapshot),
        Err(CacheSectorTagsError::SnapshotConfigMismatch {
            expected: Box::new(incompatible_config),
            actual: Box::new(tag_config),
        })
    );
}

#[test]
fn sector_tags_access_updates_lru_state_without_allocating() {
    let mut tags = CacheSectorTags::new(config(1, 2));
    tags.insert(Address::new(0x0000)).unwrap();
    tags.insert(Address::new(0x0040)).unwrap();

    let access = tags.access(Address::new(0x0008)).unwrap().unwrap();
    assert_eq!(access.line(), Address::new(0x0000));
    assert_eq!(access.update().way(), 0);
    let replacement = tags.insert(Address::new(0x0080)).unwrap();

    assert_eq!(replacement.evicted_lines(), &[Address::new(0x0040)]);
    assert!(tags.find(Address::new(0x0000)).is_some());
    assert_eq!(tags.find(Address::new(0x0040)), None);
}

#[test]
fn sector_tags_ship_policy_uses_signatures_for_insert_and_access() {
    let config = CacheSectorTagsConfig::new(
        CacheReplacementPolicyKind::Ship {
            rrpv_bits: 2,
            hit_priority: true,
            shct_entries: 8,
            insertion_threshold_percent: 50,
        },
        line_layout(),
        1,
        2,
        4,
    )
    .unwrap();
    let mut tags = CacheSectorTags::new(config);

    assert_eq!(
        tags.insert(Address::new(0x0000)).unwrap_err(),
        CacheSectorTagsError::ReplacementPolicyState {
            source: rem6_cache::CacheReplacementPolicyError::SignatureRequired,
        }
    );

    let insert = tags.insert_with_signature(Address::new(0x0000), 7).unwrap();
    assert_eq!(insert.sector_base(), Address::new(0x0000));
    assert!(tags
        .access_with_signature(Address::new(0x0008), 7)
        .unwrap()
        .is_some());
}

#[test]
fn sector_tags_restore_rejects_duplicate_sector_bases() {
    let tag_config = config(1, 2);
    let mut tags = CacheSectorTags::new(tag_config.clone());
    tags.insert(Address::new(0x0000)).unwrap();
    tags.insert(Address::new(0x0040)).unwrap();
    let snapshot = tags.snapshot();
    let replacement = snapshot.sets()[0].replacement().clone();
    let corrupt = CacheSectorTagsSnapshot::new(
        tag_config,
        vec![CacheSectorTagSetSnapshot::new(
            vec![
                CacheSectorTagEntrySnapshot::new(
                    Some(Address::new(0x0000)),
                    vec![Some(Address::new(0x0000)), None, None, None],
                ),
                CacheSectorTagEntrySnapshot::new(
                    Some(Address::new(0x0000)),
                    vec![None, Some(Address::new(0x0010)), None, None],
                ),
            ],
            replacement,
        )],
    );

    assert_eq!(
        tags.restore(&corrupt),
        Err(CacheSectorTagsError::SnapshotDuplicateSector {
            sector_base: Address::new(0x0000),
        })
    );
}

#[test]
fn sector_tags_skewed_lru_uses_comparable_cross_set_recency() {
    let config = CacheSectorTagsConfig::new_with_indexing(
        CacheReplacementPolicyKind::Lru,
        CacheIndexingPolicyKind::SkewedAssociative,
        line_layout(),
        8,
        4,
        4,
    )
    .unwrap();
    let mut tags = CacheSectorTags::new(config.clone());
    let mut resident_order = BTreeMap::new();

    for sector_index in 0..64 {
        let sector = sector_address(&config, sector_index);
        let insert = tags.insert(sector).unwrap();
        for evicted in insert.evicted_lines() {
            resident_order.remove(evicted);
        }
        resident_order.insert(insert.sector_base(), sector_index);
    }
    let snapshot = tags.snapshot();

    let mut exercised = false;
    for target_index in 64..512 {
        let target = sector_address(&config, target_index);
        if tags.find(target).is_some() {
            continue;
        }
        let candidates = candidate_sector_bases(&tags, target);
        if candidates.len() != config.ways() {
            continue;
        }
        let expected = candidates
            .iter()
            .min_by_key(|(_, sector_base)| resident_order[sector_base])
            .map(|(_, sector_base)| *sector_base)
            .unwrap();

        tags.restore(&snapshot).unwrap();
        let replacement = tags.insert(target).unwrap();
        assert_eq!(replacement.evicted_lines().first().copied(), Some(expected));
        exercised = true;
        break;
    }

    assert!(exercised);
}

#[test]
fn sector_tags_skewed_bip_keeps_lip_insertions_globally_replaceable() {
    let config = CacheSectorTagsConfig::new_with_indexing(
        CacheReplacementPolicyKind::Bip { btp_percent: 0 },
        CacheIndexingPolicyKind::SkewedAssociative,
        line_layout(),
        8,
        4,
        4,
    )
    .unwrap();
    let mut tags = CacheSectorTags::new(config.clone());
    let mut resident_order = BTreeMap::new();

    for sector_index in 0..64 {
        let sector = sector_address(&config, sector_index);
        let insert = tags.insert(sector).unwrap();
        for evicted in insert.evicted_lines() {
            resident_order.remove(evicted);
        }
        resident_order.insert(insert.sector_base(), sector_index);
    }
    let snapshot = tags.snapshot();

    let mut exercised = false;
    for target_index in 64..512 {
        let target = sector_address(&config, target_index);
        if tags.find(target).is_some() {
            continue;
        }
        let candidates = candidate_sector_bases(&tags, target);
        if candidates.len() != config.ways() {
            continue;
        }
        let expected = candidates
            .iter()
            .min_by_key(|(_, sector_base)| resident_order[sector_base])
            .map(|(_, sector_base)| *sector_base)
            .unwrap();

        tags.restore(&snapshot).unwrap();
        let replacement = tags.insert(target).unwrap();
        assert_eq!(replacement.evicted_lines().first().copied(), Some(expected));
        exercised = true;
        break;
    }

    assert!(exercised);
}

#[test]
fn sector_tags_skewed_second_chance_skips_referenced_candidate() {
    let config = CacheSectorTagsConfig::new_with_indexing(
        CacheReplacementPolicyKind::SecondChance,
        CacheIndexingPolicyKind::SkewedAssociative,
        line_layout(),
        8,
        4,
        4,
    )
    .unwrap();
    let mut tags = CacheSectorTags::new(config.clone());
    let mut resident_order = BTreeMap::new();

    for sector_index in 0..64 {
        let sector = sector_address(&config, sector_index);
        let insert = tags.insert(sector).unwrap();
        for evicted in insert.evicted_lines() {
            resident_order.remove(evicted);
        }
        resident_order.insert(insert.sector_base(), sector_index);
    }

    let mut exercised = false;
    for target_index in 64..512 {
        let target = sector_address(&config, target_index);
        if tags.find(target).is_some() {
            continue;
        }
        let candidates = candidate_sector_bases(&tags, target);
        if candidates.len() != config.ways() {
            continue;
        }
        let oldest = candidates
            .iter()
            .min_by_key(|(_, sector_base)| resident_order[sector_base])
            .map(|(_, sector_base)| *sector_base)
            .unwrap();
        let next_oldest = candidates
            .iter()
            .filter(|(_, sector_base)| *sector_base != oldest)
            .min_by_key(|(_, sector_base)| resident_order[sector_base])
            .map(|(_, sector_base)| *sector_base)
            .unwrap();

        let snapshot = tags.snapshot();
        tags.access(oldest).unwrap().unwrap();
        let replacement = tags.insert(target).unwrap();
        assert_eq!(
            replacement.evicted_lines().first().copied(),
            Some(next_oldest)
        );
        exercised = true;

        tags.restore(&snapshot).unwrap();
        break;
    }

    assert!(exercised);
}

#[test]
fn sector_tags_support_skewed_indexing_without_splitting_sector_subblocks() {
    let config = CacheSectorTagsConfig::new_with_indexing(
        CacheReplacementPolicyKind::Lru,
        CacheIndexingPolicyKind::SkewedAssociative,
        line_layout(),
        8,
        4,
        4,
    )
    .unwrap();
    let mut tags = CacheSectorTags::new(config);

    let first = tags.insert(Address::new(0x0080)).unwrap();
    let second = tags.insert(Address::new(0x00b0)).unwrap();

    assert!(!second.new_sector());
    assert_eq!(first.sector_base(), Address::new(0x0080));
    assert_eq!(second.sector_base(), Address::new(0x0080));
    assert_eq!(first.set(), second.set());
    assert_eq!(first.way(), second.way());
    assert_eq!(second.offset(), 3);
    assert_eq!(tags.valid_sector_count(), 1);
    assert_eq!(tags.valid_line_count(), 2);
}

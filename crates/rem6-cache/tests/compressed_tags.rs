use rem6_cache::{
    CacheCompressedTagEntrySnapshot, CacheCompressedTagLine, CacheCompressedTagSetSnapshot,
    CacheCompressedTags, CacheCompressedTagsConfig, CacheCompressedTagsError,
    CacheCompressedTagsSnapshot, CacheIndexingLocation, CacheIndexingPolicyKind,
    CacheReplacementPolicyKind,
};
use rem6_memory::{Address, CacheLineLayout};
use std::collections::BTreeMap;

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn config(sets: usize, ways: usize, max_compression_ratio: usize) -> CacheCompressedTagsConfig {
    CacheCompressedTagsConfig::new(
        CacheReplacementPolicyKind::Lru,
        line_layout(),
        sets,
        ways,
        max_compression_ratio,
    )
    .unwrap()
}

fn superblock_address(config: &CacheCompressedTagsConfig, superblock_index: u64) -> Address {
    Address::new(superblock_index * config.superblock_layout().bytes())
}

fn candidate_superblock_bases(
    tags: &CacheCompressedTags,
    target: Address,
) -> Vec<(CacheIndexingLocation, Address)> {
    let snapshot = tags.snapshot();
    tags.config()
        .indexing_config()
        .candidate_locations(target)
        .into_iter()
        .filter_map(|location| {
            snapshot.sets()[location.set()].entries()[location.way()]
                .superblock_base()
                .map(|superblock_base| (location, superblock_base))
        })
        .collect()
}

#[test]
fn compressed_tags_reject_invalid_gem5_superblock_shapes() {
    assert_eq!(
        CacheCompressedTagsConfig::new(CacheReplacementPolicyKind::Lru, line_layout(), 1, 1, 0),
        Err(CacheCompressedTagsError::ZeroMaxCompressionRatio)
    );
    assert_eq!(
        CacheCompressedTagsConfig::new(CacheReplacementPolicyKind::Lru, line_layout(), 1, 1, 3),
        Err(CacheCompressedTagsError::MaxCompressionRatioNotPowerOfTwo { ratio: 3 })
    );
    assert_eq!(
        CacheCompressedTagsConfig::new(
            CacheReplacementPolicyKind::Lru,
            CacheLineLayout::new(2).unwrap(),
            1,
            1,
            2,
        ),
        Err(CacheCompressedTagsError::LineSizeTooSmall { bytes: 2 })
    );
    assert_eq!(
        CacheCompressedTagsConfig::new(
            CacheReplacementPolicyKind::WeightedLru,
            line_layout(),
            1,
            1,
            2,
        ),
        Err(CacheCompressedTagsError::UnsupportedReplacementPolicy {
            kind: CacheReplacementPolicyKind::WeightedLru,
        })
    );
}

#[test]
fn compressed_tags_coallocate_only_while_superblock_capacity_fits() {
    let mut tags = CacheCompressedTags::new(config(1, 1, 4));

    let first = tags.insert(Address::new(0x1000), 64).unwrap();
    assert!(first.new_superblock());
    assert!(!first.co_allocated());
    assert_eq!(first.compression_factor(), 2);
    assert!(first.compressed());
    assert_eq!(first.evicted_lines(), &[]);

    let second = tags.insert(Address::new(0x1010), 64).unwrap();
    assert!(!second.new_superblock());
    assert!(second.co_allocated());
    assert_eq!(second.compression_factor(), 2);
    assert_eq!(second.evicted_lines(), &[]);
    assert_eq!(tags.valid_superblock_count(), 1);
    assert_eq!(tags.valid_line_count(), 2);

    let third = tags.insert(Address::new(0x1020), 64).unwrap();
    assert!(third.new_superblock());
    assert!(!third.co_allocated());
    assert_eq!(
        third.evicted_lines(),
        &[Address::new(0x1000), Address::new(0x1010)]
    );
    assert_eq!(tags.resident_lines(), vec![Address::new(0x1020)]);
    assert_eq!(tags.find(Address::new(0x1000)), None);
    assert_eq!(tags.find(Address::new(0x1010)), None);
    assert_eq!(tags.find(Address::new(0x1020)).unwrap().offset(), 2);
}

#[test]
fn compressed_tags_replace_matching_superblock_when_capacity_fails() {
    let mut tags = CacheCompressedTags::new(config(1, 2, 4));

    tags.insert(Address::new(0x1000), 64).unwrap();
    tags.insert(Address::new(0x1010), 64).unwrap();
    tags.insert(Address::new(0x2000), 64).unwrap();
    tags.access(Address::new(0x1000)).unwrap();

    let replacement = tags.insert(Address::new(0x1020), 64).unwrap();
    assert!(replacement.new_superblock());
    assert!(!replacement.co_allocated());
    assert_eq!(
        replacement.evicted_lines(),
        &[Address::new(0x1000), Address::new(0x1010)]
    );
    assert_eq!(
        tags.resident_lines(),
        vec![Address::new(0x1020), Address::new(0x2000)]
    );
    assert_eq!(tags.valid_superblock_count(), 2);
    assert_eq!(
        tags.superblock_lines(replacement.set(), replacement.way())
            .unwrap(),
        vec![None, None, Some(Address::new(0x1020)), None]
    );
}

#[test]
fn compressed_tags_uncompressed_superblocks_do_not_coallocate() {
    let mut tags = CacheCompressedTags::new(config(1, 1, 4));

    let first = tags.insert(Address::new(0x2000), 128).unwrap();
    assert_eq!(first.compression_factor(), 1);
    assert!(!first.compressed());

    let second = tags.insert(Address::new(0x2010), 32).unwrap();
    assert!(second.new_superblock());
    assert!(!second.co_allocated());
    assert_eq!(second.compression_factor(), 4);
    assert_eq!(second.evicted_lines(), &[Address::new(0x2000)]);
    assert_eq!(tags.find(Address::new(0x2000)), None);
    assert!(tags.find(Address::new(0x2010)).unwrap().compressed());
}

#[test]
fn compressed_tags_invalidate_preserves_superblock_until_last_line() {
    let mut tags = CacheCompressedTags::new(config(1, 1, 4));

    tags.insert(Address::new(0x3000), 32).unwrap();
    tags.insert(Address::new(0x3010), 32).unwrap();
    assert_eq!(tags.valid_superblock_count(), 1);
    assert_eq!(tags.valid_line_count(), 2);

    let partial = tags.invalidate(Address::new(0x3000)).unwrap().unwrap();
    assert!(partial.superblock_still_valid());
    assert_eq!(partial.compression_factor(), 4);
    assert_eq!(tags.valid_superblock_count(), 1);
    assert_eq!(tags.valid_line_count(), 1);
    assert_eq!(tags.find(Address::new(0x3000)), None);
    assert!(tags.find(Address::new(0x3010)).is_some());

    let final_invalidate = tags.invalidate(Address::new(0x3010)).unwrap().unwrap();
    assert!(!final_invalidate.superblock_still_valid());
    assert_eq!(final_invalidate.compression_factor(), 1);
    assert_eq!(tags.valid_superblock_count(), 0);
    assert_eq!(tags.valid_line_count(), 0);
}

#[test]
fn compressed_tags_snapshot_restore_preserves_replacement_state() {
    let tag_config = config(1, 2, 2);
    let mut tags = CacheCompressedTags::new(tag_config.clone());

    tags.insert(Address::new(0x4000), 64).unwrap();
    tags.insert(Address::new(0x4020), 64).unwrap();
    tags.access(Address::new(0x4000)).unwrap();
    let snapshot = tags.snapshot();

    let expected = tags.insert(Address::new(0x4040), 64).unwrap();
    assert_eq!(expected.evicted_lines(), &[Address::new(0x4020)]);
    assert!(tags.find(Address::new(0x4000)).is_some());
    assert!(tags.find(Address::new(0x4020)).is_none());

    tags.restore(&snapshot).unwrap();
    assert_eq!(tags.snapshot(), snapshot);
    let restored = tags.insert(Address::new(0x4040), 64).unwrap();
    assert_eq!(restored.evicted_lines(), &[Address::new(0x4020)]);
    assert!(tags.find(Address::new(0x4000)).is_some());
    assert!(tags.find(Address::new(0x4020)).is_none());
}

#[test]
fn compressed_tags_skewed_lru_uses_comparable_cross_set_recency() {
    let tag_config = CacheCompressedTagsConfig::new_with_indexing(
        CacheReplacementPolicyKind::Lru,
        CacheIndexingPolicyKind::SkewedAssociative,
        line_layout(),
        8,
        4,
        4,
    )
    .unwrap();
    let mut tags = CacheCompressedTags::new(tag_config.clone());
    let mut resident_order = BTreeMap::new();

    for superblock_index in 0..64 {
        let superblock = superblock_address(&tag_config, superblock_index);
        let insert = tags.insert(superblock, 64).unwrap();
        for evicted in insert.evicted_lines() {
            resident_order.remove(evicted);
        }
        resident_order.insert(insert.superblock_base(), superblock_index);
    }
    let snapshot = tags.snapshot();

    let mut exercised = false;
    for target_index in 64..512 {
        let target = superblock_address(&tag_config, target_index);
        if tags.find(target).is_some() {
            continue;
        }
        let candidates = candidate_superblock_bases(&tags, target);
        if candidates.len() != tag_config.ways() {
            continue;
        }
        let expected = candidates
            .iter()
            .min_by_key(|(_, superblock_base)| resident_order[superblock_base])
            .map(|(_, superblock_base)| *superblock_base)
            .unwrap();

        tags.restore(&snapshot).unwrap();
        let replacement = tags.insert(target, 64).unwrap();
        assert_eq!(replacement.evicted_lines().first().copied(), Some(expected));
        exercised = true;
        break;
    }

    assert!(exercised);
}

#[test]
fn compressed_tags_restore_rejects_superblock_capacity_exceeded() {
    let tag_config = config(1, 1, 4);
    let tags = CacheCompressedTags::new(tag_config.clone());
    let replacement = tags.snapshot().sets()[0].replacement().clone();
    let corrupt = CacheCompressedTagsSnapshot::new(
        tag_config,
        vec![CacheCompressedTagSetSnapshot::new(
            vec![CacheCompressedTagEntrySnapshot::new(
                Some(Address::new(0x5000)),
                vec![
                    Some(CacheCompressedTagLine::new(Address::new(0x5000), 64, true)),
                    Some(CacheCompressedTagLine::new(Address::new(0x5010), 64, true)),
                    Some(CacheCompressedTagLine::new(Address::new(0x5020), 64, true)),
                    None,
                ],
                2,
            )],
            replacement,
        )],
    );
    let mut restored = CacheCompressedTags::new(config(1, 1, 4));

    assert_eq!(
        restored.restore(&corrupt),
        Err(
            CacheCompressedTagsError::SnapshotSuperblockCapacityExceeded {
                set: 0,
                way: 0,
                valid_blocks: 3,
                compression_factor: 2,
            }
        )
    );
}

#[test]
fn compressed_tags_restore_rejects_oversized_compressed_line() {
    let tag_config = config(1, 1, 4);
    let tags = CacheCompressedTags::new(tag_config.clone());
    let replacement = tags.snapshot().sets()[0].replacement().clone();
    let corrupt = CacheCompressedTagsSnapshot::new(
        tag_config,
        vec![CacheCompressedTagSetSnapshot::new(
            vec![CacheCompressedTagEntrySnapshot::new(
                Some(Address::new(0x6000)),
                vec![
                    Some(CacheCompressedTagLine::new(Address::new(0x6000), 96, true)),
                    None,
                    None,
                    None,
                ],
                2,
            )],
            replacement,
        )],
    );
    let mut restored = CacheCompressedTags::new(config(1, 1, 4));

    assert_eq!(
        restored.restore(&corrupt),
        Err(CacheCompressedTagsError::SnapshotCompressedSizeTooLarge {
            set: 0,
            way: 0,
            offset: 0,
            compressed_size_bits: 96,
            maximum_bits: 64,
        })
    );
}

#[test]
fn compressed_tags_restore_accepts_live_rounded_up_compression_factor() {
    let tag_config = config(1, 1, 4);
    let mut tags = CacheCompressedTags::new(tag_config.clone());

    let insert = tags.insert(Address::new(0x7000), 33).unwrap();
    assert_eq!(insert.compression_factor(), 4);
    let snapshot = tags.snapshot();

    let mut restored = CacheCompressedTags::new(tag_config);
    assert_eq!(restored.restore(&snapshot), Ok(()));
    assert_eq!(restored.snapshot(), snapshot);
}

#[test]
fn compressed_tags_restore_rejects_empty_superblock_metadata() {
    let tag_config = config(1, 1, 4);
    let tags = CacheCompressedTags::new(tag_config.clone());
    let replacement = tags.snapshot().sets()[0].replacement().clone();
    let corrupt = CacheCompressedTagsSnapshot::new(
        tag_config,
        vec![CacheCompressedTagSetSnapshot::new(
            vec![CacheCompressedTagEntrySnapshot::new(
                Some(Address::new(0x8000)),
                vec![None, None, None, None],
                2,
            )],
            replacement,
        )],
    );
    let mut restored = CacheCompressedTags::new(config(1, 1, 4));

    assert_eq!(
        restored.restore(&corrupt),
        Err(CacheCompressedTagsError::SnapshotEmptySuperblock {
            set: 0,
            way: 0,
            compression_factor: 2,
            has_superblock_base: true,
        })
    );
}

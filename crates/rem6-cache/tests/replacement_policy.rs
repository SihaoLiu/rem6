use rem6_cache::{
    CacheReplacementDirectory, CacheReplacementDirectoryConfig, CacheReplacementPolicyConfig,
    CacheReplacementPolicyError, CacheReplacementPolicyKind, ReplacementEntry, ReplacementSet,
};
use rem6_memory::{Address, CacheLineLayout};

const OVERSIZED_VECTOR_LENGTH: usize = isize::MAX as usize + 1;
const REPLACEMENT_ENTRY_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<ReplacementEntry>() + 1;
const DIRECTORY_LINE_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<Option<Address>>() + 1;

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

#[test]
fn replacement_set_lru_fifo_mru_and_lfu_follow_gem5_victim_rules() {
    let mut lru = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Lru, 4).unwrap(),
    );
    for way in 0..4 {
        lru.reset(way).unwrap();
    }
    lru.touch(0).unwrap();
    lru.touch(2).unwrap();
    assert_eq!(lru.victim([0, 1, 2, 3]).unwrap().way(), 1);
    lru.invalidate(2).unwrap();
    assert_eq!(lru.victim([0, 1, 2, 3]).unwrap().way(), 2);

    let mut fifo = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Fifo, 4).unwrap(),
    );
    for way in 0..4 {
        fifo.reset(way).unwrap();
    }
    fifo.touch(0).unwrap();
    fifo.touch(1).unwrap();
    assert_eq!(fifo.victim([0, 1, 2, 3]).unwrap().way(), 0);

    let mut mru = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Mru, 4).unwrap(),
    );
    for way in 0..4 {
        mru.reset(way).unwrap();
    }
    mru.touch(1).unwrap();
    mru.touch(3).unwrap();
    assert_eq!(mru.victim([0, 1, 2, 3]).unwrap().way(), 3);
    mru.invalidate(0).unwrap();
    assert_eq!(mru.victim([0, 1, 2, 3]).unwrap().way(), 0);

    let mut lfu = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Lfu, 4).unwrap(),
    );
    for way in 0..4 {
        lfu.reset(way).unwrap();
    }
    lfu.touch(0).unwrap();
    lfu.touch(0).unwrap();
    lfu.touch(1).unwrap();
    assert_eq!(lfu.victim([0, 1, 2, 3]).unwrap().way(), 2);
    assert_eq!(lfu.entry(0).unwrap().reference_count(), 3);
    assert_eq!(lfu.entry(2).unwrap().reference_count(), 1);
}

#[test]
fn replacement_set_brrip_uses_valid_bits_rrpv_aging_and_hit_priority() {
    let mut distant = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(
            CacheReplacementPolicyKind::Brrip {
                rrpv_bits: 2,
                hit_priority: false,
                btp_percent: 0,
            },
            4,
        )
        .unwrap(),
    );
    for way in 0..4 {
        distant.reset(way).unwrap();
    }
    assert_eq!(distant.entry(0).unwrap().rrpv(), 3);
    distant.touch(0).unwrap();
    assert_eq!(distant.entry(0).unwrap().rrpv(), 2);
    assert_eq!(distant.victim([0, 1, 2, 3]).unwrap().way(), 1);
    distant.invalidate(2).unwrap();
    assert_eq!(distant.victim([0, 1, 2, 3]).unwrap().way(), 2);

    let mut long = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(
            CacheReplacementPolicyKind::Brrip {
                rrpv_bits: 2,
                hit_priority: true,
                btp_percent: 100,
            },
            4,
        )
        .unwrap(),
    );
    long.reset(0).unwrap();
    assert_eq!(long.entry(0).unwrap().rrpv(), 2);
    long.touch(0).unwrap();
    assert_eq!(long.entry(0).unwrap().rrpv(), 0);
}

#[test]
fn replacement_set_bip_uses_deterministic_bimodal_insertion() {
    let mut never_mru = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Bip { btp_percent: 0 }, 4)
            .unwrap(),
    );
    for way in 0..4 {
        never_mru.reset(way).unwrap();
    }
    assert_eq!(never_mru.entry(0).unwrap().last_touch_tick(), 1);
    assert_eq!(never_mru.entry(3).unwrap().last_touch_tick(), 1);
    never_mru.touch(0).unwrap();
    assert_eq!(never_mru.entry(0).unwrap().last_touch_tick(), 2);
    assert_eq!(never_mru.victim([0, 1, 2, 3]).unwrap().way(), 1);

    let mut always_mru = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Bip { btp_percent: 100 }, 4)
            .unwrap(),
    );
    for way in 0..4 {
        always_mru.reset(way).unwrap();
    }
    assert_eq!(always_mru.entry(0).unwrap().last_touch_tick(), 1);
    assert_eq!(always_mru.entry(3).unwrap().last_touch_tick(), 4);
    assert_eq!(always_mru.victim([0, 1, 2, 3]).unwrap().way(), 0);

    let mut periodic = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Bip { btp_percent: 25 }, 4)
            .unwrap(),
    );
    for way in 0..4 {
        periodic.reset(way).unwrap();
    }
    assert_eq!(periodic.entry(0).unwrap().last_touch_tick(), 1);
    assert_eq!(periodic.entry(2).unwrap().last_touch_tick(), 1);
    assert_eq!(periodic.entry(3).unwrap().last_touch_tick(), 2);

    let mut restore = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Bip { btp_percent: 50 }, 2)
            .unwrap(),
    );
    restore.reset(0).unwrap();
    let snapshot = restore.snapshot();
    restore.reset(1).unwrap();
    restore.restore(&snapshot).unwrap();
    restore.reset(1).unwrap();
    assert_eq!(restore.entry(1).unwrap().last_touch_tick(), 2);
}

#[test]
fn replacement_set_ship_uses_signature_history_for_rrip_insertion() {
    let mut set = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(
            CacheReplacementPolicyKind::Ship {
                rrpv_bits: 2,
                hit_priority: true,
                shct_entries: 4,
                insertion_threshold_percent: 1,
            },
            4,
        )
        .unwrap(),
    );

    set.reset_with_signature(0, 10).unwrap();
    assert_eq!(set.entry(0).unwrap().ship_signature(), 2);
    assert!(!set.entry(0).unwrap().ship_re_referenced());
    assert_eq!(set.entry(0).unwrap().rrpv(), 3);

    set.touch_with_signature(0, 10).unwrap();
    assert_eq!(set.ship_signature_counters().unwrap(), &[0, 0, 1, 0]);
    assert!(set.entry(0).unwrap().ship_re_referenced());
    assert_eq!(set.entry(0).unwrap().rrpv(), 0);

    set.reset_with_signature(1, 10).unwrap();
    set.reset_with_signature(2, 11).unwrap();
    assert_eq!(set.entry(1).unwrap().ship_signature(), 2);
    assert_eq!(set.entry(1).unwrap().rrpv(), 2);
    assert_eq!(set.entry(2).unwrap().ship_signature(), 3);
    assert_eq!(set.entry(2).unwrap().rrpv(), 3);
    assert_eq!(set.victim([0, 1, 2]).unwrap().way(), 2);
}

#[test]
fn replacement_set_ship_detrains_unused_insertions_and_snapshots_predictor() {
    let mut set = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(
            CacheReplacementPolicyKind::Ship {
                rrpv_bits: 2,
                hit_priority: false,
                shct_entries: 2,
                insertion_threshold_percent: 1,
            },
            2,
        )
        .unwrap(),
    );

    set.reset_with_signature(0, 1).unwrap();
    set.touch_with_signature(0, 1).unwrap();
    assert_eq!(set.entry(0).unwrap().rrpv(), 2);
    assert_eq!(set.ship_signature_counters().unwrap(), &[0, 1]);
    let snapshot = set.snapshot();

    set.reset_with_signature(1, 1).unwrap();
    assert_eq!(set.entry(1).unwrap().rrpv(), 2);
    set.invalidate(1).unwrap();
    assert_eq!(set.ship_signature_counters().unwrap(), &[0, 0]);

    set.restore(&snapshot).unwrap();
    assert_eq!(set.ship_signature_counters().unwrap(), &[0, 1]);
    set.reset_with_signature(1, 1).unwrap();
    assert_eq!(set.entry(1).unwrap().rrpv(), 2);
}

#[test]
fn replacement_set_second_chance_requeues_touched_fifo_victims() {
    let mut set = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::SecondChance, 4).unwrap(),
    );
    for way in 0..4 {
        set.reset(way).unwrap();
    }
    set.touch(0).unwrap();
    set.touch(1).unwrap();

    assert!(set.entry(0).unwrap().second_chance());
    assert!(set.entry(1).unwrap().second_chance());
    assert_eq!(set.victim([0, 1, 2, 3]).unwrap().way(), 2);
    assert!(!set.entry(0).unwrap().second_chance());
    assert!(!set.entry(1).unwrap().second_chance());
    assert_eq!(set.entry(0).unwrap().insertion_tick(), 5);
    assert_eq!(set.entry(1).unwrap().insertion_tick(), 6);
    assert_eq!(set.entry(2).unwrap().insertion_tick(), 3);

    set.touch(3).unwrap();
    set.invalidate(3).unwrap();
    assert!(!set.entry(3).unwrap().second_chance());
    assert_eq!(set.entry(3).unwrap().insertion_tick(), 0);
    assert_eq!(set.victim([0, 1, 2, 3]).unwrap().way(), 3);
}

#[test]
fn replacement_set_tree_plru_tracks_shared_tree_state_per_set() {
    let mut tree = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::TreePlru, 4).unwrap(),
    );
    for way in 0..4 {
        tree.reset(way).unwrap();
    }

    assert_eq!(tree.tree_bits(), Some(&[false, false, false][..]));
    assert_eq!(tree.victim([0, 1, 2, 3]).unwrap().way(), 0);
    tree.touch(0).unwrap();
    assert_eq!(tree.tree_bits(), Some(&[true, true, false][..]));
    assert_eq!(tree.victim([0, 1, 2, 3]).unwrap().way(), 2);
    tree.invalidate(0).unwrap();
    assert_eq!(tree.tree_bits(), Some(&[false, false, false][..]));
    assert_eq!(tree.victim([0, 1, 2, 3]).unwrap().way(), 0);
}

#[test]
fn replacement_set_snapshot_restore_preserves_entries_tree_and_counters() {
    let mut set = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::TreePlru, 4).unwrap(),
    );
    for way in 0..4 {
        set.reset(way).unwrap();
    }
    set.touch(2).unwrap();
    let snapshot = set.snapshot();

    set.invalidate(0).unwrap();
    set.touch(1).unwrap();
    set.restore(&snapshot).unwrap();

    assert_eq!(set.snapshot(), snapshot);
}

#[test]
fn replacement_set_rejects_bad_configs_candidates_and_snapshots() {
    assert_eq!(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Lru, 0),
        Err(CacheReplacementPolicyError::ZeroWays)
    );
    assert_eq!(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Lru, OVERSIZED_VECTOR_LENGTH),
        Err(CacheReplacementPolicyError::VectorLengthTooLarge {
            field: "ways",
            length: OVERSIZED_VECTOR_LENGTH,
            maximum: isize::MAX as usize / std::mem::size_of::<ReplacementEntry>(),
        })
    );
    assert_eq!(
        CacheReplacementPolicyConfig::new(
            CacheReplacementPolicyKind::Lru,
            REPLACEMENT_ENTRY_BYTE_OVERFLOW_LENGTH,
        ),
        Err(CacheReplacementPolicyError::VectorLengthTooLarge {
            field: "ways",
            length: REPLACEMENT_ENTRY_BYTE_OVERFLOW_LENGTH,
            maximum: isize::MAX as usize / std::mem::size_of::<ReplacementEntry>(),
        })
    );
    assert!(matches!(
        CacheReplacementDirectoryConfig::new(
            CacheReplacementPolicyKind::Lru,
            line_layout(),
            OVERSIZED_VECTOR_LENGTH,
            4,
        ),
        Err(CacheReplacementPolicyError::VectorLengthTooLarge {
            field: "sets",
            length: OVERSIZED_VECTOR_LENGTH,
            ..
        })
    ));
    assert!(CacheReplacementDirectoryConfig::new(
        CacheReplacementPolicyKind::Lru,
        line_layout(),
        4,
        DIRECTORY_LINE_BYTE_OVERFLOW_LENGTH,
    )
    .is_err());
    assert!(CacheReplacementDirectoryConfig::new(
        CacheReplacementPolicyKind::Lru,
        line_layout(),
        DIRECTORY_LINE_BYTE_OVERFLOW_LENGTH,
        4,
    )
    .is_err());
    assert_eq!(
        CacheReplacementPolicyConfig::new(
            CacheReplacementPolicyKind::Ship {
                rrpv_bits: 2,
                hit_priority: true,
                shct_entries: OVERSIZED_VECTOR_LENGTH,
                insertion_threshold_percent: 1,
            },
            4,
        ),
        Err(CacheReplacementPolicyError::VectorLengthTooLarge {
            field: "SHCT entries",
            length: OVERSIZED_VECTOR_LENGTH,
            maximum: isize::MAX as usize,
        })
    );
    assert_eq!(
        CacheReplacementPolicyConfig::new(
            CacheReplacementPolicyKind::Brrip {
                rrpv_bits: 0,
                hit_priority: false,
                btp_percent: 0,
            },
            4,
        ),
        Err(CacheReplacementPolicyError::RrpvBitsOutOfRange { bits: 0 })
    );
    assert_eq!(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Bip { btp_percent: 101 }, 4,),
        Err(CacheReplacementPolicyError::BtpOutOfRange { percent: 101 })
    );
    assert_eq!(
        CacheReplacementPolicyConfig::new(
            CacheReplacementPolicyKind::Ship {
                rrpv_bits: 8,
                hit_priority: true,
                shct_entries: 4,
                insertion_threshold_percent: 1,
            },
            4,
        ),
        Err(CacheReplacementPolicyError::RrpvBitsOutOfRange { bits: 8 })
    );
    assert_eq!(
        CacheReplacementPolicyConfig::new(
            CacheReplacementPolicyKind::Ship {
                rrpv_bits: 2,
                hit_priority: true,
                shct_entries: 0,
                insertion_threshold_percent: 1,
            },
            4,
        ),
        Err(CacheReplacementPolicyError::SignatureHistoryTableEmpty)
    );
    assert_eq!(
        CacheReplacementPolicyConfig::new(
            CacheReplacementPolicyKind::Ship {
                rrpv_bits: 2,
                hit_priority: true,
                shct_entries: 4,
                insertion_threshold_percent: 101,
            },
            4,
        ),
        Err(CacheReplacementPolicyError::InsertionThresholdOutOfRange { percent: 101 })
    );
    let mut ship = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(
            CacheReplacementPolicyKind::Ship {
                rrpv_bits: 2,
                hit_priority: true,
                shct_entries: 4,
                insertion_threshold_percent: 1,
            },
            2,
        )
        .unwrap(),
    );
    assert_eq!(
        ship.reset(0),
        Err(CacheReplacementPolicyError::SignatureRequired)
    );
    assert_eq!(
        ship.touch(0),
        Err(CacheReplacementPolicyError::SignatureRequired)
    );
    assert_eq!(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::TreePlru, 3),
        Err(CacheReplacementPolicyError::TreePlruWaysNotPowerOfTwo { ways: 3 })
    );

    let mut set = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Lru, 2).unwrap(),
    );
    assert_eq!(
        set.victim([]),
        Err(CacheReplacementPolicyError::NoCandidates)
    );
    assert_eq!(
        set.touch(2),
        Err(CacheReplacementPolicyError::UnknownWay { way: 2, ways: 2 })
    );

    let other = ReplacementSet::new(
        CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Fifo, 2).unwrap(),
    );
    assert_eq!(
        set.restore(&other.snapshot()),
        Err(CacheReplacementPolicyError::SnapshotConfigMismatch {
            expected: Box::new(
                CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Lru, 2).unwrap()
            ),
            actual: Box::new(
                CacheReplacementPolicyConfig::new(CacheReplacementPolicyKind::Fifo, 2).unwrap()
            ),
        })
    );
}

#[test]
fn replacement_directory_tracks_set_way_ownership_and_lru_victims() {
    let config =
        CacheReplacementDirectoryConfig::new(CacheReplacementPolicyKind::Lru, line_layout(), 2, 2)
            .unwrap();
    let mut directory = CacheReplacementDirectory::new(config);

    let first = directory.install(Address::new(0x0000)).unwrap();
    assert_eq!(first.set(), 0);
    assert_eq!(first.way(), 0);
    assert_eq!(first.evicted_line(), None);
    assert_eq!(directory.way_for(Address::new(0x0000)), Some((0, 0)));

    let second = directory.install(Address::new(0x0020)).unwrap();
    assert_eq!(second.set(), 0);
    assert_eq!(second.way(), 1);
    assert_eq!(
        directory.set_lines(0).unwrap(),
        vec![Some(Address::new(0x0000)), Some(Address::new(0x0020)),]
    );

    directory.touch(Address::new(0x0000)).unwrap();
    let third = directory.install(Address::new(0x0040)).unwrap();
    assert_eq!(third.set(), 0);
    assert_eq!(third.way(), 1);
    assert_eq!(third.evicted_line(), Some(Address::new(0x0020)));
    assert_eq!(directory.way_for(Address::new(0x0020)), None);
    assert_eq!(directory.way_for(Address::new(0x0040)), Some((0, 1)));
    assert_eq!(
        directory.resident_lines(),
        vec![Address::new(0x0000), Address::new(0x0040)]
    );

    let other_set = directory.install(Address::new(0x0010)).unwrap();
    assert_eq!(other_set.set(), 1);
    assert_eq!(other_set.way(), 0);
    assert_eq!(
        directory.set_lines(1).unwrap(),
        vec![Some(Address::new(0x0010)), None]
    );
}

#[test]
fn replacement_directory_moves_resident_line_without_reinterpreting_tag() {
    let line = Address::new(0x1234_5678_9abc_def0);
    let canonical_line = line_layout().line_address(line);
    let mut directory = CacheReplacementDirectory::new(
        CacheReplacementDirectoryConfig::new(CacheReplacementPolicyKind::Lru, line_layout(), 1, 2)
            .unwrap(),
    );
    let install = directory.install(line).unwrap();
    assert_eq!(install.set(), 0);
    assert_eq!(install.way(), 0);

    let relocation = directory.move_resident_line(line, 0, 1).unwrap();

    assert_eq!(relocation.line(), canonical_line);
    assert_eq!(relocation.source_set(), 0);
    assert_eq!(relocation.source_way(), 0);
    assert_eq!(relocation.destination_set(), 0);
    assert_eq!(relocation.destination_way(), 1);
    assert_eq!(directory.way_for(line), Some((0, 1)));
    assert_eq!(directory.resident_lines(), vec![canonical_line]);
    assert_eq!(
        directory.set_lines(0).unwrap(),
        vec![None, Some(canonical_line)]
    );
    assert_eq!(
        directory.snapshot().sets()[0].lines(),
        &[None, Some(canonical_line)]
    );
}

#[test]
fn replacement_directory_rejects_tag_shaped_or_wrong_set_moves() {
    let line = Address::new(0x1000);
    let mut directory = CacheReplacementDirectory::new(
        CacheReplacementDirectoryConfig::new(CacheReplacementPolicyKind::Lru, line_layout(), 4, 2)
            .unwrap(),
    );
    directory.install(line).unwrap();
    let tag_shaped_value = Address::new(line.get() / line_layout().bytes());

    assert_eq!(
        directory
            .move_resident_line(tag_shaped_value, 0, 1)
            .unwrap_err(),
        CacheReplacementPolicyError::UnknownResidentLine {
            line: line_layout().line_address(tag_shaped_value)
        }
    );
    assert_eq!(
        directory.move_resident_line(line, 1, 1).unwrap_err(),
        CacheReplacementPolicyError::LineSetMismatch {
            line: line_layout().line_address(line),
            set: 1,
            expected_set: 0,
        }
    );
    assert_eq!(
        directory.move_resident_line(line, 0, 0).unwrap_err(),
        CacheReplacementPolicyError::OccupiedDestinationWay { set: 0, way: 0 }
    );
}

#[test]
fn replacement_directory_moves_ship_line_without_requiring_new_signature() {
    let line = Address::new(0x2000);
    let mut directory = CacheReplacementDirectory::new(
        CacheReplacementDirectoryConfig::new(
            CacheReplacementPolicyKind::Ship {
                rrpv_bits: 2,
                hit_priority: true,
                shct_entries: 8,
                insertion_threshold_percent: 50,
            },
            line_layout(),
            1,
            2,
        )
        .unwrap(),
    );
    directory.install_with_signature(line, 3).unwrap();
    directory.touch_with_signature(line, 3).unwrap();

    let relocation = directory.move_resident_line(line, 0, 1).unwrap();

    assert_eq!(relocation.line(), line_layout().line_address(line));
    assert_eq!(relocation.source_way(), 0);
    assert_eq!(relocation.destination_way(), 1);
    assert_eq!(directory.way_for(line), Some((0, 1)));
}

#[test]
fn replacement_directory_snapshot_restore_preserves_policy_state() {
    let config = CacheReplacementDirectoryConfig::new(
        CacheReplacementPolicyKind::TreePlru,
        line_layout(),
        1,
        4,
    )
    .unwrap();
    let mut directory = CacheReplacementDirectory::new(config.clone());

    for line in [0x0000, 0x0010, 0x0020, 0x0030] {
        directory.install(Address::new(line)).unwrap();
    }
    directory.touch(Address::new(0x0000)).unwrap();
    let snapshot = directory.snapshot();

    let expected_evict = directory
        .install(Address::new(0x0040))
        .unwrap()
        .evicted_line();
    assert!(expected_evict.is_some());

    directory.restore(&snapshot).unwrap();
    let restored_evict = directory.install(Address::new(0x0040)).unwrap();
    assert_eq!(restored_evict.evicted_line(), expected_evict);

    let mut incompatible = CacheReplacementDirectory::new(
        CacheReplacementDirectoryConfig::new(
            CacheReplacementPolicyKind::TreePlru,
            line_layout(),
            2,
            4,
        )
        .unwrap(),
    );
    assert_eq!(
        incompatible.restore(&snapshot),
        Err(
            CacheReplacementPolicyError::SnapshotDirectoryConfigMismatch {
                expected: Box::new(incompatible.config().clone()),
                actual: Box::new(config),
            }
        )
    );

    assert_eq!(
        directory.touch(Address::new(0x0080)),
        Err(CacheReplacementPolicyError::UnknownResidentLine {
            line: Address::new(0x0080)
        })
    );
    assert_eq!(
        directory.set_lines(2),
        Err(CacheReplacementPolicyError::UnknownSet { set: 2, sets: 1 })
    );
}

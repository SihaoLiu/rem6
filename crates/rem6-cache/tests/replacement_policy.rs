use rem6_cache::{
    CacheReplacementPolicyConfig, CacheReplacementPolicyError, CacheReplacementPolicyKind,
    ReplacementSet,
};

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

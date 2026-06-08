use rem6_kernel::{
    ClockDomain, ClockDomainId, ClockDomainTree, ClockDomainTreeSnapshot, ClockError, Cycles,
    DerivedClockDomainSnapshot, SourceClockDomain, SourceClockDomainSnapshot,
};

#[test]
fn clock_domain_converts_cycles_to_ticks_with_component_periods() {
    let cpu = ClockDomain::new(3).unwrap();
    let accelerator = ClockDomain::new(5).unwrap();

    assert_eq!(cpu.period(), 3);
    assert_eq!(cpu.cycles_to_ticks(Cycles::new(4)).unwrap(), 12);
    assert_eq!(accelerator.cycles_to_ticks(Cycles::new(4)).unwrap(), 20);
}

#[test]
fn clock_domain_rounds_ticks_up_to_whole_cycles() {
    let domain = ClockDomain::new(4).unwrap();

    assert_eq!(domain.ticks_to_cycles_ceil(0), Cycles::new(0));
    assert_eq!(domain.ticks_to_cycles_ceil(1), Cycles::new(1));
    assert_eq!(domain.ticks_to_cycles_ceil(4), Cycles::new(1));
    assert_eq!(domain.ticks_to_cycles_ceil(5), Cycles::new(2));
}

#[test]
fn clock_domain_finds_current_or_future_clock_edges() {
    let domain = ClockDomain::new(5).unwrap();

    assert_eq!(domain.clock_edge(10, Cycles::new(0)).unwrap(), 10);
    assert_eq!(domain.clock_edge(11, Cycles::new(0)).unwrap(), 15);
    assert_eq!(domain.clock_edge(11, Cycles::new(2)).unwrap(), 25);
}

#[test]
fn clock_domain_rejects_zero_period() {
    let error = ClockDomain::new(0).unwrap_err();

    assert_eq!(error, ClockError::ZeroPeriod);
}

#[test]
fn clock_domain_reports_tick_overflow() {
    let domain = ClockDomain::new(u64::MAX).unwrap();

    let error = domain.cycles_to_ticks(Cycles::new(2)).unwrap_err();

    assert_eq!(
        error,
        ClockError::TickOverflow {
            period: u64::MAX,
            cycles: Cycles::new(2)
        }
    );
}

#[test]
fn source_clock_domain_switches_between_valid_performance_points() {
    let mut domain = SourceClockDomain::new(Some(ClockDomainId::new(7)), vec![2, 4, 8], 1).unwrap();

    assert_eq!(domain.domain_id(), Some(ClockDomainId::new(7)));
    assert_eq!(domain.performance_level(), 1);
    assert_eq!(domain.performance_point_count(), 3);
    assert_eq!(domain.period(), 4);
    assert_eq!(domain.clock_domain(), ClockDomain::new(4).unwrap());
    assert_eq!(domain.period_at_performance_level(0).unwrap(), 2);
    assert_eq!(domain.frequency_hz(1_000).unwrap(), 250);

    assert!(!domain.set_performance_level(1).unwrap());
    assert!(domain.set_performance_level(0).unwrap());
    assert_eq!(domain.performance_level(), 0);
    assert_eq!(domain.period(), 2);
    assert_eq!(
        domain
            .clock_domain()
            .cycles_to_ticks(Cycles::new(3))
            .unwrap(),
        6
    );
}

#[test]
fn source_clock_domain_rejects_invalid_performance_tables() {
    assert_eq!(
        SourceClockDomain::new(None, Vec::new(), 0).unwrap_err(),
        ClockError::EmptyPerformancePoints
    );
    assert_eq!(
        SourceClockDomain::new(None, vec![2, 0], 0).unwrap_err(),
        ClockError::ZeroPeriod
    );
    assert_eq!(
        SourceClockDomain::new(None, vec![4, 2], 0).unwrap_err(),
        ClockError::UnsortedPerformancePoints
    );
    assert_eq!(
        SourceClockDomain::new(None, vec![2, 4], 2).unwrap_err(),
        ClockError::InvalidPerformanceLevel { level: 2, count: 2 }
    );
}

#[test]
fn clock_domain_derives_divided_child_domains() {
    let source = SourceClockDomain::new(None, vec![3, 6], 0).unwrap();

    let derived = source.derived_clock_domain(4).unwrap();

    assert_eq!(derived.period(), 12);
    assert_eq!(derived.cycles_to_ticks(Cycles::new(2)).unwrap(), 24);
    assert_eq!(
        source.derived_clock_domain(0).unwrap_err(),
        ClockError::ZeroClockDivider
    );
    assert_eq!(
        ClockDomain::new(u64::MAX).unwrap().derived(2).unwrap_err(),
        ClockError::DerivedClockOverflow {
            period: u64::MAX,
            divider: 2
        }
    );
}

#[test]
fn clock_domain_tree_propagates_source_period_changes_to_derived_domains() {
    let mut tree = ClockDomainTree::new();
    tree.insert_source(SourceClockDomain::new(Some(ClockDomainId::new(1)), vec![2, 4], 0).unwrap())
        .unwrap();
    tree.insert_derived(ClockDomainId::new(2), ClockDomainId::new(1), 3)
        .unwrap();
    tree.insert_derived(ClockDomainId::new(3), ClockDomainId::new(2), 2)
        .unwrap();

    assert_eq!(
        tree.clock_domain(ClockDomainId::new(1)).unwrap().period(),
        2
    );
    assert_eq!(
        tree.clock_domain(ClockDomainId::new(2)).unwrap().period(),
        6
    );
    assert_eq!(
        tree.clock_domain(ClockDomainId::new(3)).unwrap().period(),
        12
    );

    assert!(tree
        .set_source_performance_level(ClockDomainId::new(1), 1)
        .unwrap());
    assert_eq!(
        tree.clock_domain(ClockDomainId::new(1)).unwrap().period(),
        4
    );
    assert_eq!(
        tree.clock_domain(ClockDomainId::new(2)).unwrap().period(),
        12
    );
    assert_eq!(
        tree.clock_domain(ClockDomainId::new(3)).unwrap().period(),
        24
    );

    assert!(!tree
        .set_source_performance_level(ClockDomainId::new(1), 1)
        .unwrap());
}

#[test]
fn clock_domain_tree_rejects_invalid_topology() {
    let mut tree = ClockDomainTree::new();
    assert_eq!(
        tree.insert_source(SourceClockDomain::new(None, vec![2], 0).unwrap())
            .unwrap_err(),
        ClockError::MissingClockDomainId
    );

    tree.insert_source(SourceClockDomain::new(Some(ClockDomainId::new(1)), vec![2], 0).unwrap())
        .unwrap();
    assert_eq!(
        tree.insert_source(
            SourceClockDomain::new(Some(ClockDomainId::new(1)), vec![3], 0).unwrap()
        )
        .unwrap_err(),
        ClockError::DuplicateClockDomain {
            domain: ClockDomainId::new(1)
        }
    );
    assert_eq!(
        tree.insert_derived(ClockDomainId::new(2), ClockDomainId::new(99), 2)
            .unwrap_err(),
        ClockError::UnknownClockDomain {
            domain: ClockDomainId::new(99)
        }
    );
    assert_eq!(
        tree.insert_derived(ClockDomainId::new(2), ClockDomainId::new(1), 0)
            .unwrap_err(),
        ClockError::ZeroClockDivider
    );

    tree.insert_derived(ClockDomainId::new(2), ClockDomainId::new(1), 2)
        .unwrap();
    assert_eq!(
        tree.derived_clock_domain(ClockDomainId::new(1))
            .unwrap_err(),
        ClockError::NotDerivedClockDomain {
            domain: ClockDomainId::new(1)
        }
    );
    assert_eq!(
        tree.set_source_performance_level(ClockDomainId::new(2), 0)
            .unwrap_err(),
        ClockError::NotSourceClockDomain {
            domain: ClockDomainId::new(2)
        }
    );
}

#[test]
fn clock_domain_tree_rejects_overflow_without_partial_update() {
    let mut tree = ClockDomainTree::new();
    tree.insert_source(
        SourceClockDomain::new(Some(ClockDomainId::new(1)), vec![u64::MAX / 2, u64::MAX], 0)
            .unwrap(),
    )
    .unwrap();
    tree.insert_derived(ClockDomainId::new(2), ClockDomainId::new(1), 2)
        .unwrap();

    assert_eq!(
        tree.set_source_performance_level(ClockDomainId::new(1), 1)
            .unwrap_err(),
        ClockError::DerivedClockOverflow {
            period: u64::MAX,
            divider: 2
        }
    );
    assert_eq!(
        tree.source_clock_domain(ClockDomainId::new(1))
            .unwrap()
            .performance_level(),
        0
    );
    assert_eq!(
        tree.clock_domain(ClockDomainId::new(1)).unwrap().period(),
        u64::MAX / 2
    );
    assert_eq!(
        tree.clock_domain(ClockDomainId::new(2)).unwrap().period(),
        (u64::MAX / 2) * 2
    );
}

#[test]
fn clock_domain_tree_rejects_transitive_overflow_without_partial_update() {
    let source_initial_period = u64::MAX / 8;
    let source_next_period = u64::MAX / 4;
    let child_initial_period = source_initial_period * 2;
    let grandchild_initial_period = child_initial_period * 3;
    let child_next_period = source_next_period * 2;
    let mut tree = ClockDomainTree::new();
    tree.insert_source(
        SourceClockDomain::new(
            Some(ClockDomainId::new(1)),
            vec![source_initial_period, source_next_period],
            0,
        )
        .unwrap(),
    )
    .unwrap();
    tree.insert_derived(ClockDomainId::new(2), ClockDomainId::new(1), 2)
        .unwrap();
    tree.insert_derived(ClockDomainId::new(3), ClockDomainId::new(2), 3)
        .unwrap();

    assert_eq!(
        tree.set_source_performance_level(ClockDomainId::new(1), 1)
            .unwrap_err(),
        ClockError::DerivedClockOverflow {
            period: child_next_period,
            divider: 3
        }
    );
    assert_eq!(
        tree.source_clock_domain(ClockDomainId::new(1))
            .unwrap()
            .performance_level(),
        0
    );
    assert_eq!(
        tree.clock_domain(ClockDomainId::new(1)).unwrap().period(),
        source_initial_period
    );
    assert_eq!(
        tree.clock_domain(ClockDomainId::new(2)).unwrap().period(),
        child_initial_period
    );
    assert_eq!(
        tree.clock_domain(ClockDomainId::new(3)).unwrap().period(),
        grandchild_initial_period
    );
}

#[test]
fn clock_domain_tree_snapshot_restores_source_levels_and_derived_domains() {
    let mut tree = ClockDomainTree::new();
    tree.insert_source(
        SourceClockDomain::new(Some(ClockDomainId::new(1)), vec![2, 4, 8], 0).unwrap(),
    )
    .unwrap();
    tree.insert_derived(ClockDomainId::new(2), ClockDomainId::new(1), 3)
        .unwrap();
    tree.insert_derived(ClockDomainId::new(3), ClockDomainId::new(2), 2)
        .unwrap();
    assert!(tree
        .set_source_performance_level(ClockDomainId::new(1), 2)
        .unwrap());

    let snapshot = tree.snapshot();
    assert_eq!(snapshot.sources().len(), 1);
    assert_eq!(snapshot.sources()[0].domain_id(), ClockDomainId::new(1));
    assert_eq!(snapshot.sources()[0].periods(), &[2, 4, 8]);
    assert_eq!(snapshot.sources()[0].performance_level(), 2);
    assert_eq!(snapshot.derived().len(), 2);
    assert_eq!(
        snapshot.derived()[0],
        DerivedClockDomainSnapshot::new(ClockDomainId::new(2), ClockDomainId::new(1), 3)
    );
    assert_eq!(
        snapshot.derived()[1],
        DerivedClockDomainSnapshot::new(ClockDomainId::new(3), ClockDomainId::new(2), 2)
    );
    let restored = ClockDomainTree::restore(snapshot).unwrap();

    let source = restored.source_clock_domain(ClockDomainId::new(1)).unwrap();
    assert_eq!(source.performance_level(), 2);
    assert_eq!(source.performance_points(), &[2, 4, 8]);
    assert_eq!(
        restored
            .clock_domain(ClockDomainId::new(1))
            .unwrap()
            .period(),
        8
    );
    assert_eq!(
        restored
            .derived_clock_domain(ClockDomainId::new(2))
            .unwrap()
            .parent_id(),
        ClockDomainId::new(1)
    );
    assert_eq!(
        restored
            .derived_clock_domain(ClockDomainId::new(2))
            .unwrap()
            .divider(),
        3
    );
    assert_eq!(
        restored
            .clock_domain(ClockDomainId::new(2))
            .unwrap()
            .period(),
        24
    );
    assert_eq!(
        restored
            .clock_domain(ClockDomainId::new(3))
            .unwrap()
            .period(),
        48
    );

    let mut restored = restored;
    assert!(restored
        .set_source_performance_level(ClockDomainId::new(1), 1)
        .unwrap());
    assert_eq!(
        restored
            .clock_domain(ClockDomainId::new(2))
            .unwrap()
            .period(),
        12
    );
    assert_eq!(
        restored
            .clock_domain(ClockDomainId::new(3))
            .unwrap()
            .period(),
        24
    );
}

#[test]
fn clock_domain_tree_restore_rejects_duplicate_snapshot_domains() {
    let snapshot = ClockDomainTreeSnapshot::new(
        vec![
            SourceClockDomainSnapshot::new(ClockDomainId::new(1), vec![2], 0),
            SourceClockDomainSnapshot::new(ClockDomainId::new(1), vec![4], 0),
        ],
        Vec::new(),
    );

    assert_eq!(
        ClockDomainTree::restore(snapshot).unwrap_err(),
        ClockError::DuplicateClockDomain {
            domain: ClockDomainId::new(1)
        }
    );
}

#[test]
fn clock_domain_tree_restore_rejects_unknown_derived_parent() {
    let snapshot = ClockDomainTreeSnapshot::new(
        vec![SourceClockDomainSnapshot::new(
            ClockDomainId::new(1),
            vec![2],
            0,
        )],
        vec![DerivedClockDomainSnapshot::new(
            ClockDomainId::new(2),
            ClockDomainId::new(99),
            4,
        )],
    );

    assert_eq!(
        ClockDomainTree::restore(snapshot).unwrap_err(),
        ClockError::UnknownClockDomain {
            domain: ClockDomainId::new(99)
        }
    );
}

#[test]
fn clock_domain_tree_restore_rejects_invalid_source_snapshots() {
    assert_eq!(
        ClockDomainTree::restore(ClockDomainTreeSnapshot::new(
            vec![SourceClockDomainSnapshot::new(
                ClockDomainId::new(1),
                Vec::new(),
                0
            )],
            Vec::new(),
        ))
        .unwrap_err(),
        ClockError::EmptyPerformancePoints
    );
    assert_eq!(
        ClockDomainTree::restore(ClockDomainTreeSnapshot::new(
            vec![SourceClockDomainSnapshot::new(
                ClockDomainId::new(1),
                vec![2, 0],
                0,
            )],
            Vec::new(),
        ))
        .unwrap_err(),
        ClockError::ZeroPeriod
    );
    assert_eq!(
        ClockDomainTree::restore(ClockDomainTreeSnapshot::new(
            vec![SourceClockDomainSnapshot::new(
                ClockDomainId::new(1),
                vec![4, 2],
                0,
            )],
            Vec::new(),
        ))
        .unwrap_err(),
        ClockError::UnsortedPerformancePoints
    );
    assert_eq!(
        ClockDomainTree::restore(ClockDomainTreeSnapshot::new(
            vec![SourceClockDomainSnapshot::new(
                ClockDomainId::new(1),
                vec![2, 4],
                2,
            )],
            Vec::new(),
        ))
        .unwrap_err(),
        ClockError::InvalidPerformanceLevel { level: 2, count: 2 }
    );
}

#[test]
fn clock_domain_tree_restore_rejects_invalid_derived_snapshots() {
    let duplicate_derived = ClockDomainTreeSnapshot::new(
        vec![SourceClockDomainSnapshot::new(
            ClockDomainId::new(1),
            vec![2],
            0,
        )],
        vec![
            DerivedClockDomainSnapshot::new(ClockDomainId::new(2), ClockDomainId::new(1), 2),
            DerivedClockDomainSnapshot::new(ClockDomainId::new(2), ClockDomainId::new(1), 3),
        ],
    );
    assert_eq!(
        ClockDomainTree::restore(duplicate_derived).unwrap_err(),
        ClockError::DuplicateClockDomain {
            domain: ClockDomainId::new(2)
        }
    );

    let source_collision = ClockDomainTreeSnapshot::new(
        vec![SourceClockDomainSnapshot::new(
            ClockDomainId::new(1),
            vec![2],
            0,
        )],
        vec![DerivedClockDomainSnapshot::new(
            ClockDomainId::new(1),
            ClockDomainId::new(1),
            2,
        )],
    );
    assert_eq!(
        ClockDomainTree::restore(source_collision).unwrap_err(),
        ClockError::DuplicateClockDomain {
            domain: ClockDomainId::new(1)
        }
    );

    let zero_divider = ClockDomainTreeSnapshot::new(
        vec![SourceClockDomainSnapshot::new(
            ClockDomainId::new(1),
            vec![2],
            0,
        )],
        vec![DerivedClockDomainSnapshot::new(
            ClockDomainId::new(2),
            ClockDomainId::new(1),
            0,
        )],
    );
    assert_eq!(
        ClockDomainTree::restore(zero_divider).unwrap_err(),
        ClockError::ZeroClockDivider
    );

    let overflow = ClockDomainTreeSnapshot::new(
        vec![SourceClockDomainSnapshot::new(
            ClockDomainId::new(1),
            vec![u64::MAX],
            0,
        )],
        vec![DerivedClockDomainSnapshot::new(
            ClockDomainId::new(2),
            ClockDomainId::new(1),
            2,
        )],
    );
    assert_eq!(
        ClockDomainTree::restore(overflow).unwrap_err(),
        ClockError::DerivedClockOverflow {
            period: u64::MAX,
            divider: 2
        }
    );
}

#[test]
fn clock_domain_tree_restore_accepts_unordered_derived_snapshots() {
    let snapshot = ClockDomainTreeSnapshot::new(
        vec![SourceClockDomainSnapshot::new(
            ClockDomainId::new(1),
            vec![3, 6],
            0,
        )],
        vec![
            DerivedClockDomainSnapshot::new(ClockDomainId::new(3), ClockDomainId::new(2), 5),
            DerivedClockDomainSnapshot::new(ClockDomainId::new(2), ClockDomainId::new(1), 4),
        ],
    );

    let restored = ClockDomainTree::restore(snapshot).unwrap();

    assert_eq!(
        restored
            .clock_domain(ClockDomainId::new(2))
            .unwrap()
            .period(),
        12
    );
    assert_eq!(
        restored
            .clock_domain(ClockDomainId::new(3))
            .unwrap()
            .period(),
        60
    );
    assert_eq!(
        restored
            .derived_clock_domain(ClockDomainId::new(3))
            .unwrap()
            .parent_id(),
        ClockDomainId::new(2)
    );

    let mut restored = restored;
    assert!(restored
        .set_source_performance_level(ClockDomainId::new(1), 1)
        .unwrap());
    assert_eq!(
        restored
            .clock_domain(ClockDomainId::new(2))
            .unwrap()
            .period(),
        24
    );
    assert_eq!(
        restored
            .clock_domain(ClockDomainId::new(3))
            .unwrap()
            .period(),
        120
    );
}

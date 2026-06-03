use rem6_cache::{
    CacheIndexingLocation, CachePartitionCandidate, CachePartitionId, CachePartitionManager,
    CachePartitionPolicy, CachePartitioningError, MaxCapacityPartitioningPolicy,
    WayPartitionAllocation, WayPartitioningPolicy,
};

fn candidate(set: usize, way: usize, owner: Option<u64>) -> CachePartitionCandidate {
    CachePartitionCandidate::new(
        CacheIndexingLocation::new(set, way),
        owner.map(CachePartitionId::new),
    )
}

fn ways(candidates: &[CachePartitionCandidate]) -> Vec<usize> {
    candidates.iter().map(|candidate| candidate.way()).collect()
}

#[test]
fn way_partitioning_filters_only_configured_partitions() {
    let policy = WayPartitioningPolicy::new(
        4,
        &[WayPartitionAllocation::new(
            CachePartitionId::new(7),
            [1, 3, 3],
        )],
    )
    .unwrap();
    let candidates = [
        candidate(0, 0, Some(1)),
        candidate(0, 1, Some(7)),
        candidate(0, 2, Some(2)),
        candidate(0, 3, Some(7)),
    ];

    assert_eq!(
        ways(&policy.filter_candidates(CachePartitionId::new(7), &candidates)),
        vec![1, 3]
    );
    assert_eq!(
        policy
            .filter_candidates(CachePartitionId::new(9), &candidates)
            .as_slice(),
        candidates.as_slice()
    );
    assert_eq!(policy.ways_for(CachePartitionId::new(7)).unwrap(), &[1, 3]);
}

#[test]
fn way_partitioning_rejects_out_of_range_ways_before_mutation() {
    let result = WayPartitioningPolicy::new(
        2,
        &[WayPartitionAllocation::new(
            CachePartitionId::new(7),
            [0, 2],
        )],
    );

    assert_eq!(
        result,
        Err(CachePartitioningError::WayOutOfRange {
            partition: CachePartitionId::new(7),
            way: 2,
            ways: 2,
        })
    );
}

#[test]
fn max_capacity_filters_owned_entries_after_partition_reaches_limit() {
    let mut policy = MaxCapacityPartitioningPolicy::new(
        8,
        &[
            (CachePartitionId::new(1), 0.5),
            (CachePartitionId::new(2), 0.25),
        ],
    )
    .unwrap();
    let candidates = [
        candidate(0, 0, Some(1)),
        candidate(0, 1, Some(2)),
        candidate(0, 2, None),
        candidate(0, 3, Some(1)),
    ];

    assert_eq!(policy.max_capacity(CachePartitionId::new(1)), Some(4));
    assert_eq!(policy.max_capacity(CachePartitionId::new(2)), Some(2));
    assert_eq!(
        policy
            .filter_candidates(CachePartitionId::new(1), &candidates)
            .as_slice(),
        candidates.as_slice()
    );

    for _ in 0..4 {
        policy.notify_acquire(CachePartitionId::new(1)).unwrap();
    }

    assert_eq!(
        policy
            .filter_candidates(CachePartitionId::new(1), &candidates)
            .as_slice(),
        &[candidate(0, 0, Some(1)), candidate(0, 3, Some(1))]
    );
}

#[test]
fn max_capacity_zero_fraction_is_unrestricted_until_partition_has_usage() {
    let mut policy =
        MaxCapacityPartitioningPolicy::new(8, &[(CachePartitionId::new(1), 0.0)]).unwrap();
    let candidates = [
        candidate(0, 0, Some(1)),
        candidate(0, 1, Some(2)),
        candidate(0, 2, None),
    ];

    assert_eq!(policy.max_capacity(CachePartitionId::new(1)), Some(0));
    assert_eq!(policy.current_capacity(CachePartitionId::new(1)), Some(0));
    assert_eq!(
        policy
            .filter_candidates(CachePartitionId::new(1), &candidates)
            .as_slice(),
        candidates.as_slice()
    );

    policy.notify_acquire(CachePartitionId::new(1)).unwrap();
    assert_eq!(policy.current_capacity(CachePartitionId::new(1)), Some(1));
    assert_eq!(
        policy
            .filter_candidates(CachePartitionId::new(1), &candidates)
            .as_slice(),
        &[candidate(0, 0, Some(1))]
    );
    assert_eq!(
        policy.notify_acquire(CachePartitionId::new(1)),
        Err(CachePartitioningError::CapacityExceeded {
            partition: CachePartitionId::new(1),
            current: 1,
            maximum: 0,
        })
    );
}

#[test]
fn max_capacity_ignores_unconfigured_notifications_and_checks_underflow() {
    let mut policy =
        MaxCapacityPartitioningPolicy::new(4, &[(CachePartitionId::new(1), 0.5)]).unwrap();

    policy.notify_acquire(CachePartitionId::new(9)).unwrap();
    policy.notify_release(CachePartitionId::new(9)).unwrap();
    assert_eq!(policy.current_capacity(CachePartitionId::new(9)), None);

    assert_eq!(
        policy.notify_release(CachePartitionId::new(1)),
        Err(CachePartitioningError::CapacityUnderflow {
            partition: CachePartitionId::new(1),
        })
    );
}

#[test]
fn partition_manager_composes_policy_filters_in_order() {
    let way_policy = WayPartitioningPolicy::new(
        4,
        &[WayPartitionAllocation::new(
            CachePartitionId::new(1),
            [1, 2, 3],
        )],
    )
    .unwrap();
    let mut max_policy =
        MaxCapacityPartitioningPolicy::new(4, &[(CachePartitionId::new(1), 0.5)]).unwrap();
    max_policy.notify_acquire(CachePartitionId::new(1)).unwrap();
    max_policy.notify_acquire(CachePartitionId::new(1)).unwrap();
    let manager = CachePartitionManager::new([
        CachePartitionPolicy::Way(way_policy),
        CachePartitionPolicy::MaxCapacity(max_policy),
    ]);
    let candidates = [
        candidate(0, 0, Some(1)),
        candidate(0, 1, Some(2)),
        candidate(0, 2, Some(1)),
        candidate(0, 3, None),
    ];

    assert_eq!(
        manager
            .filter_candidates(CachePartitionId::new(1), &candidates)
            .as_slice(),
        &[candidate(0, 2, Some(1))]
    );
}

#[test]
fn max_capacity_rejects_invalid_configuration() {
    assert_eq!(
        MaxCapacityPartitioningPolicy::new(0, &[(CachePartitionId::new(1), 0.5)]),
        Err(CachePartitioningError::ZeroTotalBlocks)
    );
    assert!(matches!(
        MaxCapacityPartitioningPolicy::new(8, &[(CachePartitionId::new(1), 1.5)]),
        Err(CachePartitioningError::InvalidCapacityFraction { .. })
    ));
    assert_eq!(
        MaxCapacityPartitioningPolicy::new(
            8,
            &[
                (CachePartitionId::new(1), 0.5),
                (CachePartitionId::new(1), 0.25),
            ],
        ),
        Err(CachePartitioningError::DuplicatePartition {
            partition: CachePartitionId::new(1),
        })
    );
}

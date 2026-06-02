use rem6_cache::{
    MshrEntry, MshrHandle, MshrQosClass, MshrQueue, MshrQueueConfig, MshrQueueError,
    MshrQueueSnapshot, MshrTarget, MshrTargetPostFillAction, MshrTargetSource,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryAccessOrdering,
    MemoryBarrierSet, MemoryOperation, MemoryRequest, MemoryRequestId,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request(sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(7), sequence),
        Address::new(address),
        AccessSize::new(8).unwrap(),
        layout(),
    )
    .unwrap()
}

fn ordered_request(sequence: u64, address: u64, ordering: MemoryAccessOrdering) -> MemoryRequest {
    request(sequence, address).with_ordering(ordering)
}

fn uncacheable_request(sequence: u64, address: u64) -> MemoryRequest {
    request(sequence, address).with_uncacheable_strict_order()
}

fn write_request(sequence: u64, address: u64, data: Vec<u8>) -> MemoryRequest {
    let access_size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::write(
        MemoryRequestId::new(AgentId::new(7), sequence),
        Address::new(address),
        access_size,
        data,
        ByteMask::full(access_size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn clean_writeback(sequence: u64, line: u64, value: u8) -> MemoryRequest {
    MemoryRequest::writeback_clean(
        MemoryRequestId::new(AgentId::new(7), sequence),
        Address::new(line),
        vec![value; layout().bytes() as usize],
        layout(),
    )
    .unwrap()
}

const MSHR_ENTRIES_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<MshrEntry>() + 1;
const MSHR_TARGETS_PER_MSHR_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<MshrTarget>() + 1;

#[test]
fn mshr_queue_allocates_merges_limits_targets_and_reuses_entries() {
    let mut queue = MshrQueue::new(MshrQueueConfig::new(2, 2, 0).unwrap());

    let allocated = queue
        .allocate_or_merge(request(0, 0x1000), 5, MshrTargetSource::Demand, true)
        .unwrap();
    assert_eq!(allocated.handle().index(), 0);
    assert!(allocated.allocated_new_entry());
    assert_eq!(queue.allocated_count(), 1);
    assert_eq!(queue.ready_handles(4), Vec::new());
    assert_eq!(queue.ready_handles(5), vec![allocated.handle()]);

    let merged = queue
        .allocate_or_merge(request(1, 0x1018), 6, MshrTargetSource::Demand, true)
        .unwrap();
    assert_eq!(merged.handle(), allocated.handle());
    assert!(!merged.allocated_new_entry());
    assert_eq!(queue.allocated_count(), 1);
    assert_eq!(
        queue
            .entry(allocated.handle())
            .unwrap()
            .targets()
            .iter()
            .map(|target| target.request().id().sequence())
            .collect::<Vec<_>>(),
        vec![0, 1]
    );

    assert_eq!(
        queue.allocate_or_merge(request(2, 0x1008), 7, MshrTargetSource::Demand, true),
        Err(MshrQueueError::TargetSlotsFull {
            handle: allocated.handle(),
            line: Address::new(0x1000),
            targets_per_mshr: 2,
        })
    );

    queue
        .mark_in_service(allocated.handle(), true)
        .expect("entry can enter service");
    assert!(queue.entry(allocated.handle()).unwrap().in_service());
    assert!(queue.entry(allocated.handle()).unwrap().pending_modified());
    assert_eq!(queue.ready_handles(10), Vec::new());

    queue
        .mark_pending(allocated.handle())
        .expect("in-service entry can become pending again");
    assert!(!queue.entry(allocated.handle()).unwrap().in_service());
    assert_eq!(queue.ready_handles(5), vec![allocated.handle()]);

    let completion = queue.complete(allocated.handle()).unwrap();
    assert_eq!(
        completion
            .targets()
            .iter()
            .map(|target| target.request().id().sequence())
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(queue.allocated_count(), 0);

    let reused = queue
        .allocate_or_merge(request(3, 0x2000), 8, MshrTargetSource::Demand, true)
        .unwrap();
    assert_eq!(reused.handle().index(), 1);
}

#[test]
fn mshr_queue_keeps_uncacheable_strict_requests_out_of_line_merging() {
    let mut queue = MshrQueue::new(MshrQueueConfig::new(4, 3, 0).unwrap());

    let normal = queue
        .allocate_or_merge(request(10, 0x1000), 5, MshrTargetSource::Demand, true)
        .unwrap();
    assert!(normal.allocated_new_entry());

    let uncached = queue
        .allocate_or_merge(
            uncacheable_request(11, 0x1018),
            6,
            MshrTargetSource::Demand,
            true,
        )
        .unwrap();
    assert!(uncached.allocated_new_entry());
    assert_ne!(uncached.handle(), normal.handle());
    assert_eq!(queue.allocated_count(), 2);
    assert_eq!(queue.entry(normal.handle()).unwrap().target_count(), 1);
    assert_eq!(queue.entry(uncached.handle()).unwrap().target_count(), 1);

    let merged_normal = queue
        .allocate_or_merge(request(12, 0x1008), 7, MshrTargetSource::Demand, true)
        .unwrap();
    assert!(!merged_normal.allocated_new_entry());
    assert_eq!(merged_normal.handle(), normal.handle());
    assert_eq!(queue.entry(normal.handle()).unwrap().target_count(), 2);

    let second_uncached = queue
        .allocate_or_merge(
            uncacheable_request(13, 0x100c),
            8,
            MshrTargetSource::Demand,
            true,
        )
        .unwrap();
    assert!(second_uncached.allocated_new_entry());
    assert_ne!(second_uncached.handle(), normal.handle());
    assert_ne!(second_uncached.handle(), uncached.handle());
    assert_eq!(queue.allocated_count(), 3);
}

#[test]
fn mshr_completion_splits_post_fill_clean_targets_from_local_targets() {
    let mut queue = MshrQueue::new(MshrQueueConfig::new(1, 3, 0).unwrap());
    let read = request(20, 0x1000);
    let write = write_request(21, 0x1018, vec![0xaa; 8]);
    let clean = clean_writeback(22, 0x1000, 0xbb);

    let allocated = queue
        .allocate_or_merge(read.clone(), 3, MshrTargetSource::Demand, true)
        .unwrap();
    queue
        .allocate_or_merge(write.clone(), 4, MshrTargetSource::Demand, true)
        .unwrap();
    queue
        .allocate_or_merge(clean.clone(), 5, MshrTargetSource::Demand, false)
        .unwrap();

    let completion = queue.complete(allocated.handle()).unwrap();

    assert_eq!(
        completion
            .targets()
            .iter()
            .map(|target| target.request().id().sequence())
            .collect::<Vec<_>>(),
        vec![20, 21, 22]
    );
    assert_eq!(
        completion
            .local_targets()
            .map(|target| target.request().id().sequence())
            .collect::<Vec<_>>(),
        vec![20, 21]
    );
    assert_eq!(
        completion.targets()[2].post_fill_action(),
        MshrTargetPostFillAction::ForwardDownstream
    );

    let downstream = completion.post_fill_downstream_requests();
    assert_eq!(downstream, vec![clean]);
    assert_eq!(downstream[0].operation(), MemoryOperation::WritebackClean);
    assert_eq!(downstream[0].data().unwrap(), &[0xbb; 64]);
}

#[test]
fn mshr_queue_reserves_demand_entries_orders_ready_entries_and_restores_snapshot() {
    let mut queue = MshrQueue::new(MshrQueueConfig::new(3, 2, 1).unwrap());

    let prefetch = queue
        .allocate_or_merge(request(0, 0x3000), 10, MshrTargetSource::Prefetch, false)
        .unwrap();
    assert_eq!(
        queue.allocate_or_merge(request(1, 0x4000), 11, MshrTargetSource::Prefetch, false),
        Err(MshrQueueError::PrefetchReserveBlocked {
            allocated: 1,
            entries: 3,
            demand_reserve: 1,
        })
    );

    let demand = queue
        .allocate_or_merge(request(2, 0x5000), 5, MshrTargetSource::Demand, true)
        .unwrap();
    assert_eq!(
        queue.ready_handles(10),
        vec![demand.handle(), prefetch.handle()]
    );

    let snapshot = queue.snapshot();
    queue.delay_until(demand.handle(), 12).unwrap();
    assert_eq!(
        queue.ready_handles(12),
        vec![prefetch.handle(), demand.handle()]
    );

    queue.restore(&snapshot).unwrap();
    assert_eq!(
        queue.ready_handles(10),
        vec![demand.handle(), prefetch.handle()]
    );

    queue
        .allocate_or_merge(request(3, 0x6000), 9, MshrTargetSource::Demand, true)
        .unwrap();
    assert_eq!(
        queue.allocate_or_merge(request(4, 0x7000), 13, MshrTargetSource::Demand, true),
        Err(MshrQueueError::EntrySlotsFull { entries: 3 })
    );
}

#[test]
fn mshr_queue_orders_ready_entries_by_qos_and_promotes_merged_targets() {
    let mut queue = MshrQueue::new(MshrQueueConfig::new(3, 3, 0).unwrap());

    let low = queue
        .allocate_or_merge_with_qos(
            request(0, 0x8000),
            10,
            MshrTargetSource::Demand,
            true,
            MshrQosClass::new(30, 4),
        )
        .unwrap();
    let high = queue
        .allocate_or_merge_with_qos(
            request(1, 0x9000),
            10,
            MshrTargetSource::Demand,
            true,
            MshrQosClass::new(10, 1),
        )
        .unwrap();

    assert_eq!(queue.ready_handles(10), vec![high.handle(), low.handle()]);
    assert_eq!(
        queue.entry(low.handle()).unwrap().effective_qos(),
        Some(MshrQosClass::new(30, 4))
    );

    let promoted = queue
        .allocate_or_merge_with_qos(
            request(2, 0x8010),
            10,
            MshrTargetSource::Demand,
            true,
            MshrQosClass::new(40, 0),
        )
        .unwrap();

    assert_eq!(promoted.handle(), low.handle());
    assert_eq!(
        queue.entry(low.handle()).unwrap().effective_qos(),
        Some(MshrQosClass::new(40, 0))
    );
    assert_eq!(queue.ready_handles(10), vec![low.handle(), high.handle()]);

    let snapshot = queue.snapshot();
    let mut restored = MshrQueue::new(MshrQueueConfig::new(3, 3, 0).unwrap());
    restored.restore(&snapshot).unwrap();
    assert_eq!(
        restored.entry(low.handle()).unwrap().effective_qos(),
        Some(MshrQosClass::new(40, 0))
    );
    assert_eq!(
        restored.ready_handles(10),
        vec![low.handle(), high.handle()]
    );
}

#[test]
fn mshr_queue_preserves_same_agent_release_ordering_for_ready_entries() {
    let mut queue = MshrQueue::new(MshrQueueConfig::new(3, 2, 0).unwrap());

    let prior = queue
        .allocate_or_merge_with_qos(
            request(10, 0x11000),
            10,
            MshrTargetSource::Demand,
            true,
            MshrQosClass::new(7, 4),
        )
        .unwrap();
    let release = queue
        .allocate_or_merge_with_qos(
            ordered_request(
                11,
                0x12000,
                MemoryAccessOrdering::new(Some(MemoryBarrierSet::memory()), None),
            ),
            10,
            MshrTargetSource::Demand,
            true,
            MshrQosClass::new(7, 0),
        )
        .unwrap();

    assert_eq!(
        queue.ready_handles(10),
        vec![prior.handle(), release.handle()]
    );
}

#[test]
fn mshr_queue_preserves_same_agent_acquire_ordering_for_ready_entries() {
    let mut queue = MshrQueue::new(MshrQueueConfig::new(3, 2, 0).unwrap());

    let acquire = queue
        .allocate_or_merge_with_qos(
            ordered_request(
                12,
                0x13000,
                MemoryAccessOrdering::new(None, Some(MemoryBarrierSet::memory())),
            ),
            10,
            MshrTargetSource::Demand,
            true,
            MshrQosClass::new(7, 4),
        )
        .unwrap();
    let later = queue
        .allocate_or_merge_with_qos(
            request(13, 0x14000),
            10,
            MshrTargetSource::Demand,
            true,
            MshrQosClass::new(7, 0),
        )
        .unwrap();

    assert_eq!(
        queue.ready_handles(10),
        vec![acquire.handle(), later.handle()]
    );
}

#[test]
fn mshr_queue_qos_profile_counts_targets_and_effective_entries() {
    let mut queue = MshrQueue::new(MshrQueueConfig::new(3, 3, 0).unwrap());

    let low = queue
        .allocate_or_merge_with_qos(
            request(0, 0x8000),
            10,
            MshrTargetSource::Demand,
            true,
            MshrQosClass::new(30, 4),
        )
        .unwrap();
    queue
        .allocate_or_merge_with_qos(
            request(1, 0x9000),
            10,
            MshrTargetSource::Demand,
            true,
            MshrQosClass::new(10, 1),
        )
        .unwrap();
    queue
        .allocate_or_merge_with_qos(
            request(2, 0x8010),
            10,
            MshrTargetSource::Demand,
            true,
            MshrQosClass::new(40, 0),
        )
        .unwrap();
    queue
        .allocate_or_merge(request(3, 0xa000), 10, MshrTargetSource::Demand, true)
        .unwrap();

    assert_eq!(
        queue.entry(low.handle()).unwrap().effective_qos(),
        Some(MshrQosClass::new(40, 0))
    );

    let profile = queue.qos_profile();
    assert!(profile.has_qos());
    assert_eq!(profile.entry_count(), 3);
    assert_eq!(profile.target_count(), 4);
    assert_eq!(profile.qos_target_count(), 3);
    assert_eq!(profile.effective_entry_count(), 2);
    assert_eq!(profile.priority_target_count(0), 1);
    assert_eq!(profile.priority_target_count(1), 1);
    assert_eq!(profile.priority_target_count(4), 1);
    assert_eq!(profile.priority_target_count(7), 0);
    assert_eq!(profile.requestor_target_count(40), 1);
    assert_eq!(profile.requestor_target_count(30), 1);
    assert_eq!(profile.effective_priority_entry_count(0), 1);
    assert_eq!(profile.effective_priority_entry_count(1), 1);
    assert_eq!(profile.effective_requestor_entry_count(40), 1);
    assert_eq!(profile.effective_requestor_entry_count(10), 1);
    assert_eq!(profile.best_effective_priority(), Some(0));
    assert_eq!(queue.snapshot().qos_profile(), profile);
}

#[test]
fn mshr_qos_class_exports_transport_qos_class() {
    let qos = MshrQosClass::new(42, 3).transport_qos_class();

    assert_eq!(qos.requestor().get(), 42);
    assert_eq!(qos.priority().get(), 3);
}

#[test]
fn mshr_queue_config_rejects_vector_lengths_above_host_limit() {
    assert!(matches!(
        MshrQueueConfig::new(MSHR_ENTRIES_BYTE_OVERFLOW_LENGTH, 1, 0),
        Err(MshrQueueError::VectorLengthTooLarge {
            field: "entries",
            length: MSHR_ENTRIES_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
    assert!(matches!(
        MshrQueueConfig::new(1, MSHR_TARGETS_PER_MSHR_BYTE_OVERFLOW_LENGTH, 0),
        Err(MshrQueueError::VectorLengthTooLarge {
            field: "targets per MSHR",
            length: MSHR_TARGETS_PER_MSHR_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
}

#[test]
fn mshr_queue_rejects_bad_configs_unknown_handles_and_wrong_snapshots() {
    assert_eq!(
        MshrQueueConfig::new(0, 2, 0),
        Err(MshrQueueError::ZeroEntries)
    );
    assert_eq!(
        MshrQueueConfig::new(2, 0, 0),
        Err(MshrQueueError::ZeroTargetsPerMshr)
    );
    assert_eq!(
        MshrQueueConfig::new(2, 1, 2),
        Err(MshrQueueError::DemandReserveExceedsEntries {
            demand_reserve: 2,
            entries: 2,
        })
    );

    let mut queue = MshrQueue::new(MshrQueueConfig::new(1, 1, 0).unwrap());
    let other = MshrQueue::new(MshrQueueConfig::new(2, 1, 0).unwrap());
    assert_eq!(
        queue.restore(&other.snapshot()),
        Err(MshrQueueError::SnapshotConfigMismatch {
            expected: MshrQueueConfig::new(1, 1, 0).unwrap(),
            actual: MshrQueueConfig::new(2, 1, 0).unwrap(),
        })
    );
    let overflowing_entries = MshrQueueSnapshot::new(
        MshrQueueConfig::new(1, 1, 0).unwrap(),
        vec![
            MshrEntry::from_parts(
                MshrHandle::new(1),
                Address::new(0x1000),
                0,
                0,
                false,
                false,
                vec![MshrTarget::from_parts(
                    request(10, 0x1000),
                    0,
                    0,
                    MshrTargetSource::Demand,
                    true,
                    None,
                )],
            ),
            MshrEntry::from_parts(
                MshrHandle::new(2),
                Address::new(0x2000),
                0,
                1,
                false,
                false,
                vec![MshrTarget::from_parts(
                    request(11, 0x2000),
                    0,
                    1,
                    MshrTargetSource::Demand,
                    true,
                    None,
                )],
            ),
        ],
        3,
        2,
    );
    assert_eq!(
        queue.restore(&overflowing_entries),
        Err(MshrQueueError::SnapshotTooManyEntries {
            entries: 2,
            max_entries: 1,
        })
    );
    let overflowing_targets = MshrQueueSnapshot::new(
        MshrQueueConfig::new(1, 1, 0).unwrap(),
        vec![MshrEntry::from_parts(
            MshrHandle::new(3),
            Address::new(0x3000),
            0,
            0,
            false,
            false,
            vec![
                MshrTarget::from_parts(
                    request(12, 0x3000),
                    0,
                    0,
                    MshrTargetSource::Demand,
                    true,
                    None,
                ),
                MshrTarget::from_parts(
                    request(13, 0x3008),
                    0,
                    1,
                    MshrTargetSource::Demand,
                    true,
                    None,
                ),
            ],
        )],
        4,
        2,
    );
    assert_eq!(
        queue.restore(&overflowing_targets),
        Err(MshrQueueError::SnapshotTooManyTargets {
            handle: MshrHandle::new(3),
            targets: 2,
            max_targets: 1,
        })
    );
    assert_eq!(
        queue.mark_in_service(MshrHandle::new(99), false),
        Err(MshrQueueError::UnknownEntry {
            handle: MshrHandle::new(99),
        })
    );
}

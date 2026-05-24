use rem6_cache::{MshrQosClass, MshrQueue, MshrQueueConfig, MshrQueueError, MshrTargetSource};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId};

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
    assert_eq!(
        queue.mark_in_service(rem6_cache::MshrHandle::new(99), false),
        Err(MshrQueueError::UnknownEntry {
            handle: rem6_cache::MshrHandle::new(99),
        })
    );
}

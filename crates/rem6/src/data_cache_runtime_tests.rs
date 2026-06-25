use std::sync::{Arc, Mutex};

use rem6_cache::{QueuedPrefetchSourceStatus, TaggedPrefetchAccess};
use rem6_dram::{DramControllerConfig, DramGeometry, DramMemoryController, DramTiming};
use rem6_memory::{
    AccessSize, Address, AddressRange, ByteMask, MemoryRequest, MemoryRequestId, MemoryTargetId,
    PartitionedMemoryStore, ResponseStatus,
};

use super::*;

const TARGET: MemoryTargetId = MemoryTargetId::new(7);

#[test]
fn respond_for_request_records_internal_error_before_retry() {
    let layout = CacheLineLayout::new(32).unwrap();
    let memory = memory_with_line(layout);
    let runtime = CliDataCacheRuntime::new_msi_bank(layout, [AgentId::new(0)], None).unwrap();
    let request = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 1),
        Address::new(0x1018),
        AccessSize::new(16).unwrap(),
        layout,
    )
    .unwrap();

    let outcome = runtime
        .respond_for_request(&memory, &[], 4, &request)
        .expect("matching layout should be handled by the data cache runtime");

    let TargetOutcome::Respond(response) = outcome else {
        panic!("internal data-cache errors should return a retry response");
    };
    assert_eq!(response.status(), ResponseStatus::Retry);
    let error = runtime
        .take_error()
        .expect("internal data-cache error should be recorded");
    let error = error.to_string();
    assert!(error.contains("outside cache line"), "{error}");
}

#[test]
fn failed_store_conditional_does_not_emit_dram_write() {
    let layout = CacheLineLayout::new(32).unwrap();
    let memory = dram_memory_with_line(layout);
    let runtime = CliDataCacheRuntime::new_msi_bank(layout, [AgentId::new(0)], None).unwrap();
    let store_conditional = store_conditional_request(layout, 1);

    let outcome = runtime
        .respond_for_request(&memory, &[], 12, &store_conditional)
        .expect("matching layout should be handled by the data cache runtime");

    let TargetOutcome::Respond(response) = outcome else {
        panic!("store conditional should produce a response");
    };
    assert_eq!(response.status(), ResponseStatus::StoreConditionalFailed);
    let summary = memory.dram_summary_until(128);
    assert_eq!(summary.accesses, 1);
    assert_eq!(summary.reads, 1);
    assert_eq!(summary.writes, 0);
}

#[test]
fn prefetch_summary_preserves_useful_but_miss_count() {
    let layout = CacheLineLayout::new(32).unwrap();
    let mut prefetch =
        CliDataCachePrefetchRuntime::new(CliCachePrefetcher::TaggedNextLine, layout).unwrap();

    prefetch.queue.record_useful_prefetch(true).unwrap();

    let summary = prefetch.summary();
    assert_eq!(summary.useful, 1);
    assert_eq!(summary.useful_but_miss, 1);
}

#[test]
fn prefetch_summary_preserves_late_and_unused_counts() {
    let layout = CacheLineLayout::new(32).unwrap();
    let mut prefetch =
        CliDataCachePrefetchRuntime::new(CliCachePrefetcher::TaggedNextLine, layout).unwrap();

    prefetch.queue.record_prefetch_unused();
    prefetch.queue.record_prefetch_hit_in_cache();
    prefetch.queue.record_prefetch_hit_in_mshr();
    prefetch.queue.record_prefetch_hit_in_write_buffer();

    let summary = prefetch.summary();
    assert_eq!(summary.unused, 1);
    assert_eq!(summary.hit_in_cache, 1);
    assert_eq!(summary.hit_in_mshr, 1);
    assert_eq!(summary.hit_in_write_buffer, 1);
    assert_eq!(summary.late, 3);
}

#[test]
fn prefetch_summary_preserves_useful_span_page_count() {
    let layout = CacheLineLayout::new(32).unwrap();
    let mut prefetch =
        CliDataCachePrefetchRuntime::new(CliCachePrefetcher::TaggedNextLine, layout).unwrap();

    let candidates = prefetch
        .tagged
        .observe(TaggedPrefetchAccess::new(
            AgentId::new(0),
            0x100,
            Address::new(0xfe0),
            false,
        ))
        .unwrap()
        .to_vec();
    prefetch
        .queue
        .enqueue_candidates_filtered_with_source(
            4,
            &candidates,
            &[],
            QueuedPrefetchSourceStatus::prefetched(),
        )
        .unwrap();

    let summary = prefetch.summary();
    assert_eq!(summary.span_page, 1);
    assert_eq!(summary.useful_span_page, 1);
}

#[test]
fn prefetch_useful_but_miss_records_upgrade_miss_on_prefetched_line() {
    let layout = CacheLineLayout::new(32).unwrap();
    let memory = memory_with_line(layout);
    let runtime = CliDataCacheRuntime::new_msi_bank(
        layout,
        [AgentId::new(0)],
        Some(CliCachePrefetcher::TaggedNextLine),
    )
    .unwrap();
    let read = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 1),
        Address::new(0x1000),
        AccessSize::new(8).unwrap(),
        layout,
    )
    .unwrap();
    let outcome = runtime
        .respond_for_request(&memory, &[], 4, &read)
        .expect("initial read should fill the data cache");
    assert!(matches!(outcome, TargetOutcome::Respond(_)));
    runtime
        .prefetch
        .as_ref()
        .expect("prefetch runtime")
        .lock()
        .expect("prefetch lock")
        .mark_issued(Address::new(0x1000), layout);

    let read_unique = MemoryRequest::read_unique(
        MemoryRequestId::new(AgentId::new(0), 2),
        Address::new(0x1000),
        AccessSize::new(8).unwrap(),
        layout,
    )
    .unwrap();
    let outcome = runtime
        .respond_for_request(&memory, &[], 8, &read_unique)
        .expect("read-unique should upgrade the prefetched line");
    assert!(matches!(outcome, TargetOutcome::Respond(_)));

    let summary = runtime.prefetch_summary();
    assert_eq!(summary.useful, 1);
    assert_eq!(summary.useful_but_miss, 1);
}

#[test]
fn l1_write_invalidates_lower_cache_fill() {
    let layout = CacheLineLayout::new(32).unwrap();
    let memory = memory_with_line(layout);
    let l1 = CliDataCacheRuntime::new_msi_bank(layout, [AgentId::new(0)], None).unwrap();
    let l2 = CliDataCacheRuntime::new_msi_bank(layout, [AgentId::new(0)], None).unwrap();
    let read = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 1),
        Address::new(0x1000),
        AccessSize::new(8).unwrap(),
        layout,
    )
    .unwrap();

    let outcome = l1
        .respond_for_request(&memory, std::slice::from_ref(&l2), 4, &read)
        .expect("L1 should handle a read through L2");
    assert!(matches!(outcome, TargetOutcome::Respond(_)));
    assert!(l2.contains_line(Address::new(0x1000)));

    let size = AccessSize::new(8).unwrap();
    let write = MemoryRequest::write(
        MemoryRequestId::new(AgentId::new(0), 2),
        Address::new(0x1000),
        size,
        0x1122_3344_5566_7788u64.to_le_bytes().to_vec(),
        ByteMask::full(size).unwrap(),
        layout,
    )
    .unwrap();
    let outcome = l1
        .respond_for_request(&memory, std::slice::from_ref(&l2), 8, &write)
        .expect("L1 should handle a write through the lower hierarchy");
    assert!(matches!(outcome, TargetOutcome::Respond(_)));

    assert!(!l2.contains_line(Address::new(0x1000)));
    assert_eq!(
        memory.read_guest_memory(0x1000, 8, layout),
        Some(0x1122_3344_5566_7788u64.to_le_bytes().to_vec())
    );
}

fn memory_with_line(layout: CacheLineLayout) -> CliMemoryRuntime {
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(TARGET, layout).unwrap();
    store
        .map_region(
            TARGET,
            Address::new(0x1000),
            AccessSize::new(layout.bytes()).unwrap(),
        )
        .unwrap();
    store
        .insert_line(
            TARGET,
            Address::new(0x1000),
            vec![0xa5; layout.bytes() as usize],
        )
        .unwrap();
    CliMemoryRuntime::Store {
        store: Arc::new(Mutex::new(store)),
        full_line_backing: Arc::new(Mutex::new(vec![AddressRange::new(
            Address::new(0x1000),
            AccessSize::new(layout.bytes()).unwrap(),
        )
        .unwrap()])),
    }
}

fn dram_memory_with_line(layout: CacheLineLayout) -> CliMemoryRuntime {
    let mut memory = DramMemoryController::new();
    memory
        .add_target(DramControllerConfig::new(
            TARGET,
            layout,
            DramGeometry::new(4, 64, layout.bytes()).unwrap(),
            DramTiming::new(3, 5, 7, 2, 4).unwrap(),
        ))
        .unwrap();
    memory
        .map_region(
            TARGET,
            Address::new(0x1000),
            AccessSize::new(layout.bytes()).unwrap(),
        )
        .unwrap();
    memory
        .insert_line(
            TARGET,
            Address::new(0x1000),
            vec![0xa5; layout.bytes() as usize],
        )
        .unwrap();
    CliMemoryRuntime::Dram {
        memory: Arc::new(Mutex::new(memory)),
        full_line_backing: Arc::new(Mutex::new(vec![AddressRange::new(
            Address::new(0x1000),
            AccessSize::new(layout.bytes()).unwrap(),
        )
        .unwrap()])),
    }
}

fn store_conditional_request(layout: CacheLineLayout, sequence: u64) -> MemoryRequest {
    let size = AccessSize::new(8).unwrap();
    MemoryRequest::store_conditional(
        MemoryRequestId::new(AgentId::new(0), sequence),
        Address::new(0x1000),
        size,
        0x0123_4567_89ab_cdefu64.to_le_bytes().to_vec(),
        ByteMask::full(size).unwrap(),
        layout,
    )
    .unwrap()
}

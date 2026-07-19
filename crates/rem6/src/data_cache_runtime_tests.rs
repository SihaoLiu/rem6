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

    let TargetOutcome::RespondAfter { delay, response } = outcome else {
        panic!("cold store conditional should wait for its DRAM backing read");
    };
    assert_eq!(delay, 8);
    assert_eq!(response.status(), ResponseStatus::StoreConditionalFailed);
    let summary = memory.dram_summary_until(128);
    assert_eq!(summary.accesses, 1);
    assert_eq!(summary.reads, 1);
    assert_eq!(summary.writes, 0);
}

#[test]
fn cold_dram_backed_cache_fill_waits_for_dram_ready_cycle() {
    let layout = CacheLineLayout::new(32).unwrap();
    let memory = dram_memory_with_line(layout);
    let runtime = CliDataCacheRuntime::new_msi_bank(layout, [AgentId::new(0)], None).unwrap();
    let read = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 2),
        Address::new(0x1000),
        AccessSize::new(8).unwrap(),
        layout,
    )
    .unwrap();

    let outcome = runtime
        .respond_for_request(&memory, &[], 12, &read)
        .expect("matching layout should be handled by the data cache runtime");

    let TargetOutcome::RespondAfter { delay, response } = outcome else {
        panic!("cold DRAM-backed cache fill should wait for the DRAM response");
    };
    assert_eq!(delay, 8);
    assert_eq!(response.status(), ResponseStatus::Completed);
    assert_eq!(response.data(), Some(&[0xa5; 8][..]));
}

#[test]
fn cold_multilevel_cache_fill_preserves_dram_ready_cycle() {
    let layout = CacheLineLayout::new(32).unwrap();
    let memory = dram_memory_with_line(layout);
    let l1 = CliDataCacheRuntime::new_msi_bank(layout, [AgentId::new(0)], None).unwrap();
    let l2 = CliDataCacheRuntime::new_msi_bank(layout, [AgentId::new(0)], None).unwrap();
    let read = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 5),
        Address::new(0x1000),
        AccessSize::new(8).unwrap(),
        layout,
    )
    .unwrap();

    let outcome = l1
        .respond_for_request(&memory, std::slice::from_ref(&l2), 12, &read)
        .expect("matching hierarchy should handle the cache read");

    let TargetOutcome::RespondAfter { delay, response } = outcome else {
        panic!("cold multilevel fill should preserve the DRAM response tick");
    };
    assert_eq!(delay, 8);
    assert_eq!(response.status(), ResponseStatus::Completed);
    assert!(l1.contains_line(Address::new(0x1000)));
    assert!(l2.contains_line(Address::new(0x1000)));
    assert_eq!(memory.dram_summary_until(128).accesses, 1);
}

#[test]
fn pending_dram_backed_cache_fill_delays_same_line_demand_without_second_access() {
    let layout = CacheLineLayout::new(32).unwrap();
    let memory = dram_memory_with_line(layout);
    let runtime = CliDataCacheRuntime::new_msi_bank(layout, [AgentId::new(0)], None).unwrap();
    let cold_read = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 7),
        Address::new(0x1000),
        AccessSize::new(8).unwrap(),
        layout,
    )
    .unwrap();
    assert!(matches!(
        runtime.respond_for_request(&memory, &[], 12, &cold_read),
        Some(TargetOutcome::RespondAfter { delay: 8, .. })
    ));
    let pending_read = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 8),
        Address::new(0x1000),
        AccessSize::new(8).unwrap(),
        layout,
    )
    .unwrap();

    let outcome = runtime
        .respond_for_request(&memory, &[], 15, &pending_read)
        .expect("pending cache fill should retain same-line ownership");

    let TargetOutcome::RespondAfter { delay, response } = outcome else {
        panic!("same-line demand before fill readiness should remain delayed");
    };
    assert_eq!(delay, 5);
    assert_eq!(response.status(), ResponseStatus::Completed);
    assert_eq!(memory.dram_summary_until(128).accesses, 1);
}

#[test]
fn pending_dram_prefetch_delays_demand_and_records_useful_miss() {
    let layout = CacheLineLayout::new(32).unwrap();
    let memory = dram_memory_with_two_lines(layout);
    let runtime = CliDataCacheRuntime::new_msi_bank(
        layout,
        [AgentId::new(0)],
        Some(CliCachePrefetcher::TaggedNextLine),
    )
    .unwrap();
    let first = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 9),
        Address::new(0x1000),
        AccessSize::new(8).unwrap(),
        layout,
    )
    .unwrap();
    assert!(matches!(
        runtime.respond_for_request(&memory, &[], 12, &first),
        Some(TargetOutcome::RespondAfter { .. })
    ));
    assert_eq!(memory.dram_summary_until(128).accesses, 2);
    let prefetched = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 10),
        Address::new(0x1020),
        AccessSize::new(8).unwrap(),
        layout,
    )
    .unwrap();

    let outcome = runtime
        .respond_for_request(&memory, &[], 13, &prefetched)
        .expect("pending prefetched line should retain cache ownership");

    let TargetOutcome::RespondAfter { response, .. } = outcome else {
        panic!("demand must wait for the pending DRAM prefetch");
    };
    assert_eq!(response.data(), Some(&[0x5a; 8][..]));
    assert_eq!(memory.dram_summary_until(128).accesses, 2);
    let summary = runtime.prefetch_summary();
    assert_eq!(summary.useful, 1);
    assert_eq!(summary.useful_but_miss, 1);
    assert_eq!(summary.demand_mshr_misses, 2);
}

#[test]
fn pending_prefetch_is_classified_as_mshr_resident_until_ready() {
    let layout = CacheLineLayout::new(32).unwrap();
    let memory = dram_memory_with_two_lines(layout);
    let runtime = CliDataCacheRuntime::new_msi_bank(
        layout,
        [AgentId::new(0)],
        Some(CliCachePrefetcher::TaggedNextLine),
    )
    .unwrap();
    let first = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 11),
        Address::new(0x1000),
        AccessSize::new(8).unwrap(),
        layout,
    )
    .unwrap();
    assert!(matches!(
        runtime.respond_for_request(&memory, &[], 12, &first),
        Some(TargetOutcome::RespondAfter { .. })
    ));
    let repeated = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 12),
        Address::new(0x1000),
        AccessSize::new(8).unwrap(),
        layout,
    )
    .unwrap();

    assert!(matches!(
        runtime.respond_for_request(&memory, &[], 13, &repeated),
        Some(TargetOutcome::RespondAfter { .. })
    ));

    let summary = runtime.prefetch_summary();
    assert_eq!(summary.hit_in_mshr, 1);
    assert_eq!(summary.hit_in_cache, 0);
    assert_eq!(memory.dram_summary_until(128).accesses, 2);
}

#[test]
fn resident_dram_backed_cache_hit_responds_without_dram_delay() {
    let layout = CacheLineLayout::new(32).unwrap();
    let memory = dram_memory_with_line(layout);
    let runtime = CliDataCacheRuntime::new_msi_bank(layout, [AgentId::new(0)], None).unwrap();
    let cold_read = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 3),
        Address::new(0x1000),
        AccessSize::new(8).unwrap(),
        layout,
    )
    .unwrap();
    assert!(matches!(
        runtime.respond_for_request(&memory, &[], 12, &cold_read),
        Some(TargetOutcome::RespondAfter { delay: 8, .. })
    ));
    let hit_read = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 4),
        Address::new(0x1000),
        AccessSize::new(8).unwrap(),
        layout,
    )
    .unwrap();

    let outcome = runtime
        .respond_for_request(&memory, &[], 21, &hit_read)
        .expect("resident cache line should be handled by the data cache runtime");

    let TargetOutcome::Respond(response) = outcome else {
        panic!("resident cache hit should respond without a DRAM delay");
    };
    assert_eq!(response.status(), ResponseStatus::Completed);
    assert_eq!(response.data(), Some(&[0xa5; 8][..]));
    assert_eq!(memory.dram_summary_until(128).accesses, 1);
}

#[test]
fn cache_backing_delay_is_a_floor_not_a_replacement() {
    let layout = CacheLineLayout::new(32).unwrap();
    let read = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 6),
        Address::new(0x1000),
        AccessSize::new(8).unwrap(),
        layout,
    )
    .unwrap();
    let response = MemoryResponse::completed(&read, Some(vec![0xa5; 8])).unwrap();

    let adjusted = delay_target_outcome_until(
        TargetOutcome::RespondAfter {
            delay: 12,
            response,
        },
        10,
        18,
    );

    let TargetOutcome::RespondAfter { delay, .. } = adjusted else {
        panic!("delayed cache response should remain delayed");
    };
    assert_eq!(delay, 12);
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
fn lower_fill_request_preserves_prefetch_read_for_hierarchy_consumers() {
    let layout = CacheLineLayout::new(32).unwrap();
    let prefetch = MemoryRequest::prefetch_read(
        MemoryRequestId::new(AgentId::new(0), 9),
        Address::new(0x1040),
        AccessSize::new(32).unwrap(),
        layout,
    )
    .unwrap();
    let prefetch =
        MemoryRequest::from_snapshot(&prefetch.snapshot().with_response_required()).unwrap();

    let fill = lower_fill_request(&prefetch, Address::new(0x1040), layout).unwrap();

    assert_eq!(fill.operation(), MemoryOperation::PrefetchRead);
    assert!(fill.requires_response());
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

#[test]
fn cross_line_cli_memory_response_merges_read_data() {
    let layout = CacheLineLayout::new(16).unwrap();
    let memory = memory_with_two_lines(layout);
    let read = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 1),
        Address::new(0x1008),
        AccessSize::new(24).unwrap(),
        layout,
    )
    .unwrap();

    let outcome = cli_cross_line_memory_response_with(&read, |request| {
        cli_memory_response_for_request(&memory, 7, request)
    })
    .expect("cross-line read should split into line requests");

    let TargetOutcome::Respond(response) = outcome else {
        panic!("store-backed cross-line read should respond immediately");
    };
    assert_eq!(response.status(), ResponseStatus::Completed);
    assert_eq!(
        response.data(),
        Some(
            (0x1008_u64..0x1020)
                .map(|address| address as u8)
                .collect::<Vec<_>>()
                .as_slice()
        )
    );
}

#[test]
fn cross_line_cli_memory_response_applies_write_chunks() {
    let layout = CacheLineLayout::new(16).unwrap();
    let memory = memory_with_two_lines(layout);
    let size = AccessSize::new(24).unwrap();
    let data = (0x80_u8..0x98).collect::<Vec<_>>();
    let write = MemoryRequest::write(
        MemoryRequestId::new(AgentId::new(0), 2),
        Address::new(0x1008),
        size,
        data.clone(),
        ByteMask::full(size).unwrap(),
        layout,
    )
    .unwrap();

    let outcome = cli_cross_line_memory_response_with(&write, |request| {
        cli_memory_response_for_request(&memory, 11, request)
    })
    .expect("cross-line write should split into line requests");

    let TargetOutcome::Respond(response) = outcome else {
        panic!("store-backed cross-line write should respond immediately");
    };
    assert_eq!(response.status(), ResponseStatus::Completed);
    assert_eq!(response.data(), None);
    assert_eq!(
        memory.read_guest_memory(0x1008, 24, layout),
        Some(data),
        "split write chunks should update both touched cache lines"
    );
}

#[test]
fn cross_line_cli_memory_response_preserves_write_masks() {
    let layout = CacheLineLayout::new(16).unwrap();
    let memory = memory_with_two_lines(layout);
    let size = AccessSize::new(24).unwrap();
    let mut mask = vec![true; 24];
    mask[3] = false;
    mask[20] = false;
    let data = (0x80_u8..0x98).collect::<Vec<_>>();
    let write = MemoryRequest::write(
        MemoryRequestId::new(AgentId::new(0), 3),
        Address::new(0x1008),
        size,
        data.clone(),
        ByteMask::from_bits(mask.clone()).unwrap(),
        layout,
    )
    .unwrap();

    let mut seen_masks = Vec::new();
    let outcome = cli_cross_line_memory_response_with(&write, |request| {
        seen_masks.push(request.byte_mask().unwrap().bits().to_vec());
        cli_memory_response_for_request(&memory, 13, request)
    })
    .expect("cross-line masked write should split into masked line requests");

    let TargetOutcome::Respond(response) = outcome else {
        panic!("store-backed cross-line masked write should respond immediately");
    };
    assert_eq!(response.status(), ResponseStatus::Completed);
    assert_eq!(response.data(), None);
    assert_eq!(seen_masks, vec![mask[..8].to_vec(), mask[8..].to_vec()]);

    let expected = data
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            if mask[index] {
                *byte
            } else {
                (0x1008 + index as u64) as u8
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(memory.read_guest_memory(0x1008, 24, layout), Some(expected));
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

fn dram_memory_with_two_lines(layout: CacheLineLayout) -> CliMemoryRuntime {
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
            AccessSize::new(layout.bytes() * 2).unwrap(),
        )
        .unwrap();
    for (line, byte) in [(0x1000, 0xa5), (0x1020, 0x5a)] {
        memory
            .insert_line(
                TARGET,
                Address::new(line),
                vec![byte; layout.bytes() as usize],
            )
            .unwrap();
    }
    CliMemoryRuntime::Dram {
        memory: Arc::new(Mutex::new(memory)),
        full_line_backing: Arc::new(Mutex::new(vec![AddressRange::new(
            Address::new(0x1000),
            AccessSize::new(layout.bytes() * 2).unwrap(),
        )
        .unwrap()])),
    }
}

fn memory_with_two_lines(layout: CacheLineLayout) -> CliMemoryRuntime {
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(TARGET, layout).unwrap();
    store
        .map_region(
            TARGET,
            Address::new(0x1000),
            AccessSize::new(layout.bytes() * 2).unwrap(),
        )
        .unwrap();
    for line in 0..2 {
        let address = 0x1000 + line * layout.bytes();
        let bytes = (address..address + layout.bytes())
            .map(|byte| byte as u8)
            .collect();
        store
            .insert_line(TARGET, Address::new(address), bytes)
            .unwrap();
    }
    CliMemoryRuntime::Store {
        store: Arc::new(Mutex::new(store)),
        full_line_backing: Arc::new(Mutex::new(vec![AddressRange::new(
            Address::new(0x1000),
            AccessSize::new(layout.bytes() * 2).unwrap(),
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

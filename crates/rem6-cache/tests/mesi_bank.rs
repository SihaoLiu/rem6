use rem6_cache::CacheWriteQueueHandle;
use rem6_cache::{
    CacheWriteQueueConfig, CacheWriteQueueEntryKind, CacheWriteQueueError, MesiCacheBank,
    MesiCacheBankError, MesiCacheControllerError, MesiCacheControllerResultKind,
    MesiPendingUncacheableReadSnapshot, MshrQosClass, MshrQueueConfig,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryError, MemoryOperation,
    MemoryRequest, MemoryRequestId, MemoryResponse,
};
use rem6_protocol_mesi::{MesiEvent, MesiState};
use rem6_transport::TargetOutcome;

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn wide_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn size(bytes: u64) -> AccessSize {
    AccessSize::new(bytes).unwrap()
}

fn read(agent_id: AgentId, sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(address),
        size(8),
        layout(),
    )
    .unwrap()
}

fn uncacheable_read(agent_id: AgentId, sequence: u64, address: u64) -> MemoryRequest {
    read(agent_id, sequence, address).with_uncacheable_strict_order()
}

fn uncacheable_atomic(
    agent_id: AgentId,
    sequence: u64,
    address: u64,
    data: Vec<u8>,
) -> MemoryRequest {
    let access_size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::atomic(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(address),
        access_size,
        data,
        ByteMask::full(access_size).unwrap(),
        layout(),
    )
    .unwrap()
    .with_uncacheable_strict_order()
}

fn write(agent_id: AgentId, sequence: u64, address: u64, data: Vec<u8>) -> MemoryRequest {
    let access_size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::write(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(address),
        access_size,
        data,
        ByteMask::full(access_size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn dirty_writeback(agent_id: AgentId, sequence: u64, line: u64, value: u8) -> MemoryRequest {
    dirty_writeback_with_layout(agent_id, sequence, line, value, layout())
}

fn dirty_writeback_with_layout(
    agent_id: AgentId,
    sequence: u64,
    line: u64,
    value: u8,
    line_layout: CacheLineLayout,
) -> MemoryRequest {
    MemoryRequest::writeback_dirty(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(line),
        vec![value; line_layout.bytes() as usize],
        line_layout,
    )
    .unwrap()
}

fn clean_writeback(agent_id: AgentId, sequence: u64, line: u64, value: u8) -> MemoryRequest {
    MemoryRequest::writeback_clean(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(line),
        vec![value; layout().bytes() as usize],
        layout(),
    )
    .unwrap()
}

fn clean_evict(agent_id: AgentId, sequence: u64, line: u64) -> MemoryRequest {
    MemoryRequest::clean_evict(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(line),
        layout(),
    )
    .unwrap()
}

fn uncacheable_write(
    agent_id: AgentId,
    sequence: u64,
    address: u64,
    data: Vec<u8>,
    mask: Vec<bool>,
) -> MemoryRequest {
    MemoryRequest::write(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(address),
        AccessSize::new(data.len() as u64).unwrap(),
        data,
        ByteMask::from_bits(mask).unwrap(),
        layout(),
    )
    .unwrap()
    .with_uncacheable_strict_order()
}

fn fill(request: &MemoryRequest, byte: u8) -> MemoryResponse {
    MemoryResponse::completed(request, Some(vec![byte; layout().bytes() as usize])).unwrap()
}

fn response_data(outcome: &TargetOutcome) -> &[u8] {
    match outcome {
        TargetOutcome::Respond(response) => response.data().unwrap(),
        other => panic!("expected response, got {other:?}"),
    }
}

fn response_id(outcome: &TargetOutcome) -> MemoryRequestId {
    match outcome {
        TargetOutcome::Respond(response) => response.request_id(),
        TargetOutcome::RespondAfter { response, .. } => response.request_id(),
        TargetOutcome::NoResponse => panic!("expected response outcome"),
    }
}

#[test]
fn mesi_cache_bank_mshr_coalesces_same_line_read_misses() {
    let cache_agent = agent(20);
    let mut bank = MesiCacheBank::new_with_mshr(
        cache_agent,
        layout(),
        MshrQueueConfig::new(2, 2, 0).unwrap(),
    );

    let first = read(cache_agent, 100, 0x2004);
    let first_miss = bank.accept_cpu_request(first.clone()).unwrap();
    let first_downstream = first_miss.downstream_request().unwrap().clone();
    assert_eq!(bank.pending_fill_count(), 1);
    assert_eq!(bank.mshr_allocated_count(), 1);
    assert_eq!(bank.mshr_target_count(Address::new(0x2000)), Some(1));

    let second = read(cache_agent, 101, 0x2008);
    let second_miss = bank.accept_cpu_request(second.clone()).unwrap();
    assert!(second_miss.downstream_request().is_none());
    assert_eq!(second_miss.target_outcomes(), &[]);
    assert_eq!(bank.pending_fill_count(), 1);
    assert_eq!(bank.mshr_allocated_count(), 1);
    assert_eq!(bank.mshr_target_count(Address::new(0x2000)), Some(2));

    let fill_result = bank
        .accept_fill(fill(&first_downstream, 0x44), MesiEvent::DataShared)
        .unwrap();
    assert_eq!(fill_result.target_outcomes().len(), 2);
    assert_eq!(
        fill_result
            .target_outcomes()
            .iter()
            .map(response_id)
            .collect::<Vec<_>>(),
        vec![first.id(), second.id()]
    );
    assert_eq!(response_data(&fill_result.target_outcomes()[0]), &[0x44; 8]);
    assert_eq!(response_data(&fill_result.target_outcomes()[1]), &[0x44; 8]);
    assert_eq!(bank.pending_fill_count(), 0);
    assert_eq!(bank.mshr_allocated_count(), 0);
}

#[test]
fn mesi_cache_bank_records_mshr_qos_for_merged_read_misses() {
    let cache_agent = agent(20);
    let config = MshrQueueConfig::new(2, 3, 0).unwrap();
    let mut bank = MesiCacheBank::new_with_mshr(cache_agent, layout(), config.clone());

    bank.accept_cpu_request_with_qos(read(cache_agent, 300, 0x2804), MshrQosClass::new(30, 5))
        .unwrap();
    assert_eq!(
        bank.mshr_effective_qos(Address::new(0x2800)),
        Some(MshrQosClass::new(30, 5))
    );

    bank.accept_cpu_request_with_qos(read(cache_agent, 301, 0x2808), MshrQosClass::new(40, 1))
        .unwrap();
    assert_eq!(
        bank.mshr_effective_qos(Address::new(0x2800)),
        Some(MshrQosClass::new(40, 1))
    );

    let snapshot = bank.snapshot();
    let bank_profile = bank.mshr_qos_profile().unwrap();
    assert_eq!(bank_profile.entry_count(), 1);
    assert_eq!(bank_profile.target_count(), 2);
    assert_eq!(bank_profile.qos_target_count(), 2);
    assert_eq!(bank_profile.effective_entry_count(), 1);
    assert_eq!(bank_profile.priority_target_count(1), 1);
    assert_eq!(bank_profile.priority_target_count(5), 1);
    assert_eq!(bank_profile.effective_priority_entry_count(1), 1);
    assert_eq!(bank_profile.effective_requestor_entry_count(40), 1);
    assert_eq!(bank_profile.best_effective_priority(), Some(1));
    assert_eq!(snapshot.mshr_qos_profile(), Some(bank_profile.clone()));

    let mut restored = MesiCacheBank::new_with_mshr(cache_agent, layout(), config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(
        restored.mshr_effective_qos(Address::new(0x2800)),
        Some(MshrQosClass::new(40, 1))
    );
    assert_eq!(restored.mshr_qos_profile(), Some(bank_profile));
}

#[test]
fn mesi_cache_bank_write_queue_orders_issues_and_restores_snapshot() {
    let cache_agent = agent(20);
    let mut bank = MesiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(2, 1).unwrap(),
    );

    let dirty = bank
        .enqueue_writeback(dirty_writeback(cache_agent, 400, 0x1000, 0xaa), false, 20)
        .unwrap();
    assert_eq!(dirty.handle().index(), 0);
    assert_eq!(bank.write_queue_allocated_count(), 1);
    assert_eq!(bank.write_queue_next_ready_tick(), Some(20));

    let evict = bank
        .enqueue_writeback(clean_evict(cache_agent, 401, 0x2000), true, 10)
        .unwrap();
    assert_eq!(bank.write_queue_allocated_count(), 2);
    assert_eq!(bank.write_queue_ready_handles(9), Vec::new());
    assert_eq!(bank.write_queue_ready_handles(10), vec![evict.handle()]);
    assert_eq!(
        bank.write_queue_ready_handles(20),
        vec![evict.handle(), dirty.handle()]
    );

    assert_eq!(
        bank.enqueue_writeback(clean_writeback(cache_agent, 402, 0x3000, 0xbb), false, 5),
        Err(MesiCacheBankError::WriteQueue(
            CacheWriteQueueError::EntrySlotsFull {
                entries: 2,
                reserve: 1,
            },
        ))
    );

    let reserve = bank
        .enqueue_reserved_writeback(clean_writeback(cache_agent, 402, 0x3000, 0xbb), false, 5)
        .unwrap();
    assert_eq!(reserve.reserve_used(), 1);
    assert_eq!(bank.write_queue_allocated_count(), 3);

    let snapshot = bank.snapshot();
    let issued = bank.issue_write_queue(20).unwrap().unwrap();
    assert_eq!(issued.handle(), reserve.handle());
    assert_eq!(issued.kind(), CacheWriteQueueEntryKind::WritebackClean);
    assert_eq!(bank.write_queue_allocated_count(), 2);

    bank.restore(&snapshot).unwrap();
    assert_eq!(
        bank.write_queue_ready_handles(20),
        vec![reserve.handle(), evict.handle(), dirty.handle()]
    );
    let reissued = bank.issue_write_queue(20).unwrap().unwrap();
    assert_eq!(reissued.handle(), reserve.handle());
    assert_eq!(
        reissued.request().operation(),
        MemoryOperation::WritebackClean
    );
}

#[test]
fn mesi_cache_bank_write_queue_tracks_conflicts_functional_reads_and_uncached_matches() {
    let cache_agent = agent(20);
    let mut bank = MesiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(4, 0).unwrap(),
    );
    let dirty = MemoryRequest::writeback_dirty(
        MemoryRequestId::new(cache_agent, 410),
        Address::new(0x1000),
        (0_u8..16).collect(),
        layout(),
    )
    .unwrap();
    let dirty = bank.enqueue_writeback(dirty, false, 10).unwrap();

    assert_eq!(
        bank.write_queue_find_match(Address::new(0x1000), false, true),
        Some(dirty.handle())
    );
    assert_eq!(
        bank.write_queue_pending_conflict(Address::new(0x1000), false),
        Some(dirty.handle())
    );
    assert_eq!(
        bank.write_queue_satisfy_read(Address::new(0x1004), size(4), false)
            .unwrap(),
        Some(vec![4, 5, 6, 7])
    );
    assert_eq!(
        bank.write_queue_satisfy_read(Address::new(0x1004), size(4), true)
            .unwrap(),
        None
    );

    let snapshot = bank.snapshot();
    let issued = bank.issue_write_queue(10).unwrap().unwrap();
    assert_eq!(issued.handle(), dirty.handle());
    assert_eq!(
        bank.write_queue_pending_conflict(Address::new(0x1000), false),
        None
    );
    bank.restore(&snapshot).unwrap();
    assert_eq!(
        bank.write_queue_satisfy_read(Address::new(0x1004), size(4), false)
            .unwrap(),
        Some(vec![4, 5, 6, 7])
    );

    let uncached = bank
        .enqueue_uncacheable_write(
            uncacheable_write(
                cache_agent,
                411,
                0x2020,
                vec![0xde, 0xad, 0xbe, 0xef],
                vec![true, false, true, true],
            ),
            false,
            12,
        )
        .unwrap();
    assert_eq!(
        bank.write_queue_find_match(Address::new(0x2020), false, true),
        None
    );
    assert_eq!(
        bank.write_queue_find_match(Address::new(0x2020), false, false),
        Some(uncached.handle())
    );
    assert_eq!(
        bank.write_queue_satisfy_read(Address::new(0x2020), size(1), false)
            .unwrap(),
        Some(vec![0xde])
    );
    assert_eq!(
        bank.write_queue_satisfy_read(Address::new(0x2021), size(1), false)
            .unwrap(),
        None
    );
}

#[test]
fn mesi_cache_bank_write_queue_rejects_foreign_line_layouts() {
    let cache_agent = agent(20);
    let mut bank = MesiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );
    let request = dirty_writeback_with_layout(cache_agent, 420, 0x4000, 0xee, wide_layout());
    let expected_error = MesiCacheBankError::Controller(MesiCacheControllerError::Memory(
        MemoryError::LineLayoutMismatch {
            request: request.id(),
            expected: layout(),
            actual: wide_layout(),
        },
    ));

    assert_eq!(
        bank.enqueue_writeback(request.clone(), false, 1),
        Err(expected_error)
    );
    assert_eq!(bank.write_queue_allocated_count(), 0);
}

#[test]
fn mesi_cache_bank_snapshot_reports_and_restores_dirty_lines() {
    let cache_agent = agent(20);
    let mut bank = MesiCacheBank::new(cache_agent, layout());
    let store = write(cache_agent, 120, 0x2004, vec![0xde, 0xad]);

    let miss = bank.accept_cpu_request(store.clone()).unwrap();
    assert_eq!(
        miss.downstream_request().unwrap().operation(),
        MemoryOperation::ReadUnique
    );
    bank.accept_fill(
        fill(miss.downstream_request().unwrap(), 0x00),
        MesiEvent::DataModified,
    )
    .unwrap();
    assert_eq!(bank.state(Address::new(0x2000)), Some(MesiState::Modified));

    let snapshot = bank.snapshot();
    assert_eq!(snapshot.dirty_line_count(), 1);
    assert_eq!(snapshot.dirty_line_addresses(), vec![Address::new(0x2000)]);

    bank.accept_snoop(Address::new(0x2000), MesiEvent::SnoopWrite)
        .unwrap();
    assert_eq!(bank.state(Address::new(0x2000)), Some(MesiState::Invalid));

    bank.restore(&snapshot).unwrap();
    assert_eq!(bank.state(Address::new(0x2000)), Some(MesiState::Modified));
    let read_back = read(cache_agent, 121, 0x2004);
    let hit = bank.accept_cpu_request(read_back).unwrap();
    assert_eq!(
        response_data(hit.target_outcome().unwrap()),
        &[0xde, 0xad, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    );
}

#[test]
fn mesi_cache_bank_uncacheable_read_preserves_dirty_resident_line() {
    let cache_agent = agent(20);
    let mut bank = MesiCacheBank::new(cache_agent, layout());
    let store = write(cache_agent, 122, 0x2404, vec![0xde, 0xad]);

    let miss = bank.accept_cpu_request(store).unwrap();
    assert_eq!(
        miss.downstream_request().unwrap().operation(),
        MemoryOperation::ReadUnique
    );
    bank.accept_fill(
        fill(miss.downstream_request().unwrap(), 0x00),
        MesiEvent::DataModified,
    )
    .unwrap();
    assert_eq!(bank.state(Address::new(0x2400)), Some(MesiState::Modified));

    let result = bank.accept_cpu_request(uncacheable_read(cache_agent, 123, 0x2408));
    assert!(result.is_err());
    assert_eq!(bank.state(Address::new(0x2400)), Some(MesiState::Modified));
    assert_eq!(
        bank.snapshot().dirty_line_addresses(),
        vec![Address::new(0x2400)]
    );
}

#[test]
fn mesi_cache_bank_uncacheable_atomic_blocks_same_line_until_response() {
    let cache_agent = agent(20);
    let mut bank = MesiCacheBank::new(cache_agent, layout());

    let atomic = uncacheable_atomic(cache_agent, 124, 0x2a08, vec![0x33, 0x44]);
    let atomic_miss = bank.accept_cpu_request(atomic.clone()).unwrap();
    let downstream = atomic_miss.downstream_request().unwrap().clone();

    assert_eq!(atomic_miss.kind(), MesiCacheControllerResultKind::Miss);
    assert_eq!(downstream.id(), atomic.id());
    assert_eq!(downstream.operation(), MemoryOperation::Atomic);
    assert!(downstream.is_uncacheable());
    assert_eq!(bank.pending_fill_count(), 1);

    let write_error = bank
        .accept_cpu_request(write(cache_agent, 125, 0x2a04, vec![0x55]))
        .expect_err("pending uncacheable atomic must block same-line writes");
    assert_eq!(
        write_error.to_string(),
        "MESI cache bank has pending uncacheable request for line 0x2a00"
    );

    let atomic_error = bank
        .accept_cpu_request(uncacheable_atomic(
            cache_agent,
            126,
            0x2a08,
            vec![0x66, 0x77],
        ))
        .expect_err("pending uncacheable atomic must block same-line atomics");
    assert_eq!(
        atomic_error.to_string(),
        "MESI cache bank has pending uncacheable request for line 0x2a00"
    );

    let atomic_fill = bank
        .accept_fill(
            MemoryResponse::completed(&downstream, Some(vec![0xaa, 0xbb])).unwrap(),
            MesiEvent::DataShared,
        )
        .unwrap();
    assert_eq!(
        response_data(atomic_fill.target_outcome().unwrap()),
        &[0xaa, 0xbb]
    );
    assert_eq!(bank.pending_fill_count(), 0);

    let later_write = bank
        .accept_cpu_request(write(cache_agent, 127, 0x2a04, vec![0x88]))
        .unwrap();
    assert_eq!(later_write.kind(), MesiCacheControllerResultKind::Miss);
}

#[test]
fn mesi_cache_bank_dirty_uncacheable_atomic_blocks_same_line_reads_until_writeback() {
    let cache_agent = agent(20);
    let mut bank = MesiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(1, 0).unwrap(),
    );

    let store = write(cache_agent, 128, 0x2b04, vec![0xde, 0xad]);
    let store_miss = bank.accept_cpu_request(store).unwrap();
    bank.accept_fill(
        fill(store_miss.downstream_request().unwrap(), 0x11),
        MesiEvent::DataModified,
    )
    .unwrap();

    let atomic = uncacheable_atomic(cache_agent, 129, 0x2b08, vec![0x33, 0x44]);
    let atomic_miss = bank.accept_cpu_request(atomic).unwrap();
    assert!(atomic_miss.downstream_request().is_none());
    assert_eq!(bank.write_queue_allocated_count(), 1);

    let read_error = bank
        .accept_cpu_request(read(cache_agent, 130, 0x2b04))
        .expect_err("dirty pending atomic must block same-line reads");
    assert_eq!(
        read_error.to_string(),
        "MESI cache bank has pending uncacheable request for line 0x2b00"
    );
}

#[test]
fn mesi_cache_bank_uncacheable_read_queues_dirty_writeback_before_forwarding() {
    let cache_agent = agent(20);
    let mut bank = MesiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(1, 0).unwrap(),
    );
    let store = write(cache_agent, 128, 0x2504, vec![0xde, 0xad]);
    let miss = bank.accept_cpu_request(store).unwrap();
    bank.accept_fill(
        fill(miss.downstream_request().unwrap(), 0x00),
        MesiEvent::DataModified,
    )
    .unwrap();
    assert_eq!(bank.state(Address::new(0x2500)), Some(MesiState::Modified));

    let uncached = uncacheable_read(cache_agent, 129, 0x2508);
    let result = bank.accept_cpu_request(uncached.clone()).unwrap();

    assert_eq!(result.kind(), MesiCacheControllerResultKind::Miss);
    assert!(result.downstream_request().is_none());
    assert_eq!(bank.state(Address::new(0x2500)), None);
    assert_eq!(bank.write_queue_allocated_count(), 1);

    let forwarded = bank
        .accept_cpu_request(read(cache_agent, 130, 0x2504))
        .unwrap();
    assert_eq!(forwarded.kind(), MesiCacheControllerResultKind::Hit);
    assert!(forwarded.downstream_request().is_none());
    assert_eq!(
        response_data(forwarded.target_outcome().unwrap()),
        &[0xde, 0xad, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    );
    assert!(matches!(
        bank.accept_cpu_request(write(cache_agent, 131, 0x2504, vec![0x55])),
        Err(MesiCacheBankError::WriteQueueConflict { line })
            if line == Address::new(0x2500)
    ));
    assert!(matches!(
        bank.accept_cpu_request(uncacheable_read(cache_agent, 132, 0x2508)),
        Err(MesiCacheBankError::WriteQueueConflict { line })
            if line == Address::new(0x2500)
    ));

    let writeback = bank.issue_write_queue(0).unwrap().unwrap();
    assert_eq!(writeback.kind(), CacheWriteQueueEntryKind::WritebackDirty);
    assert_eq!(writeback.request().line_address(), Address::new(0x2500));
    assert_eq!(&writeback.request().data().unwrap()[4..6], &[0xde, 0xad]);
    let downstream = writeback.post_issue_downstream_request().unwrap().clone();
    assert_eq!(downstream.id(), uncached.id());
    assert_eq!(downstream.range(), uncached.range());
    assert!(downstream.is_uncacheable());
    assert!(downstream.is_strict_ordered());

    let uncached_fill = bank
        .accept_fill(
            MemoryResponse::completed(&downstream, Some(vec![0x99; 8])).unwrap(),
            MesiEvent::DataShared,
        )
        .unwrap();
    assert_eq!(
        response_data(uncached_fill.target_outcome().unwrap()),
        &[0x99; 8]
    );
    assert_eq!(bank.state(Address::new(0x2500)), None);
}

#[test]
fn mesi_cache_bank_snapshot_restores_pending_uncacheable_read_after_dirty_writeback() {
    let cache_agent = agent(20);
    let mut bank = MesiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(1, 0).unwrap(),
    );
    let store = write(cache_agent, 133, 0x2604, vec![0xde, 0xad]);
    let miss = bank.accept_cpu_request(store).unwrap();
    bank.accept_fill(
        fill(miss.downstream_request().unwrap(), 0x00),
        MesiEvent::DataModified,
    )
    .unwrap();

    let uncached = uncacheable_read(cache_agent, 134, 0x2608);
    let result = bank.accept_cpu_request(uncached.clone()).unwrap();
    assert!(result.downstream_request().is_none());
    let snapshot = bank.snapshot();

    let mut restored = MesiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(1, 0).unwrap(),
    );
    restored.restore(&snapshot).unwrap();
    let writeback = restored.issue_write_queue(0).unwrap().unwrap();
    let downstream = writeback.post_issue_downstream_request().unwrap().clone();
    assert_eq!(downstream.id(), uncached.id());

    let uncached_fill = restored
        .accept_fill(
            MemoryResponse::completed(&downstream, Some(vec![0x77; 8])).unwrap(),
            MesiEvent::DataShared,
        )
        .unwrap();
    assert_eq!(
        response_data(uncached_fill.target_outcome().unwrap()),
        &[0x77; 8]
    );
    assert_eq!(restored.state(Address::new(0x2600)), None);
}

#[test]
fn mesi_cache_bank_snapshot_keeps_clean_pending_read_from_dirty_writeback_read() {
    let cache_agent = agent(20);
    let mut bank = MesiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(1, 0).unwrap(),
    );
    let cached = read(cache_agent, 135, 0x2704);
    let cached_miss = bank.accept_cpu_request(cached).unwrap();
    bank.accept_fill(
        fill(cached_miss.downstream_request().unwrap(), 0x11),
        MesiEvent::DataShared,
    )
    .unwrap();

    let clean_uncached = uncacheable_read(cache_agent, 136, 0x2708);
    let clean_result = bank.accept_cpu_request(clean_uncached.clone()).unwrap();
    assert_eq!(
        clean_result.downstream_request().unwrap().id(),
        clean_uncached.id()
    );

    let store = write(cache_agent, 137, 0x2704, vec![0xde, 0xad]);
    let store_miss = bank.accept_cpu_request(store).unwrap();
    bank.accept_fill(
        fill(store_miss.downstream_request().unwrap(), 0x22),
        MesiEvent::DataModified,
    )
    .unwrap();

    let dirty_uncached = uncacheable_read(cache_agent, 138, 0x270c);
    let dirty_result = bank.accept_cpu_request(dirty_uncached.clone()).unwrap();
    assert!(dirty_result.downstream_request().is_none());

    let snapshot = bank.snapshot();
    let mut restored = MesiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(1, 0).unwrap(),
    );
    restored.restore(&snapshot).unwrap();
    let writeback = restored.issue_write_queue(0).unwrap().unwrap();
    assert_eq!(
        writeback.post_issue_downstream_request().unwrap().id(),
        dirty_uncached.id()
    );

    let clean_fill = restored
        .accept_fill(
            MemoryResponse::completed(&clean_uncached, Some(vec![0x33; 8])).unwrap(),
            MesiEvent::DataShared,
        )
        .unwrap();
    assert_eq!(
        response_data(clean_fill.target_outcome().unwrap()),
        &[0x33; 8]
    );
}

#[test]
fn mesi_cache_bank_restore_rejects_uncacheable_read_with_missing_blocking_writeback() {
    let cache_agent = agent(20);
    let mut bank = MesiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(1, 0).unwrap(),
    );
    let store = write(cache_agent, 139, 0x2904, vec![0xde, 0xad]);
    let miss = bank.accept_cpu_request(store).unwrap();
    bank.accept_fill(
        fill(miss.downstream_request().unwrap(), 0x00),
        MesiEvent::DataModified,
    )
    .unwrap();

    let uncached = uncacheable_read(cache_agent, 140, 0x2908);
    bank.accept_cpu_request(uncached.clone()).unwrap();
    let missing = CacheWriteQueueHandle::new(99);
    let snapshot = bank.snapshot().with_pending_uncacheable_reads(vec![
        MesiPendingUncacheableReadSnapshot::new(uncached.clone(), Some(missing)),
    ]);

    let mut restored = MesiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(1, 0).unwrap(),
    );
    assert!(matches!(
        restored.restore(&snapshot),
        Err(MesiCacheBankError::SnapshotPendingUncacheableReadWritebackMismatch {
            response,
            handle,
        }) if response == uncached.id() && handle == missing
    ));
}

#[test]
fn mesi_cache_bank_restore_rejects_malformed_pending_uncacheable_request() {
    let cache_agent = agent(20);

    let cacheable = read(cache_agent, 141, 0x2918);
    let cacheable_id = cacheable.id();
    let snapshot = MesiCacheBank::new(cache_agent, layout())
        .snapshot()
        .with_pending_uncacheable_reads(vec![MesiPendingUncacheableReadSnapshot::new(
            cacheable, None,
        )]);
    let mut restored = MesiCacheBank::new(cache_agent, layout());
    assert!(matches!(
        restored.restore(&snapshot),
        Err(MesiCacheBankError::SnapshotPendingUncacheableRequestMismatch {
            response,
            operation: MemoryOperation::ReadShared,
            uncacheable: false,
        }) if response == cacheable_id
    ));

    let uncached_no_response =
        clean_evict(cache_agent, 142, 0x2920).with_uncacheable_strict_order();
    let uncached_no_response_id = uncached_no_response.id();
    let snapshot = MesiCacheBank::new(cache_agent, layout())
        .snapshot()
        .with_pending_uncacheable_reads(vec![MesiPendingUncacheableReadSnapshot::new(
            uncached_no_response,
            None,
        )]);
    assert!(matches!(
        restored.restore(&snapshot),
        Err(MesiCacheBankError::SnapshotPendingUncacheableRequestMismatch {
            response,
            operation: MemoryOperation::CleanEvict,
            uncacheable: true,
        }) if response == uncached_no_response_id
    ));

    let uncached_write =
        uncacheable_write(cache_agent, 143, 0x2930, vec![0xde, 0xad], vec![true, true]);
    let uncached_write_id = uncached_write.id();
    let snapshot = MesiCacheBank::new(cache_agent, layout())
        .snapshot()
        .with_pending_uncacheable_reads(vec![MesiPendingUncacheableReadSnapshot::new(
            uncached_write,
            None,
        )]);
    assert!(matches!(
        restored.restore(&snapshot),
        Err(MesiCacheBankError::SnapshotPendingUncacheableRequestMismatch {
            response,
            operation: MemoryOperation::Write,
            uncacheable: true,
        }) if response == uncached_write_id
    ));

    let foreign_agent_read = uncacheable_read(agent(21), 144, 0x2940);
    let snapshot = MesiCacheBank::new(cache_agent, layout())
        .snapshot()
        .with_pending_uncacheable_reads(vec![MesiPendingUncacheableReadSnapshot::new(
            foreign_agent_read,
            None,
        )]);
    assert_eq!(
        restored.restore(&snapshot),
        Err(MesiCacheBankError::WrongAgent {
            expected: cache_agent,
            actual: agent(21),
        })
    );

    let wrong_layout_read = MemoryRequest::read_shared(
        MemoryRequestId::new(cache_agent, 145),
        Address::new(0x2950),
        AccessSize::new(8).unwrap(),
        wide_layout(),
    )
    .unwrap()
    .with_uncacheable_strict_order();
    let expected_error = MesiCacheBankError::Controller(MesiCacheControllerError::Memory(
        MemoryError::LineLayoutMismatch {
            request: wrong_layout_read.id(),
            expected: layout(),
            actual: wide_layout(),
        },
    ));
    let snapshot = MesiCacheBank::new(cache_agent, layout())
        .snapshot()
        .with_pending_uncacheable_reads(vec![MesiPendingUncacheableReadSnapshot::new(
            wrong_layout_read,
            None,
        )]);
    assert_eq!(restored.restore(&snapshot), Err(expected_error));
}

#[test]
fn mesi_cache_bank_uncacheable_write_enters_write_queue_without_mshr() {
    let cache_agent = agent(20);
    let mut bank = MesiCacheBank::new_with_mshr_and_write_queue(
        cache_agent,
        layout(),
        MshrQueueConfig::new(2, 2, 0).unwrap(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );
    let request = uncacheable_write(
        cache_agent,
        124,
        0x2824,
        vec![0xde, 0xad, 0xbe, 0xef],
        vec![true, false, true, true],
    );

    let result = bank.accept_cpu_request(request.clone()).unwrap();

    assert_eq!(result.kind(), MesiCacheControllerResultKind::Miss);
    assert!(result.downstream_request().is_none());
    assert!(result.target_outcome().is_none());
    assert_eq!(bank.mshr_allocated_count(), 0);
    assert_eq!(bank.pending_fill_count(), 0);
    assert_eq!(bank.write_queue_allocated_count(), 1);
    assert_eq!(bank.write_queue_next_ready_tick(), Some(0));
    assert_eq!(bank.state(Address::new(0x2820)), None);

    let issued = bank.issue_write_queue(0).unwrap().unwrap();
    assert_eq!(issued.kind(), CacheWriteQueueEntryKind::UncacheableWrite);
    assert_eq!(issued.request().id(), request.id());
    assert_eq!(issued.request().range(), request.range());
    assert_eq!(issued.request().data(), request.data());
    assert_eq!(issued.request().byte_mask(), request.byte_mask());
    assert!(issued.request().is_uncacheable());
    assert!(issued.request().is_strict_ordered());
}

#[test]
fn mesi_cache_bank_uncacheable_write_queues_dirty_writeback_first() {
    let cache_agent = agent(7);
    let mut bank = MesiCacheBank::new_with_mshr_and_write_queue(
        cache_agent,
        layout(),
        MshrQueueConfig::new(2, 2, 0).unwrap(),
        CacheWriteQueueConfig::new(3, 0).unwrap(),
    );
    let store = write(cache_agent, 124, 0x2444, vec![0xde, 0xad]);
    let miss = bank.accept_cpu_request(store).unwrap();
    let downstream = miss.downstream_request().unwrap().clone();
    bank.accept_fill(fill(&downstream, 0x00), MesiEvent::DataModified)
        .unwrap();
    assert_eq!(bank.state(Address::new(0x2440)), Some(MesiState::Modified));

    let uncached = uncacheable_write(cache_agent, 125, 0x2448, vec![0xca, 0xfe], vec![true, true]);
    let result = bank.accept_cpu_request(uncached.clone()).unwrap();

    assert_eq!(result.kind(), MesiCacheControllerResultKind::Miss);
    assert!(result.downstream_request().is_none());
    assert_eq!(bank.state(Address::new(0x2440)), None);
    assert_eq!(bank.write_queue_allocated_count(), 2);

    let forwarded = bank
        .accept_cpu_request(read(cache_agent, 126, 0x2444))
        .unwrap();
    assert_eq!(forwarded.kind(), MesiCacheControllerResultKind::Hit);
    assert!(forwarded.downstream_request().is_none());
    assert_eq!(
        response_data(forwarded.target_outcome().unwrap()),
        &[0xde, 0xad, 0x00, 0x00, 0xca, 0xfe, 0x00, 0x00]
    );
    assert!(matches!(
        bank.accept_cpu_request(write(cache_agent, 127, 0x2444, vec![0x55])),
        Err(MesiCacheBankError::WriteQueueConflict { line })
            if line == Address::new(0x2440)
    ));

    let writeback = bank.issue_write_queue(0).unwrap().unwrap();
    assert_eq!(writeback.kind(), CacheWriteQueueEntryKind::WritebackDirty);
    assert_eq!(writeback.request().line_address(), Address::new(0x2440));
    assert_eq!(&writeback.request().data().unwrap()[4..6], &[0xde, 0xad]);

    let issued_uncached = bank.issue_write_queue(0).unwrap().unwrap();
    assert_eq!(
        issued_uncached.kind(),
        CacheWriteQueueEntryKind::UncacheableWrite
    );
    assert_eq!(issued_uncached.request().id(), uncached.id());
    assert!(issued_uncached.request().is_uncacheable());
    assert!(issued_uncached.request().is_strict_ordered());
}

#[test]
fn mesi_cache_bank_uncacheable_write_response_uses_inflight_record_after_restore() {
    let cache_agent = agent(20);
    let mut bank = MesiCacheBank::new_with_mshr_and_write_queue(
        cache_agent,
        layout(),
        MshrQueueConfig::new(2, 2, 0).unwrap(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );
    let request = uncacheable_write(
        cache_agent,
        125,
        0x2864,
        vec![0xde, 0xad, 0xbe, 0xef],
        vec![true, false, true, true],
    );

    bank.accept_cpu_request(request.clone()).unwrap();
    let issued = bank.issue_write_queue(0).unwrap().unwrap();

    assert_eq!(issued.kind(), CacheWriteQueueEntryKind::UncacheableWrite);
    assert_eq!(bank.inflight_uncacheable_write_count(), 1);

    let snapshot = bank.snapshot();
    assert_eq!(snapshot.inflight_uncacheable_write_count(), 1);

    let mut restored = MesiCacheBank::new_with_mshr_and_write_queue(
        cache_agent,
        layout(),
        MshrQueueConfig::new(2, 2, 0).unwrap(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.inflight_uncacheable_write_count(), 1);

    let malformed_source = MemoryRequest::read_shared(
        request.id(),
        request.range().start(),
        request.range().size(),
        layout(),
    )
    .unwrap();
    let malformed_response = MemoryResponse::completed(
        &malformed_source,
        Some(vec![0; request.range().size().bytes() as usize]),
    )
    .unwrap();
    assert!(restored
        .accept_uncacheable_write_response(malformed_response)
        .is_err());
    assert_eq!(restored.inflight_uncacheable_write_count(), 1);

    let outcome = restored
        .accept_uncacheable_write_response(
            MemoryResponse::completed(issued.request(), None).unwrap(),
        )
        .unwrap();

    assert_eq!(response_id(&outcome), request.id());
    match outcome {
        TargetOutcome::Respond(response) => assert!(response.data().is_none()),
        other => panic!("expected immediate response, got {other:?}"),
    }
    assert_eq!(restored.inflight_uncacheable_write_count(), 0);
    assert!(matches!(
        restored.accept_uncacheable_write_response(MemoryResponse::retry(issued.request())),
        Err(MesiCacheBankError::UnknownUncacheableWriteResponse { response })
            if response == request.id()
    ));
}

#[test]
fn mesi_cache_bank_restore_rejects_malformed_inflight_uncacheable_write() {
    let cache_agent = agent(20);
    let mut restored = MesiCacheBank::new(cache_agent, layout());

    let cacheable_write = write(cache_agent, 144, 0x2940, vec![0xde, 0xad]);
    let cacheable_write_id = cacheable_write.id();
    let snapshot = MesiCacheBank::new(cache_agent, layout())
        .snapshot()
        .with_inflight_uncacheable_writes(vec![cacheable_write]);
    assert!(matches!(
        restored.restore(&snapshot),
        Err(MesiCacheBankError::SnapshotInflightUncacheableWriteMismatch {
            response,
            operation: MemoryOperation::Write,
            uncacheable: false,
        }) if response == cacheable_write_id
    ));

    let uncached_read = uncacheable_read(cache_agent, 145, 0x2950);
    let uncached_read_id = uncached_read.id();
    let snapshot = MesiCacheBank::new(cache_agent, layout())
        .snapshot()
        .with_inflight_uncacheable_writes(vec![uncached_read]);
    assert!(matches!(
        restored.restore(&snapshot),
        Err(MesiCacheBankError::SnapshotInflightUncacheableWriteMismatch {
            response,
            operation: MemoryOperation::ReadShared,
            uncacheable: true,
        }) if response == uncached_read_id
    ));

    let uncached_atomic = uncacheable_atomic(cache_agent, 146, 0x2960, vec![0x11, 0x22]);
    let uncached_atomic_id = uncached_atomic.id();
    let snapshot = MesiCacheBank::new(cache_agent, layout())
        .snapshot()
        .with_inflight_uncacheable_writes(vec![uncached_atomic]);
    assert!(matches!(
        restored.restore(&snapshot),
        Err(MesiCacheBankError::SnapshotInflightUncacheableWriteMismatch {
            response,
            operation: MemoryOperation::Atomic,
            uncacheable: true,
        }) if response == uncached_atomic_id
    ));

    let uncached_no_response =
        clean_evict(cache_agent, 147, 0x2970).with_uncacheable_strict_order();
    let uncached_no_response_id = uncached_no_response.id();
    let snapshot = MesiCacheBank::new(cache_agent, layout())
        .snapshot()
        .with_inflight_uncacheable_writes(vec![uncached_no_response]);
    assert!(matches!(
        restored.restore(&snapshot),
        Err(MesiCacheBankError::SnapshotInflightUncacheableWriteMismatch {
            response,
            operation: MemoryOperation::CleanEvict,
            uncacheable: true,
        }) if response == uncached_no_response_id
    ));

    let foreign_agent_write =
        uncacheable_write(agent(21), 148, 0x2980, vec![0xde, 0xad], vec![true, true]);
    let snapshot = MesiCacheBank::new(cache_agent, layout())
        .snapshot()
        .with_inflight_uncacheable_writes(vec![foreign_agent_write]);
    assert_eq!(
        restored.restore(&snapshot),
        Err(MesiCacheBankError::WrongAgent {
            expected: cache_agent,
            actual: agent(21),
        })
    );

    let wrong_layout_write = MemoryRequest::write(
        MemoryRequestId::new(cache_agent, 149),
        Address::new(0x2990),
        AccessSize::new(2).unwrap(),
        vec![0xde, 0xad],
        ByteMask::full(AccessSize::new(2).unwrap()).unwrap(),
        wide_layout(),
    )
    .unwrap()
    .with_uncacheable_strict_order();
    let expected_error = MesiCacheBankError::Controller(MesiCacheControllerError::Memory(
        MemoryError::LineLayoutMismatch {
            request: wrong_layout_write.id(),
            expected: layout(),
            actual: wide_layout(),
        },
    ));
    let snapshot = MesiCacheBank::new(cache_agent, layout())
        .snapshot()
        .with_inflight_uncacheable_writes(vec![wrong_layout_write]);
    assert_eq!(restored.restore(&snapshot), Err(expected_error));
}

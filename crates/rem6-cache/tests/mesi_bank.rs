use rem6_cache::{
    CacheWriteQueueConfig, CacheWriteQueueEntryKind, CacheWriteQueueError, MesiCacheBank,
    MesiCacheBankError, MesiCacheControllerError, MshrQosClass, MshrQueueConfig,
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

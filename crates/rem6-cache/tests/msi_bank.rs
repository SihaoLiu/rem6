use rem6_cache::{
    CacheControllerError, CacheControllerResultKind, CacheWriteQueueConfig,
    CacheWriteQueueEntryKind, CacheWriteQueueError, MshrEntry, MshrQosClass, MshrQueueConfig,
    MshrQueueError, MshrQueueSnapshot, MshrTarget, MshrTargetSource, MsiCacheBank,
    MsiCacheBankError, MsiCacheBankSnapshot,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryError, MemoryOperation,
    MemoryRequest, MemoryRequestId, MemoryResponse,
};
use rem6_protocol_msi::{MsiEvent, MsiState};
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
    .with_uncacheable_strict_order()
}

fn fill(request: &MemoryRequest, byte: u8) -> MemoryResponse {
    MemoryResponse::completed(request, Some(vec![byte; layout().bytes() as usize])).unwrap()
}

fn response_data(outcome: &TargetOutcome) -> &[u8] {
    match outcome {
        TargetOutcome::Respond(response) => response.data().unwrap(),
        other => panic!("expected immediate response, got {other:?}"),
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
fn msi_cache_bank_tracks_multiple_lines_with_unique_downstream_ids() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new(cache_agent, layout());

    let first = read(cache_agent, 100, 0x1004);
    let first_miss = bank.accept_cpu_request(first.clone()).unwrap();
    let first_downstream = first_miss.downstream_request().unwrap().clone();
    assert_eq!(first_miss.kind(), CacheControllerResultKind::Miss);
    assert_eq!(first_downstream.id(), MemoryRequestId::new(cache_agent, 0));
    assert_eq!(first_downstream.line_address(), Address::new(0x1000));

    let second = read(cache_agent, 101, 0x1018);
    let second_miss = bank.accept_cpu_request(second.clone()).unwrap();
    let second_downstream = second_miss.downstream_request().unwrap().clone();
    assert_eq!(second_miss.kind(), CacheControllerResultKind::Miss);
    assert_eq!(second_downstream.id(), MemoryRequestId::new(cache_agent, 1));
    assert_eq!(second_downstream.line_address(), Address::new(0x1010));

    assert_eq!(bank.line_count(), 2);
    assert_eq!(
        bank.line_addresses(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
    assert_eq!(bank.next_sequence(), 2);
    assert_eq!(
        bank.pending_fill_line(first_downstream.id()),
        Some(Address::new(0x1000))
    );
    assert_eq!(
        bank.pending_fill_line(second_downstream.id()),
        Some(Address::new(0x1010))
    );
    assert_eq!(
        bank.state(Address::new(0x1000)),
        Some(MsiState::InvalidToShared)
    );
    assert_eq!(
        bank.state(Address::new(0x1010)),
        Some(MsiState::InvalidToShared)
    );

    let second_fill = bank.accept_fill(fill(&second_downstream, 0x22)).unwrap();
    assert_eq!(second_fill.kind(), CacheControllerResultKind::Fill);
    assert_eq!(
        response_data(second_fill.target_outcome().unwrap()),
        &[0x22; 8]
    );
    assert_eq!(bank.pending_fill_line(second_downstream.id()), None);
    assert_eq!(bank.state(Address::new(0x1010)), Some(MsiState::Shared));

    let first_fill = bank.accept_fill(fill(&first_downstream, 0x11)).unwrap();
    assert_eq!(first_fill.kind(), CacheControllerResultKind::Fill);
    assert_eq!(
        response_data(first_fill.target_outcome().unwrap()),
        &[0x11; 8]
    );
    assert_eq!(bank.state(Address::new(0x1000)), Some(MsiState::Shared));

    let first_hit = bank.accept_cpu_request(first).unwrap();
    assert_eq!(first_hit.kind(), CacheControllerResultKind::Hit);
    assert_eq!(
        response_data(first_hit.target_outcome().unwrap()),
        &[0x11; 8]
    );

    let second_hit = bank.accept_cpu_request(second).unwrap();
    assert_eq!(second_hit.kind(), CacheControllerResultKind::Hit);
    assert_eq!(
        response_data(second_hit.target_outcome().unwrap()),
        &[0x22; 8]
    );
}

#[test]
fn msi_cache_bank_mshr_coalesces_same_line_read_misses() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new_with_mshr(
        cache_agent,
        layout(),
        MshrQueueConfig::new(2, 2, 0).unwrap(),
    );

    let first = read(cache_agent, 100, 0x1004);
    let first_miss = bank.accept_cpu_request(first.clone()).unwrap();
    let first_downstream = first_miss.downstream_request().unwrap().clone();
    assert_eq!(first_miss.kind(), CacheControllerResultKind::Miss);
    assert_eq!(bank.pending_fill_count(), 1);
    assert_eq!(bank.mshr_allocated_count(), 1);
    assert_eq!(bank.mshr_target_count(Address::new(0x1000)), Some(1));

    let second = read(cache_agent, 101, 0x1008);
    let second_miss = bank.accept_cpu_request(second.clone()).unwrap();
    assert_eq!(second_miss.kind(), CacheControllerResultKind::Miss);
    assert!(second_miss.downstream_request().is_none());
    assert_eq!(second_miss.target_outcomes(), &[]);
    assert_eq!(bank.pending_fill_count(), 1);
    assert_eq!(bank.mshr_allocated_count(), 1);
    assert_eq!(bank.mshr_target_count(Address::new(0x1000)), Some(2));

    let fill_result = bank.accept_fill(fill(&first_downstream, 0x44)).unwrap();
    assert_eq!(fill_result.kind(), CacheControllerResultKind::Fill);
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
    assert_eq!(bank.state(Address::new(0x1000)), Some(MsiState::Shared));
}

#[test]
fn msi_cache_bank_uncacheable_read_bypasses_clean_resident_line() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new(cache_agent, layout());

    let cached = read(cache_agent, 130, 0x1804);
    let cached_miss = bank.accept_cpu_request(cached.clone()).unwrap();
    let cached_downstream = cached_miss.downstream_request().unwrap().clone();
    bank.accept_fill(fill(&cached_downstream, 0x11)).unwrap();
    assert_eq!(bank.state(Address::new(0x1800)), Some(MsiState::Shared));

    let uncached = uncacheable_read(cache_agent, 131, 0x1808);
    let uncached_miss = bank.accept_cpu_request(uncached.clone()).unwrap();
    let uncached_downstream = uncached_miss.downstream_request().unwrap();

    assert_eq!(uncached_miss.kind(), CacheControllerResultKind::Miss);
    assert_eq!(uncached_downstream.id(), uncached.id());
    assert_eq!(uncached_downstream.range(), uncached.range());
    assert!(uncached_downstream.is_uncacheable());
    assert!(uncached_downstream.is_strict_ordered());
    assert_eq!(bank.state(Address::new(0x1800)), None);

    let uncached_fill = bank
        .accept_fill(MemoryResponse::completed(uncached_downstream, Some(vec![0x99; 8])).unwrap())
        .unwrap();
    assert_eq!(
        response_data(uncached_fill.target_outcome().unwrap()),
        &[0x99; 8]
    );
    assert_eq!(bank.state(Address::new(0x1800)), None);

    let normal_again = bank.accept_cpu_request(cached).unwrap();
    assert_eq!(normal_again.kind(), CacheControllerResultKind::Miss);
}

#[test]
fn msi_cache_bank_uncacheable_read_preserves_dirty_resident_line() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new(cache_agent, layout());
    let store = write(cache_agent, 132, 0x1c04, vec![0xde, 0xad]);

    let miss = bank.accept_cpu_request(store).unwrap();
    assert_eq!(
        miss.downstream_request().unwrap().operation(),
        MemoryOperation::ReadUnique
    );
    bank.accept_fill(fill(miss.downstream_request().unwrap(), 0x00))
        .unwrap();
    assert_eq!(bank.state(Address::new(0x1c00)), Some(MsiState::Modified));

    let result = bank.accept_cpu_request(uncacheable_read(cache_agent, 133, 0x1c08));
    assert!(result.is_err());
    assert_eq!(bank.state(Address::new(0x1c00)), Some(MsiState::Modified));
    assert_eq!(
        bank.snapshot().dirty_line_addresses(),
        vec![Address::new(0x1c00)]
    );
}

#[test]
fn msi_cache_bank_uncacheable_write_enters_write_queue_without_mshr() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new_with_mshr_and_write_queue(
        cache_agent,
        layout(),
        MshrQueueConfig::new(2, 2, 0).unwrap(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );
    let request = uncacheable_write(
        cache_agent,
        134,
        0x2024,
        vec![0xde, 0xad, 0xbe, 0xef],
        vec![true, false, true, true],
    );

    let result = bank.accept_cpu_request(request.clone()).unwrap();

    assert_eq!(result.kind(), CacheControllerResultKind::Miss);
    assert!(result.downstream_request().is_none());
    assert!(result.target_outcome().is_none());
    assert_eq!(bank.mshr_allocated_count(), 0);
    assert_eq!(bank.pending_fill_count(), 0);
    assert_eq!(bank.write_queue_allocated_count(), 1);
    assert_eq!(bank.write_queue_next_ready_tick(), Some(0));
    assert_eq!(bank.state(Address::new(0x2020)), None);

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
fn msi_cache_bank_uncacheable_write_queues_dirty_writeback_first() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new_with_mshr_and_write_queue(
        cache_agent,
        layout(),
        MshrQueueConfig::new(2, 2, 0).unwrap(),
        CacheWriteQueueConfig::new(3, 0).unwrap(),
    );
    let store = write(cache_agent, 135, 0x2044, vec![0xde, 0xad]);
    let miss = bank.accept_cpu_request(store).unwrap();
    let downstream = miss.downstream_request().unwrap().clone();
    bank.accept_fill(fill(&downstream, 0x00)).unwrap();
    assert_eq!(bank.state(Address::new(0x2040)), Some(MsiState::Modified));

    let uncached = uncacheable_write(cache_agent, 136, 0x2048, vec![0xca, 0xfe], vec![true, true]);
    let result = bank.accept_cpu_request(uncached.clone()).unwrap();

    assert_eq!(result.kind(), CacheControllerResultKind::Miss);
    assert!(result.downstream_request().is_none());
    assert_eq!(bank.state(Address::new(0x2040)), None);
    assert_eq!(bank.write_queue_allocated_count(), 2);

    let forwarded = bank
        .accept_cpu_request(read(cache_agent, 137, 0x2044))
        .unwrap();
    assert_eq!(forwarded.kind(), CacheControllerResultKind::Hit);
    assert!(forwarded.downstream_request().is_none());
    assert_eq!(
        response_data(forwarded.target_outcome().unwrap()),
        &[0xde, 0xad, 0x00, 0x00, 0xca, 0xfe, 0x00, 0x00]
    );
    assert!(bank
        .accept_cpu_request(write(cache_agent, 138, 0x2044, vec![0x55]))
        .is_err());

    let writeback = bank.issue_write_queue(0).unwrap().unwrap();
    assert_eq!(writeback.kind(), CacheWriteQueueEntryKind::WritebackDirty);
    assert_eq!(writeback.request().line_address(), Address::new(0x2040));
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
fn msi_cache_bank_uncacheable_write_response_uses_inflight_record_after_restore() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new_with_mshr_and_write_queue(
        cache_agent,
        layout(),
        MshrQueueConfig::new(2, 2, 0).unwrap(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );
    let request = uncacheable_write(
        cache_agent,
        135,
        0x2064,
        vec![0xde, 0xad, 0xbe, 0xef],
        vec![true, false, true, true],
    );

    bank.accept_cpu_request(request.clone()).unwrap();
    let issued = bank.issue_write_queue(0).unwrap().unwrap();

    assert_eq!(issued.kind(), CacheWriteQueueEntryKind::UncacheableWrite);
    assert_eq!(bank.inflight_uncacheable_write_count(), 1);

    let snapshot = bank.snapshot();
    assert_eq!(snapshot.inflight_uncacheable_write_count(), 1);

    let mut restored = MsiCacheBank::new_with_mshr_and_write_queue(
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
        Err(MsiCacheBankError::UnknownUncacheableWriteResponse { response })
            if response == request.id()
    ));
}

#[test]
fn msi_cache_bank_exposes_post_fill_writeback_targets_downstream() {
    let cache_agent = agent(7);
    let config = MshrQueueConfig::new(2, 3, 0).unwrap();
    let mut bank = MsiCacheBank::new_with_mshr(cache_agent, layout(), config.clone());

    let first = read(cache_agent, 120, 0x1004);
    let first_miss = bank.accept_cpu_request(first.clone()).unwrap();
    let first_downstream = first_miss.downstream_request().unwrap().clone();

    let snapshot = bank.snapshot();
    let current_mshr = snapshot.mshr().unwrap();
    let current_entry = &current_mshr.entries()[0];
    let clean = clean_writeback(cache_agent, 121, 0x1000, 0xcc);
    let mut targets = current_entry.targets().to_vec();
    targets.push(MshrTarget::from_parts(
        clean.clone(),
        1,
        1,
        MshrTargetSource::Demand,
        false,
        None,
    ));
    let mshr_snapshot = MshrQueueSnapshot::new(
        config.clone(),
        vec![MshrEntry::from_parts(
            current_entry.handle(),
            current_entry.line(),
            current_entry.ready_tick(),
            current_entry.order(),
            current_entry.in_service(),
            current_entry.pending_modified(),
            targets,
        )],
        current_mshr.next_handle(),
        current_mshr.next_order(),
    );
    let restored_snapshot = MsiCacheBankSnapshot::new_with_mshr(
        snapshot.agent(),
        snapshot.layout(),
        snapshot.next_sequence(),
        snapshot.lines().to_vec(),
        mshr_snapshot,
    );
    let mut restored = MsiCacheBank::new_with_mshr(cache_agent, layout(), config);
    restored.restore(&restored_snapshot).unwrap();

    let fill_result = restored.accept_fill(fill(&first_downstream, 0x44)).unwrap();

    assert_eq!(fill_result.kind(), CacheControllerResultKind::Fill);
    assert_eq!(
        fill_result
            .target_outcomes()
            .iter()
            .map(response_id)
            .collect::<Vec<_>>(),
        vec![first.id()]
    );
    assert_eq!(fill_result.post_fill_downstream_requests(), &[clean]);
    assert_eq!(
        fill_result.post_fill_downstream_requests()[0]
            .data()
            .unwrap(),
        &[0xcc; 16]
    );
    assert_eq!(restored.mshr_allocated_count(), 0);
}

#[test]
fn msi_cache_bank_records_mshr_qos_for_merged_read_misses() {
    let cache_agent = agent(7);
    let config = MshrQueueConfig::new(2, 3, 0).unwrap();
    let mut bank = MsiCacheBank::new_with_mshr(cache_agent, layout(), config.clone());

    let first = read(cache_agent, 300, 0x1804);
    bank.accept_cpu_request_with_qos(first, MshrQosClass::new(30, 4))
        .unwrap();
    assert_eq!(
        bank.mshr_effective_qos(Address::new(0x1800)),
        Some(MshrQosClass::new(30, 4))
    );

    let second = read(cache_agent, 301, 0x1808);
    bank.accept_cpu_request_with_qos(second, MshrQosClass::new(40, 0))
        .unwrap();
    assert_eq!(
        bank.mshr_effective_qos(Address::new(0x1800)),
        Some(MshrQosClass::new(40, 0))
    );

    let snapshot = bank.snapshot();
    let bank_profile = bank.mshr_qos_profile().unwrap();
    assert_eq!(bank_profile.entry_count(), 1);
    assert_eq!(bank_profile.target_count(), 2);
    assert_eq!(bank_profile.qos_target_count(), 2);
    assert_eq!(bank_profile.effective_entry_count(), 1);
    assert_eq!(bank_profile.priority_target_count(0), 1);
    assert_eq!(bank_profile.priority_target_count(4), 1);
    assert_eq!(bank_profile.effective_priority_entry_count(0), 1);
    assert_eq!(bank_profile.effective_requestor_entry_count(40), 1);
    assert_eq!(bank_profile.best_effective_priority(), Some(0));
    assert_eq!(snapshot.mshr_qos_profile(), Some(bank_profile.clone()));

    let mut restored = MsiCacheBank::new_with_mshr(cache_agent, layout(), config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(
        restored.mshr_effective_qos(Address::new(0x1800)),
        Some(MshrQosClass::new(40, 0))
    );
    assert_eq!(restored.mshr_qos_profile(), Some(bank_profile));
}

#[test]
fn msi_cache_bank_mshr_restore_preserves_coalesced_targets_and_limits() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new_with_mshr(
        cache_agent,
        layout(),
        MshrQueueConfig::new(1, 2, 0).unwrap(),
    );

    let first = read(cache_agent, 100, 0x2000);
    let first_downstream = bank
        .accept_cpu_request(first.clone())
        .unwrap()
        .downstream_request()
        .unwrap()
        .clone();
    let second = read(cache_agent, 101, 0x2008);
    bank.accept_cpu_request(second.clone()).unwrap();

    assert_eq!(
        bank.accept_cpu_request(read(cache_agent, 102, 0x2004)),
        Err(MsiCacheBankError::Mshr(MshrQueueError::TargetSlotsFull {
            handle: rem6_cache::MshrHandle::new(0),
            line: Address::new(0x2000),
            targets_per_mshr: 2,
        }))
    );

    let snapshot = bank.snapshot();
    let mut restored = MsiCacheBank::new_with_mshr(
        cache_agent,
        layout(),
        MshrQueueConfig::new(1, 2, 0).unwrap(),
    );
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.mshr_target_count(Address::new(0x2000)), Some(2));

    let fill_result = restored.accept_fill(fill(&first_downstream, 0x55)).unwrap();
    assert_eq!(
        fill_result
            .target_outcomes()
            .iter()
            .map(response_id)
            .collect::<Vec<_>>(),
        vec![first.id(), second.id()]
    );
}

#[test]
fn msi_cache_bank_write_queue_orders_issues_and_restores_snapshot() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new_with_write_queue(
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
        Err(MsiCacheBankError::WriteQueue(
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
fn msi_cache_bank_write_queue_tracks_conflicts_functional_reads_and_uncached_matches() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new_with_write_queue(
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
fn msi_cache_bank_write_queue_rejects_foreign_line_layouts() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );
    let request = dirty_writeback_with_layout(cache_agent, 420, 0x4000, 0xee, wide_layout());
    let expected_error = MsiCacheBankError::Controller(CacheControllerError::Memory(
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
fn msi_cache_bank_rejects_foreign_agent_requests() {
    let mut bank = MsiCacheBank::new(agent(7), layout());

    let error = bank
        .accept_cpu_request(read(agent(8), 1, 0x1000))
        .unwrap_err();

    assert_eq!(
        error,
        MsiCacheBankError::WrongAgent {
            expected: agent(7),
            actual: agent(8),
        }
    );
}

#[test]
fn msi_cache_bank_rejects_unknown_fill_response() {
    let mut bank = MsiCacheBank::new(agent(7), layout());
    let request = MemoryRequest::read_shared(
        MemoryRequestId::new(agent(7), 99),
        Address::new(0x2000),
        size(16),
        layout(),
    )
    .unwrap();

    let error = bank.accept_fill(fill(&request, 0x33)).unwrap_err();

    assert_eq!(
        error,
        MsiCacheBankError::UnknownPendingFill {
            response: MemoryRequestId::new(agent(7), 99),
        }
    );
}

#[test]
fn msi_cache_bank_snapshot_restores_all_lines_and_pending_fills() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new(cache_agent, layout());

    let first = read(cache_agent, 100, 0x1004);
    let first_downstream = bank
        .accept_cpu_request(first.clone())
        .unwrap()
        .downstream_request()
        .unwrap()
        .clone();
    bank.accept_fill(fill(&first_downstream, 0x11)).unwrap();

    let second = read(cache_agent, 101, 0x1018);
    let second_downstream = bank
        .accept_cpu_request(second.clone())
        .unwrap()
        .downstream_request()
        .unwrap()
        .clone();

    let snapshot = bank.snapshot();
    assert_eq!(snapshot.line_count(), 2);
    assert_eq!(
        snapshot.line_addresses(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
    assert_eq!(snapshot.next_sequence(), 2);

    let mut restored = MsiCacheBank::new(cache_agent, layout());
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.line_count(), 2);
    assert_eq!(restored.next_sequence(), 2);

    let first_hit = restored.accept_cpu_request(first).unwrap();
    assert_eq!(first_hit.kind(), CacheControllerResultKind::Hit);
    assert_eq!(
        response_data(first_hit.target_outcome().unwrap()),
        &[0x11; 8]
    );

    let second_fill = restored
        .accept_fill(fill(&second_downstream, 0x22))
        .unwrap();
    assert_eq!(second_fill.kind(), CacheControllerResultKind::Fill);
    assert_eq!(
        response_data(second_fill.target_outcome().unwrap()),
        &[0x22; 8]
    );
    assert_eq!(restored.state(Address::new(0x1010)), Some(MsiState::Shared));
}

#[test]
fn msi_cache_bank_snapshot_reports_and_restores_dirty_lines() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new(cache_agent, layout());
    let store = write(cache_agent, 120, 0x1004, vec![0xde, 0xad]);

    let miss = bank.accept_cpu_request(store.clone()).unwrap();
    assert_eq!(miss.kind(), CacheControllerResultKind::Miss);
    assert_eq!(
        miss.downstream_request().unwrap().operation(),
        MemoryOperation::ReadUnique
    );
    bank.accept_fill(fill(miss.downstream_request().unwrap(), 0x00))
        .unwrap();
    assert_eq!(bank.state(Address::new(0x1000)), Some(MsiState::Modified));

    let snapshot = bank.snapshot();
    assert_eq!(snapshot.dirty_line_count(), 1);
    assert_eq!(snapshot.dirty_line_addresses(), vec![Address::new(0x1000)]);

    bank.accept_snoop(Address::new(0x1000), MsiEvent::SnoopWrite)
        .unwrap();
    assert_eq!(bank.state(Address::new(0x1000)), Some(MsiState::Invalid));
    assert_eq!(bank.cached_data(Address::new(0x1000)), None);

    bank.restore(&snapshot).unwrap();
    assert_eq!(bank.state(Address::new(0x1000)), Some(MsiState::Modified));
    let read_back = read(cache_agent, 121, 0x1004);
    let hit = bank.accept_cpu_request(read_back).unwrap();
    assert_eq!(hit.kind(), CacheControllerResultKind::Hit);
    assert_eq!(
        response_data(hit.target_outcome().unwrap()),
        &[0xde, 0xad, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    );
}

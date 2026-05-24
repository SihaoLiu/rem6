use rem6_cache::{
    CacheControllerResultKind, MshrQosClass, MshrQueueConfig, MshrQueueError, MsiCacheBank,
    MsiCacheBankError,
};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryResponse,
};
use rem6_protocol_msi::MsiState;
use rem6_transport::TargetOutcome;

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
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
    let mut restored = MsiCacheBank::new_with_mshr(cache_agent, layout(), config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(
        restored.mshr_effective_qos(Address::new(0x1800)),
        Some(MshrQosClass::new(40, 0))
    );
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

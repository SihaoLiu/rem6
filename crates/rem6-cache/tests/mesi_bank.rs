use rem6_cache::{MesiCacheBank, MshrQosClass, MshrQueueConfig};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryResponse,
};
use rem6_protocol_mesi::MesiEvent;
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

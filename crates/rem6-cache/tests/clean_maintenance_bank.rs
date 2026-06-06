use rem6_cache::{
    CacheControllerResultKind, CacheReplacementDirectoryConfig, CacheReplacementPolicyKind,
    CacheWriteQueueConfig, ChiCacheBank, ChiCacheControllerResultKind, MesiCacheBank,
    MesiCacheControllerResultKind, MoesiCacheBank, MoesiCacheControllerResultKind, MshrQueueConfig,
    MsiCacheBank,
};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequest, MemoryRequestId,
};
use rem6_transport::TargetOutcome;

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn lru_replacement_config(sets: usize, ways: usize) -> CacheReplacementDirectoryConfig {
    CacheReplacementDirectoryConfig::new(CacheReplacementPolicyKind::Lru, layout(), sets, ways)
        .unwrap()
}

fn clean_shared(agent_id: AgentId, sequence: u64, line: u64) -> MemoryRequest {
    MemoryRequest::clean_shared(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(line),
        layout(),
    )
    .unwrap()
}

fn read_shared(agent_id: AgentId, sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(address),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap()
}

fn invalidate(agent_id: AgentId, sequence: u64, line: u64) -> MemoryRequest {
    MemoryRequest::invalidate(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(line),
        layout(),
    )
    .unwrap()
}

fn no_access(agent_id: AgentId, sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::no_access(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(address),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap()
}

fn write_clean(agent_id: AgentId, sequence: u64, line: u64) -> MemoryRequest {
    MemoryRequest::write_clean(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(line),
        vec![0x5a; layout().bytes() as usize],
        layout(),
    )
    .unwrap()
}

fn clean_maintenance_requests(agent_id: AgentId) -> [MemoryRequest; 2] {
    [
        clean_shared(agent_id, 601, 0x6000),
        invalidate(agent_id, 602, 0x6040),
    ]
}

fn assert_downstream_request_shape(request: &MemoryRequest) {
    assert!(matches!(
        request.operation(),
        MemoryOperation::CleanShared | MemoryOperation::Invalidate
    ));
    assert!(request.requires_response());
    assert!(!request.requires_writable());
    assert!(!request.returns_data());
    assert!(!request.carries_data());
    assert!(request.byte_mask().is_none());
}

#[test]
fn cache_banks_no_access_bypasses_replacement_directory_touch() {
    let mut msi = MsiCacheBank::new_with_replacement_directory(
        agent(12),
        layout(),
        lru_replacement_config(1, 2),
    )
    .unwrap();
    let request = no_access(agent(12), 612, 0x7604);
    let result = msi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(result.kind(), CacheControllerResultKind::Hit);
    assert_eq!(result.downstream_request(), None);
    match result.target_outcome().unwrap() {
        TargetOutcome::Respond(response) => assert_eq!(response.request_id(), request.id()),
        outcome => panic!("expected immediate response, got {outcome:?}"),
    }

    let mut mesi = MesiCacheBank::new_with_replacement_directory(
        agent(22),
        layout(),
        lru_replacement_config(1, 2),
    )
    .unwrap();
    let request = no_access(agent(22), 613, 0x7704);
    let result = mesi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(result.kind(), MesiCacheControllerResultKind::Hit);
    assert_eq!(result.downstream_request(), None);
    match result.target_outcome().unwrap() {
        TargetOutcome::Respond(response) => assert_eq!(response.request_id(), request.id()),
        outcome => panic!("expected immediate response, got {outcome:?}"),
    }

    let mut moesi = MoesiCacheBank::new_with_replacement_directory(
        agent(32),
        layout(),
        lru_replacement_config(1, 2),
    )
    .unwrap();
    let request = no_access(agent(32), 614, 0x7804);
    let result = moesi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(result.kind(), MoesiCacheControllerResultKind::Hit);
    assert_eq!(result.downstream_request(), None);
    match result.target_outcome().unwrap() {
        TargetOutcome::Respond(response) => assert_eq!(response.request_id(), request.id()),
        outcome => panic!("expected immediate response, got {outcome:?}"),
    }

    let mut chi = ChiCacheBank::new_with_replacement_directory(
        agent(42),
        layout(),
        lru_replacement_config(1, 2),
    )
    .unwrap();
    let request = no_access(agent(42), 615, 0x7904);
    let result = chi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(result.kind(), ChiCacheControllerResultKind::Hit);
    assert_eq!(result.downstream_request(), None);
    match result.target_outcome().unwrap() {
        TargetOutcome::Respond(response) => assert_eq!(response.request_id(), request.id()),
        outcome => panic!("expected immediate response, got {outcome:?}"),
    }
}

#[test]
fn msi_cache_bank_no_access_bypasses_mshr_preflight() {
    let cache_agent = agent(11);
    let mut bank = MsiCacheBank::new_with_mshr(
        cache_agent,
        layout(),
        MshrQueueConfig::new(1, 1, 0).unwrap(),
    );
    let miss = bank
        .accept_cpu_request(read_shared(cache_agent, 610, 0x7404))
        .unwrap();
    assert_eq!(miss.kind(), CacheControllerResultKind::Miss);
    assert_eq!(bank.pending_fill_count(), 1);

    let request = no_access(cache_agent, 611, 0x7504);
    let result = bank.accept_cpu_request(request.clone()).unwrap();

    assert_eq!(result.kind(), CacheControllerResultKind::Hit);
    assert_eq!(result.downstream_request(), None);
    match result.target_outcome().unwrap() {
        TargetOutcome::Respond(response) => {
            assert_eq!(response.request_id(), request.id());
            assert_eq!(response.data(), None);
        }
        outcome => panic!("expected immediate response, got {outcome:?}"),
    }
    assert_eq!(bank.pending_fill_count(), 1);
}

#[test]
fn cache_banks_uncacheable_no_access_completes_without_downstream() {
    let mut msi = MsiCacheBank::new_with_write_queue(
        agent(10),
        layout(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );
    let request = no_access(agent(10), 606, 0x7084).with_uncacheable();
    let result = msi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(result.kind(), CacheControllerResultKind::Hit);
    assert_eq!(result.downstream_request(), None);
    assert_eq!(result.transition(), None);
    match result.target_outcome().unwrap() {
        TargetOutcome::Respond(response) => {
            assert_eq!(response.request_id(), request.id());
            assert_eq!(response.data(), None);
        }
        outcome => panic!("expected immediate response, got {outcome:?}"),
    }
    assert_eq!(msi.write_queue_allocated_count(), 0);

    let mut mesi = MesiCacheBank::new_with_write_queue(
        agent(21),
        layout(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );
    let request = no_access(agent(21), 607, 0x7184).with_uncacheable();
    let result = mesi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(result.kind(), MesiCacheControllerResultKind::Hit);
    assert_eq!(result.downstream_request(), None);
    assert_eq!(result.transition(), None);
    match result.target_outcome().unwrap() {
        TargetOutcome::Respond(response) => {
            assert_eq!(response.request_id(), request.id());
            assert_eq!(response.data(), None);
        }
        outcome => panic!("expected immediate response, got {outcome:?}"),
    }
    assert_eq!(mesi.write_queue_allocated_count(), 0);

    let mut moesi = MoesiCacheBank::new_with_write_queue(
        agent(31),
        layout(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );
    let request = no_access(agent(31), 608, 0x7284).with_uncacheable();
    let result = moesi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(result.kind(), MoesiCacheControllerResultKind::Hit);
    assert_eq!(result.downstream_request(), None);
    assert_eq!(result.transition(), None);
    match result.target_outcome().unwrap() {
        TargetOutcome::Respond(response) => {
            assert_eq!(response.request_id(), request.id());
            assert_eq!(response.data(), None);
        }
        outcome => panic!("expected immediate response, got {outcome:?}"),
    }
    assert_eq!(moesi.write_queue_allocated_count(), 0);

    let mut chi = ChiCacheBank::new_with_write_queue(
        agent(41),
        layout(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );
    let request = no_access(agent(41), 609, 0x7384).with_uncacheable();
    let result = chi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(result.kind(), ChiCacheControllerResultKind::Hit);
    assert_eq!(result.downstream_request(), None);
    assert_eq!(result.transition(), None);
    match result.target_outcome().unwrap() {
        TargetOutcome::Respond(response) => {
            assert_eq!(response.request_id(), request.id());
            assert_eq!(response.data(), None);
        }
        outcome => panic!("expected immediate response, got {outcome:?}"),
    }
    assert_eq!(chi.write_queue_allocated_count(), 0);
}

#[test]
fn msi_cache_bank_no_access_bypasses_same_line_write_queue_conflicts() {
    let cache_agent = agent(9);
    let mut bank = MsiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );
    let queued = bank
        .accept_cpu_request(write_clean(cache_agent, 604, 0x7040))
        .unwrap();
    assert_eq!(queued.kind(), CacheControllerResultKind::Miss);
    assert_eq!(bank.write_queue_allocated_count(), 1);

    let request = no_access(cache_agent, 605, 0x7044);
    let result = bank.accept_cpu_request(request.clone()).unwrap();

    assert_eq!(result.kind(), CacheControllerResultKind::Hit);
    assert_eq!(result.downstream_request(), None);
    assert_eq!(result.transition(), None);
    match result.target_outcome().unwrap() {
        TargetOutcome::Respond(response) => {
            assert_eq!(response.request_id(), request.id());
            assert_eq!(response.data(), None);
        }
        outcome => panic!("expected immediate response, got {outcome:?}"),
    }
    assert_eq!(bank.write_queue_allocated_count(), 1);
}

#[test]
fn msi_cache_bank_no_access_completes_without_downstream_or_local_state() {
    let cache_agent = agent(8);
    let mut bank = MsiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );
    let request = no_access(cache_agent, 603, 0x7004);

    let result = bank.accept_cpu_request(request.clone()).unwrap();

    assert_eq!(result.kind(), CacheControllerResultKind::Hit);
    assert_eq!(result.downstream_request(), None);
    assert_eq!(result.transition(), None);
    match result.target_outcome().unwrap() {
        TargetOutcome::Respond(response) => {
            assert_eq!(response.request_id(), request.id());
            assert_eq!(response.data(), None);
        }
        outcome => panic!("expected immediate response, got {outcome:?}"),
    }
    assert_eq!(bank.write_queue_allocated_count(), 0);
}

#[test]
fn msi_cache_bank_direct_clean_maintenance_forwards_without_local_write() {
    let cache_agent = agent(7);
    let mut bank = MsiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );

    for request in clean_maintenance_requests(cache_agent) {
        let result = bank.accept_cpu_request(request.clone()).unwrap();

        assert_eq!(result.kind(), CacheControllerResultKind::Miss);
        assert_eq!(result.downstream_request(), Some(&request));
        assert_eq!(result.target_outcome(), None);
        assert_downstream_request_shape(result.downstream_request().unwrap());
        assert_eq!(bank.write_queue_allocated_count(), 0);
    }
}

#[test]
fn mesi_cache_bank_direct_clean_maintenance_forwards_without_local_write() {
    let cache_agent = agent(20);
    let mut bank = MesiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );

    for request in clean_maintenance_requests(cache_agent) {
        let result = bank.accept_cpu_request(request.clone()).unwrap();

        assert_eq!(result.kind(), MesiCacheControllerResultKind::Miss);
        assert_eq!(result.downstream_request(), Some(&request));
        assert_eq!(result.target_outcome(), None);
        assert_downstream_request_shape(result.downstream_request().unwrap());
        assert_eq!(bank.write_queue_allocated_count(), 0);
    }
}

#[test]
fn moesi_cache_bank_direct_clean_maintenance_forwards_without_local_write() {
    let cache_agent = agent(30);
    let mut bank = MoesiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );

    for request in clean_maintenance_requests(cache_agent) {
        let result = bank.accept_cpu_request(request.clone()).unwrap();

        assert_eq!(result.kind(), MoesiCacheControllerResultKind::Miss);
        assert_eq!(result.downstream_request(), Some(&request));
        assert_eq!(result.target_outcome(), None);
        assert_downstream_request_shape(result.downstream_request().unwrap());
        assert_eq!(bank.write_queue_allocated_count(), 0);
    }
}

#[test]
fn chi_cache_bank_direct_clean_maintenance_forwards_without_local_write() {
    let cache_agent = agent(40);
    let mut bank = ChiCacheBank::new_with_write_queue(
        cache_agent,
        layout(),
        CacheWriteQueueConfig::new(2, 0).unwrap(),
    );

    for request in clean_maintenance_requests(cache_agent) {
        let result = bank.accept_cpu_request(request.clone()).unwrap();

        assert_eq!(result.kind(), ChiCacheControllerResultKind::Miss);
        assert_eq!(result.downstream_request(), Some(&request));
        assert_eq!(result.target_outcome(), None);
        assert_downstream_request_shape(result.downstream_request().unwrap());
        assert_eq!(bank.write_queue_allocated_count(), 0);
    }
}

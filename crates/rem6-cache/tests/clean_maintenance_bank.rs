use rem6_cache::{
    CacheControllerResultKind, CacheWriteQueueConfig, ChiCacheBank, ChiCacheControllerResultKind,
    MesiCacheBank, MesiCacheControllerResultKind, MoesiCacheBank, MoesiCacheControllerResultKind,
    MsiCacheBank,
};
use rem6_memory::{
    Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequest, MemoryRequestId,
};

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn clean_shared(agent_id: AgentId, sequence: u64, line: u64) -> MemoryRequest {
    MemoryRequest::clean_shared(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(line),
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

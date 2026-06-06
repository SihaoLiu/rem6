use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestAttributes,
    MemoryRequestCheckpointPayload, MemoryRequestId,
};

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(3), sequence)
}

#[test]
fn memory_request_attributes_default_empty_and_builder_preserves_operation() {
    let request = MemoryRequest::read_shared(
        request_id(1),
        Address::new(0x1000),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(request.attributes(), MemoryRequestAttributes::default());
    assert!(!request.is_privileged());
    assert!(!request.is_secure());
    assert!(!request.is_page_table_walk());
    assert!(!request.is_evict_next());

    let attributed = request
        .with_privileged()
        .with_secure()
        .with_page_table_walk()
        .with_evict_next();

    assert_eq!(
        attributed.attributes(),
        MemoryRequestAttributes::new(true, true, true).with_evict_next()
    );
    assert!(attributed.is_privileged());
    assert!(attributed.is_secure());
    assert!(attributed.is_page_table_walk());
    assert!(attributed.is_evict_next());
    assert_eq!(
        attributed.operation(),
        rem6_memory::MemoryOperation::ReadShared
    );
}

#[test]
fn memory_request_checkpoint_payload_round_trips_attributes() {
    let request = MemoryRequest::prefetch_read(
        request_id(2),
        Address::new(0x2000),
        AccessSize::new(16).unwrap(),
        line_layout(),
    )
    .unwrap()
    .with_privileged()
    .with_secure()
    .with_page_table_walk()
    .with_evict_next();

    let payload = MemoryRequestCheckpointPayload::from_request(&request);
    let decoded = MemoryRequestCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = MemoryRequest::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(decoded.snapshot().attributes(), request.attributes());
    assert_eq!(restored, request);
    assert!(restored.is_privileged());
    assert!(restored.is_secure());
    assert!(restored.is_page_table_walk());
    assert!(restored.is_evict_next());
}

use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, CoherenceIntent, MemoryAccessOrdering,
    MemoryBarrierSet, MemoryOperation, MemoryRequest, MemoryRequestAttributes,
    MemoryRequestCheckpointPayload, MemoryRequestId,
};

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

#[test]
fn no_access_request_has_no_payload_or_data_return() {
    let request = MemoryRequest::no_access(
        request_id(44),
        Address::new(0x5608),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(request.operation(), MemoryOperation::NoAccess);
    assert_eq!(request.coherence_intent(), CoherenceIntent::NoAccess);
    assert!(!request.carries_data());
    assert_eq!(request.byte_mask(), None);
    assert!(!request.requires_writable());
    assert!(request.requires_response());
    assert!(!request.returns_data());
}

#[test]
fn no_access_checkpoint_payload_round_trips_ordering_and_attributes() {
    let request = MemoryRequest::no_access(
        request_id(26),
        Address::new(0x7818),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap()
    .with_attributes(MemoryRequestAttributes::new(true, true, false))
    .with_ordering(MemoryAccessOrdering::new(
        Some(MemoryBarrierSet::memory()),
        Some(MemoryBarrierSet::new(false, true)),
    ));

    let payload = MemoryRequestCheckpointPayload::from_request(&request);
    let decoded = MemoryRequestCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = MemoryRequest::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(restored, request);
    assert_eq!(restored.operation(), MemoryOperation::NoAccess);
    assert_eq!(restored.data(), None);
    assert_eq!(restored.byte_mask(), None);
    assert!(restored.is_privileged());
    assert!(restored.is_secure());
    assert!(!restored.is_page_table_walk());
}

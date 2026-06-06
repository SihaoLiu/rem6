use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, CoherenceIntent, LineMemoryStore,
    MemoryOperation, MemoryRequest, MemoryRequestCheckpointPayload, MemoryRequestId,
    ResponseStatus,
};

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

#[test]
fn store_conditional_fail_request_never_mutates_backing_data() {
    let size = AccessSize::new(4).unwrap();
    let mask = ByteMask::from_bits(vec![true, false, true, true]).unwrap();
    let forced_fail = MemoryRequest::store_conditional_fail(
        request_id(1),
        Address::new(0x2210),
        size,
        vec![0x80, 0x81, 0x82, 0x83],
        mask.clone(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(
        forced_fail.operation(),
        MemoryOperation::StoreConditionalFail
    );
    assert_eq!(forced_fail.coherence_intent(), CoherenceIntent::WriteUnique);
    assert_eq!(forced_fail.range().start(), Address::new(0x2210));
    assert_eq!(forced_fail.size(), size);
    assert_eq!(forced_fail.data(), Some(&[0x80, 0x81, 0x82, 0x83][..]));
    assert_eq!(forced_fail.byte_mask(), Some(&mask));
    assert_eq!(forced_fail.atomic_op(), None);
    assert!(forced_fail.carries_data());
    assert!(forced_fail.requires_writable());
    assert!(forced_fail.requires_response());
    assert!(!forced_fail.returns_data());

    let mut memory = LineMemoryStore::new(line_layout());
    memory
        .insert_line(Address::new(0x2200), vec![0x55; 64])
        .unwrap();
    let load = MemoryRequest::load_locked(request_id(2), Address::new(0x2210), size, line_layout())
        .unwrap();
    memory.respond(&load).unwrap().unwrap();

    let response = memory.respond(&forced_fail).unwrap().unwrap();
    assert_eq!(response.status(), ResponseStatus::StoreConditionalFailed);
    assert_eq!(response.data(), None);

    let read_back =
        MemoryRequest::read_shared(request_id(3), Address::new(0x2210), size, line_layout())
            .unwrap();
    let read_back = memory.respond(&read_back).unwrap().unwrap();
    assert_eq!(read_back.data(), Some(&[0x55; 4][..]));
}

#[test]
fn store_conditional_fail_checkpoint_payload_round_trips() {
    let size = AccessSize::new(8).unwrap();
    let request = MemoryRequest::store_conditional_fail(
        request_id(4),
        Address::new(0x7e18),
        size,
        vec![0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7],
        ByteMask::full(size).unwrap(),
        line_layout(),
    )
    .unwrap();

    let payload = MemoryRequestCheckpointPayload::from_request(&request);
    let decoded = MemoryRequestCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = MemoryRequest::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(decoded.snapshot(), &request.snapshot());
    assert_eq!(restored, request);
    assert_eq!(restored.operation(), MemoryOperation::StoreConditionalFail);
    assert!(restored.requires_writable());
    assert!(!restored.returns_data());
}

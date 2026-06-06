use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequest,
    MemoryRequestCheckpointPayload, MemoryRequestId,
};

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

#[test]
fn memory_request_checkpoint_payload_round_trips_store_conditional_upgrade_requests() {
    let upgrade = MemoryRequest::store_conditional_upgrade(
        request_id(50),
        Address::new(0x7e40),
        AccessSize::new(64).unwrap(),
        line_layout(),
    )
    .unwrap();
    let upgrade_payload = MemoryRequestCheckpointPayload::from_request(&upgrade);
    let upgrade_decoded =
        MemoryRequestCheckpointPayload::decode(upgrade_payload.encode().as_slice()).unwrap();
    let upgrade_restored = MemoryRequest::from_snapshot(upgrade_decoded.snapshot()).unwrap();

    assert_eq!(upgrade_decoded.snapshot(), &upgrade.snapshot());
    assert_eq!(upgrade_restored, upgrade);
    assert_eq!(
        upgrade_restored.operation(),
        MemoryOperation::StoreConditionalUpgrade
    );
    assert!(upgrade_restored.requires_writable());
    assert!(!upgrade_restored.returns_data());

    let fail = MemoryRequest::store_conditional_upgrade_fail(
        request_id(51),
        Address::new(0x7e80),
        AccessSize::new(64).unwrap(),
        line_layout(),
    )
    .unwrap();
    let fail_payload = MemoryRequestCheckpointPayload::from_request(&fail);
    let fail_decoded =
        MemoryRequestCheckpointPayload::decode(fail_payload.encode().as_slice()).unwrap();
    let fail_restored = MemoryRequest::from_snapshot(fail_decoded.snapshot()).unwrap();

    assert_eq!(fail_decoded.snapshot(), &fail.snapshot());
    assert_eq!(fail_restored, fail);
    assert_eq!(
        fail_restored.operation(),
        MemoryOperation::StoreConditionalUpgradeFail
    );
    assert!(fail_restored.requires_writable());
    assert!(fail_restored.returns_data());
}

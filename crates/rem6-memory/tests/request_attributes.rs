use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryAccessOrdering, MemoryError,
    MemoryOperation, MemoryRequest, MemoryRequestAttributes, MemoryRequestCheckpointPayload,
    MemoryRequestId, MemoryRequestSnapshot,
};

const REQUEST_CHECKPOINT_FLAGS_OFFSET: usize = 12;
const REQUEST_CHECKPOINT_SUBSTREAM_OFFSET: usize = 76;
const REQUEST_CHECKPOINT_SUBSTREAM_ID_PRESENT_FLAG: u32 = 1 << 17;

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
    assert!(!request.is_kernel_sync());

    let attributed = request
        .with_privileged()
        .with_secure()
        .with_page_table_walk()
        .with_evict_next()
        .with_kernel_sync();

    assert_eq!(
        attributed.attributes(),
        MemoryRequestAttributes::new(true, true, true)
            .with_evict_next()
            .with_kernel_sync()
    );
    assert!(attributed.is_privileged());
    assert!(attributed.is_secure());
    assert!(attributed.is_page_table_walk());
    assert!(attributed.is_evict_next());
    assert!(attributed.is_kernel_sync());
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
    .with_evict_next()
    .with_kernel_sync();

    let payload = MemoryRequestCheckpointPayload::from_request(&request);
    let decoded = MemoryRequestCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = MemoryRequest::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(decoded.snapshot().attributes(), request.attributes());
    assert_eq!(restored, request);
    assert!(restored.is_privileged());
    assert!(restored.is_secure());
    assert!(restored.is_page_table_walk());
    assert!(restored.is_evict_next());
    assert!(restored.is_kernel_sync());
}

#[test]
fn memory_request_attributes_carry_stream_and_substream_ids() {
    let request = MemoryRequest::read_shared(
        request_id(3),
        Address::new(0x3000),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap()
    .with_stream_id(17)
    .with_substream_id(33)
    .unwrap();

    assert_eq!(request.stream_id(), Some(17));
    assert_eq!(request.substream_id(), Some(33));
    assert_eq!(request.attributes().stream_id(), Some(17));
    assert_eq!(request.attributes().substream_id(), Some(33));

    let payload = MemoryRequestCheckpointPayload::from_request(&request);
    let decoded = MemoryRequestCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = MemoryRequest::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(decoded.snapshot().attributes(), request.attributes());
    assert_eq!(restored.stream_id(), Some(17));
    assert_eq!(restored.substream_id(), Some(33));
}

#[test]
fn memory_request_checkpoint_rejects_substream_without_stream() {
    let request = MemoryRequest::read_shared(
        request_id(4),
        Address::new(0x4000),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();
    let mut payload = MemoryRequestCheckpointPayload::from_request(&request).encode();
    payload[REQUEST_CHECKPOINT_FLAGS_OFFSET..REQUEST_CHECKPOINT_FLAGS_OFFSET + 4]
        .copy_from_slice(&REQUEST_CHECKPOINT_SUBSTREAM_ID_PRESENT_FLAG.to_le_bytes());
    payload[REQUEST_CHECKPOINT_SUBSTREAM_OFFSET..REQUEST_CHECKPOINT_SUBSTREAM_OFFSET + 4]
        .copy_from_slice(&9u32.to_le_bytes());

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidRequestCheckpointFlags {
            flags: REQUEST_CHECKPOINT_SUBSTREAM_ID_PRESENT_FLAG
        }
    );
}

#[test]
fn memory_request_rejects_substream_without_stream() {
    let request = MemoryRequest::read_shared(
        request_id(5),
        Address::new(0x5000),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(
        request.with_substream_id(11).unwrap_err(),
        MemoryError::InvalidRequestStreamAttributes {
            request: request_id(5),
        }
    );
}

#[test]
fn memory_request_attributes_builder_rejects_substream_without_stream() {
    let request = MemoryRequest::read_shared(
        request_id(6),
        Address::new(0x6000),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(
        request
            .with_attributes(MemoryRequestAttributes::default().with_substream_id(11))
            .unwrap_err(),
        MemoryError::InvalidRequestStreamAttributes {
            request: request_id(6),
        }
    );
}

#[test]
fn memory_request_snapshot_rejects_substream_without_stream() {
    assert!(MemoryRequestSnapshot::new_with_attributes(
        request_id(7),
        MemoryOperation::ReadShared,
        Address::new(0x7000),
        AccessSize::new(8).unwrap(),
        line_layout(),
        MemoryAccessOrdering::none(),
        false,
        false,
        MemoryRequestAttributes::default().with_substream_id(11),
        None,
        None,
        None,
    )
    .is_err());
}

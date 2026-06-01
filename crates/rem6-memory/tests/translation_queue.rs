use rem6_memory::{
    AccessSize, Address, AgentId, TranslationAccessKind, TranslationError, TranslationQueue,
    TranslationQueueCheckpointPayload, TranslationQueueConfig, TranslationQueueEntrySnapshot,
    TranslationQueueSnapshot, TranslationRequest, TranslationRequestId, TranslationResolution,
};

fn request_id(sequence: u64) -> TranslationRequestId {
    TranslationRequestId::new(AgentId::new(11), sequence)
}

fn request(sequence: u64, address: u64, access: TranslationAccessKind) -> TranslationRequest {
    TranslationRequest::new(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(8).unwrap(),
        access,
    )
    .unwrap()
}

#[test]
fn translation_queue_orders_ready_requests_and_restores_snapshot() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let mut queue = TranslationQueue::new(config);
    let first = request(1, 0x4000, TranslationAccessKind::Load);
    let second = request(2, 0x8000, TranslationAccessKind::InstructionFetch);

    queue.enqueue(7, first.clone()).unwrap();
    queue.enqueue(5, second.clone()).unwrap();

    assert_eq!(queue.pending_count(), 2);
    assert_eq!(queue.ready_request_ids(7), Vec::new());
    assert_eq!(queue.ready_request_ids(8), vec![second.id()]);
    assert_eq!(queue.ready_request_ids(10), vec![second.id(), first.id()]);

    let snapshot = queue.snapshot();
    let mut restored = TranslationQueue::new(TranslationQueueConfig::new(1, 0).unwrap());
    restored.restore(&snapshot).unwrap();

    assert_eq!(restored.config(), config);
    assert_eq!(
        restored.pending_request_ids(),
        vec![second.id(), first.id()]
    );

    let completions = restored.complete_ready(10, |request| {
        TranslationResolution::mapped(Address::new(request.virtual_address().get() + 0x1000))
    });

    assert_eq!(completions.len(), 2);
    assert_eq!(completions[0].request().id(), second.id());
    assert_eq!(
        completions[0].physical_address(),
        Some(Address::new(0x9000))
    );
    assert_eq!(completions[1].request().id(), first.id());
    assert_eq!(
        completions[1].physical_address(),
        Some(Address::new(0x5000))
    );
    assert_eq!(restored.pending_count(), 0);
}

#[test]
fn translation_queue_restore_rejects_non_monotonic_next_order() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let mut queue = TranslationQueue::new(config);
    let first = request(10, 0x4000, TranslationAccessKind::Load);
    let second = request(11, 0x8000, TranslationAccessKind::InstructionFetch);

    queue.enqueue(7, first).unwrap();
    queue.enqueue(5, second.clone()).unwrap();
    let snapshot = queue.snapshot();
    let second_order = snapshot
        .entries()
        .iter()
        .find(|entry| entry.request().id() == second.id())
        .unwrap()
        .order();
    let corrupt = TranslationQueueSnapshot::new(config, snapshot.entries().to_vec(), second_order);

    assert_eq!(
        queue.restore(&corrupt),
        Err(TranslationError::SnapshotNextOrderTooSmall {
            next_order: second_order,
            request: second.id(),
            order: second_order,
        })
    );
}

#[test]
fn translation_queue_checkpoint_payload_round_trips_snapshot() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let mut queue = TranslationQueue::new(config);
    let first = request(12, 0x4000, TranslationAccessKind::Load);
    let second = request(13, 0x8000, TranslationAccessKind::InstructionFetch);
    queue.enqueue(7, first.clone()).unwrap();
    queue.enqueue(5, second.clone()).unwrap();
    let snapshot = queue.snapshot();
    let payload = TranslationQueueCheckpointPayload::from_snapshot(snapshot.clone()).unwrap();

    let decoded = TranslationQueueCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = TranslationQueue::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(
        restored.pending_request_ids(),
        vec![second.id(), first.id()]
    );
}

#[test]
fn translation_queue_checkpoint_payload_uses_stable_single_entry_bytes() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let mut queue = TranslationQueue::new(config);
    let pending = request(12, 0x4000, TranslationAccessKind::Load);
    queue.enqueue(9, pending.clone()).unwrap();
    let snapshot = queue.snapshot();
    let payload = TranslationQueueCheckpointPayload::from_snapshot(snapshot.clone()).unwrap();
    let mut stable_payload = vec![
        b'M', b'T', b'L', b'Q', 1, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 1, 0,
        0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 11, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    stable_payload.extend_from_slice(&12_u64.to_le_bytes());
    stable_payload.extend_from_slice(&0x4000_u64.to_le_bytes());
    stable_payload.extend_from_slice(&8_u64.to_le_bytes());
    stable_payload.extend_from_slice(&9_u64.to_le_bytes());
    stable_payload.extend_from_slice(&12_u64.to_le_bytes());
    stable_payload.extend_from_slice(&0_u64.to_le_bytes());

    let decoded = TranslationQueueCheckpointPayload::decode(&stable_payload).unwrap();
    let restored = TranslationQueue::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(payload.encode(), stable_payload);
    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(restored.pending_request_ids(), vec![pending.id()]);
}

#[test]
fn translation_queue_checkpoint_payload_rejects_invalid_magic() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let mut queue = TranslationQueue::new(config);
    queue
        .enqueue(9, request(15, 0x5000, TranslationAccessKind::Store))
        .unwrap();
    let mut payload = TranslationQueueCheckpointPayload::from_snapshot(queue.snapshot())
        .unwrap()
        .encode();
    payload[0] = b'X';

    assert_eq!(
        TranslationQueueCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::InvalidQueueCheckpointMagic
    );
}

#[test]
fn translation_queue_checkpoint_payload_rejects_unsupported_version() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let mut queue = TranslationQueue::new(config);
    queue
        .enqueue(9, request(15, 0x5000, TranslationAccessKind::Store))
        .unwrap();
    let mut payload = TranslationQueueCheckpointPayload::from_snapshot(queue.snapshot())
        .unwrap()
        .encode();
    payload[QUEUE_CHECKPOINT_VERSION_OFFSET..QUEUE_CHECKPOINT_VERSION_OFFSET + 4]
        .copy_from_slice(&2_u32.to_le_bytes());

    assert_eq!(
        TranslationQueueCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::UnsupportedQueueCheckpointVersion { version: 2 }
    );
}

#[test]
fn translation_queue_checkpoint_payload_rejects_reserved_field() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let mut queue = TranslationQueue::new(config);
    queue
        .enqueue(9, request(15, 0x5000, TranslationAccessKind::Store))
        .unwrap();
    let mut payload = TranslationQueueCheckpointPayload::from_snapshot(queue.snapshot())
        .unwrap()
        .encode();
    payload[QUEUE_CHECKPOINT_RESERVED_OFFSET..QUEUE_CHECKPOINT_RESERVED_OFFSET + 4]
        .copy_from_slice(&1_u32.to_le_bytes());

    assert_eq!(
        TranslationQueueCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::InvalidQueueCheckpointReserved { value: 1 }
    );
}

#[test]
fn translation_queue_checkpoint_payload_rejects_short_payload() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let mut queue = TranslationQueue::new(config);
    queue
        .enqueue(9, request(15, 0x5000, TranslationAccessKind::Store))
        .unwrap();
    let mut payload = TranslationQueueCheckpointPayload::from_snapshot(queue.snapshot())
        .unwrap()
        .encode();
    payload.truncate(QUEUE_CHECKPOINT_HEADER_SIZE - 1);

    assert_eq!(
        TranslationQueueCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::InvalidQueueCheckpointPayloadSize {
            expected: QUEUE_CHECKPOINT_HEADER_SIZE,
            actual: QUEUE_CHECKPOINT_HEADER_SIZE - 1
        }
    );
}

#[test]
fn translation_queue_checkpoint_payload_rejects_declared_entry_count_mismatch() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let mut queue = TranslationQueue::new(config);
    queue
        .enqueue(9, request(15, 0x5000, TranslationAccessKind::Store))
        .unwrap();
    let mut payload = TranslationQueueCheckpointPayload::from_snapshot(queue.snapshot())
        .unwrap()
        .encode();
    payload[QUEUE_CHECKPOINT_COUNT_OFFSET..QUEUE_CHECKPOINT_COUNT_OFFSET + 4]
        .copy_from_slice(&2_u32.to_le_bytes());

    assert_eq!(
        TranslationQueueCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::InvalidQueueCheckpointPayloadSize {
            expected: QUEUE_CHECKPOINT_HEADER_SIZE + QUEUE_CHECKPOINT_ENTRY_SIZE * 2,
            actual: QUEUE_CHECKPOINT_HEADER_SIZE + QUEUE_CHECKPOINT_ENTRY_SIZE
        }
    );
}

#[test]
fn translation_queue_checkpoint_payload_rejects_extra_entry_record() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let mut queue = TranslationQueue::new(config);
    queue
        .enqueue(9, request(15, 0x5000, TranslationAccessKind::Store))
        .unwrap();
    let mut payload = TranslationQueueCheckpointPayload::from_snapshot(queue.snapshot())
        .unwrap()
        .encode();
    payload[QUEUE_CHECKPOINT_COUNT_OFFSET..QUEUE_CHECKPOINT_COUNT_OFFSET + 4]
        .copy_from_slice(&0_u32.to_le_bytes());

    assert_eq!(
        TranslationQueueCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::InvalidQueueCheckpointPayloadSize {
            expected: QUEUE_CHECKPOINT_HEADER_SIZE,
            actual: QUEUE_CHECKPOINT_HEADER_SIZE + QUEUE_CHECKPOINT_ENTRY_SIZE
        }
    );
}

#[test]
fn translation_queue_checkpoint_payload_rejects_entry_reserved_fields() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let mut queue = TranslationQueue::new(config);
    queue
        .enqueue(9, request(15, 0x5000, TranslationAccessKind::Store))
        .unwrap();
    for offset in [
        QUEUE_CHECKPOINT_FIRST_ENTRY_RESERVED_OFFSET,
        QUEUE_CHECKPOINT_FIRST_ENTRY_RESERVED2_OFFSET,
    ] {
        let mut payload = TranslationQueueCheckpointPayload::from_snapshot(queue.snapshot())
            .unwrap()
            .encode();
        payload[offset..offset + 4].copy_from_slice(&1_u32.to_le_bytes());

        assert_eq!(
            TranslationQueueCheckpointPayload::decode(&payload).unwrap_err(),
            TranslationError::InvalidQueueCheckpointReserved { value: 1 }
        );
    }
}

#[test]
fn translation_queue_checkpoint_payload_rejects_zero_access_size() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let mut queue = TranslationQueue::new(config);
    queue
        .enqueue(9, request(15, 0x5000, TranslationAccessKind::Store))
        .unwrap();
    let mut payload = TranslationQueueCheckpointPayload::from_snapshot(queue.snapshot())
        .unwrap()
        .encode();
    payload[QUEUE_CHECKPOINT_FIRST_ENTRY_SIZE_OFFSET..QUEUE_CHECKPOINT_FIRST_ENTRY_SIZE_OFFSET + 8]
        .copy_from_slice(&0_u64.to_le_bytes());

    assert_eq!(
        TranslationQueueCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::InvalidQueueCheckpointAccessSize { bytes: 0 }
    );
}

#[test]
fn translation_queue_checkpoint_payload_rejects_forged_ready_tick() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let mut queue = TranslationQueue::new(config);
    let store = request(15, 0x5000, TranslationAccessKind::Store);
    queue.enqueue(9, store.clone()).unwrap();
    let mut payload = TranslationQueueCheckpointPayload::from_snapshot(queue.snapshot())
        .unwrap()
        .encode();
    payload[QUEUE_CHECKPOINT_FIRST_ENTRY_READY_TICK_OFFSET
        ..QUEUE_CHECKPOINT_FIRST_ENTRY_READY_TICK_OFFSET + 8]
        .copy_from_slice(&11_u64.to_le_bytes());

    assert_eq!(
        TranslationQueueCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::SnapshotReadyTickMismatch {
            request: store.id(),
            issue_tick: 9,
            latency: 3,
            ready_tick: 11,
        }
    );
}

#[test]
fn translation_queue_checkpoint_payload_rejects_duplicate_request_ids() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let mut queue = TranslationQueue::new(config);
    let first = request(14, 0x5000, TranslationAccessKind::Store);
    queue.enqueue(9, first.clone()).unwrap();
    let payload = TranslationQueueCheckpointPayload::from_snapshot(queue.snapshot())
        .unwrap()
        .encode();
    let duplicate_payload = duplicate_first_queue_checkpoint_entry(payload);

    assert_eq!(
        TranslationQueueCheckpointPayload::decode(&duplicate_payload).unwrap_err(),
        TranslationError::DuplicateRequest {
            request: first.id(),
        }
    );
}

#[test]
fn translation_queue_checkpoint_payload_rejects_invalid_access_code() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let mut queue = TranslationQueue::new(config);
    queue
        .enqueue(9, request(15, 0x5000, TranslationAccessKind::Store))
        .unwrap();
    let mut payload = TranslationQueueCheckpointPayload::from_snapshot(queue.snapshot())
        .unwrap()
        .encode();
    payload[QUEUE_CHECKPOINT_FIRST_ENTRY_ACCESS_OFFSET
        ..QUEUE_CHECKPOINT_FIRST_ENTRY_ACCESS_OFFSET + 4]
        .copy_from_slice(&99_u32.to_le_bytes());

    assert_eq!(
        TranslationQueueCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::InvalidQueueCheckpointAccessKind { code: 99 }
    );
}

#[test]
fn translation_queue_restore_rejects_forged_ready_ticks() {
    let config = TranslationQueueConfig::new(4, 3).unwrap();
    let load = request(16, 0x6000, TranslationAccessKind::Load);
    let entry = TranslationQueueEntrySnapshot::new(load.clone(), 9, 10, 0);
    let snapshot = TranslationQueueSnapshot::new(config, vec![entry], 1);

    assert_eq!(
        TranslationQueue::from_snapshot(&snapshot).unwrap_err(),
        TranslationError::SnapshotReadyTickMismatch {
            request: load.id(),
            issue_tick: 9,
            latency: 3,
            ready_tick: 10,
        }
    );
}

#[test]
fn translation_queue_rejects_invalid_capacity_duplicates_overflow_and_unknown_completion() {
    assert_eq!(
        TranslationQueueConfig::new(0, 1).unwrap_err(),
        TranslationError::ZeroCapacity
    );

    let config = TranslationQueueConfig::new(1, 2).unwrap();
    let mut queue = TranslationQueue::new(config);
    let load = request(3, 0x1000, TranslationAccessKind::Load);
    queue.enqueue(4, load.clone()).unwrap();

    assert_eq!(
        queue.enqueue(5, load).unwrap_err(),
        TranslationError::DuplicateRequest {
            request: request_id(3),
        }
    );
    assert_eq!(
        queue
            .enqueue(5, request(4, 0x2000, TranslationAccessKind::Store))
            .unwrap_err(),
        TranslationError::QueueFull { capacity: 1 }
    );

    assert_eq!(
        TranslationRequest::new(
            request_id(5),
            Address::new(u64::MAX - 3),
            AccessSize::new(8).unwrap(),
            TranslationAccessKind::Atomic,
        )
        .unwrap_err(),
        TranslationError::AddressOverflow {
            start: Address::new(u64::MAX - 3),
            size: AccessSize::new(8).unwrap(),
        }
    );

    assert_eq!(
        queue
            .complete(
                request_id(99),
                TranslationResolution::mapped(Address::new(0x3000)),
            )
            .unwrap_err(),
        TranslationError::UnknownRequest {
            request: request_id(99),
        }
    );
}

const QUEUE_CHECKPOINT_HEADER_SIZE: usize = 40;
const QUEUE_CHECKPOINT_ENTRY_SIZE: usize = 64;
const QUEUE_CHECKPOINT_VERSION_OFFSET: usize = 4;
const QUEUE_CHECKPOINT_COUNT_OFFSET: usize = 32;
const QUEUE_CHECKPOINT_RESERVED_OFFSET: usize = 36;
const QUEUE_CHECKPOINT_FIRST_ENTRY_OFFSET: usize = QUEUE_CHECKPOINT_HEADER_SIZE;
const QUEUE_CHECKPOINT_FIRST_ENTRY_ACCESS_OFFSET: usize = QUEUE_CHECKPOINT_FIRST_ENTRY_OFFSET + 4;
const QUEUE_CHECKPOINT_FIRST_ENTRY_RESERVED_OFFSET: usize = QUEUE_CHECKPOINT_FIRST_ENTRY_OFFSET + 8;
const QUEUE_CHECKPOINT_FIRST_ENTRY_RESERVED2_OFFSET: usize =
    QUEUE_CHECKPOINT_FIRST_ENTRY_OFFSET + 12;
const QUEUE_CHECKPOINT_FIRST_ENTRY_SIZE_OFFSET: usize = QUEUE_CHECKPOINT_FIRST_ENTRY_OFFSET + 32;
const QUEUE_CHECKPOINT_FIRST_ENTRY_READY_TICK_OFFSET: usize =
    QUEUE_CHECKPOINT_FIRST_ENTRY_OFFSET + 48;

fn duplicate_first_queue_checkpoint_entry(mut payload: Vec<u8>) -> Vec<u8> {
    let count_offset = 32;
    payload[count_offset..count_offset + 4].copy_from_slice(&2_u32.to_le_bytes());
    let first_entry_offset = 40;
    let entry_record_bytes = 64;
    let first_entry = payload[first_entry_offset..first_entry_offset + entry_record_bytes].to_vec();
    payload.splice(first_entry_offset..first_entry_offset, first_entry);
    payload
}

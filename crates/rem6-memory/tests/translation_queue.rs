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
    let access_offset = 44;
    payload[access_offset..access_offset + 4].copy_from_slice(&99_u32.to_le_bytes());

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

fn duplicate_first_queue_checkpoint_entry(mut payload: Vec<u8>) -> Vec<u8> {
    let count_offset = 32;
    payload[count_offset..count_offset + 4].copy_from_slice(&2_u32.to_le_bytes());
    let first_entry_offset = 40;
    let entry_record_bytes = 64;
    let first_entry = payload[first_entry_offset..first_entry_offset + entry_record_bytes].to_vec();
    payload.splice(first_entry_offset..first_entry_offset, first_entry);
    payload
}

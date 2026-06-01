use rem6_memory::{
    AccessSize, Address, AgentId, TranslationAccessKind, TranslationError, TranslationFault,
    TranslationFaultKind, TranslationPageMap, TranslationPageMapCheckpointPayload,
    TranslationPagePermissions, TranslationPageSize, TranslationQueue, TranslationQueueConfig,
    TranslationRequest, TranslationRequestId, TranslationResolution, TranslationSegment,
    TranslationSegmentedResolution,
};

fn request_id(sequence: u64) -> TranslationRequestId {
    TranslationRequestId::new(AgentId::new(23), sequence)
}

fn request(
    sequence: u64,
    address: u64,
    bytes: u64,
    access: TranslationAccessKind,
) -> TranslationRequest {
    TranslationRequest::new(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(bytes).unwrap(),
        access,
    )
    .unwrap()
}

#[test]
fn page_translation_map_resolves_offsets_permissions_and_queue_completions() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    let code_base = Address::new(0xffff_0000_8000_0000);
    let data_base = Address::new(0xffff_0000_9000_0000);

    map.map(
        code_base,
        Address::new(0x0000_0000_8000_0000),
        2,
        TranslationPagePermissions::read_execute(),
    )
    .unwrap();
    map.map(
        data_base,
        Address::new(0x0000_0000_9000_0000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();

    assert_eq!(
        map.translate(&request(
            1,
            code_base.get() + 0x34,
            4,
            TranslationAccessKind::InstructionFetch,
        )),
        TranslationResolution::mapped(Address::new(0x0000_0000_8000_0034))
    );
    assert_eq!(
        map.translate(&request(
            2,
            data_base.get() + 0x80,
            8,
            TranslationAccessKind::Store,
        )),
        TranslationResolution::mapped(Address::new(0x0000_0000_9000_0080))
    );
    assert_eq!(
        map.translate(&request(
            3,
            code_base.get() + 0x40,
            8,
            TranslationAccessKind::Store,
        )),
        TranslationResolution::fault(TranslationFault::new(
            Address::new(code_base.get() + 0x40),
            TranslationFaultKind::PermissionFault,
        ))
    );
    assert_eq!(
        map.translate(&request(
            4,
            data_base.get() + 0xffe,
            4,
            TranslationAccessKind::Load,
        )),
        TranslationResolution::fault(TranslationFault::new(
            Address::new(data_base.get() + 0xffe),
            TranslationFaultKind::PageFault,
        ))
    );

    let mut queue = TranslationQueue::new(TranslationQueueConfig::new(2, 1).unwrap());
    let fetch = request(
        5,
        code_base.get() + 0x1000,
        4,
        TranslationAccessKind::InstructionFetch,
    );
    let load = request(6, data_base.get() + 0x10, 4, TranslationAccessKind::Load);
    queue.enqueue(7, fetch.clone()).unwrap();
    queue.enqueue(6, load.clone()).unwrap();

    let completions = queue.complete_ready(8, |request| map.translate(request));
    assert_eq!(completions.len(), 2);
    assert_eq!(completions[0].request().id(), load.id());
    assert_eq!(
        completions[0].physical_address(),
        Some(Address::new(0x0000_0000_9000_0010))
    );
    assert_eq!(completions[1].request().id(), fetch.id());
    assert_eq!(
        completions[1].physical_address(),
        Some(Address::new(0x0000_0000_8000_1000))
    );
}

#[test]
fn page_translation_map_rejects_bad_shapes_overlaps_and_restores_snapshot() {
    assert_eq!(
        TranslationPageSize::new(0).unwrap_err(),
        TranslationError::ZeroPageSize
    );
    assert_eq!(
        TranslationPageSize::new(3000).unwrap_err(),
        TranslationError::NonPowerOfTwoPageSize { bytes: 3000 }
    );

    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    assert_eq!(
        map.map(
            Address::new(0x1001),
            Address::new(0x8000),
            1,
            TranslationPagePermissions::read_only(),
        )
        .unwrap_err(),
        TranslationError::UnalignedVirtualPage {
            address: Address::new(0x1001),
            page_size,
        }
    );
    assert_eq!(
        map.map(
            Address::new(0x1000),
            Address::new(0x8001),
            1,
            TranslationPagePermissions::read_only(),
        )
        .unwrap_err(),
        TranslationError::UnalignedPhysicalPage {
            address: Address::new(0x8001),
            page_size,
        }
    );
    assert_eq!(
        map.map(
            Address::new(0x1000),
            Address::new(0x8000),
            0,
            TranslationPagePermissions::read_only(),
        )
        .unwrap_err(),
        TranslationError::ZeroPageCount
    );

    map.map(
        Address::new(0x1000),
        Address::new(0x8000),
        2,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    assert_eq!(
        map.map(
            Address::new(0x2000),
            Address::new(0xa000),
            1,
            TranslationPagePermissions::read_only(),
        )
        .unwrap_err(),
        TranslationError::OverlappingTranslationMapping {
            existing_start: Address::new(0x1000),
            existing_size: AccessSize::new(0x2000).unwrap(),
            requested_start: Address::new(0x2000),
            requested_size: AccessSize::new(0x1000).unwrap(),
        }
    );

    let snapshot = map.snapshot();
    let mut restored = TranslationPageMap::new(TranslationPageSize::new(8192).unwrap());
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.page_size(), page_size);
    assert_eq!(restored.mapping_count(), 1);
    assert_eq!(
        restored.translate(&request(7, 0x1808, 8, TranslationAccessKind::Load)),
        TranslationResolution::mapped(Address::new(0x8808))
    );
}

#[test]
fn page_translation_map_checkpoint_payload_round_trips_snapshot() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        Address::new(0x4000),
        Address::new(0x8000),
        2,
        TranslationPagePermissions::read_execute(),
    )
    .unwrap();
    map.map(
        Address::new(0x9000),
        Address::new(0xc000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    let snapshot = map.snapshot();
    let payload = TranslationPageMapCheckpointPayload::from_snapshot(snapshot.clone()).unwrap();

    let decoded = TranslationPageMapCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = TranslationPageMap::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(restored.page_size(), page_size);
    assert_eq!(restored.mapping_count(), 2);
    assert_eq!(
        restored.translate(&request(
            8,
            0x4008,
            4,
            TranslationAccessKind::InstructionFetch
        )),
        TranslationResolution::mapped(Address::new(0x8008))
    );
}

#[test]
fn page_translation_map_checkpoint_payload_uses_stable_single_mapping_bytes() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        Address::new(0x4000),
        Address::new(0x8000),
        2,
        TranslationPagePermissions::read_execute(),
    )
    .unwrap();
    let snapshot = map.snapshot();
    let payload = TranslationPageMapCheckpointPayload::from_snapshot(snapshot.clone()).unwrap();
    let mut stable_payload = vec![
        b'M', b'T', b'P', b'M', 1, 0, 0, 0, 0, 0x10, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
    ];
    stable_payload.extend_from_slice(&0x4000_u64.to_le_bytes());
    stable_payload.extend_from_slice(&0x8000_u64.to_le_bytes());
    stable_payload.extend_from_slice(&2_u64.to_le_bytes());
    stable_payload.extend_from_slice(&5_u32.to_le_bytes());
    stable_payload.extend_from_slice(&0_u32.to_le_bytes());

    let decoded = TranslationPageMapCheckpointPayload::decode(&stable_payload).unwrap();
    let restored = TranslationPageMap::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(payload.encode(), stable_payload);
    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(
        restored.translate(&request(
            8,
            0x4008,
            4,
            TranslationAccessKind::InstructionFetch
        )),
        TranslationResolution::mapped(Address::new(0x8008))
    );
}

#[test]
fn page_translation_map_checkpoint_payload_rejects_invalid_magic() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        Address::new(0x1000),
        Address::new(0x8000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    let mut payload = TranslationPageMapCheckpointPayload::from_snapshot(map.snapshot())
        .unwrap()
        .encode();
    payload[0] = b'X';

    assert_eq!(
        TranslationPageMapCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::InvalidPageMapCheckpointMagic
    );
}

#[test]
fn page_translation_map_checkpoint_payload_rejects_unsupported_version() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        Address::new(0x1000),
        Address::new(0x8000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    let mut payload = TranslationPageMapCheckpointPayload::from_snapshot(map.snapshot())
        .unwrap()
        .encode();
    payload[PAGE_MAP_CHECKPOINT_VERSION_OFFSET..PAGE_MAP_CHECKPOINT_VERSION_OFFSET + 4]
        .copy_from_slice(&2_u32.to_le_bytes());

    assert_eq!(
        TranslationPageMapCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::UnsupportedPageMapCheckpointVersion { version: 2 }
    );
}

#[test]
fn page_translation_map_checkpoint_payload_rejects_reserved_field() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        Address::new(0x1000),
        Address::new(0x8000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    let mut payload = TranslationPageMapCheckpointPayload::from_snapshot(map.snapshot())
        .unwrap()
        .encode();
    payload[PAGE_MAP_CHECKPOINT_RESERVED_OFFSET..PAGE_MAP_CHECKPOINT_RESERVED_OFFSET + 4]
        .copy_from_slice(&1_u32.to_le_bytes());

    assert_eq!(
        TranslationPageMapCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::InvalidPageMapCheckpointReserved { value: 1 }
    );
}

#[test]
fn page_translation_map_checkpoint_payload_rejects_short_payload() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        Address::new(0x1000),
        Address::new(0x8000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    let mut payload = TranslationPageMapCheckpointPayload::from_snapshot(map.snapshot())
        .unwrap()
        .encode();
    payload.truncate(PAGE_MAP_CHECKPOINT_HEADER_SIZE - 1);

    assert_eq!(
        TranslationPageMapCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::InvalidPageMapCheckpointPayloadSize {
            expected: PAGE_MAP_CHECKPOINT_HEADER_SIZE,
            actual: PAGE_MAP_CHECKPOINT_HEADER_SIZE - 1
        }
    );
}

#[test]
fn page_translation_map_checkpoint_payload_rejects_declared_mapping_count_mismatch() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        Address::new(0x1000),
        Address::new(0x8000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    let mut payload = TranslationPageMapCheckpointPayload::from_snapshot(map.snapshot())
        .unwrap()
        .encode();
    payload[PAGE_MAP_CHECKPOINT_COUNT_OFFSET..PAGE_MAP_CHECKPOINT_COUNT_OFFSET + 4]
        .copy_from_slice(&2_u32.to_le_bytes());

    assert_eq!(
        TranslationPageMapCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::InvalidPageMapCheckpointPayloadSize {
            expected: PAGE_MAP_CHECKPOINT_HEADER_SIZE + PAGE_MAP_CHECKPOINT_ENTRY_SIZE * 2,
            actual: PAGE_MAP_CHECKPOINT_HEADER_SIZE + PAGE_MAP_CHECKPOINT_ENTRY_SIZE
        }
    );
}

#[test]
fn page_translation_map_checkpoint_payload_rejects_extra_mapping_record() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        Address::new(0x1000),
        Address::new(0x8000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    let mut payload = TranslationPageMapCheckpointPayload::from_snapshot(map.snapshot())
        .unwrap()
        .encode();
    payload[PAGE_MAP_CHECKPOINT_COUNT_OFFSET..PAGE_MAP_CHECKPOINT_COUNT_OFFSET + 4]
        .copy_from_slice(&0_u32.to_le_bytes());

    assert_eq!(
        TranslationPageMapCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::InvalidPageMapCheckpointPayloadSize {
            expected: PAGE_MAP_CHECKPOINT_HEADER_SIZE,
            actual: PAGE_MAP_CHECKPOINT_HEADER_SIZE + PAGE_MAP_CHECKPOINT_ENTRY_SIZE
        }
    );
}

#[test]
fn page_translation_map_checkpoint_payload_rejects_overlapping_mapping_records() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        Address::new(0x1000),
        Address::new(0x8000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    let payload = TranslationPageMapCheckpointPayload::from_snapshot(map.snapshot())
        .unwrap()
        .encode();
    let duplicate_payload = duplicate_first_page_map_checkpoint_entry(payload);

    assert_eq!(
        TranslationPageMapCheckpointPayload::decode(&duplicate_payload).unwrap_err(),
        TranslationError::OverlappingTranslationMapping {
            existing_start: Address::new(0x1000),
            existing_size: AccessSize::new(0x1000).unwrap(),
            requested_start: Address::new(0x1000),
            requested_size: AccessSize::new(0x1000).unwrap(),
        }
    );
}

#[test]
fn page_translation_map_checkpoint_payload_rejects_mapping_record_reserved_field() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        Address::new(0x1000),
        Address::new(0x8000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    let mut payload = TranslationPageMapCheckpointPayload::from_snapshot(map.snapshot())
        .unwrap()
        .encode();
    payload[PAGE_MAP_CHECKPOINT_FIRST_ENTRY_RESERVED_OFFSET
        ..PAGE_MAP_CHECKPOINT_FIRST_ENTRY_RESERVED_OFFSET + 4]
        .copy_from_slice(&1_u32.to_le_bytes());

    assert_eq!(
        TranslationPageMapCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::InvalidPageMapCheckpointReserved { value: 1 }
    );
}

#[test]
fn page_translation_map_checkpoint_payload_rejects_invalid_permission_bits() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        Address::new(0x1000),
        Address::new(0x8000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    let mut payload = TranslationPageMapCheckpointPayload::from_snapshot(map.snapshot())
        .unwrap()
        .encode();
    payload[PAGE_MAP_CHECKPOINT_FIRST_ENTRY_PERMISSIONS_OFFSET
        ..PAGE_MAP_CHECKPOINT_FIRST_ENTRY_PERMISSIONS_OFFSET + 4]
        .copy_from_slice(&8_u32.to_le_bytes());

    assert_eq!(
        TranslationPageMapCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::InvalidPageMapCheckpointPermissions { code: 8 }
    );
}

#[test]
fn page_translation_map_checkpoint_payload_rejects_zero_page_count() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        Address::new(0x1000),
        Address::new(0x8000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    let mut payload = TranslationPageMapCheckpointPayload::from_snapshot(map.snapshot())
        .unwrap()
        .encode();
    payload[PAGE_MAP_CHECKPOINT_FIRST_ENTRY_PAGE_COUNT_OFFSET
        ..PAGE_MAP_CHECKPOINT_FIRST_ENTRY_PAGE_COUNT_OFFSET + 8]
        .copy_from_slice(&0_u64.to_le_bytes());

    assert_eq!(
        TranslationPageMapCheckpointPayload::decode(&payload).unwrap_err(),
        TranslationError::ZeroPageCount
    );
}

const PAGE_MAP_CHECKPOINT_HEADER_SIZE: usize = 24;
const PAGE_MAP_CHECKPOINT_ENTRY_SIZE: usize = 32;
const PAGE_MAP_CHECKPOINT_VERSION_OFFSET: usize = 4;
const PAGE_MAP_CHECKPOINT_COUNT_OFFSET: usize = 16;
const PAGE_MAP_CHECKPOINT_RESERVED_OFFSET: usize = 20;
const PAGE_MAP_CHECKPOINT_FIRST_ENTRY_OFFSET: usize = PAGE_MAP_CHECKPOINT_HEADER_SIZE;
const PAGE_MAP_CHECKPOINT_FIRST_ENTRY_PAGE_COUNT_OFFSET: usize =
    PAGE_MAP_CHECKPOINT_FIRST_ENTRY_OFFSET + 16;
const PAGE_MAP_CHECKPOINT_FIRST_ENTRY_PERMISSIONS_OFFSET: usize =
    PAGE_MAP_CHECKPOINT_FIRST_ENTRY_OFFSET + 24;
const PAGE_MAP_CHECKPOINT_FIRST_ENTRY_RESERVED_OFFSET: usize =
    PAGE_MAP_CHECKPOINT_FIRST_ENTRY_OFFSET + 28;

#[test]
fn page_translation_map_splits_cross_page_translation_into_explicit_segments() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        Address::new(0x4000),
        Address::new(0x9000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    map.map(
        Address::new(0x5000),
        Address::new(0xb000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();

    let crossing = request(8, 0x4ff8, 16, TranslationAccessKind::Load);
    assert_eq!(
        map.translate_segments(&crossing),
        TranslationSegmentedResolution::mapped(vec![
            TranslationSegment::new(
                Address::new(0x4ff8),
                AccessSize::new(8).unwrap(),
                Address::new(0x9ff8),
            )
            .unwrap(),
            TranslationSegment::new(
                Address::new(0x5000),
                AccessSize::new(8).unwrap(),
                Address::new(0xb000),
            )
            .unwrap(),
        ])
    );

    let single_page = request(9, 0x4080, 8, TranslationAccessKind::Store);
    assert_eq!(
        map.translate_segments(&single_page),
        TranslationSegmentedResolution::mapped(vec![TranslationSegment::new(
            Address::new(0x4080),
            AccessSize::new(8).unwrap(),
            Address::new(0x9080),
        )
        .unwrap()])
    );
}

fn duplicate_first_page_map_checkpoint_entry(mut payload: Vec<u8>) -> Vec<u8> {
    let count_offset = 16;
    payload[count_offset..count_offset + 4].copy_from_slice(&2_u32.to_le_bytes());
    let first_entry_offset = 24;
    let entry_record_bytes = 32;
    let first_entry = payload[first_entry_offset..first_entry_offset + entry_record_bytes].to_vec();
    payload.splice(first_entry_offset..first_entry_offset, first_entry);
    payload
}

#[test]
fn page_translation_map_reports_first_failed_cross_page_segment() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        Address::new(0x8000),
        Address::new(0x18000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    map.map(
        Address::new(0x9000),
        Address::new(0x1a000),
        1,
        TranslationPagePermissions::read_only(),
    )
    .unwrap();

    let permission_fault = request(10, 0x8ff0, 32, TranslationAccessKind::Store);
    assert_eq!(
        map.translate_segments(&permission_fault),
        TranslationSegmentedResolution::fault(TranslationFault::new(
            Address::new(0x9000),
            TranslationFaultKind::PermissionFault,
        ))
    );

    let page_fault = request(11, 0x9ff0, 32, TranslationAccessKind::Load);
    assert_eq!(
        map.translate_segments(&page_fault),
        TranslationSegmentedResolution::fault(TranslationFault::new(
            Address::new(0xa000),
            TranslationFaultKind::PageFault,
        ))
    );
}

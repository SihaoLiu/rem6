use rem6_memory::{
    AccessSize, Address, AgentId, TranslationAccessKind, TranslationError, TranslationFault,
    TranslationFaultKind, TranslationPageMap, TranslationPagePermissions, TranslationPageSize,
    TranslationQueue, TranslationQueueConfig, TranslationRequest, TranslationRequestId,
    TranslationResolution,
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

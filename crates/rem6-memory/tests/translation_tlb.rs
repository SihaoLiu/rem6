use rem6_memory::{
    AccessSize, Address, AgentId, TranslationAccessKind, TranslationError, TranslationFault,
    TranslationFaultKind, TranslationPageMap, TranslationPagePermissions, TranslationPageSize,
    TranslationRequest, TranslationRequestId, TranslationResolution, TranslationTlb,
    TranslationTlbConfig, TranslationTlbLookupKind, TranslationTlbStats,
};

fn request_id(sequence: u64) -> TranslationRequestId {
    TranslationRequestId::new(AgentId::new(31), sequence)
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

fn mapped_page_map() -> TranslationPageMap {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        Address::new(0xffff_0000_8000_0000),
        Address::new(0x0000_0000_8000_0000),
        1,
        TranslationPagePermissions::read_execute(),
    )
    .unwrap();
    map.map(
        Address::new(0xffff_0000_9000_0000),
        Address::new(0x0000_0000_9000_0000),
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    map.map(
        Address::new(0xffff_0000_a000_0000),
        Address::new(0x0000_0000_a000_0000),
        1,
        TranslationPagePermissions::read_execute(),
    )
    .unwrap();
    map
}

#[test]
fn translation_tlb_caches_page_entries_uses_lru_and_restores_snapshot() {
    let map = mapped_page_map();
    let code_base = Address::new(0xffff_0000_8000_0000);
    let data_base = Address::new(0xffff_0000_9000_0000);
    let next_code_base = Address::new(0xffff_0000_a000_0000);

    let mut tlb = TranslationTlb::new(TranslationTlbConfig::new(2).unwrap());
    let first_fetch = tlb
        .translate(
            &request(
                1,
                code_base.get() + 0x40,
                4,
                TranslationAccessKind::InstructionFetch,
            ),
            &map,
        )
        .unwrap();
    assert_eq!(first_fetch.kind(), TranslationTlbLookupKind::Miss);
    assert_eq!(
        first_fetch.resolution(),
        &TranslationResolution::mapped(Address::new(0x0000_0000_8000_0040))
    );

    let data_load = tlb
        .translate(
            &request(2, data_base.get() + 0x20, 8, TranslationAccessKind::Load),
            &map,
        )
        .unwrap();
    assert_eq!(data_load.kind(), TranslationTlbLookupKind::Miss);

    let second_fetch = tlb
        .translate(
            &request(
                3,
                code_base.get() + 0x80,
                4,
                TranslationAccessKind::InstructionFetch,
            ),
            &map,
        )
        .unwrap();
    assert_eq!(second_fetch.kind(), TranslationTlbLookupKind::Hit);
    assert_eq!(
        second_fetch.physical_address(),
        Some(Address::new(0x0000_0000_8000_0080))
    );

    let third_page = tlb
        .translate(
            &request(
                4,
                next_code_base.get() + 0x10,
                4,
                TranslationAccessKind::InstructionFetch,
            ),
            &map,
        )
        .unwrap();
    assert_eq!(third_page.kind(), TranslationTlbLookupKind::Miss);
    assert_eq!(tlb.entry_count(), 2);
    assert!(tlb.contains_virtual_page(code_base));
    assert!(tlb.contains_virtual_page(next_code_base));
    assert!(!tlb.contains_virtual_page(data_base));
    assert_eq!(tlb.stats(), TranslationTlbStats::new(1, 3, 0, 3, 1));

    let snapshot = tlb.snapshot();
    let mut restored = TranslationTlb::new(TranslationTlbConfig::new(1).unwrap());
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.config(), TranslationTlbConfig::new(2).unwrap());
    assert_eq!(restored.entry_count(), 2);
    assert_eq!(restored.stats(), TranslationTlbStats::new(1, 3, 0, 3, 1));

    let restored_fetch = restored
        .translate(
            &request(
                5,
                code_base.get() + 0x100,
                4,
                TranslationAccessKind::InstructionFetch,
            ),
            &map,
        )
        .unwrap();
    assert_eq!(restored_fetch.kind(), TranslationTlbLookupKind::Hit);
    assert_eq!(
        restored_fetch.physical_address(),
        Some(Address::new(0x0000_0000_8000_0100))
    );
    assert_eq!(restored.stats(), TranslationTlbStats::new(2, 3, 0, 3, 1));
}

#[test]
fn translation_tlb_rechecks_permissions_faults_without_polluting_cache() {
    assert_eq!(
        TranslationTlbConfig::new(0).unwrap_err(),
        TranslationError::ZeroTlbCapacity
    );

    let map = mapped_page_map();
    let code_base = Address::new(0xffff_0000_8000_0000);
    let unmapped_base = Address::new(0xffff_0000_b000_0000);

    let mut tlb = TranslationTlb::new(TranslationTlbConfig::new(4).unwrap());
    assert_eq!(
        tlb.translate(
            &request(
                6,
                code_base.get() + 0x8,
                4,
                TranslationAccessKind::InstructionFetch,
            ),
            &map,
        )
        .unwrap()
        .physical_address(),
        Some(Address::new(0x0000_0000_8000_0008))
    );
    assert_eq!(tlb.entry_count(), 1);

    let forbidden_store = tlb
        .translate(
            &request(7, code_base.get() + 0x10, 4, TranslationAccessKind::Store),
            &map,
        )
        .unwrap();
    assert_eq!(forbidden_store.kind(), TranslationTlbLookupKind::Hit);
    assert_eq!(
        forbidden_store.fault(),
        Some(&TranslationFault::new(
            Address::new(code_base.get() + 0x10),
            TranslationFaultKind::PermissionFault,
        ))
    );
    assert_eq!(tlb.entry_count(), 1);

    let missing_page = tlb
        .translate(
            &request(
                8,
                unmapped_base.get() + 0x20,
                4,
                TranslationAccessKind::Load,
            ),
            &map,
        )
        .unwrap();
    assert_eq!(missing_page.kind(), TranslationTlbLookupKind::Miss);
    assert_eq!(
        missing_page.fault(),
        Some(&TranslationFault::new(
            Address::new(unmapped_base.get() + 0x20),
            TranslationFaultKind::PageFault,
        ))
    );
    assert_eq!(tlb.entry_count(), 1);
    assert!(!tlb.contains_virtual_page(unmapped_base));
    assert_eq!(tlb.stats(), TranslationTlbStats::new(1, 2, 2, 1, 0));
}

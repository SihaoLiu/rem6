use rem6_memory::{
    AccessSize, Address, AgentId, TranslationAccessKind, TranslationAddressSpaceId,
    TranslationError, TranslationFault, TranslationFaultKind, TranslationPageMap,
    TranslationPagePermissions, TranslationPageSize, TranslationRequest, TranslationRequestId,
    TranslationResolution, TranslationSegmentedResolution, TranslationTlb,
    TranslationTlbCheckpointPayload, TranslationTlbConfig, TranslationTlbEntryScope,
    TranslationTlbEntrySnapshot, TranslationTlbLookupKind, TranslationTlbSnapshot,
    TranslationTlbStats,
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

fn single_page_map(
    virtual_base: Address,
    physical_base: Address,
    permissions: TranslationPagePermissions,
) -> TranslationPageMap {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(virtual_base, physical_base, 1, permissions)
        .unwrap();
    map
}

fn two_page_map(
    virtual_base: Address,
    first_physical_base: Address,
    second_physical_base: Address,
) -> TranslationPageMap {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let mut map = TranslationPageMap::new(page_size);
    map.map(
        virtual_base,
        first_physical_base,
        1,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();
    map.map(
        Address::new(virtual_base.get() + page_size.bytes()),
        second_physical_base,
        1,
        TranslationPagePermissions::read_write(),
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
fn translation_tlb_restore_rejects_non_monotonic_next_lru() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let entry = TranslationTlbEntrySnapshot::new(
        Address::new(0xffff_0000_b000_0000),
        Address::new(0x0000_0000_b000_0000),
        page_size,
        TranslationPagePermissions::read_write(),
        7,
    );
    let snapshot = TranslationTlbSnapshot::new(
        TranslationTlbConfig::new(2).unwrap(),
        vec![entry.clone()],
        entry.last_used(),
        TranslationTlbStats::new(0, 0, 0, 1, 0),
    );
    let mut tlb = TranslationTlb::new(TranslationTlbConfig::new(2).unwrap());

    assert_eq!(
        tlb.restore(&snapshot),
        Err(TranslationError::SnapshotNextLruTooSmall {
            next_lru: entry.last_used(),
            virtual_page: entry.virtual_page(),
            last_used: entry.last_used(),
        })
    );
}

#[test]
fn translation_tlb_checkpoint_payload_round_trips_snapshot() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let snapshot = TranslationTlbSnapshot::new(
        TranslationTlbConfig::new(4).unwrap(),
        vec![
            TranslationTlbEntrySnapshot::new(
                Address::new(0xffff_0000_d000_0000),
                Address::new(0x0000_0000_d000_0000),
                page_size,
                TranslationPagePermissions::read_write(),
                3,
            )
            .with_scope(TranslationTlbEntryScope::Global),
            TranslationTlbEntrySnapshot::new_in_address_space(
                TranslationAddressSpaceId::new(7),
                Address::new(0xffff_0000_c000_0000),
                Address::new(0x0000_0000_c000_0000),
                page_size,
                TranslationPagePermissions::read_execute(),
                2,
            ),
        ],
        4,
        TranslationTlbStats::new(5, 6, 7, 8, 9),
    );
    let payload = TranslationTlbCheckpointPayload::from_snapshot(snapshot.clone()).unwrap();

    let decoded = TranslationTlbCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = TranslationTlb::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(restored.snapshot(), snapshot);
}

#[test]
fn translation_tlb_checkpoint_payload_rejects_duplicate_entries() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let entry = TranslationTlbEntrySnapshot::new(
        Address::new(0xffff_0000_e000_0000),
        Address::new(0x0000_0000_e000_0000),
        page_size,
        TranslationPagePermissions::read_write(),
        1,
    );
    let snapshot = TranslationTlbSnapshot::new(
        TranslationTlbConfig::new(4).unwrap(),
        vec![entry],
        2,
        TranslationTlbStats::new(0, 0, 0, 1, 0),
    );
    let payload = TranslationTlbCheckpointPayload::from_snapshot(snapshot)
        .unwrap()
        .encode();
    let duplicate_payload = duplicate_first_tlb_checkpoint_entry(payload);

    assert_eq!(
        TranslationTlbCheckpointPayload::decode(&duplicate_payload).unwrap_err(),
        TranslationError::DuplicateTlbEntry {
            virtual_page: Address::new(0xffff_0000_e000_0000),
        }
    );
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

fn duplicate_first_tlb_checkpoint_entry(mut payload: Vec<u8>) -> Vec<u8> {
    let count_offset = 68;
    payload[count_offset..count_offset + 4].copy_from_slice(&2_u32.to_le_bytes());
    let first_entry_offset = 72;
    let entry_record_bytes = 48;
    let first_entry = payload[first_entry_offset..first_entry_offset + entry_record_bytes].to_vec();
    payload.splice(first_entry_offset..first_entry_offset, first_entry);
    payload
}

#[test]
fn translation_tlb_isolates_address_spaces_and_restores_asid_entries() {
    let virtual_base = Address::new(0xffff_0000_c000_0000);
    let asid_one = TranslationAddressSpaceId::new(1);
    let asid_two = TranslationAddressSpaceId::new(2);
    let map_one = single_page_map(
        virtual_base,
        Address::new(0x0000_0000_c000_0000),
        TranslationPagePermissions::read_write(),
    );
    let map_two = single_page_map(
        virtual_base,
        Address::new(0x0000_0000_d000_0000),
        TranslationPagePermissions::read_write(),
    );

    let mut tlb = TranslationTlb::new(TranslationTlbConfig::new(4).unwrap());
    let first = tlb
        .translate_in_address_space(
            asid_one,
            &request(9, virtual_base.get() + 0x18, 8, TranslationAccessKind::Load),
            &map_one,
        )
        .unwrap();
    assert_eq!(first.kind(), TranslationTlbLookupKind::Miss);
    assert_eq!(
        first.physical_address(),
        Some(Address::new(0x0000_0000_c000_0018))
    );

    let second = tlb
        .translate_in_address_space(
            asid_two,
            &request(
                10,
                virtual_base.get() + 0x18,
                8,
                TranslationAccessKind::Load,
            ),
            &map_two,
        )
        .unwrap();
    assert_eq!(second.kind(), TranslationTlbLookupKind::Miss);
    assert_eq!(
        second.physical_address(),
        Some(Address::new(0x0000_0000_d000_0018))
    );
    assert_eq!(tlb.entry_count(), 2);
    assert!(tlb.contains_entry(asid_one, virtual_base));
    assert!(tlb.contains_entry(asid_two, virtual_base));

    let hit_one = tlb
        .translate_in_address_space(
            asid_one,
            &request(
                11,
                virtual_base.get() + 0x40,
                8,
                TranslationAccessKind::Load,
            ),
            &map_one,
        )
        .unwrap();
    assert_eq!(hit_one.kind(), TranslationTlbLookupKind::Hit);
    assert_eq!(
        hit_one.physical_address(),
        Some(Address::new(0x0000_0000_c000_0040))
    );

    let snapshot = tlb.snapshot();
    let mut restored = TranslationTlb::new(TranslationTlbConfig::new(1).unwrap());
    restored.restore(&snapshot).unwrap();
    assert!(restored.contains_entry(asid_one, virtual_base));
    assert!(restored.contains_entry(asid_two, virtual_base));
    assert_eq!(restored.snapshot().entries()[0].address_space(), asid_one);
    assert_eq!(restored.snapshot().entries()[1].address_space(), asid_two);

    let restored_hit_two = restored
        .translate_in_address_space(
            asid_two,
            &request(
                12,
                virtual_base.get() + 0x80,
                8,
                TranslationAccessKind::Load,
            ),
            &map_two,
        )
        .unwrap();
    assert_eq!(restored_hit_two.kind(), TranslationTlbLookupKind::Hit);
    assert_eq!(
        restored_hit_two.physical_address(),
        Some(Address::new(0x0000_0000_d000_0080))
    );
}

#[test]
fn translation_tlb_flushes_all_address_space_and_page_scopes_explicitly() {
    let virtual_base = Address::new(0xffff_0000_e000_0000);
    let next_virtual_base = Address::new(0xffff_0000_e000_1000);
    let asid_one = TranslationAddressSpaceId::new(3);
    let asid_two = TranslationAddressSpaceId::new(4);
    let mut map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    map.map(
        virtual_base,
        Address::new(0x0000_0000_e000_0000),
        2,
        TranslationPagePermissions::read_write(),
    )
    .unwrap();

    let mut tlb = TranslationTlb::new(TranslationTlbConfig::new(8).unwrap());
    for (sequence, address_space, page) in [
        (13, asid_one, virtual_base),
        (14, asid_one, next_virtual_base),
        (15, asid_two, virtual_base),
        (16, asid_two, next_virtual_base),
    ] {
        tlb.translate_in_address_space(
            address_space,
            &request(sequence, page.get() + 0x20, 8, TranslationAccessKind::Load),
            &map,
        )
        .unwrap();
    }
    assert_eq!(tlb.entry_count(), 4);

    assert_eq!(tlb.demap_page(asid_one, virtual_base), 1);
    assert!(!tlb.contains_entry(asid_one, virtual_base));
    assert!(tlb.contains_entry(asid_one, next_virtual_base));
    assert!(tlb.contains_entry(asid_two, virtual_base));
    assert!(tlb.contains_entry(asid_two, next_virtual_base));

    assert_eq!(tlb.flush_address_space(asid_two), 2);
    assert!(tlb.contains_entry(asid_one, next_virtual_base));
    assert!(!tlb.contains_entry(asid_two, virtual_base));
    assert!(!tlb.contains_entry(asid_two, next_virtual_base));

    assert_eq!(tlb.demap_page_all_address_spaces(next_virtual_base), 1);
    assert!(tlb.is_empty());

    tlb.translate_in_address_space(
        asid_one,
        &request(
            17,
            virtual_base.get() + 0x20,
            8,
            TranslationAccessKind::Load,
        ),
        &map,
    )
    .unwrap();
    tlb.translate_in_address_space(
        asid_two,
        &request(
            18,
            next_virtual_base.get() + 0x20,
            8,
            TranslationAccessKind::Load,
        ),
        &map,
    )
    .unwrap();
    assert_eq!(tlb.flush_all(), 2);
    assert!(tlb.is_empty());
}

#[test]
fn translation_tlb_asid_flush_preserves_global_entries() {
    let page_size = TranslationPageSize::new(4096).unwrap();
    let asid = TranslationAddressSpaceId::new(9);
    let global_page = Address::new(0xffff_0002_1000_0000);
    let non_global_page = Address::new(0xffff_0002_1000_1000);
    let other_asid_page = Address::new(0xffff_0002_1000_2000);
    let snapshot = TranslationTlbSnapshot::new(
        TranslationTlbConfig::new(8).unwrap(),
        vec![
            TranslationTlbEntrySnapshot::new_in_address_space(
                asid,
                global_page,
                Address::new(0x0000_0002_1000_0000),
                page_size,
                TranslationPagePermissions::read_execute(),
                0,
            )
            .with_scope(TranslationTlbEntryScope::Global),
            TranslationTlbEntrySnapshot::new_in_address_space(
                asid,
                non_global_page,
                Address::new(0x0000_0002_1000_1000),
                page_size,
                TranslationPagePermissions::read_write(),
                1,
            )
            .with_scope(TranslationTlbEntryScope::NonGlobal),
            TranslationTlbEntrySnapshot::new_in_address_space(
                TranslationAddressSpaceId::new(10),
                other_asid_page,
                Address::new(0x0000_0002_1000_2000),
                page_size,
                TranslationPagePermissions::read_write(),
                2,
            )
            .with_scope(TranslationTlbEntryScope::NonGlobal),
        ],
        3,
        TranslationTlbStats::new(0, 0, 0, 3, 0),
    );
    let mut tlb = TranslationTlb::from_snapshot(&snapshot).unwrap();

    assert_eq!(tlb.flush_non_global_address_space(asid), 1);

    assert!(tlb.contains_entry(asid, global_page));
    assert!(!tlb.contains_entry(asid, non_global_page));
    assert!(tlb.contains_entry(TranslationAddressSpaceId::new(10), other_asid_page));
    let remaining = tlb.snapshot();
    assert_eq!(remaining.entries().len(), 2);
    assert!(remaining
        .entries()
        .iter()
        .any(|entry| entry.virtual_page() == global_page
            && entry.scope() == TranslationTlbEntryScope::Global));
    assert!(remaining
        .entries()
        .iter()
        .any(|entry| entry.virtual_page() == other_asid_page
            && entry.scope() == TranslationTlbEntryScope::NonGlobal));

    assert_eq!(tlb.flush_address_space(asid), 1);
    assert!(!tlb.contains_entry(asid, global_page));
    assert!(tlb.contains_entry(TranslationAddressSpaceId::new(10), other_asid_page));
}

#[test]
fn translation_tlb_fills_cross_page_segments_as_scoped_page_entries() {
    let virtual_base = Address::new(0xffff_0001_1000_0000);
    let asid = TranslationAddressSpaceId::new(5);
    let map = two_page_map(
        virtual_base,
        Address::new(0x0000_0001_3000_0000),
        Address::new(0x0000_0001_4000_0000),
    );
    let mut tlb = TranslationTlb::new(TranslationTlbConfig::new(4).unwrap());

    let resolution = tlb
        .fill_segments_from_page_map_in_address_space(
            asid,
            &request(
                19,
                virtual_base.get() + 0xff8,
                16,
                TranslationAccessKind::Load,
            ),
            &map,
        )
        .unwrap();
    let TranslationSegmentedResolution::Mapped(segments) = resolution else {
        panic!("cross-page request should map into segments");
    };
    assert_eq!(segments.len(), 2);
    assert!(tlb.contains_entry(asid, virtual_base));
    assert!(tlb.contains_entry(asid, Address::new(virtual_base.get() + 4096)));
    assert_eq!(tlb.stats(), TranslationTlbStats::new(0, 0, 0, 2, 0));

    let first_hit = tlb
        .lookup_cached_in_address_space(
            asid,
            &request(
                20,
                virtual_base.get() + 0x40,
                8,
                TranslationAccessKind::Load,
            ),
        )
        .unwrap()
        .expect("first segment page should be cached");
    assert_eq!(
        first_hit.physical_address(),
        Some(Address::new(0x0000_0001_3000_0040))
    );

    let second_hit = tlb
        .lookup_cached_in_address_space(
            asid,
            &request(
                21,
                virtual_base.get() + 0x1080,
                8,
                TranslationAccessKind::Load,
            ),
        )
        .unwrap()
        .expect("second segment page should be cached");
    assert_eq!(
        second_hit.physical_address(),
        Some(Address::new(0x0000_0001_4000_0080))
    );
    assert_eq!(tlb.stats(), TranslationTlbStats::new(2, 0, 0, 2, 0));
}

use rem6_cpu::{
    CpuSegmentedTranslationOutcome, CpuTranslatedMemoryOperation, CpuTranslationFaultRecord,
    CpuTranslationFrontend, CpuTranslationFrontendError, CpuTranslationOutcome,
    CpuTranslationRequest,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryOperation, MemoryRequestId,
    TranslationAddressSpaceId, TranslationError, TranslationFault, TranslationFaultKind,
    TranslationPageMap, TranslationPagePermissions, TranslationPageSize, TranslationQueueConfig,
    TranslationRequestId, TranslationResolution, TranslationTlbConfig, TranslationTlbStats,
};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

fn route() -> MemoryRouteId {
    MemoryRouteId::new(9)
}

fn endpoint() -> TransportEndpointId {
    TransportEndpointId::new("cpu0.dmem").unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn translation_id(sequence: u64) -> TranslationRequestId {
    TranslationRequestId::new(AgentId::new(4), sequence)
}

fn memory_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn single_page_map(virtual_base: Address, physical_base: Address) -> TranslationPageMap {
    let mut map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    map.map(
        virtual_base,
        physical_base,
        1,
        TranslationPagePermissions::read_write_execute(),
    )
    .unwrap();
    map
}

fn two_page_map(
    virtual_base: Address,
    first_physical_base: Address,
    second_physical_base: Address,
    second_permissions: TranslationPagePermissions,
) -> TranslationPageMap {
    let mut map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    map.map(
        virtual_base,
        first_physical_base,
        1,
        TranslationPagePermissions::read_write_execute(),
    )
    .unwrap();
    map.map(
        Address::new(virtual_base.get() + 4096),
        second_physical_base,
        1,
        second_permissions,
    )
    .unwrap();
    map
}

#[test]
fn cpu_translation_frontend_maps_ready_translations_to_memory_requests() {
    let mut frontend = CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 2).unwrap());
    let fetch = CpuTranslationRequest::fetch(
        translation_id(1),
        memory_id(10),
        route(),
        endpoint(),
        Address::new(0xffff_0000_8000_0004),
        AccessSize::new(4).unwrap(),
    )
    .unwrap();
    let store = CpuTranslationRequest::store(
        translation_id(2),
        memory_id(11),
        route(),
        endpoint(),
        Address::new(0xffff_0000_9000_0008),
        AccessSize::new(4).unwrap(),
        vec![0xaa, 0xbb, 0xcc, 0xdd],
        ByteMask::full(AccessSize::new(4).unwrap()).unwrap(),
    )
    .unwrap();

    frontend.enqueue(5, fetch).unwrap();
    frontend.enqueue(4, store).unwrap();

    assert_eq!(frontend.pending_count(), 2);
    assert_eq!(frontend.ready_request_ids(5), Vec::new());
    assert_eq!(frontend.ready_request_ids(6), vec![translation_id(2)]);
    assert_eq!(
        frontend.ready_request_ids(7),
        vec![translation_id(2), translation_id(1)]
    );

    let outcomes = frontend.complete_ready(7, |request| {
        TranslationResolution::mapped(Address::new(request.virtual_address().get() & 0x0000_ffff))
    });

    assert_eq!(outcomes.len(), 2);
    let CpuTranslationOutcome::Mapped(store) = &outcomes[0] else {
        panic!("store translation should map");
    };
    assert_eq!(store.translation_id(), translation_id(2));
    assert_eq!(store.memory_request_id(), memory_id(11));
    assert_eq!(store.virtual_address(), Address::new(0xffff_0000_9000_0008));
    assert_eq!(store.physical_address(), Address::new(0x0008));
    assert_eq!(store.route(), route());
    assert_eq!(store.endpoint(), &endpoint());
    assert_eq!(store.operation(), &CpuTranslatedMemoryOperation::Write);
    let store_request = store.memory_request(layout()).unwrap();
    assert_eq!(store_request.id(), memory_id(11));
    assert_eq!(store_request.operation(), MemoryOperation::Write);
    assert_eq!(store_request.range().start(), Address::new(0x0008));
    assert_eq!(store_request.data(), Some(&[0xaa, 0xbb, 0xcc, 0xdd][..]));

    let CpuTranslationOutcome::Mapped(fetch) = &outcomes[1] else {
        panic!("fetch translation should map");
    };
    let fetch_request = fetch.memory_request(layout()).unwrap();
    assert_eq!(fetch_request.id(), memory_id(10));
    assert_eq!(fetch_request.operation(), MemoryOperation::InstructionFetch);
    assert_eq!(fetch_request.range().start(), Address::new(0x0004));
    assert!(frontend.is_empty());
}

#[test]
fn cpu_translation_frontend_restores_snapshot_and_records_faults() {
    let mut frontend = CpuTranslationFrontend::new(TranslationQueueConfig::new(2, 3).unwrap());
    let load = CpuTranslationRequest::load(
        translation_id(3),
        memory_id(12),
        route(),
        endpoint(),
        Address::new(0xffff_0000_a000_0040),
        AccessSize::new(8).unwrap(),
    )
    .unwrap();

    frontend.enqueue(8, load.clone()).unwrap();
    assert_eq!(
        frontend.enqueue(9, load).unwrap_err(),
        CpuTranslationFrontendError::Translation(TranslationError::DuplicateRequest {
            request: translation_id(3),
        })
    );

    let snapshot = frontend.snapshot();
    let mut restored = CpuTranslationFrontend::new(TranslationQueueConfig::new(1, 0).unwrap());
    restored.restore(&snapshot).unwrap();

    assert_eq!(restored.pending_request_ids(), vec![translation_id(3)]);
    let outcomes = restored.complete_ready(11, |request| {
        TranslationResolution::fault(TranslationFault::new(
            request.virtual_address(),
            TranslationFaultKind::PageFault,
        ))
    });

    assert_eq!(
        outcomes,
        vec![CpuTranslationOutcome::Fault(
            CpuTranslationFaultRecord::new(
                translation_id(3),
                memory_id(12),
                route(),
                endpoint(),
                Address::new(0xffff_0000_a000_0040),
                AccessSize::new(8).unwrap(),
                CpuTranslatedMemoryOperation::Read,
                TranslationFault::new(
                    Address::new(0xffff_0000_a000_0040),
                    TranslationFaultKind::PageFault,
                ),
            )
        )]
    );
    assert!(restored.is_empty());
}

#[test]
fn cpu_translation_frontend_uses_tlb_hits_without_queueing_and_fills_on_miss_completion() {
    let virtual_base = Address::new(0xffff_0000_b000_0000);
    let map = single_page_map(virtual_base, Address::new(0x0000_0000_b000_0000));
    let mut frontend = CpuTranslationFrontend::with_tlb(
        TranslationQueueConfig::new(4, 3).unwrap(),
        TranslationTlbConfig::new(4).unwrap(),
    );

    let first = CpuTranslationRequest::load(
        translation_id(4),
        memory_id(13),
        route(),
        endpoint(),
        Address::new(virtual_base.get() + 0x20),
        AccessSize::new(8).unwrap(),
    )
    .unwrap();
    assert_eq!(
        frontend.enqueue_or_translate_cached(10, first).unwrap(),
        None
    );
    assert_eq!(frontend.pending_count(), 1);
    assert_eq!(
        frontend.tlb().unwrap().stats(),
        TranslationTlbStats::new(0, 1, 0, 0, 0)
    );
    assert!(frontend
        .complete_ready_with_tlb_page_map(12, &map)
        .unwrap()
        .is_empty());

    let completed = frontend.complete_ready_with_tlb_page_map(13, &map).unwrap();
    assert_eq!(completed.len(), 1);
    let CpuTranslationOutcome::Mapped(first_mapped) = &completed[0] else {
        panic!("first load should map after page walk");
    };
    assert_eq!(
        first_mapped.physical_address(),
        Address::new(0x0000_0000_b000_0020)
    );
    assert_eq!(frontend.pending_count(), 0);
    assert_eq!(
        frontend.tlb().unwrap().stats(),
        TranslationTlbStats::new(0, 1, 0, 1, 0)
    );

    let second = CpuTranslationRequest::load(
        translation_id(5),
        memory_id(14),
        route(),
        endpoint(),
        Address::new(virtual_base.get() + 0x80),
        AccessSize::new(8).unwrap(),
    )
    .unwrap();
    let immediate = frontend
        .enqueue_or_translate_cached(14, second)
        .unwrap()
        .expect("second same-page load should hit the TLB");
    let CpuTranslationOutcome::Mapped(second_mapped) = immediate else {
        panic!("TLB hit should map");
    };
    assert_eq!(
        second_mapped.physical_address(),
        Address::new(0x0000_0000_b000_0080)
    );
    assert!(frontend.pending_request_ids().is_empty());
    assert_eq!(
        frontend.tlb().unwrap().stats(),
        TranslationTlbStats::new(1, 1, 0, 1, 0)
    );
}

#[test]
fn cpu_translation_frontend_keeps_tlb_address_spaces_separate_and_snapshots() {
    let virtual_base = Address::new(0xffff_0000_c000_0000);
    let asid_one = TranslationAddressSpaceId::new(11);
    let asid_two = TranslationAddressSpaceId::new(12);
    let map_one = single_page_map(virtual_base, Address::new(0x0000_0000_c000_0000));
    let map_two = single_page_map(virtual_base, Address::new(0x0000_0000_d000_0000));
    let mut frontend = CpuTranslationFrontend::with_tlb(
        TranslationQueueConfig::new(4, 1).unwrap(),
        TranslationTlbConfig::new(4).unwrap(),
    );

    let request_one = CpuTranslationRequest::load(
        translation_id(6),
        memory_id(15),
        route(),
        endpoint(),
        Address::new(virtual_base.get() + 0x10),
        AccessSize::new(8).unwrap(),
    )
    .unwrap()
    .in_address_space(asid_one);
    assert_eq!(
        frontend
            .enqueue_or_translate_cached(20, request_one)
            .unwrap(),
        None
    );
    assert_eq!(
        frontend
            .complete_ready_with_tlb_page_map(21, &map_one)
            .unwrap()
            .len(),
        1
    );

    let request_two = CpuTranslationRequest::load(
        translation_id(7),
        memory_id(16),
        route(),
        endpoint(),
        Address::new(virtual_base.get() + 0x10),
        AccessSize::new(8).unwrap(),
    )
    .unwrap()
    .in_address_space(asid_two);
    assert_eq!(
        frontend
            .enqueue_or_translate_cached(22, request_two)
            .unwrap(),
        None
    );
    assert_eq!(
        frontend
            .complete_ready_with_tlb_page_map(23, &map_two)
            .unwrap()
            .len(),
        1
    );

    let snapshot = frontend.snapshot();
    let mut restored = CpuTranslationFrontend::new(TranslationQueueConfig::new(1, 0).unwrap());
    restored.restore(&snapshot).unwrap();
    assert!(restored
        .tlb()
        .unwrap()
        .contains_entry(asid_one, virtual_base));
    assert!(restored
        .tlb()
        .unwrap()
        .contains_entry(asid_two, virtual_base));

    let hit_one = CpuTranslationRequest::load(
        translation_id(8),
        memory_id(17),
        route(),
        endpoint(),
        Address::new(virtual_base.get() + 0x90),
        AccessSize::new(8).unwrap(),
    )
    .unwrap()
    .in_address_space(asid_one);
    let CpuTranslationOutcome::Mapped(mapped_one) = restored
        .enqueue_or_translate_cached(24, hit_one)
        .unwrap()
        .expect("restored ASID one entry should hit")
    else {
        panic!("ASID one hit should map");
    };
    assert_eq!(
        mapped_one.physical_address(),
        Address::new(0x0000_0000_c000_0090)
    );

    restored.tlb_mut().unwrap().flush_address_space(asid_one);
    assert!(!restored
        .tlb()
        .unwrap()
        .contains_entry(asid_one, virtual_base));
    assert!(restored
        .tlb()
        .unwrap()
        .contains_entry(asid_two, virtual_base));

    let hit_two = CpuTranslationRequest::load(
        translation_id(9),
        memory_id(18),
        route(),
        endpoint(),
        Address::new(virtual_base.get() + 0xa0),
        AccessSize::new(8).unwrap(),
    )
    .unwrap()
    .in_address_space(asid_two);
    let CpuTranslationOutcome::Mapped(mapped_two) = restored
        .enqueue_or_translate_cached(25, hit_two)
        .unwrap()
        .expect("ASID two entry should survive ASID one flush")
    else {
        panic!("ASID two hit should map");
    };
    assert_eq!(
        mapped_two.physical_address(),
        Address::new(0x0000_0000_d000_00a0)
    );
}

#[test]
fn cpu_translation_frontend_maps_cross_page_translation_into_segments() {
    let virtual_base = Address::new(0xffff_0000_e000_0000);
    let map = two_page_map(
        virtual_base,
        Address::new(0x0000_0000_1000_0000),
        Address::new(0x0000_0000_2000_0000),
        TranslationPagePermissions::read_write(),
    );
    let mut frontend = CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 2).unwrap());
    let store = CpuTranslationRequest::store(
        translation_id(10),
        memory_id(19),
        route(),
        endpoint(),
        Address::new(virtual_base.get() + 0xff8),
        AccessSize::new(16).unwrap(),
        (0u8..16).collect(),
        ByteMask::from_bits(vec![
            true, false, true, false, true, false, true, false, false, true, false, true, false,
            true, false, true,
        ])
        .unwrap(),
    )
    .unwrap();
    frontend.enqueue(30, store).unwrap();

    assert!(frontend
        .complete_ready_segmented_with_page_map(31, &map)
        .unwrap()
        .is_empty());
    let outcomes = frontend
        .complete_ready_segmented_with_page_map(32, &map)
        .unwrap();
    assert_eq!(outcomes.len(), 1);
    let CpuSegmentedTranslationOutcome::Mapped(segments) = &outcomes[0] else {
        panic!("cross-page store should map into segments");
    };
    assert_eq!(segments.len(), 2);

    assert_eq!(segments[0].translation_id(), translation_id(10));
    assert_eq!(segments[0].memory_request_id(), memory_id(19));
    assert_eq!(
        segments[0].virtual_address(),
        Address::new(virtual_base.get() + 0xff8)
    );
    assert_eq!(
        segments[0].physical_address(),
        Address::new(0x0000_0000_1000_0ff8)
    );
    assert_eq!(segments[0].size(), AccessSize::new(8).unwrap());
    assert_eq!(
        segments[0].write_data(),
        Some(&[0, 1, 2, 3, 4, 5, 6, 7][..])
    );
    assert_eq!(
        segments[0].byte_mask().unwrap().bits(),
        &[true, false, true, false, true, false, true, false]
    );

    assert_eq!(
        segments[1].virtual_address(),
        Address::new(virtual_base.get() + 0x1000)
    );
    assert_eq!(
        segments[1].physical_address(),
        Address::new(0x0000_0000_2000_0000)
    );
    assert_eq!(segments[1].size(), AccessSize::new(8).unwrap());
    assert_eq!(
        segments[1].write_data(),
        Some(&[8, 9, 10, 11, 12, 13, 14, 15][..])
    );
    assert_eq!(
        segments[1].byte_mask().unwrap().bits(),
        &[false, true, false, true, false, true, false, true]
    );

    let first_request = segments[0]
        .memory_request_with_id(memory_id(101), layout())
        .unwrap();
    assert_eq!(first_request.id(), memory_id(101));
    assert_eq!(first_request.operation(), MemoryOperation::Write);
    assert_eq!(
        first_request.range().start(),
        Address::new(0x0000_0000_1000_0ff8)
    );
    assert_eq!(first_request.size(), AccessSize::new(8).unwrap());
    assert_eq!(first_request.data(), Some(&[0, 1, 2, 3, 4, 5, 6, 7][..]));
    assert!(frontend.is_empty());
}

#[test]
fn cpu_translation_frontend_faults_first_failed_cross_page_segment() {
    let virtual_base = Address::new(0xffff_0000_f000_0000);
    let map = two_page_map(
        virtual_base,
        Address::new(0x0000_0000_3000_0000),
        Address::new(0x0000_0000_4000_0000),
        TranslationPagePermissions::read_only(),
    );
    let mut frontend = CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 1).unwrap());
    let store = CpuTranslationRequest::store(
        translation_id(11),
        memory_id(20),
        route(),
        endpoint(),
        Address::new(virtual_base.get() + 0xff8),
        AccessSize::new(16).unwrap(),
        vec![0xaa; 16],
        ByteMask::full(AccessSize::new(16).unwrap()).unwrap(),
    )
    .unwrap();
    frontend.enqueue(40, store).unwrap();

    let outcomes = frontend
        .complete_ready_segmented_with_page_map(41, &map)
        .unwrap();
    assert_eq!(
        outcomes,
        vec![CpuSegmentedTranslationOutcome::Fault(
            CpuTranslationFaultRecord::new(
                translation_id(11),
                memory_id(20),
                route(),
                endpoint(),
                Address::new(virtual_base.get() + 0xff8),
                AccessSize::new(16).unwrap(),
                CpuTranslatedMemoryOperation::Write,
                TranslationFault::new(
                    Address::new(virtual_base.get() + 0x1000),
                    TranslationFaultKind::PermissionFault,
                ),
            )
        )]
    );
    assert!(frontend.is_empty());
}

#[test]
fn cpu_translation_frontend_segmented_completion_fills_tlb_entries() {
    let virtual_base = Address::new(0xffff_0001_0000_0000);
    let asid = TranslationAddressSpaceId::new(21);
    let map = two_page_map(
        virtual_base,
        Address::new(0x0000_0001_1000_0000),
        Address::new(0x0000_0001_2000_0000),
        TranslationPagePermissions::read_write(),
    );
    let mut frontend = CpuTranslationFrontend::with_tlb(
        TranslationQueueConfig::new(4, 2).unwrap(),
        TranslationTlbConfig::new(8).unwrap(),
    );

    let cross_page = CpuTranslationRequest::load(
        translation_id(12),
        memory_id(21),
        route(),
        endpoint(),
        Address::new(virtual_base.get() + 0xff8),
        AccessSize::new(16).unwrap(),
    )
    .unwrap()
    .in_address_space(asid);
    assert_eq!(
        frontend
            .enqueue_or_translate_cached(50, cross_page)
            .unwrap(),
        None
    );
    assert_eq!(
        frontend.tlb().unwrap().stats(),
        TranslationTlbStats::new(0, 1, 0, 0, 0)
    );

    assert!(frontend
        .complete_ready_segmented_with_tlb_page_map(51, &map)
        .unwrap()
        .is_empty());
    let outcomes = frontend
        .complete_ready_segmented_with_tlb_page_map(52, &map)
        .unwrap();
    let CpuSegmentedTranslationOutcome::Mapped(segments) = &outcomes[0] else {
        panic!("cross-page load should map into TLB-backed segments");
    };
    assert_eq!(segments.len(), 2);
    assert!(frontend.tlb().unwrap().contains_entry(asid, virtual_base));
    assert!(frontend
        .tlb()
        .unwrap()
        .contains_entry(asid, Address::new(virtual_base.get() + 4096)));
    assert_eq!(
        frontend.tlb().unwrap().stats(),
        TranslationTlbStats::new(0, 1, 0, 2, 0)
    );

    let first_page = CpuTranslationRequest::load(
        translation_id(13),
        memory_id(22),
        route(),
        endpoint(),
        Address::new(virtual_base.get() + 0x20),
        AccessSize::new(8).unwrap(),
    )
    .unwrap()
    .in_address_space(asid);
    let CpuTranslationOutcome::Mapped(first_hit) = frontend
        .enqueue_or_translate_cached(53, first_page)
        .unwrap()
        .expect("first page should hit the segmented fill")
    else {
        panic!("first page TLB hit should map");
    };
    assert_eq!(
        first_hit.physical_address(),
        Address::new(0x0000_0001_1000_0020)
    );

    let second_page = CpuTranslationRequest::load(
        translation_id(14),
        memory_id(23),
        route(),
        endpoint(),
        Address::new(virtual_base.get() + 0x1080),
        AccessSize::new(8).unwrap(),
    )
    .unwrap()
    .in_address_space(asid);
    let CpuTranslationOutcome::Mapped(second_hit) = frontend
        .enqueue_or_translate_cached(54, second_page)
        .unwrap()
        .expect("second page should hit the segmented fill")
    else {
        panic!("second page TLB hit should map");
    };
    assert_eq!(
        second_hit.physical_address(),
        Address::new(0x0000_0001_2000_0080)
    );
    assert!(frontend.is_empty());
    assert_eq!(
        frontend.tlb().unwrap().stats(),
        TranslationTlbStats::new(2, 1, 0, 2, 0)
    );
}

use rem6_cpu::{
    CpuTranslatedMemoryOperation, CpuTranslationFaultRecord, CpuTranslationFrontend,
    CpuTranslationFrontendError, CpuTranslationOutcome, CpuTranslationRequest,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryOperation, MemoryRequestId,
    TranslationError, TranslationFault, TranslationFaultKind, TranslationQueueConfig,
    TranslationRequestId, TranslationResolution,
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

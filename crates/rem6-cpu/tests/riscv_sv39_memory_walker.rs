use std::sync::{Arc, Mutex};

use rem6_cpu::{
    CpuTranslationFrontend, CpuTranslationOutcome, CpuTranslationRequest, RiscvSv39MemoryWalk,
    RiscvSv39MemoryWalkAdvance, RiscvSv39MemoryWalker, RiscvSv39MemoryWalkerAdvance,
    RiscvSv39MemoryWalkerError,
};
use rem6_isa_riscv::{RiscvSv39PageTableLevel, RiscvSv39Pte};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequest, MemoryRequestId,
    MemoryResponse, TranslationFaultKind, TranslationQueueConfig, TranslationRequestId,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn translation_id(sequence: u64) -> TranslationRequestId {
    TranslationRequestId::new(AgentId::new(17), sequence)
}

fn memory_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(19), sequence)
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

const SV39_PTE_V: u64 = 1 << 0;
const SV39_PTE_R: u64 = 1 << 1;
const SV39_PTE_W: u64 = 1 << 2;
const SV39_PTE_A: u64 = 1 << 6;
const SV39_PTE_D: u64 = 1 << 7;

fn pte_response(request: &MemoryRequest, pte: RiscvSv39Pte) -> MemoryResponse {
    MemoryResponse::completed(request, Some(pte.raw().to_le_bytes().to_vec())).unwrap()
}

fn assert_sv39_pte_request(
    request: &MemoryRequest,
    expected_id: MemoryRequestId,
    expected_address: Address,
    line_layout: CacheLineLayout,
) {
    assert_eq!(request.id(), expected_id);
    assert_eq!(request.operation(), MemoryOperation::ReadShared);
    assert_eq!(request.range().start(), expected_address);
    assert_eq!(request.size(), AccessSize::new(8).unwrap());
    assert_eq!(request.line_layout(), line_layout);
    assert!(request.data().is_none());
}

#[test]
fn riscv_sv39_memory_walk_issues_pte_requests_and_completes_translation() {
    let virtual_address = (0x015_u64 << 30) | (0x037_u64 << 21) | (0x059_u64 << 12) | 0xabc;
    let root_ppn = 0x130;
    let level1_ppn = 0x230;
    let level0_ppn = 0x330;
    let leaf_ppn = 0x75678;
    let level2_pte_address = Address::new((root_ppn << 12) + (0x015 * 8));
    let level1_pte_address = Address::new((level1_ppn << 12) + (0x037 * 8));
    let level0_pte_address = Address::new((level0_ppn << 12) + (0x059 * 8));
    let request = CpuTranslationRequest::load(
        translation_id(85),
        memory_id(98),
        MemoryRouteId::new(6),
        endpoint("cpu0.dmem"),
        Address::new(virtual_address),
        AccessSize::new(8).unwrap(),
    )
    .unwrap();
    let first_pte_request = MemoryRequestId::new(AgentId::new(31), 900);
    let line_layout = layout();

    let RiscvSv39MemoryWalkAdvance::ReadPte(walk) =
        RiscvSv39MemoryWalk::start(request, root_ppn, first_pte_request, line_layout).unwrap()
    else {
        panic!("canonical address should start with a PTE read");
    };
    assert_sv39_pte_request(
        walk.pte_request(),
        first_pte_request,
        level2_pte_address,
        line_layout,
    );

    let response = pte_response(
        walk.pte_request(),
        RiscvSv39Pte::new((level1_ppn << 10) | SV39_PTE_V),
    );
    let RiscvSv39MemoryWalkAdvance::ReadPte(walk) = walk.advance(&response).unwrap() else {
        panic!("level 2 pointer should request another PTE");
    };
    assert_sv39_pte_request(
        walk.pte_request(),
        MemoryRequestId::new(first_pte_request.agent(), 901),
        level1_pte_address,
        line_layout,
    );

    let response = pte_response(
        walk.pte_request(),
        RiscvSv39Pte::new((level0_ppn << 10) | SV39_PTE_V),
    );
    let RiscvSv39MemoryWalkAdvance::ReadPte(walk) = walk.advance(&response).unwrap() else {
        panic!("level 1 pointer should request another PTE");
    };
    assert_sv39_pte_request(
        walk.pte_request(),
        MemoryRequestId::new(first_pte_request.agent(), 902),
        level0_pte_address,
        line_layout,
    );

    let response = pte_response(
        walk.pte_request(),
        RiscvSv39Pte::new(
            (leaf_ppn << 10) | SV39_PTE_V | SV39_PTE_R | SV39_PTE_W | SV39_PTE_A | SV39_PTE_D,
        ),
    );
    let RiscvSv39MemoryWalkAdvance::Complete(resolved) = walk.advance(&response).unwrap() else {
        panic!("level 0 leaf should complete the translation");
    };
    let CpuTranslationOutcome::Mapped(mapped) = resolved.outcome() else {
        panic!("Sv39 memory walk should map");
    };
    assert_eq!(
        mapped.physical_address(),
        Address::new((leaf_ppn << 12) | 0xabc)
    );
    assert_eq!(mapped.virtual_address(), Address::new(virtual_address));
    assert_eq!(mapped.size(), AccessSize::new(8).unwrap());
    assert_eq!(
        resolved.pte_addresses(),
        &[level2_pte_address, level1_pte_address, level0_pte_address]
    );
    assert_eq!(resolved.leaf_level(), Some(RiscvSv39PageTableLevel::Level0));
    assert_eq!(resolved.page_fault(), None);
}

#[test]
fn riscv_sv39_memory_walker_drives_ready_frontend_request_from_pte_responses() {
    let virtual_address = (0x016_u64 << 30) | (0x038_u64 << 21) | (0x05a_u64 << 12) | 0xdef;
    let root_ppn = 0x140;
    let level1_ppn = 0x240;
    let level0_ppn = 0x340;
    let leaf_ppn = 0x76543;
    let level2_pte_address = Address::new((root_ppn << 12) + (0x016 * 8));
    let level1_pte_address = Address::new((level1_ppn << 12) + (0x038 * 8));
    let level0_pte_address = Address::new((level0_ppn << 12) + (0x05a * 8));
    let request = CpuTranslationRequest::load(
        translation_id(86),
        memory_id(99),
        MemoryRouteId::new(6),
        endpoint("cpu0.dmem"),
        Address::new(virtual_address),
        AccessSize::new(8).unwrap(),
    )
    .unwrap();
    let first_pte_request = MemoryRequestId::new(AgentId::new(31), 1000);
    let line_layout = layout();
    let mut frontend = CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 1).unwrap());
    frontend.enqueue(10, request).unwrap();
    let mut walker = RiscvSv39MemoryWalker::new(root_ppn, first_pte_request, line_layout);

    assert_eq!(walker.start_ready(&mut frontend, 10).unwrap(), Vec::new());
    assert_eq!(walker.active_count(), 0);
    assert_eq!(frontend.pending_count(), 1);

    let advances = walker.start_ready(&mut frontend, 11).unwrap();
    assert_eq!(advances.len(), 1);
    let RiscvSv39MemoryWalkerAdvance::ReadPte(level2_request) = &advances[0] else {
        panic!("ready Sv39 translation should issue the first PTE read");
    };
    assert_sv39_pte_request(
        level2_request,
        first_pte_request,
        level2_pte_address,
        line_layout,
    );
    assert_eq!(walker.active_count(), 1);
    assert_eq!(frontend.pending_count(), 1);

    let response = pte_response(
        level2_request,
        RiscvSv39Pte::new((level1_ppn << 10) | SV39_PTE_V),
    );
    let RiscvSv39MemoryWalkerAdvance::ReadPte(level1_request) =
        walker.advance(&mut frontend, &response).unwrap()
    else {
        panic!("level 2 pointer should issue another PTE read");
    };
    assert_sv39_pte_request(
        &level1_request,
        MemoryRequestId::new(first_pte_request.agent(), 1001),
        level1_pte_address,
        line_layout,
    );
    assert_eq!(walker.active_count(), 1);
    assert_eq!(frontend.pending_count(), 1);

    let response = pte_response(
        &level1_request,
        RiscvSv39Pte::new((level0_ppn << 10) | SV39_PTE_V),
    );
    let RiscvSv39MemoryWalkerAdvance::ReadPte(level0_request) =
        walker.advance(&mut frontend, &response).unwrap()
    else {
        panic!("level 1 pointer should issue another PTE read");
    };
    assert_sv39_pte_request(
        &level0_request,
        MemoryRequestId::new(first_pte_request.agent(), 1002),
        level0_pte_address,
        line_layout,
    );
    assert_eq!(walker.active_count(), 1);
    assert_eq!(frontend.pending_count(), 1);

    let response = pte_response(
        &level0_request,
        RiscvSv39Pte::new(
            (leaf_ppn << 10) | SV39_PTE_V | SV39_PTE_R | SV39_PTE_W | SV39_PTE_A | SV39_PTE_D,
        ),
    );
    let RiscvSv39MemoryWalkerAdvance::Complete(outcome) =
        walker.advance(&mut frontend, &response).unwrap()
    else {
        panic!("level 0 leaf should complete the frontend request");
    };
    let CpuTranslationOutcome::Mapped(mapped) = outcome else {
        panic!("Sv39 memory walker should map");
    };
    assert_eq!(mapped.translation_id(), translation_id(86));
    assert_eq!(mapped.memory_request_id(), memory_id(99));
    assert_eq!(
        mapped.physical_address(),
        Address::new((leaf_ppn << 12) | 0xdef)
    );
    assert_eq!(walker.active_count(), 0);
    assert!(frontend.is_empty());
}

#[test]
fn riscv_sv39_memory_walker_start_ready_rejects_batch_without_partial_active_walks() {
    let mut frontend = CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 0).unwrap());
    for (sequence, vpn2) in [(87, 0x017), (88, 0x018)] {
        let virtual_address = (vpn2 << 30) | (0x039_u64 << 21) | (0x05b_u64 << 12) | 0x123;
        frontend
            .enqueue(
                10,
                CpuTranslationRequest::load(
                    translation_id(sequence),
                    memory_id(sequence + 20),
                    MemoryRouteId::new(6),
                    endpoint("cpu0.dmem"),
                    Address::new(virtual_address),
                    AccessSize::new(8).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
    }
    let first_pte_request = MemoryRequestId::new(AgentId::new(31), u64::MAX - 3);
    let mut walker = RiscvSv39MemoryWalker::new(0x150, first_pte_request, layout());

    assert_eq!(
        walker.start_ready(&mut frontend, 10).unwrap_err(),
        RiscvSv39MemoryWalkerError::PteRequestSequenceOverflow {
            first: MemoryRequestId::new(first_pte_request.agent(), u64::MAX),
        }
    );
    assert_eq!(walker.active_count(), 0);
    assert_eq!(walker.next_pte_request(), first_pte_request);
    assert_eq!(frontend.pending_count(), 2);
}

#[test]
fn riscv_sv39_memory_walker_keeps_active_walk_when_frontend_completion_fails() {
    let virtual_address = (0x019_u64 << 30) | (0x001_u64 << 21) | (0x002_u64 << 12) | 0x345;
    let root_ppn = 0x160;
    let leaf_ppn = 0x40000;
    let level2_pte_address = Address::new((root_ppn << 12) + (0x019 * 8));
    let request = CpuTranslationRequest::load(
        translation_id(89),
        memory_id(109),
        MemoryRouteId::new(6),
        endpoint("cpu0.dmem"),
        Address::new(virtual_address),
        AccessSize::new(8).unwrap(),
    )
    .unwrap();
    let first_pte_request = MemoryRequestId::new(AgentId::new(31), 1100);
    let line_layout = layout();
    let mut frontend = CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 0).unwrap());
    frontend.enqueue(12, request).unwrap();
    let mut walker = RiscvSv39MemoryWalker::new(root_ppn, first_pte_request, line_layout);
    let advances = walker.start_ready(&mut frontend, 12).unwrap();
    let RiscvSv39MemoryWalkerAdvance::ReadPte(level2_request) = &advances[0] else {
        panic!("ready Sv39 translation should issue the first PTE read");
    };
    assert_sv39_pte_request(
        level2_request,
        first_pte_request,
        level2_pte_address,
        line_layout,
    );
    let response = pte_response(
        level2_request,
        RiscvSv39Pte::new(
            (leaf_ppn << 10) | SV39_PTE_V | SV39_PTE_R | SV39_PTE_W | SV39_PTE_A | SV39_PTE_D,
        ),
    );
    let mut mismatched_frontend =
        CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 0).unwrap());

    assert!(walker.advance(&mut mismatched_frontend, &response).is_err());
    assert_eq!(walker.active_count(), 1);

    let RiscvSv39MemoryWalkerAdvance::Complete(outcome) =
        walker.advance(&mut frontend, &response).unwrap()
    else {
        panic!("preserved active walk should complete after frontend retry");
    };
    let CpuTranslationOutcome::Mapped(mapped) = outcome else {
        panic!("Sv39 memory walker should map after frontend retry");
    };
    assert_eq!(mapped.translation_id(), translation_id(89));
    assert_eq!(mapped.memory_request_id(), memory_id(109));
    assert_eq!(walker.active_count(), 0);
    assert!(frontend.is_empty());
}

#[test]
fn riscv_sv39_memory_walker_starts_multiple_ready_frontend_requests() {
    let mut frontend = CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 0).unwrap());
    for (sequence, vpn2) in [(90, 0x01a), (91, 0x01b)] {
        let virtual_address = (vpn2 << 30) | (0x003_u64 << 21) | (0x004_u64 << 12) | 0x567;
        frontend
            .enqueue(
                14,
                CpuTranslationRequest::load(
                    translation_id(sequence),
                    memory_id(sequence + 30),
                    MemoryRouteId::new(6),
                    endpoint("cpu0.dmem"),
                    Address::new(virtual_address),
                    AccessSize::new(8).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
    }
    let root_ppn = 0x170;
    let first_pte_request = MemoryRequestId::new(AgentId::new(31), 1200);
    let line_layout = layout();
    let mut walker = RiscvSv39MemoryWalker::new(root_ppn, first_pte_request, line_layout);

    let advances = walker.start_ready(&mut frontend, 14).unwrap();

    assert_eq!(advances.len(), 2);
    let RiscvSv39MemoryWalkerAdvance::ReadPte(first_request) = &advances[0] else {
        panic!("first ready translation should issue a PTE read");
    };
    assert_sv39_pte_request(
        first_request,
        first_pte_request,
        Address::new((root_ppn << 12) + (0x01a * 8)),
        line_layout,
    );
    let RiscvSv39MemoryWalkerAdvance::ReadPte(second_request) = &advances[1] else {
        panic!("second ready translation should issue a PTE read");
    };
    assert_sv39_pte_request(
        second_request,
        MemoryRequestId::new(first_pte_request.agent(), 1203),
        Address::new((root_ppn << 12) + (0x01b * 8)),
        line_layout,
    );
    assert_eq!(walker.active_count(), 2);
    assert_eq!(walker.start_ready(&mut frontend, 14).unwrap(), Vec::new());
    assert_eq!(walker.active_count(), 2);
}

#[test]
fn riscv_sv39_memory_walker_keeps_active_walk_after_pte_response_error() {
    let virtual_address = (0x01c_u64 << 30) | (0x005_u64 << 21) | (0x006_u64 << 12) | 0x789;
    let root_ppn = 0x180;
    let leaf_ppn = 0x80000;
    let first_pte_request = MemoryRequestId::new(AgentId::new(31), 1300);
    let line_layout = layout();
    let mut frontend = CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 0).unwrap());
    frontend
        .enqueue(
            16,
            CpuTranslationRequest::load(
                translation_id(92),
                memory_id(122),
                MemoryRouteId::new(6),
                endpoint("cpu0.dmem"),
                Address::new(virtual_address),
                AccessSize::new(8).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    let mut walker = RiscvSv39MemoryWalker::new(root_ppn, first_pte_request, line_layout);
    let advances = walker.start_ready(&mut frontend, 16).unwrap();
    let RiscvSv39MemoryWalkerAdvance::ReadPte(level2_request) = &advances[0] else {
        panic!("ready Sv39 translation should issue the first PTE read");
    };

    let retry = MemoryResponse::retry(level2_request);
    assert!(walker.advance(&mut frontend, &retry).is_err());
    assert_eq!(walker.active_count(), 1);

    let response = pte_response(
        level2_request,
        RiscvSv39Pte::new(
            (leaf_ppn << 10) | SV39_PTE_V | SV39_PTE_R | SV39_PTE_W | SV39_PTE_A | SV39_PTE_D,
        ),
    );
    let RiscvSv39MemoryWalkerAdvance::Complete(outcome) =
        walker.advance(&mut frontend, &response).unwrap()
    else {
        panic!("preserved active walk should complete after valid PTE response");
    };
    let CpuTranslationOutcome::Mapped(mapped) = outcome else {
        panic!("Sv39 memory walker should map after valid PTE response");
    };
    assert_eq!(mapped.translation_id(), translation_id(92));
    assert_eq!(walker.active_count(), 0);
    assert!(frontend.is_empty());
}

#[test]
fn riscv_sv39_memory_walker_completes_frontend_fault() {
    let virtual_address = (0x01d_u64 << 30) | (0x007_u64 << 21) | (0x008_u64 << 12) | 0x9ab;
    let root_ppn = 0x190;
    let first_pte_request = MemoryRequestId::new(AgentId::new(31), 1400);
    let line_layout = layout();
    let mut frontend = CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 0).unwrap());
    frontend
        .enqueue(
            18,
            CpuTranslationRequest::load(
                translation_id(93),
                memory_id(123),
                MemoryRouteId::new(6),
                endpoint("cpu0.dmem"),
                Address::new(virtual_address),
                AccessSize::new(8).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    let mut walker = RiscvSv39MemoryWalker::new(root_ppn, first_pte_request, line_layout);
    let advances = walker.start_ready(&mut frontend, 18).unwrap();
    let RiscvSv39MemoryWalkerAdvance::ReadPte(level2_request) = &advances[0] else {
        panic!("ready Sv39 translation should issue the first PTE read");
    };

    let response = pte_response(level2_request, RiscvSv39Pte::new(0));
    let RiscvSv39MemoryWalkerAdvance::Complete(outcome) =
        walker.advance(&mut frontend, &response).unwrap()
    else {
        panic!("invalid PTE should complete the frontend request with a fault");
    };
    let CpuTranslationOutcome::Fault(fault) = outcome else {
        panic!("invalid PTE should fault");
    };
    assert_eq!(fault.translation_id(), translation_id(93));
    assert_eq!(fault.memory_request_id(), memory_id(123));
    assert_eq!(
        fault.fault().virtual_address(),
        Address::new(virtual_address)
    );
    assert_eq!(fault.fault().kind(), TranslationFaultKind::PageFault);
    assert_eq!(walker.active_count(), 0);
    assert!(frontend.is_empty());
}

#[test]
fn riscv_sv39_memory_walker_records_transport_response_and_advances_frontend() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ptw"),
                PartitionId::new(0),
                endpoint("l1d0"),
                PartitionId::new(0),
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap();
    let virtual_address = (0x01e_u64 << 30) | (0x009_u64 << 21) | (0x00a_u64 << 12) | 0xbcd;
    let root_ppn = 0x1a0;
    let leaf_ppn = 0x80000;
    let first_pte_request = MemoryRequestId::new(AgentId::new(31), 1500);
    let line_layout = layout();
    let mut frontend = CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 0).unwrap());
    frontend
        .enqueue(
            20,
            CpuTranslationRequest::load(
                translation_id(94),
                memory_id(124),
                MemoryRouteId::new(6),
                endpoint("cpu0.dmem"),
                Address::new(virtual_address),
                AccessSize::new(8).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    let walker = Arc::new(Mutex::new(RiscvSv39MemoryWalker::new(
        root_ppn,
        first_pte_request,
        line_layout,
    )));
    let advances = walker
        .lock()
        .unwrap()
        .start_ready(&mut frontend, 20)
        .unwrap();
    let RiscvSv39MemoryWalkerAdvance::ReadPte(level2_request) = &advances[0] else {
        panic!("ready Sv39 translation should issue the first PTE read");
    };

    let response_walker = Arc::clone(&walker);
    transport
        .submit_parallel(
            &mut scheduler,
            route,
            level2_request.clone(),
            MemoryTrace::new(),
            move |delivery, _context| {
                assert_eq!(delivery.route(), route);
                assert_eq!(delivery.endpoint(), &endpoint("l1d0"));
                assert_eq!(delivery.request().id(), first_pte_request);
                TargetOutcome::Respond(pte_response(
                    delivery.request(),
                    RiscvSv39Pte::new(
                        (leaf_ppn << 10)
                            | SV39_PTE_V
                            | SV39_PTE_R
                            | SV39_PTE_W
                            | SV39_PTE_A
                            | SV39_PTE_D,
                    ),
                ))
            },
            move |delivery| {
                response_walker.lock().unwrap().record_response(delivery);
            },
        )
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(walker.lock().unwrap().pending_response_count(), 1);
    let RiscvSv39MemoryWalkerAdvance::Complete(outcome) = walker
        .lock()
        .unwrap()
        .advance_next_response(&mut frontend)
        .unwrap()
        .expect("transport response should be pending")
    else {
        panic!("transport PTE response should complete the frontend request");
    };
    let CpuTranslationOutcome::Mapped(mapped) = outcome else {
        panic!("Sv39 memory walker should map after transport response");
    };
    assert_eq!(mapped.translation_id(), translation_id(94));
    assert_eq!(
        mapped.physical_address(),
        Address::new((leaf_ppn << 12) | (0x009 << 21) | (0x00a << 12) | 0xbcd)
    );
    assert_eq!(walker.lock().unwrap().pending_response_count(), 0);
    assert!(walker.lock().unwrap().is_idle());
    assert!(frontend.is_empty());
}

#[test]
fn riscv_sv39_memory_walker_submits_ready_frontend_request_through_parallel_transport() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ptw"),
                PartitionId::new(0),
                endpoint("l1d0"),
                PartitionId::new(0),
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap();
    let virtual_address = (0x021_u64 << 30) | (0x00f_u64 << 21) | (0x010_u64 << 12) | 0xef0;
    let root_ppn = 0x1d0;
    let leaf_ppn = 0x80000;
    let first_pte_request = MemoryRequestId::new(AgentId::new(31), 1800);
    let mut frontend = CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 0).unwrap());
    frontend
        .enqueue(
            26,
            CpuTranslationRequest::load(
                translation_id(97),
                memory_id(127),
                MemoryRouteId::new(6),
                endpoint("cpu0.dmem"),
                Address::new(virtual_address),
                AccessSize::new(8).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    let walker = Arc::new(Mutex::new(RiscvSv39MemoryWalker::new(
        root_ppn,
        first_pte_request,
        layout(),
    )));

    let submission = RiscvSv39MemoryWalker::submit_ready_parallel(
        Arc::clone(&walker),
        &mut frontend,
        26,
        &mut scheduler,
        &transport,
        route,
        MemoryTrace::new(),
        move |delivery, _context| {
            assert_eq!(delivery.route(), route);
            assert_eq!(delivery.endpoint(), &endpoint("l1d0"));
            assert_eq!(delivery.request().id(), first_pte_request);
            TargetOutcome::Respond(pte_response(
                delivery.request(),
                RiscvSv39Pte::new(
                    (leaf_ppn << 10)
                        | SV39_PTE_V
                        | SV39_PTE_R
                        | SV39_PTE_W
                        | SV39_PTE_A
                        | SV39_PTE_D,
                ),
            ))
        },
    )
    .unwrap();

    assert_eq!(submission.events().len(), 1);
    assert!(submission.completions().is_empty());
    assert_eq!(walker.lock().unwrap().active_count(), 1);
    scheduler.run_until_idle_parallel().unwrap();
    assert_eq!(walker.lock().unwrap().pending_response_count(), 1);

    let RiscvSv39MemoryWalkerAdvance::Complete(outcome) = walker
        .lock()
        .unwrap()
        .advance_next_response(&mut frontend)
        .unwrap()
        .expect("transport response should be pending")
    else {
        panic!("submitted PTE response should complete the frontend request");
    };
    let CpuTranslationOutcome::Mapped(mapped) = outcome else {
        panic!("Sv39 memory walker should map after submitted PTE response");
    };
    assert_eq!(mapped.translation_id(), translation_id(97));
    assert_eq!(
        mapped.physical_address(),
        Address::new((leaf_ppn << 12) | (0x00f << 21) | (0x010 << 12) | 0xef0)
    );
    assert!(walker.lock().unwrap().is_idle());
    assert!(frontend.is_empty());
}

#[test]
fn riscv_sv39_memory_walker_keeps_queued_response_when_frontend_completion_fails() {
    let virtual_address = (0x01f_u64 << 30) | (0x00b_u64 << 21) | (0x00c_u64 << 12) | 0xcde;
    let root_ppn = 0x1b0;
    let leaf_ppn = 0x80000;
    let first_pte_request = MemoryRequestId::new(AgentId::new(31), 1600);
    let mut frontend = CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 0).unwrap());
    frontend
        .enqueue(
            22,
            CpuTranslationRequest::load(
                translation_id(95),
                memory_id(125),
                MemoryRouteId::new(6),
                endpoint("cpu0.dmem"),
                Address::new(virtual_address),
                AccessSize::new(8).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    let mut walker = RiscvSv39MemoryWalker::new(root_ppn, first_pte_request, layout());
    let advances = walker.start_ready(&mut frontend, 22).unwrap();
    let RiscvSv39MemoryWalkerAdvance::ReadPte(level2_request) = &advances[0] else {
        panic!("ready Sv39 translation should issue the first PTE read");
    };
    let response = pte_response(
        level2_request,
        RiscvSv39Pte::new(
            (leaf_ppn << 10) | SV39_PTE_V | SV39_PTE_R | SV39_PTE_W | SV39_PTE_A | SV39_PTE_D,
        ),
    );
    walker.record_memory_response(response);
    let mut mismatched_frontend =
        CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 0).unwrap());

    assert!(walker
        .advance_next_response(&mut mismatched_frontend)
        .is_err());
    assert_eq!(walker.active_count(), 1);
    assert_eq!(walker.pending_response_count(), 1);

    let RiscvSv39MemoryWalkerAdvance::Complete(outcome) = walker
        .advance_next_response(&mut frontend)
        .unwrap()
        .expect("queued PTE response should still be pending")
    else {
        panic!("preserved queued response should complete after frontend retry");
    };
    let CpuTranslationOutcome::Mapped(mapped) = outcome else {
        panic!("Sv39 memory walker should map after queued retry");
    };
    assert_eq!(mapped.translation_id(), translation_id(95));
    assert_eq!(walker.pending_response_count(), 0);
    assert_eq!(walker.active_count(), 0);
    assert!(frontend.is_empty());
}

#[test]
fn riscv_sv39_memory_walker_discards_bad_queued_response_before_next_response() {
    let virtual_address = (0x020_u64 << 30) | (0x00d_u64 << 21) | (0x00e_u64 << 12) | 0xdef;
    let root_ppn = 0x1c0;
    let leaf_ppn = 0x80000;
    let first_pte_request = MemoryRequestId::new(AgentId::new(31), 1700);
    let mut frontend = CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 0).unwrap());
    frontend
        .enqueue(
            24,
            CpuTranslationRequest::load(
                translation_id(96),
                memory_id(126),
                MemoryRouteId::new(6),
                endpoint("cpu0.dmem"),
                Address::new(virtual_address),
                AccessSize::new(8).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    let mut walker = RiscvSv39MemoryWalker::new(root_ppn, first_pte_request, layout());
    let advances = walker.start_ready(&mut frontend, 24).unwrap();
    let RiscvSv39MemoryWalkerAdvance::ReadPte(level2_request) = &advances[0] else {
        panic!("ready Sv39 translation should issue the first PTE read");
    };
    walker.record_memory_response(MemoryResponse::retry(level2_request));
    walker.record_memory_response(pte_response(
        level2_request,
        RiscvSv39Pte::new(
            (leaf_ppn << 10) | SV39_PTE_V | SV39_PTE_R | SV39_PTE_W | SV39_PTE_A | SV39_PTE_D,
        ),
    ));

    assert!(walker.advance_next_response(&mut frontend).is_err());
    assert_eq!(walker.active_count(), 1);
    assert_eq!(walker.pending_response_count(), 1);

    let RiscvSv39MemoryWalkerAdvance::Complete(outcome) = walker
        .advance_next_response(&mut frontend)
        .unwrap()
        .expect("valid queued PTE response should remain after bad response")
    else {
        panic!("valid queued PTE response should complete the frontend request");
    };
    let CpuTranslationOutcome::Mapped(mapped) = outcome else {
        panic!("Sv39 memory walker should map after discarding bad queued response");
    };
    assert_eq!(mapped.translation_id(), translation_id(96));
    assert_eq!(walker.pending_response_count(), 0);
    assert_eq!(walker.active_count(), 0);
    assert!(frontend.is_empty());
}

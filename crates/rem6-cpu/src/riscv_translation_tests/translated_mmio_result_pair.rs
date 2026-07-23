use rem6_isa_riscv::Register;
use rem6_kernel::PartitionId;
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequestId, TranslationAccessKind,
    TranslationAddressSpaceId, TranslationPageMap, TranslationPagePermissions, TranslationPageSize,
    TranslationQueueConfig, TranslationRequest, TranslationRequestId, TranslationTlbConfig,
};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

use super::super::*;
use crate::{
    riscv_fetch_ahead::O3MemoryResultWindowRoute, CpuCore, CpuDataConfig, CpuFetchConfig,
    CpuFetchEvent, CpuFetchRecord, CpuId, CpuResetState, CpuTranslationFrontend, RiscvCore,
};

#[test]
fn translation_preserves_and_binds_each_result_pair_authorization_once() {
    let core = translated_pair_core();
    assert_eq!(
        core.next_cached_translated_memory_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );

    let spans = [
        (request(0), 0x4000, 0x9000, 11),
        (request(1), 0x5000, 0xa000, 12),
    ];
    let mut state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.memory_result_window_authorizations.len(), 2);
    for (fetch_request, virtual_address, physical_address, rd) in spans {
        let size = AccessSize::new(8).unwrap();
        assert!(state.translated_result_authorization_is_pending(
            fetch_request,
            Address::new(virtual_address),
            size,
        ));
        let translated = TranslatedDataAccess {
            request_id: MemoryRequestId::new(AgentId::new(7), fetch_request.sequence() + 20),
            fetch_request,
            access: rem6_isa_riscv::MemoryAccessKind::Load {
                rd: Register::new(rd).unwrap(),
                address: virtual_address,
                width: rem6_isa_riscv::MemoryWidth::Doubleword,
                signed: false,
            },
            virtual_address: Address::new(virtual_address),
            size,
            physical_address: Address::new(physical_address),
            request_byte_offset: 0,
        };
        assert!(state.bind_translated_result_range(&translated));
        assert!(state.bind_translated_result_range(&translated));
        assert!(!state.translated_result_authorization_is_pending(
            fetch_request,
            Address::new(virtual_address),
            size,
        ));
        assert!(
            state.bind_translated_result_target(fetch_request, O3MemoryResultWindowRoute::Memory,)
        );
        assert!(
            state.bind_translated_result_target(fetch_request, O3MemoryResultWindowRoute::Memory,)
        );
        let authorization = state
            .memory_result_window_authorizations
            .get(&fetch_request)
            .copied()
            .unwrap();
        assert!(authorization.matches_bound_target(
            O3MemoryResultWindowRoute::Memory,
            Address::new(physical_address),
            size,
        ));
    }
}

fn translated_pair_core() -> RiscvCore {
    let core = RiscvCore::with_data_translation(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                MemoryRouteId::new(0),
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
        CpuDataConfig::new(
            endpoint("cpu0.dmem"),
            MemoryRouteId::new(1),
            CacheLineLayout::new(16).unwrap(),
        ),
        CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), 0x4000);
    core.write_register(Register::new(3).unwrap(), 0x5000);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch(0, 0x8000, ld(11, 2)),
            completed_fetch(1, 0x8004, ld(12, 3)),
        ]);

    let mut page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    page_map
        .map(
            Address::new(0x4000),
            Address::new(0x9000),
            1,
            TranslationPagePermissions::read_write_execute(),
        )
        .unwrap();
    let fill = TranslationRequest::new(
        TranslationRequestId::new(AgentId::new(7), 99),
        Address::new(0x4000),
        AccessSize::new(8).unwrap(),
        TranslationAccessKind::Load,
    )
    .unwrap();
    let mut state = core.state.lock().expect("riscv core lock");
    let address_space = TranslationAddressSpaceId::new(state.hart.translation_address_space());
    state
        .data_translation
        .as_mut()
        .unwrap()
        .tlb_mut()
        .unwrap()
        .translate_in_address_space(address_space, &fill, &page_map)
        .unwrap();
    drop(state);
    core
}

fn completed_fetch(sequence: u64, pc: u64, raw: u32) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            4,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            endpoint("cpu0.ifetch"),
            request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        raw.to_le_bytes().to_vec(),
    )
}

fn request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn ld(rd: u8, rs1: u8) -> u32 {
    (u32::from(rs1) << 15) | (0b011 << 12) | (u32::from(rd) << 7) | 0x03
}

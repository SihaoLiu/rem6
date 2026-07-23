use rem6_memory::{
    TranslationAccessKind, TranslationAddressSpaceId, TranslationRequest, TranslationRequestId,
};

use super::*;
use crate::riscv_translation::{PendingDataTranslation, TranslatedDataAccess};

pub(super) const HEAD_PC: u64 = 0x8000;
const YOUNGER_PC: u64 = 0x8004;
pub(super) const HEAD_VIRTUAL_ADDRESS: u64 = 0x4000;
const YOUNGER_VIRTUAL_ADDRESS: u64 = 0x5000;

pub(super) fn translated_result_pair_with_outstanding_head(memory_issue_width: usize) -> RiscvCore {
    translated_result_pair_with_fetches(
        memory_issue_width,
        vec![
            completed_fetch_with_raw(0, HEAD_PC, ld(11, 2)),
            completed_fetch_with_raw(1, YOUNGER_PC, ld(12, 3)),
        ],
        fetch_request(0),
        fetch_request(1),
    )
}

pub(super) fn translated_split_gapped_result_pair_with_outstanding_head(
    memory_issue_width: usize,
) -> RiscvCore {
    let head_bytes = ld(11, 2).to_le_bytes();
    translated_result_pair_with_fetches(
        memory_issue_width,
        vec![
            completed_fetch_with_data(10, HEAD_PC, head_bytes[..2].to_vec()),
            completed_fetch_with_data(20, HEAD_PC + 2, head_bytes[2..].to_vec()),
            completed_fetch_with_raw(40, YOUNGER_PC, ld(12, 3)),
        ],
        fetch_request(10),
        fetch_request(40),
    )
}

fn translated_result_pair_with_fetches(
    memory_issue_width: usize,
    fetches: Vec<CpuFetchEvent>,
    head_fetch_request: MemoryRequestId,
    younger_fetch_request: MemoryRequestId,
) -> RiscvCore {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data_translation(
        cpu_core(fetch_route, HEAD_PC),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
        CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.set_o3_issue_width(4);
    core.set_o3_memory_issue_width(memory_issue_width);
    core.write_register(reg(2), HEAD_VIRTUAL_ADDRESS);
    core.write_register(reg(3), YOUNGER_VIRTUAL_ADDRESS);
    for fetch in &fetches {
        core.core.advance_sequence_past(fetch.request_id());
    }
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend(fetches);

    let page_map = translated_page_map();
    install_cached_head_translation(&core, &page_map);
    assert_eq!(
        core.next_cached_translated_memory_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );
    let head_execution = core
        .execute_next_completed_fetch()
        .unwrap()
        .or_else(|| core.execute_next_completed_fetch().unwrap())
        .expect("translated pair head executes");
    assert_eq!(head_execution.fetch_pc(), Address::new(HEAD_PC));
    core.issue_next_translated_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        &page_map,
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .expect("translated pair head issues");

    {
        let mut state = core.state.lock().expect("riscv core lock");
        let (_, issued) = sole_outstanding(&state);
        let (resident, forwarded, completed_partial, younger_rows) = state
            .o3_runtime
            .live_scalar_memory_handoff()
            .expect("real translated issue has one resident row");
        assert_eq!(resident.len(), 1);
        assert!(forwarded.is_empty());
        assert!(completed_partial.is_empty());
        assert_eq!(younger_rows, 0);
        assert!(!state.o3_runtime.matches_exact_memory_result_head(
            head_fetch_request,
            issued.request,
            issued.tick,
            resident[0].o3_sequence,
            &issued.access,
        ));

        // Task 5 makes the real translated issue path establish this policy directly.
        let mut runtime = crate::o3_runtime::O3RuntimeState::default();
        assert!(runtime.set_window_depths(4, 4));
        assert!(runtime.set_issue_width(4));
        assert!(runtime.set_memory_issue_width(memory_issue_width));
        assert!(runtime.stage_live_data_access_issue(
            &head_execution,
            issued.request,
            issued.tick,
            O3DataAccessWindowPolicy::MemoryResultWindow,
        ));
        state.o3_runtime = runtime;
    }

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.outstanding_data.len(), 1);
    assert_eq!(state.memory_result_window_authorizations.len(), 1);
    let authorization = state
        .memory_result_window_authorizations
        .get(&younger_fetch_request)
        .copied()
        .expect("translated younger authorization remains");
    assert_eq!(
        authorization.role(),
        crate::riscv_fetch_ahead::O3MemoryResultWindowRole::YoungerRead
    );
    assert!(authorization.is_translated());
    let (resident, forwarded, completed_partial, younger_rows) = state
        .o3_runtime
        .live_scalar_memory_handoff()
        .expect("resident translated head has handoff identity");
    assert_eq!(resident.len(), 1);
    assert!(forwarded.is_empty());
    assert!(completed_partial.is_empty());
    assert_eq!(younger_rows, 0);
    let (data_request, issued) = state.outstanding_data.iter().next().unwrap();
    assert_eq!(resident[0].fetch_request, head_fetch_request);
    assert_eq!(resident[0].data_request, *data_request);
    assert_eq!(resident[0].data_request, issued.request);
    assert_eq!(resident[0].o3_sequence, 0);
    assert_eq!(resident[0].issue_tick, issued.tick);
    assert_eq!(
        resident[0].operation,
        crate::riscv_execution_mode_handoff::RiscvO3LiveDataHandoffOperation::Load
    );
    assert!(matches!(
        issued.access,
        rem6_isa_riscv::MemoryAccessKind::Load { rd, .. } if !rd.is_zero()
    ));
    assert!(state.o3_runtime.matches_exact_memory_result_head(
        resident[0].fetch_request,
        issued.request,
        issued.tick,
        resident[0].o3_sequence,
        &issued.access,
    ));
    drop(state);

    core
}

pub(super) fn install_pending_younger_translation(core: &RiscvCore, wrong_key: bool) {
    let mut state = core.state.lock().expect("riscv core lock");
    let younger_fetch_request = sole_younger_authorization_request(&state);
    let request_id = fetch_request(200);
    let key_sequence = request_id.sequence() + u64::from(wrong_key);
    state.pending_data_translations.insert(
        TranslationRequestId::new(request_id.agent(), key_sequence),
        PendingDataTranslation {
            request_id,
            fetch_request: younger_fetch_request,
            access: translated_younger_access(),
            virtual_address: Address::new(YOUNGER_VIRTUAL_ADDRESS),
            size: AccessSize::new(8).unwrap(),
            request_byte_offset: 0,
        },
    );
}

pub(super) fn install_ready_younger_translation(core: &RiscvCore, wrong_key: bool) {
    let mut state = core.state.lock().expect("riscv core lock");
    let younger_fetch_request = sole_younger_authorization_request(&state);
    let map_key = if wrong_key {
        fetch_request(99)
    } else {
        younger_fetch_request
    };
    state.ready_translated_data.insert(
        map_key,
        TranslatedDataAccess {
            request_id: fetch_request(200),
            fetch_request: younger_fetch_request,
            access: translated_younger_access(),
            virtual_address: Address::new(YOUNGER_VIRTUAL_ADDRESS),
            size: AccessSize::new(8).unwrap(),
            physical_address: Address::new(0xa000),
            request_byte_offset: 0,
        },
    );
}

fn sole_younger_authorization_request(state: &RiscvCoreState) -> MemoryRequestId {
    *state
        .memory_result_window_authorizations
        .keys()
        .next()
        .expect("translated younger authorization")
}

pub(super) fn mutate_sole_outstanding(
    core: &RiscvCore,
    mutate: impl FnOnce(&mut IssuedDataAccess),
) {
    let mut state = core.state.lock().expect("riscv core lock");
    let request = *state.outstanding_data.keys().next().unwrap();
    mutate(state.outstanding_data.get_mut(&request).unwrap());
}

pub(super) fn translated_head_access(rd: u8, address: u64) -> MemoryAccessKind {
    MemoryAccessKind::Load {
        rd: reg(rd),
        address,
        width: MemoryWidth::Doubleword,
        signed: false,
    }
}

fn translated_younger_access() -> MemoryAccessKind {
    translated_head_access(12, YOUNGER_VIRTUAL_ADDRESS)
}

pub(super) fn sole_outstanding(state: &RiscvCoreState) -> (MemoryRequestId, IssuedDataAccess) {
    let (request, issued) = state.outstanding_data.iter().next().unwrap();
    (*request, issued.clone())
}

pub(super) fn outstanding_issue_tick(core: &RiscvCore) -> Tick {
    core.state
        .lock()
        .expect("riscv core lock")
        .outstanding_data
        .values()
        .next()
        .unwrap()
        .tick
}

fn translated_page_map() -> TranslationPageMap {
    let mut page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    page_map
        .map(
            Address::new(HEAD_VIRTUAL_ADDRESS),
            Address::new(0x9000),
            1,
            TranslationPagePermissions::read_write_execute(),
        )
        .unwrap();
    page_map
}

fn install_cached_head_translation(core: &RiscvCore, page_map: &TranslationPageMap) {
    let request = TranslationRequest::new(
        TranslationRequestId::new(AgentId::new(7), 99),
        Address::new(HEAD_VIRTUAL_ADDRESS),
        AccessSize::new(8).unwrap(),
        TranslationAccessKind::Load,
    )
    .unwrap();
    let mut state = core.state.lock().expect("riscv core lock");
    let address_space = TranslationAddressSpaceId::new(state.hart.translation_address_space());
    state
        .data_translation
        .as_mut()
        .expect("translated pair has data translation")
        .tlb_mut()
        .expect("translated pair has a TLB")
        .translate_in_address_space(address_space, &request, page_map)
        .unwrap();
}

fn completed_fetch_with_raw(sequence: u64, pc: u64, raw: u32) -> CpuFetchEvent {
    completed_fetch_with_data(sequence, pc, raw.to_le_bytes().to_vec())
}

fn completed_fetch_with_data(sequence: u64, pc: u64, data: Vec<u8>) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            endpoint("cpu0.ifetch"),
            fetch_request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        data,
    )
}

pub(super) fn fetch_request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn ld(rd: u8, rs1: u8) -> u32 {
    i_type(0, rs1, 0b011, rd, 0x03)
}

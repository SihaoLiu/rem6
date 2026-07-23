use rem6_memory::{
    TranslationAccessKind, TranslationAddressSpaceId, TranslationRequest, TranslationRequestId,
};

use super::*;
use crate::riscv_translation::{PendingDataTranslation, TranslatedDataAccess};

const HEAD_PC: u64 = 0x8000;
const YOUNGER_PC: u64 = 0x8004;
const HEAD_VIRTUAL_ADDRESS: u64 = 0x4000;
const YOUNGER_VIRTUAL_ADDRESS: u64 = 0x5000;

#[test]
fn translated_result_pair_without_outstanding_data_is_ordinary() {
    let (_scheduler, _transport, fetch_route, _data_route) = memory_routes();
    let core = RiscvCore::new(cpu_core(fetch_route, HEAD_PC));

    assert_eq!(
        core.translated_result_pair_progress(0),
        O3ResultPairProgress::Ordinary
    );
}

#[test]
fn translated_result_pair_exact_resident_pair_is_ready() {
    let core = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&core);

    assert_eq!(
        core.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Ready { issue_tick }
    );
}

#[test]
fn translated_result_pair_memory_width_waits_for_selected_tick() {
    let core = translated_result_pair_with_outstanding_head(1);
    let issue_tick = outstanding_issue_tick(&core);

    assert_eq!(
        core.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::WaitUntil(issue_tick + 1)
    );
}

#[test]
fn translated_result_pair_rejects_unrelated_outstanding_request() {
    let core = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&core);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let (request_id, mut issued) = sole_outstanding(&state);
        state.outstanding_data.remove(&request_id);
        let unrelated_request = fetch_request(100);
        issued.request = unrelated_request;
        issued.fetch_request = fetch_request(99);
        state.outstanding_data.insert(unrelated_request, issued);
    }

    assert_eq!(
        core.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked
    );
}

#[test]
fn translated_result_pair_blocks_multiple_or_unrelated_auxiliary_state() {
    let multiple = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&multiple);
    {
        let mut state = multiple.state.lock().expect("riscv core lock");
        let (_, mut extra) = sole_outstanding(&state);
        extra.request = fetch_request(100);
        extra.fetch_request = fetch_request(99);
        state.outstanding_data.insert(extra.request, extra);
    }
    assert_eq!(
        multiple.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked
    );

    let pending = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&pending);
    {
        let mut state = pending.state.lock().expect("riscv core lock");
        let (_, issued) = sole_outstanding(&state);
        state.pending_data_translations.insert(
            TranslationRequestId::new(AgentId::new(7), 200),
            PendingDataTranslation {
                request_id: fetch_request(200),
                fetch_request: fetch_request(99),
                access: issued.access,
                virtual_address: Address::new(0x6000),
                size: issued.size,
                request_byte_offset: issued.request_byte_offset,
            },
        );
    }
    assert_eq!(
        pending.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked
    );

    let ready = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&ready);
    {
        let mut state = ready.state.lock().expect("riscv core lock");
        let (_, issued) = sole_outstanding(&state);
        state.ready_translated_data.insert(
            fetch_request(99),
            TranslatedDataAccess {
                request_id: fetch_request(200),
                fetch_request: fetch_request(99),
                access: issued.access,
                virtual_address: Address::new(0x6000),
                size: issued.size,
                physical_address: Address::new(0xa000),
                request_byte_offset: issued.request_byte_offset,
            },
        );
    }
    assert_eq!(
        ready.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked
    );

    let buffered = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&buffered);
    {
        let mut state = buffered.state.lock().expect("riscv core lock");
        let (_, issued) = sole_outstanding(&state);
        let issue = OutstandingDataAccess {
            tick: issued.tick,
            partition: issued.partition,
            target: issued.target,
            request_id: fetch_request(200),
            fetch_request: fetch_request(99),
            access: issued.access,
            size: issued.size,
            physical_address: issued.physical_address,
            request_byte_offset: issued.request_byte_offset,
            line_layout: Some(line_layout()),
            forwarded_load_data: None,
            store_load_forwarding_plan: issued.store_load_forwarding_plan,
        };
        let request = issue.memory_request().unwrap();
        state.buffered_o3_effects.insert(
            issue.request_id,
            BufferedO3Effect {
                predecessor: issued.request,
                issue,
                request,
            },
        );
    }
    assert_eq!(
        buffered.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked
    );

    let full = translated_result_pair_with_outstanding_head(2);
    let issue_tick = outstanding_issue_tick(&full);
    full.set_o3_window_depths(1, 1);
    assert_eq!(
        full.translated_result_pair_progress(issue_tick),
        O3ResultPairProgress::Blocked
    );
}

fn translated_result_pair_with_outstanding_head(memory_issue_width: usize) -> RiscvCore {
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
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(0, HEAD_PC, ld(11, 2)),
            completed_fetch_with_raw(1, YOUNGER_PC, ld(12, 3)),
        ]);

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
        .get(&fetch_request(1))
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
    assert_eq!(resident[0].fetch_request, fetch_request(0));
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
    assert!(state.has_exact_translated_result_pair_window(
        resident[0].fetch_request,
        resident[0].o3_sequence,
    ));
    assert_eq!(
        state.o3_runtime.next_memory_result_issue_tick(issued.tick),
        Some(if memory_issue_width == 1 {
            issued.tick + 1
        } else {
            issued.tick
        })
    );
    drop(state);

    core
}

fn sole_outstanding(state: &RiscvCoreState) -> (MemoryRequestId, IssuedDataAccess) {
    let (request, issued) = state.outstanding_data.iter().next().unwrap();
    (*request, issued.clone())
}

fn outstanding_issue_tick(core: &RiscvCore) -> Tick {
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
        raw.to_le_bytes().to_vec(),
    )
}

fn fetch_request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn ld(rd: u8, rs1: u8) -> u32 {
    i_type(0, rs1, 0b011, rd, 0x03)
}

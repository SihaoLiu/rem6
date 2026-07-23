use std::sync::Mutex;

use super::*;
use crate::{
    BranchPredictor, BranchPredictorCheckpointPayload, BranchPredictorConfig, BranchTargetBuffer,
    BranchTargetBufferConfig, BranchTargetProvider, CpuCore, CpuFetchConfig, CpuFetchRecord, CpuId,
    CpuResetState, CpuTranslationFrontend, InOrderPipelineInstruction, InOrderPipelineSnapshot,
    InOrderPipelineStage, MultiperspectivePerceptron, MultiperspectivePerceptronConfig,
    MultiperspectivePerceptronFeature, OutstandingFetch, RiscvBranchPredictorKind, RiscvCore,
    RiscvCpuExecutionEvent, TournamentBranchPredictor, TournamentBranchPredictorConfig,
    DEFAULT_RISCV_BRANCH_PREDICTOR_ENTRIES, RISCV_LOCAL_BIMODE_THREAD, RISCV_LOCAL_GSHARE_THREAD,
    RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD, RISCV_LOCAL_TOURNAMENT_THREAD,
};
use rem6_isa_riscv::{
    Immediate, MemoryAccessKind, MemoryWidth, Register, RiscvExecutionRecord, RiscvPmaRange,
};
use rem6_kernel::PartitionId;
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequestId, TranslationAccessKind,
    TranslationAddressSpaceId, TranslationPageMap, TranslationPagePermissions, TranslationPageSize,
    TranslationQueueConfig, TranslationRequest, TranslationRequestId, TranslationTlbConfig,
};
use rem6_mmio::{MmioBus, MmioRegisterBank, MmioRoute};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

mod btb;
mod checkpoint;
mod data_access_result;
mod data_access_result_effect;
mod data_access_result_pair;
mod dependent_result_address;
mod dependent_result_address_three_pending;
mod dependent_result_address_two_pending;
mod detailed_o3_control;
mod producer_forwarded_chain_validation;
mod producer_forwarded_control_validation;
mod producer_forwarded_return;
mod producer_forwarded_return_link_shapes;
mod producer_forwarded_scalar_return;
mod producer_forwarded_scalar_return_link_shapes;
mod ras_required_validation;
mod selected;
mod speculative_history;

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn install_cached_data_translation(
    core: &RiscvCore,
    virtual_base: u64,
    physical_base: u64,
    permissions: TranslationPagePermissions,
    fill_access: TranslationAccessKind,
) {
    let mut page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    page_map
        .map(
            Address::new(virtual_base),
            Address::new(physical_base),
            1,
            permissions,
        )
        .unwrap();
    let request = TranslationRequest::new(
        TranslationRequestId::new(AgentId::new(7), 99),
        Address::new(virtual_base),
        AccessSize::new(4).unwrap(),
        fill_access,
    )
    .unwrap();
    let mut state = core.state.lock().expect("riscv core lock");
    let address_space = TranslationAddressSpaceId::new(state.hart.translation_address_space());
    state
        .data_translation
        .as_mut()
        .expect("test core has data translation")
        .tlb_mut()
        .expect("test translation frontend has a TLB")
        .translate_in_address_space(address_space, &request, &page_map)
        .unwrap();
}

fn b_type(offset: i32, rs1: u8, rs2: u8, funct3: u32) -> u32 {
    let imm = offset as u32;
    ((imm & 0x1000) << 19)
        | ((imm & 0x07e0) << 20)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x001e) << 7)
        | ((imm & 0x0800) >> 4)
        | 0x63
}

fn j_type(offset: i32, rd: u8) -> u32 {
    let imm = offset as u32;
    (((imm >> 20) & 0x1) << 31)
        | (((imm >> 1) & 0x3ff) << 21)
        | (((imm >> 11) & 0x1) << 20)
        | (((imm >> 12) & 0xff) << 12)
        | (u32::from(rd) << 7)
        | 0x6f
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    ((imm as u32 & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn r_type(funct7: u32, rs1: u8, rs2: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = imm as u32 & 0x0fff;
    ((imm >> 5) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | 0x23
}

fn completed(sequence: u64, pc: u64) -> crate::CpuFetchEvent {
    crate::CpuFetchEvent::completed(
        CpuFetchRecord::new(
            0,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        vec![0; 4],
    )
}

fn core_with_completed_fetch(data: Vec<u8>) -> RiscvCore {
    core_with_completed_fetches([(0, 0x8000, data)])
}

fn btb_entry_kind(core: &RiscvCore, pc: u64) -> Option<BranchTargetKind> {
    core.branch_target_buffer_snapshot()
        .entries()
        .iter()
        .flatten()
        .find(|entry| entry.pc() == Address::new(pc))
        .map(|entry| entry.kind())
}

fn core_with_completed_fetches(
    fetches: impl IntoIterator<Item = (u64, u64, Vec<u8>)>,
) -> RiscvCore {
    core_with_completed_fetches_at(0x8000, fetches)
}

#[test]
#[should_panic(expected = "RISC-V branch lookahead must be between 1 and 3")]
fn riscv_core_rejects_branch_lookahead_above_supported_maximum() {
    let core = core_with_completed_fetches([]);
    core.set_branch_lookahead(4);
}

fn core_with_completed_fetches_at(
    entry: u64,
    fetches: impl IntoIterator<Item = (u64, u64, Vec<u8>)>,
) -> RiscvCore {
    let core = RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                MemoryRouteId::new(0),
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    );
    let mut core_state = core.core.state.lock().expect("cpu core lock");
    for (sequence, pc, data) in fetches {
        let size = AccessSize::new(data.len() as u64).unwrap();
        core_state.events.push(crate::CpuFetchEvent::completed(
            CpuFetchRecord::new(
                4,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                endpoint("cpu0.ifetch"),
                request(sequence),
                Address::new(pc),
                size,
            ),
            data,
        ));
    }
    drop(core_state);
    core
}

fn scalar_load_execution_event(
    pc: u64,
    sequence: u64,
    rd: u8,
    rs1: u8,
    address: u64,
) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Load {
        rd: Register::new(rd).unwrap(),
        rs1: Register::new(rs1).unwrap(),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    let access = MemoryAccessKind::Load {
        rd: Register::new(rd).unwrap(),
        address,
        width: MemoryWidth::Word,
        signed: false,
    };
    RiscvCpuExecutionEvent::new(
        crate::CpuFetchEvent::completed(
            CpuFetchRecord::new(
                4,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                endpoint("cpu0.ifetch"),
                request(sequence),
                Address::new(pc),
                AccessSize::new(4).unwrap(),
            ),
            vec![0; 4],
        ),
        instruction,
        RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
    )
}

fn scalar_store_execution_event(
    pc: u64,
    sequence: u64,
    rs1: u8,
    rs2: u8,
    address: u64,
) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Store {
        rs1: Register::new(rs1).unwrap(),
        rs2: Register::new(rs2).unwrap(),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
    };
    let access = MemoryAccessKind::Store {
        address,
        width: MemoryWidth::Word,
        value: 0x2a,
    };
    RiscvCpuExecutionEvent::new(
        crate::CpuFetchEvent::completed(
            CpuFetchRecord::new(
                4,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                endpoint("cpu0.ifetch"),
                request(sequence),
                Address::new(pc),
                AccessSize::new(4).unwrap(),
            ),
            vec![0; 4],
        ),
        instruction,
        RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
    )
}

#[test]
fn detailed_o3_gate_fetches_third_row_after_independent_scalar_younger() {
    let div = r_type(1, 1, 2, 0x4, 3, 0x33);
    let independent_addi = i_type(5, 0, 0x0, 4, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, div.to_le_bytes().to_vec()),
        (1, 0x8004, independent_addi.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
}

#[test]
fn detailed_o3_gate_fetches_fourth_row_after_two_younger_scalar_alus() {
    let div = r_type(1, 1, 2, 0x4, 3, 0x33);
    let producer = i_type(5, 0, 0x0, 4, 0x13);
    let consumer = i_type(11, 4, 0x0, 5, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, div.to_le_bytes().to_vec()),
        (1, 0x8004, producer.to_le_bytes().to_vec()),
        (2, 0x8008, consumer.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
}

#[test]
fn detailed_o3_gate_assembles_split_second_younger_before_fetching_fourth_row() {
    let div = r_type(1, 1, 2, 0x4, 3, 0x33);
    let producer = i_type(5, 0, 0x0, 4, 0x13);
    let consumer = i_type(11, 4, 0x0, 5, 0x13);
    let bytes = consumer.to_le_bytes();
    let core = core_with_completed_fetches([
        (0, 0x8000, div.to_le_bytes().to_vec()),
        (1, 0x8004, producer.to_le_bytes().to_vec()),
        (2, 0x8008, bytes[..2].to_vec()),
        (3, 0x800a, bytes[2..].to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
}

#[test]
fn detailed_o3_gate_does_not_duplicate_a_pending_first_younger_fetch() {
    let div = r_type(1, 1, 2, 0x4, 3, 0x33);
    let core = core_with_completed_fetch(div.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    let younger = CpuFetchRecord::new(
        5,
        PartitionId::new(0),
        MemoryRouteId::new(0),
        endpoint("cpu0.ifetch"),
        request(1),
        Address::new(0x8004),
        AccessSize::new(4).unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::issued(younger));

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_o3_gate_allows_head_destination_shadowing_before_a_later_read() {
    let div = r_type(1, 1, 2, 0x4, 3, 0x33);
    let shadow = i_type(5, 0, 0x0, 3, 0x13);
    let consumer = i_type(11, 3, 0x0, 5, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, div.to_le_bytes().to_vec()),
        (1, 0x8004, shadow.to_le_bytes().to_vec()),
        (2, 0x8008, consumer.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
}

#[test]
fn detailed_o3_gate_does_not_fetch_beyond_four_scalar_fu_rows() {
    let div = r_type(1, 1, 2, 0x4, 3, 0x33);
    let first = i_type(5, 0, 0x0, 4, 0x13);
    let second = i_type(11, 4, 0x0, 5, 0x13);
    let third = r_type(0, 5, 4, 0x0, 6, 0x33);
    let core = core_with_completed_fetches([
        (0, 0x8000, div.to_le_bytes().to_vec()),
        (1, 0x8004, first.to_le_bytes().to_vec()),
        (2, 0x8008, second.to_le_bytes().to_vec()),
        (3, 0x800c, third.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_o3_gate_does_not_duplicate_a_pending_second_younger_fetch() {
    let div = r_type(1, 1, 2, 0x4, 3, 0x33);
    let first = i_type(5, 0, 0x0, 4, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, div.to_le_bytes().to_vec()),
        (1, 0x8004, first.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    let second = CpuFetchRecord::new(
        5,
        PartitionId::new(0),
        MemoryRouteId::new(0),
        endpoint("cpu0.ifetch"),
        request(2),
        Address::new(0x8008),
        AccessSize::new(4).unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::issued(second));

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_o3_gate_stops_after_an_unshadowed_head_dependency() {
    let div = r_type(1, 1, 2, 0x4, 3, 0x33);
    let independent = i_type(5, 0, 0x0, 4, 0x13);
    let dependent = i_type(11, 3, 0x0, 5, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, div.to_le_bytes().to_vec()),
        (1, 0x8004, independent.to_le_bytes().to_vec()),
        (2, 0x8008, dependent.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_load_fetches_one_sequential_younger_before_data_issue() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let core = core_with_completed_fetch(load.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8004));
}

#[test]
fn detailed_scalar_load_fu_window_fetches_third_after_independent_alu() {
    let load = i_type(0, 2, 0x2, 12, 0x03);
    let first = i_type(5, 0, 0x0, 13, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, first.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
}

#[test]
fn detailed_scalar_load_fu_window_fetches_fourth_after_transitive_alu() {
    let load = i_type(0, 2, 0x2, 12, 0x03);
    let first = i_type(5, 0, 0x0, 13, 0x13);
    let second = i_type(11, 13, 0x0, 14, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, first.to_le_bytes().to_vec()),
        (2, 0x8008, second.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
}

#[test]
fn detailed_scalar_load_fu_window_stops_at_four_total_rows() {
    let load = i_type(0, 2, 0x2, 12, 0x03);
    let first = i_type(5, 0, 0x0, 13, 0x13);
    let second = i_type(11, 13, 0x0, 14, 0x13);
    let third = r_type(0, 14, 13, 0x0, 15, 0x33);
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, first.to_le_bytes().to_vec()),
        (2, 0x8008, second.to_le_bytes().to_vec()),
        (3, 0x800c, third.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_load_fu_window_allows_shadow_before_later_read() {
    let load = i_type(0, 2, 0x2, 12, 0x03);
    let shadow = i_type(5, 0, 0x0, 12, 0x13);
    let consumer = i_type(11, 12, 0x0, 13, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, shadow.to_le_bytes().to_vec()),
        (2, 0x8008, consumer.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
}

#[test]
fn detailed_scalar_load_fu_window_stops_after_unshadowed_load_dependency() {
    let load = i_type(0, 2, 0x2, 12, 0x03);
    let independent = i_type(5, 0, 0x0, 13, 0x13);
    let dependent = i_type(11, 12, 0x0, 14, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, independent.to_le_bytes().to_vec()),
        (2, 0x8008, dependent.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_load_fu_window_does_not_refetch_pending_second_alu() {
    let load = i_type(0, 2, 0x2, 12, 0x03);
    let first = i_type(5, 0, 0x0, 13, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, first.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    let second = CpuFetchRecord::new(
        5,
        PartitionId::new(0),
        MemoryRouteId::new(0),
        endpoint("cpu0.ifetch"),
        request(2),
        Address::new(0x8008),
        AccessSize::new(4).unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::issued(second));

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_load_fu_window_does_not_refetch_incomplete_split_second_alu() {
    let load = i_type(0, 2, 0x2, 12, 0x03);
    let first = i_type(5, 0, 0x0, 13, 0x13);
    let second = i_type(11, 13, 0x0, 14, 0x13).to_le_bytes();
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, first.to_le_bytes().to_vec()),
        (2, 0x8008, second[..2].to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_memory_prefix_continues_after_independent_alu() {
    let older = i_type(0, 2, 0x2, 12, 0x03);
    let younger = i_type(64, 2, 0x2, 13, 0x03);
    let alu = i_type(5, 0, 0x0, 14, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, older.to_le_bytes().to_vec()),
        (1, 0x8004, younger.to_le_bytes().to_vec()),
        (2, 0x8008, alu.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
}

#[test]
fn detailed_store_load_prefix_continues_after_independent_alu() {
    let store = s_type(0, 11, 10, 0x2);
    let load = i_type(0, 10, 0x2, 12, 0x03);
    let alu = i_type(5, 0, 0x0, 13, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, store.to_le_bytes().to_vec()),
        (1, 0x8004, load.to_le_bytes().to_vec()),
        (2, 0x8008, alu.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.state
        .lock()
        .expect("riscv core lock")
        .hart
        .write(Register::new(10).unwrap(), 0x9000);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
}

#[test]
fn detailed_scalar_memory_prefix_stops_after_dependency_on_any_load() {
    for (source, label) in [(12, "first load"), (13, "second load")] {
        let older = i_type(0, 2, 0x2, 12, 0x03);
        let younger = i_type(64, 2, 0x2, 13, 0x03);
        let dependent = i_type(5, source, 0x0, 14, 0x13);
        let core = core_with_completed_fetches([
            (0, 0x8000, older.to_le_bytes().to_vec()),
            (1, 0x8004, younger.to_le_bytes().to_vec()),
            (2, 0x8008, dependent.to_le_bytes().to_vec()),
        ]);
        core.set_detailed_live_retire_gate_enabled(true);
        core.set_o3_scalar_memory_depth(4);

        assert_eq!(
            core.next_fetch_ahead_before_retire(),
            None,
            "dependency on the {label} must terminate the mixed window"
        );
    }
}

#[test]
fn detailed_scalar_store_fetches_one_sequential_younger_before_data_issue() {
    let store = s_type(0, 3, 2, 0x2);
    let core = core_with_completed_fetch(store.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    core.state
        .lock()
        .expect("riscv core lock")
        .hart
        .write(Register::new(2).unwrap(), 0x9000);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8004));
}

#[test]
fn detailed_same_range_store_prefix_prefetches_a_younger_load_before_data_issue() {
    let older_store = s_type(0, 11, 10, 0x2);
    let younger_store = s_type(0, 14, 10, 0x2);
    let core = core_with_completed_fetches([
        (0, 0x8000, older_store.to_le_bytes().to_vec()),
        (1, 0x8004, younger_store.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(3);
    core.state
        .lock()
        .expect("riscv core lock")
        .hart
        .write(Register::new(10).unwrap(), 0x9000);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
}

#[test]
fn detailed_disjoint_store_prefix_prefetches_a_younger_load_before_data_issue() {
    let older_store = s_type(0, 11, 10, 0x2);
    let disjoint_store = s_type(24, 12, 10, 0x2);
    let overlapping_store = s_type(2, 13, 10, 0x1);
    let core = core_with_completed_fetches([
        (0, 0x8000, older_store.to_le_bytes().to_vec()),
        (1, 0x8004, disjoint_store.to_le_bytes().to_vec()),
        (2, 0x8008, overlapping_store.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.state
        .lock()
        .expect("riscv core lock")
        .hart
        .write(Register::new(10).unwrap(), 0x9000);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
}

#[test]
fn detailed_store_led_window_prefetches_a_second_younger_load() {
    let store = s_type(0, 11, 10, 0x2);
    let first_load = i_type(64, 10, 0x2, 13, 0x03);
    let core = core_with_completed_fetches([
        (0, 0x8000, store.to_le_bytes().to_vec()),
        (1, 0x8004, first_load.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(3);
    core.state
        .lock()
        .expect("riscv core lock")
        .hart
        .write(Register::new(10).unwrap(), 0x9000);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
}

#[test]
fn detailed_store_led_window_prefetches_a_third_younger_load_at_depth_four() {
    let store = s_type(0, 11, 10, 0x2);
    let first_load = i_type(64, 10, 0x2, 13, 0x03);
    let second_load = i_type(128, 10, 0x2, 14, 0x03);
    let core = core_with_completed_fetches([
        (0, 0x8000, store.to_le_bytes().to_vec()),
        (1, 0x8004, first_load.to_le_bytes().to_vec()),
        (2, 0x8008, second_load.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.state
        .lock()
        .expect("riscv core lock")
        .hart
        .write(Register::new(10).unwrap(), 0x9000);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
}

#[test]
fn detailed_uncacheable_scalar_store_does_not_fetch_ahead() {
    let store = s_type(0, 3, 2, 0x2);
    let core = core_with_completed_fetch(store.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    core.state
        .lock()
        .expect("riscv core lock")
        .hart
        .write(Register::new(2).unwrap(), 0x9000);
    core.add_pma_uncacheable_range(RiscvPmaRange::new(0x9000, 0x9004).unwrap())
        .unwrap();

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_load_waits_for_completed_younger_fetch() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let addi = i_type(7, 0, 0x0, 6, 0x13);
    let core = core_with_completed_fetch(load.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    let younger = CpuFetchRecord::new(
        5,
        PartitionId::new(0),
        MemoryRouteId::new(0),
        endpoint("cpu0.ifetch"),
        request(1),
        Address::new(0x8004),
        AccessSize::new(4).unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::issued(younger.clone()));

    assert!(!core
        .can_retire_completed_fetch_while_fetch_pending()
        .unwrap());

    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::completed(
            younger,
            addi.to_le_bytes().to_vec(),
        ));
    assert!(core
        .can_retire_completed_fetch_while_fetch_pending()
        .unwrap());
}

#[test]
fn detailed_float_load_result_waits_for_its_younger_scalar_fetch() {
    let float_load = i_type(0, 5, 0b011, 1, 0x07);
    let div = (1_u32 << 25) | (2 << 20) | (1 << 15) | (4 << 12) | (3 << 7) | 0x33;
    let core = core_with_completed_fetch(float_load.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    let younger = CpuFetchRecord::new(
        5,
        PartitionId::new(0),
        MemoryRouteId::new(0),
        endpoint("cpu0.ifetch"),
        request(1),
        Address::new(0x8004),
        AccessSize::new(4).unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::issued(younger.clone()));

    assert!(!core
        .can_retire_completed_fetch_while_fetch_pending()
        .unwrap());

    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::completed(
            younger,
            div.to_le_bytes().to_vec(),
        ));
    assert!(core
        .can_retire_completed_fetch_while_fetch_pending()
        .unwrap());
}

#[test]
fn detailed_scalar_load_does_not_refetch_pending_younger_with_branch_lookahead() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let core = core_with_completed_fetch(load.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(3);
    let younger = CpuFetchRecord::new(
        5,
        PartitionId::new(0),
        MemoryRouteId::new(0),
        endpoint("cpu0.ifetch"),
        request(1),
        Address::new(0x8004),
        AccessSize::new(4).unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::issued(younger));

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_load_advances_past_completed_younger_alu_with_branch_lookahead() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let addi = i_type(7, 0, 0x0, 6, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, addi.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(3);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
}

#[test]
fn detailed_scalar_load_window_fetches_third_with_two_entry_lookahead() {
    let older = i_type(0, 2, 0x2, 5, 0x03);
    let younger = i_type(64, 2, 0x2, 6, 0x03);
    let core = core_with_completed_fetches([
        (0, 0x8000, older.to_le_bytes().to_vec()),
        (1, 0x8004, younger.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
}

#[test]
fn explicit_scalar_memory_depth_survives_later_branch_lookahead() {
    let older = i_type(0, 2, 0x2, 5, 0x03);
    let younger = i_type(64, 2, 0x2, 6, 0x03);
    let core = core_with_completed_fetches([
        (0, 0x8000, older.to_le_bytes().to_vec()),
        (1, 0x8004, younger.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(2);
    core.set_branch_lookahead(2);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_load_window_does_not_refetch_pending_third() {
    let older = i_type(0, 2, 0x2, 5, 0x03);
    let younger = i_type(64, 2, 0x2, 6, 0x03);
    let core = core_with_completed_fetches([
        (0, 0x8000, older.to_le_bytes().to_vec()),
        (1, 0x8004, younger.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(3);
    let third = CpuFetchRecord::new(
        5,
        PartitionId::new(0),
        MemoryRouteId::new(0),
        endpoint("cpu0.ifetch"),
        request(2),
        Address::new(0x8008),
        AccessSize::new(4).unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::issued(third));

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_load_window_keeps_third_fetch_suppressed_by_default() {
    let older = i_type(0, 2, 0x2, 5, 0x03);
    let younger = i_type(64, 2, 0x2, 6, 0x03);
    let core = core_with_completed_fetches([
        (0, 0x8000, older.to_le_bytes().to_vec()),
        (1, 0x8004, younger.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_load_window_depth_one_suppresses_younger_fetch() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let core = core_with_completed_fetch(load.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(1);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_zero_destination_scalar_load_does_not_bypass_window_depth() {
    let load = i_type(0, 2, 0x2, 0, 0x03);
    let core = core_with_completed_fetch(load.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(4);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_load_window_depth_two_counts_live_older_row() {
    let current = i_type(64, 2, 0x2, 6, 0x03);
    let core = core_with_completed_fetches([(1, 0x8004, current.to_le_bytes().to_vec())]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(2);
    let older = scalar_load_execution_event(0x8000, 0, 5, 2, 0x9000);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.hart.set_pc(0x8004);
        assert!(state
            .o3_runtime
            .stage_live_data_access_issue_for_test(&older, request(20), 31));
    }

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_store_led_window_fetches_a_second_younger_load() {
    let current = i_type(64, 10, 0x2, 13, 0x03);
    let core = core_with_completed_fetches([(1, 0x8004, current.to_le_bytes().to_vec())]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(3);
    let store = scalar_store_execution_event(0x8000, 0, 10, 11, 0x9000);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.hart.set_pc(0x8004);
        state.hart.write(Register::new(10).unwrap(), 0x9000);
        assert!(state
            .o3_runtime
            .stage_live_data_access_issue_for_test(&store, request(20), 31));
    }

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
}

#[test]
fn detailed_resident_store_accepts_a_same_range_store_and_fetches_the_load() {
    let current = s_type(0, 14, 10, 0x2);
    let core = core_with_completed_fetches([(1, 0x8004, current.to_le_bytes().to_vec())]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(3);
    let store = scalar_store_execution_event(0x8000, 0, 10, 11, 0x9000);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.hart.set_pc(0x8004);
        state.hart.write(Register::new(10).unwrap(), 0x9000);
        assert!(state
            .o3_runtime
            .stage_live_data_access_issue_for_test(&store, request(20), 31));
    }

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
}

#[test]
fn detailed_resident_store_accepts_a_disjoint_store_prefix_and_fetches_the_load() {
    let disjoint_store = s_type(24, 12, 10, 0x2);
    let overlapping_store = s_type(2, 13, 10, 0x1);
    let core = core_with_completed_fetches([
        (1, 0x8004, disjoint_store.to_le_bytes().to_vec()),
        (2, 0x8008, overlapping_store.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(4);
    let store = scalar_store_execution_event(0x8000, 0, 10, 11, 0x9000);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.hart.set_pc(0x8004);
        state.hart.write(Register::new(10).unwrap(), 0x9000);
        assert!(state
            .o3_runtime
            .stage_live_data_access_issue_for_test(&store, request(20), 31));
    }

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
}

#[test]
fn translated_cached_memory_driver_fetches_third_younger_alu() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let first = i_type(7, 0, 0x0, 6, 0x13);
    let second = i_type(11, 6, 0x0, 7, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, first.to_le_bytes().to_vec()),
        (2, 0x8008, second.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.hart.write(Register::new(2).unwrap(), 0x4000);
        state.data_translation = Some(CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ));
    }
    install_cached_data_translation(
        &core,
        0x4000,
        0x9000,
        TranslationPagePermissions::read_write(),
        TranslationAccessKind::Load,
    );

    assert_eq!(core.next_fetch_ahead_before_retire(), None);

    let decision = core
        .next_cached_translated_memory_fetch_ahead_before_retire()
        .unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
}

#[test]
fn detailed_float_load_result_requests_a_bounded_scalar_suffix() {
    let float_load = i_type(0, 5, 0b011, 1, 0x07);
    let core = core_with_completed_fetch(float_load.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(2);

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8004))
    );
}

#[test]
fn mmio_aware_compressed_load_is_terminal_before_retirement() {
    let core = compressed_cached_translated_load_core();
    let bank =
        MmioRegisterBank::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        rem6_memory::AddressRange::new(Address::new(0x1000), AccessSize::new(0x100).unwrap())
            .unwrap(),
        MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 2).unwrap(),
        Mutex::new(bank),
    )
    .unwrap();

    assert_eq!(
        core.next_mmio_aware_fetch_ahead_before_retire(&bus)
            .map(|decision| decision.pc()),
        None
    );
}

fn compressed_cached_translated_load_core() -> RiscvCore {
    let compressed_ld_x9_from_x8 = 0x6004_u16;
    let core = core_with_completed_fetches_at(
        0x800e,
        [(0, 0x800e, compressed_ld_x9_from_x8.to_le_bytes().to_vec())],
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(2);
    core.write_register(Register::new(8).unwrap(), 0x4000);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.data_translation = Some(CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ));
    }
    install_cached_data_translation(
        &core,
        0x4000,
        0x1000,
        TranslationPagePermissions::read_write(),
        TranslationAccessKind::Load,
    );
    core
}

#[test]
fn translated_cached_scalar_load_window_stops_at_configured_depth() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let first = i_type(7, 0, 0x0, 6, 0x13);
    let second = i_type(11, 6, 0x0, 7, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, first.to_le_bytes().to_vec()),
        (2, 0x8008, second.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(3);
    core.write_register(Register::new(2).unwrap(), 0x4000);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.data_translation = Some(CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ));
    }
    install_cached_data_translation(
        &core,
        0x4000,
        0x9000,
        TranslationPagePermissions::read_write(),
        TranslationAccessKind::Load,
    );

    assert_eq!(
        core.next_cached_translated_memory_fetch_ahead_before_retire(),
        None
    );
}

#[test]
fn translated_uncached_scalar_load_is_terminal_before_retirement() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let core = core_with_completed_fetch(load.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.hart.write(Register::new(2).unwrap(), 0x4000);
        state.data_translation = Some(CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ));
    }

    assert_eq!(
        core.next_cached_translated_memory_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        None
    );
}

#[test]
fn translated_cached_permission_fault_does_not_fetch_younger_alu() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let core = core_with_completed_fetch(load.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.hart.write(Register::new(2).unwrap(), 0x4000);
        state.data_translation = Some(CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ));
    }
    install_cached_data_translation(
        &core,
        0x4000,
        0x9000,
        TranslationPagePermissions::new(false, true, false),
        TranslationAccessKind::Store,
    );

    assert_eq!(
        core.next_cached_translated_memory_fetch_ahead_before_retire(),
        None
    );
}

#[test]
fn translated_cached_uncacheable_load_is_terminal_before_retirement() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let core = core_with_completed_fetch(load.to_le_bytes().to_vec());
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.hart.write(Register::new(2).unwrap(), 0x4000);
        state.data_translation = Some(CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ));
    }
    install_cached_data_translation(
        &core,
        0x4000,
        0x9000,
        TranslationPagePermissions::read_write(),
        TranslationAccessKind::Load,
    );
    core.add_pma_uncacheable_range(RiscvPmaRange::new(0x9000, 0x9004).unwrap())
        .unwrap();

    assert_eq!(
        core.next_cached_translated_memory_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        None
    );
}

#[test]
fn translated_cached_scalar_load_window_rejects_younger_memory_prefix() {
    let older = i_type(0, 2, 0x2, 5, 0x03);
    let middle = i_type(64, 2, 0x2, 6, 0x03);
    let third = i_type(128, 2, 0x2, 7, 0x03);
    let core = core_with_completed_fetches([
        (0, 0x8000, older.to_le_bytes().to_vec()),
        (1, 0x8004, middle.to_le_bytes().to_vec()),
        (2, 0x8008, third.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.hart.write(Register::new(2).unwrap(), 0x4000);
        state.data_translation = Some(CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ));
    }
    install_cached_data_translation(
        &core,
        0x4000,
        0x9000,
        TranslationPagePermissions::read_write(),
        TranslationAccessKind::Load,
    );

    assert_eq!(
        core.next_cached_translated_memory_fetch_ahead_before_retire(),
        None
    );
}

#[test]
fn detailed_scalar_load_window_blocks_third_fetch_on_address_dependency() {
    let older = i_type(0, 2, 0x2, 5, 0x03);
    let dependent = i_type(0, 5, 0x2, 6, 0x03);
    let core = core_with_completed_fetches([
        (0, 0x8000, older.to_le_bytes().to_vec()),
        (1, 0x8004, dependent.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(3);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_load_window_fetches_fourth_at_explicit_depth() {
    let older = i_type(0, 2, 0x2, 5, 0x03);
    let middle = i_type(64, 2, 0x2, 6, 0x03);
    let third = i_type(128, 2, 0x2, 7, 0x03);
    let core = core_with_completed_fetches([
        (0, 0x8000, older.to_le_bytes().to_vec()),
        (1, 0x8004, middle.to_le_bytes().to_vec()),
        (2, 0x8008, third.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
}

#[test]
fn detailed_scalar_load_window_assembles_split_third_before_fetching_fourth() {
    let older = i_type(0, 2, 0x2, 5, 0x03);
    let middle = i_type(64, 2, 0x2, 6, 0x03);
    let third = i_type(128, 2, 0x2, 7, 0x03).to_le_bytes();
    let core = core_with_completed_fetches([
        (0, 0x8000, older.to_le_bytes().to_vec()),
        (1, 0x8004, middle.to_le_bytes().to_vec()),
        (2, 0x8008, third[..2].to_vec()),
        (3, 0x800a, third[2..].to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
}

#[test]
fn detailed_scalar_load_window_does_not_refetch_incomplete_split_third() {
    let older = i_type(0, 2, 0x2, 5, 0x03);
    let middle = i_type(64, 2, 0x2, 6, 0x03);
    let third = i_type(128, 2, 0x2, 7, 0x03).to_le_bytes();
    let core = core_with_completed_fetches([
        (0, 0x8000, older.to_le_bytes().to_vec()),
        (1, 0x8004, middle.to_le_bytes().to_vec()),
        (2, 0x8008, third[..2].to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(4);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_load_window_does_not_fetch_beyond_load_dependent_completed_branch() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let branch = b_type(8, 5, 2, 0x0);
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, branch.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(4);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_load_window_does_not_refetch_pending_fourth() {
    let older = i_type(0, 2, 0x2, 5, 0x03);
    let middle = i_type(64, 2, 0x2, 6, 0x03);
    let third = i_type(128, 2, 0x2, 7, 0x03);
    let core = core_with_completed_fetches([
        (0, 0x8000, older.to_le_bytes().to_vec()),
        (1, 0x8004, middle.to_le_bytes().to_vec()),
        (2, 0x8008, third.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    let fourth = CpuFetchRecord::new(
        5,
        PartitionId::new(0),
        MemoryRouteId::new(0),
        endpoint("cpu0.ifetch"),
        request(3),
        Address::new(0x800c),
        AccessSize::new(4).unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::issued(fourth));

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_load_window_blocks_fourth_after_any_older_dependency() {
    let older = i_type(0, 2, 0x2, 5, 0x03);
    let middle = i_type(64, 2, 0x2, 6, 0x03);
    let dependent_third = i_type(0, 5, 0x2, 7, 0x03);
    let core = core_with_completed_fetches([
        (0, 0x8000, older.to_le_bytes().to_vec()),
        (1, 0x8004, middle.to_le_bytes().to_vec()),
        (2, 0x8008, dependent_third.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn timing_scalar_load_does_not_enable_detailed_fetch_ahead() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let core = core_with_completed_fetch(load.to_le_bytes().to_vec());

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_o3_gate_ignores_duplicate_younger_completion_when_fetching_third_row() {
    let div = r_type(1, 1, 2, 0x4, 3, 0x33);
    let independent_addi = i_type(5, 0, 0x0, 4, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, div.to_le_bytes().to_vec()),
        (1, 0x8004, independent_addi.to_le_bytes().to_vec()),
        (2, 0x8004, independent_addi.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
}

#[test]
fn detailed_o3_gate_assembles_split_younger_before_fetching_third_row() {
    let div = r_type(1, 1, 2, 0x4, 3, 0x33);
    let independent_addi = i_type(5, 0, 0x0, 4, 0x13);
    let bytes = independent_addi.to_le_bytes();
    let core = core_with_completed_fetches([
        (0, 0x8000, div.to_le_bytes().to_vec()),
        (1, 0x8004, bytes[..2].to_vec()),
        (2, 0x8006, bytes[2..].to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
}

#[test]
fn detailed_o3_gate_does_not_refetch_incomplete_split_younger_with_branch_lookahead() {
    let div = r_type(1, 1, 2, 0x4, 3, 0x33);
    let independent_addi = i_type(5, 0, 0x0, 4, 0x13);
    let bytes = independent_addi.to_le_bytes();
    let core = core_with_completed_fetches([
        (0, 0x8000, div.to_le_bytes().to_vec()),
        (1, 0x8004, bytes[..2].to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_o3_gate_keeps_two_rows_when_younger_depends_on_gate_destination() {
    let div = r_type(1, 1, 2, 0x4, 3, 0x33);
    let dependent_addi = i_type(5, 3, 0x0, 4, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, div.to_le_bytes().to_vec()),
        (1, 0x8004, dependent_addi.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

fn core_with_recorded_selected_direct_speculation(kind: RiscvBranchPredictorKind) -> RiscvCore {
    let core = core_with_completed_fetch(j_type(12, 0).to_le_bytes().to_vec());
    core.set_branch_predictor_kind(kind);
    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x800c));
    record_fetch_ahead_speculation(&core, &decision).unwrap();
    core
}

fn record_fetch_ahead_speculation(
    core: &RiscvCore,
    decision: &RiscvFetchAheadDecision,
) -> Result<(), RiscvCpuError> {
    let prepared = core.prepare_fetch_ahead_speculation(decision)?;
    core.record_prepared_fetch_ahead_speculation(prepared);
    Ok(())
}

fn selected_family_global_history(state: &RiscvCoreState, kind: RiscvBranchPredictorKind) -> u64 {
    match kind {
        RiscvBranchPredictorKind::GShare => {
            state.gshare_branch_predictor.snapshot().threads()[0].global_history()
        }
        RiscvBranchPredictorKind::BiMode => {
            state.bimode_branch_predictor.snapshot().threads()[0].global_history()
        }
        RiscvBranchPredictorKind::Tournament => {
            state.tournament_branch_predictor.snapshot().threads()[0].global_history()
        }
        other => panic!("unsupported selected predictor family: {other:?}"),
    }
}

fn selected_family_speculation_count(
    state: &RiscvCoreState,
    kind: RiscvBranchPredictorKind,
) -> usize {
    state
        .selected_branch_speculations
        .values()
        .filter(|speculation| {
            matches!(
                (kind, speculation),
                (
                    RiscvBranchPredictorKind::GShare,
                    RiscvSelectedBranchSpeculation::GShare { .. }
                ) | (
                    RiscvBranchPredictorKind::BiMode,
                    RiscvSelectedBranchSpeculation::BiMode { .. }
                ) | (
                    RiscvBranchPredictorKind::Tournament,
                    RiscvSelectedBranchSpeculation::Tournament { .. }
                ) | (
                    RiscvBranchPredictorKind::TageScL,
                    RiscvSelectedBranchSpeculation::TageScL { .. }
                ) | (
                    RiscvBranchPredictorKind::MultiperspectivePerceptron,
                    RiscvSelectedBranchSpeculation::MultiperspectivePerceptron { .. }
                )
            )
        })
        .count()
}

fn restore_selected_family_checkpoint(core: &RiscvCore, kind: RiscvBranchPredictorKind) {
    match kind {
        RiscvBranchPredictorKind::GShare => core
            .restore_gshare_branch_predictor_checkpoint_payload(
                core.gshare_branch_predictor_checkpoint_payload(),
            )
            .unwrap(),
        RiscvBranchPredictorKind::BiMode => core
            .restore_bimode_branch_predictor_checkpoint_payload(
                core.bimode_branch_predictor_checkpoint_payload(),
            )
            .unwrap(),
        RiscvBranchPredictorKind::Tournament => core
            .restore_tournament_branch_predictor_checkpoint_payload(
                core.tournament_branch_predictor_checkpoint_payload(),
            )
            .unwrap(),
        RiscvBranchPredictorKind::TageScL => core
            .restore_tage_sc_l_branch_predictor_checkpoint_payload(
                core.tage_sc_l_branch_predictor_checkpoint_payload(),
            )
            .unwrap(),
        RiscvBranchPredictorKind::MultiperspectivePerceptron => core
            .restore_multiperspective_perceptron_checkpoint_payload(
                core.multiperspective_perceptron_checkpoint_payload(),
            )
            .unwrap(),
        other => panic!("unsupported selected predictor family: {other:?}"),
    }
}

fn train_selected_gshare_taken(state: &mut RiscvCoreState, pc: Address) {
    for _ in 0..2 {
        let prediction = state
            .gshare_branch_predictor
            .predict(RISCV_LOCAL_GSHARE_THREAD, pc)
            .unwrap();
        state
            .gshare_branch_predictor
            .train(prediction.history(), true, false)
            .unwrap();
    }
    let trained = state
        .gshare_branch_predictor
        .predict(RISCV_LOCAL_GSHARE_THREAD, pc)
        .unwrap();
    assert!(trained.predicted_taken());
}

fn train_selected_bimode_taken(state: &mut RiscvCoreState, pc: Address) {
    for _ in 0..4 {
        let prediction = state
            .bimode_branch_predictor
            .predict(RISCV_LOCAL_BIMODE_THREAD, pc)
            .unwrap();
        state
            .bimode_branch_predictor
            .train(prediction.history(), true, false)
            .unwrap();
    }
    let trained = state
        .bimode_branch_predictor
        .predict(RISCV_LOCAL_BIMODE_THREAD, pc)
        .unwrap();
    assert!(trained.predicted_taken());
}

fn use_small_tournament_predictor(state: &mut RiscvCoreState) {
    state.tournament_branch_predictor = TournamentBranchPredictor::new(
        TournamentBranchPredictorConfig::new(1, 2, 2, 2, 2).unwrap(),
    );
}

fn use_local_bias_multiperspective_perceptron(state: &mut RiscvCoreState) {
    state.multiperspective_perceptron = MultiperspectivePerceptron::new(
        MultiperspectivePerceptronConfig::with_options(
            1,
            0,
            1,
            1,
            16,
            -4,
            1,
            -5,
            5,
            -1,
            1,
            1,
            4,
            -2,
            0,
            0,
            0,
            64,
            2,
            2,
            0,
            0xff,
            false,
            true,
            0,
            4,
            3,
            128,
            1,
            false,
            vec![MultiperspectivePerceptronFeature::bias(64, 1, 6)],
        )
        .unwrap(),
    )
    .unwrap();
}

fn train_selected_tournament_local_history_one_taken(state: &mut RiscvCoreState, pc: Address) {
    let history_seed = state
        .tournament_branch_predictor
        .predict(RISCV_LOCAL_TOURNAMENT_THREAD, pc)
        .unwrap();
    state
        .tournament_branch_predictor
        .update_history(history_seed.history(), true)
        .unwrap();
    for _ in 0..2 {
        let prediction = state
            .tournament_branch_predictor
            .predict(RISCV_LOCAL_TOURNAMENT_THREAD, pc)
            .unwrap();
        assert_eq!(prediction.local_history_before(), 1);
        assert_eq!(prediction.local_predictor_index(), 1);
        state
            .tournament_branch_predictor
            .train(prediction.history(), true, false)
            .unwrap();
    }
    state
        .tournament_branch_predictor
        .squash(history_seed.history())
        .unwrap();
}

fn train_selected_tournament_global_history_one_taken(
    state: &mut RiscvCoreState,
    training_pc: Address,
) {
    let history_seed = state
        .tournament_branch_predictor
        .predict(RISCV_LOCAL_TOURNAMENT_THREAD, training_pc)
        .unwrap();
    state
        .tournament_branch_predictor
        .update_history(history_seed.history(), true)
        .unwrap();
    for _ in 0..2 {
        let prediction = state
            .tournament_branch_predictor
            .predict_unconditional(RISCV_LOCAL_TOURNAMENT_THREAD, Address::new(0xa000))
            .unwrap();
        assert_eq!(prediction.global_history_before(), 1);
        state
            .tournament_branch_predictor
            .train(prediction.history(), true, false)
            .unwrap();
    }
    for _ in 0..2 {
        let prediction = state
            .tournament_branch_predictor
            .predict(RISCV_LOCAL_TOURNAMENT_THREAD, training_pc)
            .unwrap();
        assert_eq!(prediction.global_history_before(), 1);
        assert_eq!(prediction.local_history_before(), 1);
        assert!(!prediction.local_predicted_taken());
        assert!(prediction.global_predicted_taken());
        state
            .tournament_branch_predictor
            .train(prediction.history(), true, false)
            .unwrap();
    }
    state
        .tournament_branch_predictor
        .squash(history_seed.history())
        .unwrap();
}

fn insert_pending_branch_speculation(
    state: &mut RiscvCoreState,
    sequence: u64,
    pc: Address,
    target: Address,
) {
    let speculation =
        state
            .branch_predictor
            .predict_speculative_with_prediction(pc, true, Some(target));
    state.branch_speculations.insert(sequence, speculation.id());
}

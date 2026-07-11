use super::*;
use crate::{
    BranchPredictor, BranchPredictorCheckpointPayload, BranchPredictorConfig, BranchTargetBuffer,
    BranchTargetBufferConfig, BranchTargetProvider, CpuCore, CpuFetchConfig, CpuFetchRecord, CpuId,
    CpuResetState, CpuTranslationFrontend, InOrderPipelineError, InOrderPipelineInstruction,
    InOrderPipelineSnapshot, InOrderPipelineStage, MultiperspectivePerceptron,
    MultiperspectivePerceptronConfig, MultiperspectivePerceptronFeature, OutstandingFetch,
    RiscvBranchPredictorKind, RiscvCpuExecutionEvent, TournamentBranchPredictor,
    TournamentBranchPredictorConfig, DEFAULT_RISCV_BRANCH_PREDICTOR_ENTRIES,
    RISCV_LOCAL_BIMODE_THREAD, RISCV_LOCAL_GSHARE_THREAD,
    RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD, RISCV_LOCAL_TOURNAMENT_THREAD,
};
use rem6_isa_riscv::{
    Immediate, MemoryAccessKind, MemoryWidth, Register, RiscvExecutionRecord, RiscvPmaRange,
};
use rem6_kernel::PartitionId;
use rem6_memory::{
    AccessSize, AgentId, CacheLineLayout, MemoryRequestId, TranslationQueueConfig,
    TranslationTlbConfig,
};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

mod btb;
mod checkpoint;
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
    let core = RiscvCore::new(
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
fn detailed_scalar_load_does_not_refetch_completed_younger_with_branch_lookahead() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let addi = i_type(7, 0, 0x0, 6, 0x13);
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, addi.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(3);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
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
            .stage_live_scalar_memory_issue(&older, request(20), 31));
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
            .stage_live_scalar_memory_issue(&store, request(20), 31));
    }

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
}

#[test]
fn translated_scalar_load_window_does_not_fetch_fourth() {
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
    core.state.lock().expect("riscv core lock").data_translation =
        Some(CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ));

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
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
fn detailed_scalar_load_window_does_not_fetch_beyond_completed_branch() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let branch = b_type(8, 1, 2, 0x0);
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

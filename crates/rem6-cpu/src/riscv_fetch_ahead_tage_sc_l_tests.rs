use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, AgentId, CacheLineLayout, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

use crate::{
    CpuCore, CpuFetchConfig, CpuFetchRecord, CpuId, CpuResetState, LTageBranchPredictorConfig,
    LoopBranchPredictorConfig, RiscvBranchPredictorKind, RiscvCore, RiscvCoreState,
    StatisticalCorrectorBranchKind, StatisticalCorrectorConfig, TageBranchPredictorConfig,
    TageScLBranchPredictor, TageScLBranchPredictorConfig, RISCV_LOCAL_TAGE_SC_L_THREAD,
};
use rem6_memory::Address;

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
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
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    );
    let mut core_state = core.core.state.lock().expect("cpu core lock");
    for (sequence, pc, data) in fetches {
        core_state.events.push(crate::CpuFetchEvent::completed(
            CpuFetchRecord::new(
                4,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                endpoint("cpu0.ifetch"),
                request(sequence),
                Address::new(pc),
                AccessSize::new(4).unwrap(),
            ),
            data,
        ));
    }
    drop(core_state);
    core
}

fn tage_config() -> TageBranchPredictorConfig {
    TageBranchPredictorConfig::with_options(
        1,
        2,
        2,
        6,
        vec![0, 4, 5],
        vec![4, 3, 3],
        1,
        3,
        2,
        8,
        4,
        1,
        4,
        1,
        2,
        false,
        false,
    )
    .unwrap()
}

fn loop_config() -> LoopBranchPredictorConfig {
    LoopBranchPredictorConfig::with_options(
        1, 3, 1, 3, 2, 4, 4, 3, 2, false, false, false, false, 1, 3, true,
    )
    .unwrap()
}

fn use_small_tage_sc_l_predictor(state: &mut RiscvCoreState) {
    state.tage_sc_l_branch_predictor = TageScLBranchPredictor::new(
        TageScLBranchPredictorConfig::new(
            LTageBranchPredictorConfig::new(tage_config(), loop_config()).unwrap(),
            StatisticalCorrectorConfig::tage_sc_l_8kb(1, 2, false).unwrap(),
        )
        .unwrap(),
    );
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

fn train_selected_tage_sc_l_taken_after_pending_history(
    state: &mut RiscvCoreState,
    older_pc: Address,
    younger_pc: Address,
    older_target: Address,
) {
    train_selected_tage_sc_l_taken_after_history_update(
        state,
        older_pc,
        younger_pc,
        older_target,
        true,
        StatisticalCorrectorBranchKind::DirectConditional,
    );
}

fn train_selected_tage_sc_l_taken_after_direct_jump_history(
    state: &mut RiscvCoreState,
    older_pc: Address,
    younger_pc: Address,
    older_target: Address,
) {
    train_selected_tage_sc_l_taken_after_history_update(
        state,
        older_pc,
        younger_pc,
        older_target,
        false,
        StatisticalCorrectorBranchKind::DirectUnconditional,
    );
}

fn train_selected_tage_sc_l_taken_after_history_update(
    state: &mut RiscvCoreState,
    older_pc: Address,
    younger_pc: Address,
    older_target: Address,
    older_conditional: bool,
    older_kind: StatisticalCorrectorBranchKind,
) {
    let committed = state
        .tage_sc_l_branch_predictor
        .predict(RISCV_LOCAL_TAGE_SC_L_THREAD, younger_pc, true)
        .unwrap();
    let mut speculative = state.tage_sc_l_branch_predictor.clone();
    let older = speculative
        .predict(RISCV_LOCAL_TAGE_SC_L_THREAD, older_pc, older_conditional)
        .unwrap();
    speculative
        .update_history(older.history(), true, older_kind, older_target)
        .unwrap();
    let younger = speculative
        .predict(RISCV_LOCAL_TAGE_SC_L_THREAD, younger_pc, true)
        .unwrap();
    let committed_tage = committed.ltage_prediction().tage_prediction();
    let speculative_tage = younger.ltage_prediction().tage_prediction();
    let bank = (1..speculative_tage.tagged_indices().len())
        .rev()
        .find(|bank| {
            committed_tage.tagged_indices()[*bank] != speculative_tage.tagged_indices()[*bank]
                || committed_tage.tagged_tags()[*bank] != speculative_tage.tagged_tags()[*bank]
        })
        .expect("pending TAGE-SC-L history changes at least one tagged lookup");

    state
        .tage_sc_l_branch_predictor
        .ltage_mut()
        .tage_mut()
        .write_tagged_entry(
            bank,
            speculative_tage.tagged_indices()[bank],
            speculative_tage.tagged_tags()[bank],
            2,
            1,
        )
        .unwrap();

    let base = state
        .tage_sc_l_branch_predictor
        .predict(RISCV_LOCAL_TAGE_SC_L_THREAD, younger_pc, true)
        .unwrap();
    assert!(!base.predicted_taken());
    let mut overlay = state.tage_sc_l_branch_predictor.clone();
    let older = overlay
        .predict(RISCV_LOCAL_TAGE_SC_L_THREAD, older_pc, older_conditional)
        .unwrap();
    overlay
        .update_history(older.history(), true, older_kind, older_target)
        .unwrap();
    let overlay_prediction = overlay
        .predict(RISCV_LOCAL_TAGE_SC_L_THREAD, younger_pc, true)
        .unwrap();
    assert!(overlay_prediction.predicted_taken());
}

#[test]
fn selected_tage_sc_l_fetch_ahead_uses_pending_history_for_younger_branch() {
    let core = core_with_completed_fetches([
        (0, 0x8000, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
        (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
    ]);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::TageScL);
    core.set_branch_lookahead(2);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        use_small_tage_sc_l_predictor(&mut state);
        let older_pc = Address::new(0x8000);
        let younger_pc = Address::new(0x8008);
        let older_target = Address::new(0x8008);
        train_selected_tage_sc_l_taken_after_pending_history(
            &mut state,
            older_pc,
            younger_pc,
            older_target,
        );
        insert_pending_branch_speculation(&mut state, 0, older_pc, older_target);
    }

    let second = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(second.pc(), Address::new(0x8010));
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state
            .tage_sc_l_branch_predictor
            .snapshot()
            .history_update_count(),
        0
    );
}

#[test]
fn selected_tage_sc_l_fetch_ahead_uses_pending_direct_jump_history_for_younger_branch() {
    let core = core_with_completed_fetches([
        (0, 0x8000, j_type(8, 0).to_le_bytes().to_vec()),
        (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
    ]);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::TageScL);
    core.set_branch_lookahead(2);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        use_small_tage_sc_l_predictor(&mut state);
        let older_pc = Address::new(0x8000);
        let younger_pc = Address::new(0x8008);
        let older_target = Address::new(0x8008);
        train_selected_tage_sc_l_taken_after_direct_jump_history(
            &mut state,
            older_pc,
            younger_pc,
            older_target,
        );
        insert_pending_branch_speculation(&mut state, 0, older_pc, older_target);
    }

    let second = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(second.pc(), Address::new(0x8010));
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state
            .tage_sc_l_branch_predictor
            .snapshot()
            .history_update_count(),
        0
    );
}

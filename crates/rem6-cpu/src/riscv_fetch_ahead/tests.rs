use super::*;
use crate::{
    BranchPredictor, BranchPredictorCheckpointPayload, BranchPredictorConfig, BranchTargetBuffer,
    BranchTargetBufferConfig, BranchTargetProvider, CpuCore, CpuFetchConfig, CpuFetchRecord, CpuId,
    CpuResetState, InOrderPipelineError, InOrderPipelineInstruction, InOrderPipelineSnapshot,
    InOrderPipelineStage, MultiperspectivePerceptron, MultiperspectivePerceptronConfig,
    MultiperspectivePerceptronFeature, OutstandingFetch, RiscvBranchPredictorKind,
    TournamentBranchPredictor, TournamentBranchPredictorConfig,
    DEFAULT_RISCV_BRANCH_PREDICTOR_ENTRIES, RISCV_LOCAL_BIMODE_THREAD, RISCV_LOCAL_GSHARE_THREAD,
    RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD, RISCV_LOCAL_TOURNAMENT_THREAD,
};
use rem6_isa_riscv::Register;
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, AgentId, CacheLineLayout, MemoryRequestId};
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

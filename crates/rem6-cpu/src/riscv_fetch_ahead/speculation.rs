use rem6_isa_riscv::RiscvInstruction;
use rem6_memory::Address;

use crate::{
    BranchTargetProvider, CpuFetchEvent, RiscvCoreState, StatisticalCorrectorBranchKind,
    RISCV_LOCAL_TAGE_SC_L_THREAD,
};

use super::{
    completed_fetch_instruction, conditional_branch_target, instruction_is_conditional_branch,
    RiscvFetchAheadBranchPrediction,
};

pub(super) fn selected_tage_sc_l_branch_prediction(
    state: &RiscvCoreState,
    completed_fetches: &[&CpuFetchEvent],
    fetch_pc: Address,
    instruction: RiscvInstruction,
) -> Option<RiscvFetchAheadBranchPrediction> {
    let mut predictor = state.tage_sc_l_branch_predictor.clone();
    for (sequence, speculation) in &state.branch_speculations {
        let pending = state.branch_predictor.pending_speculation(*speculation)?;
        let pending_instruction = completed_fetch_instruction(completed_fetches, *sequence)?;
        let conditional = instruction_is_conditional_branch(pending_instruction);
        let prediction = predictor
            .predict(RISCV_LOCAL_TAGE_SC_L_THREAD, pending.pc(), conditional)
            .ok()?;
        let target =
            pending_tage_sc_l_history_target(pending.pc(), pending.target(), pending_instruction)?;
        predictor
            .update_history(
                prediction.history(),
                pending.predicted_taken(),
                statistical_corrector_branch_kind(pending_instruction),
                target,
            )
            .ok()?;
    }

    let prediction = predictor
        .predict(RISCV_LOCAL_TAGE_SC_L_THREAD, fetch_pc, true)
        .ok()?;
    let target = prediction
        .predicted_taken()
        .then(|| conditional_branch_target(fetch_pc, instruction))
        .flatten();
    Some(RiscvFetchAheadBranchPrediction {
        predicted_taken: prediction.predicted_taken(),
        target,
        branch_target_prediction: None,
        target_provider: BranchTargetProvider::NoTarget,
    })
}

fn pending_tage_sc_l_history_target(
    fetch_pc: Address,
    recorded_target: Option<Address>,
    instruction: RiscvInstruction,
) -> Option<Address> {
    if instruction_is_conditional_branch(instruction) {
        conditional_branch_target(fetch_pc, instruction).or(recorded_target)
    } else {
        recorded_target
    }
}

const fn statistical_corrector_branch_kind(
    instruction: RiscvInstruction,
) -> StatisticalCorrectorBranchKind {
    match instruction {
        RiscvInstruction::Jal { .. } => StatisticalCorrectorBranchKind::DirectUnconditional,
        RiscvInstruction::Jalr { .. } => StatisticalCorrectorBranchKind::IndirectUnconditional,
        _ => StatisticalCorrectorBranchKind::DirectConditional,
    }
}

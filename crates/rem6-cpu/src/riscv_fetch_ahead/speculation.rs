use rem6_isa_riscv::RiscvInstruction;
use rem6_memory::Address;

use crate::{
    BranchTargetProvider, CpuFetchEvent, RiscvCoreState, RiscvSelectedBranchSpeculation,
    StatisticalCorrectorBranchKind, RISCV_LOCAL_TAGE_SC_L_THREAD,
};

use super::{
    conditional_branch_target, selected_tage_sc_l_speculative_predictor,
    RiscvFetchAheadBranchPrediction,
};

pub(super) fn selected_tage_sc_l_branch_prediction(
    state: &RiscvCoreState,
    completed_fetches: &[&CpuFetchEvent],
    fetch_pc: Address,
    instruction: RiscvInstruction,
) -> Option<RiscvFetchAheadBranchPrediction> {
    let mut predictor = selected_tage_sc_l_speculative_predictor(state, completed_fetches)?;
    let prediction = predictor
        .predict(RISCV_LOCAL_TAGE_SC_L_THREAD, fetch_pc, true)
        .ok()?;
    let target = prediction
        .predicted_taken()
        .then(|| conditional_branch_target(fetch_pc, instruction))
        .flatten();
    let selected_speculation = conditional_branch_target(fetch_pc, instruction).map(|target| {
        RiscvSelectedBranchSpeculation::TageScL {
            prediction: prediction.clone(),
            kind: StatisticalCorrectorBranchKind::DirectConditional,
            target,
            snapshot_before_update: None,
        }
    });
    Some(RiscvFetchAheadBranchPrediction {
        predicted_taken: prediction.predicted_taken(),
        target,
        selected_speculation,
        branch_target_prediction: None,
        target_provider: BranchTargetProvider::NoTarget,
    })
}

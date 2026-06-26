use crate::{
    default_riscv_tage_sc_l_branch_predictor, RiscvCore, TageScLBranchPredictorCheckpointPayload,
    TageScLBranchPredictorError,
};

impl RiscvCore {
    pub fn tage_sc_l_branch_predictor_checkpoint_payload(
        &self,
    ) -> TageScLBranchPredictorCheckpointPayload {
        let state = self.state.lock().expect("riscv core lock");
        TageScLBranchPredictorCheckpointPayload::from_snapshot(
            state.committed_tage_sc_l_branch_predictor_snapshot(),
        )
        .expect("captured RISC-V TAGE-SC-L branch predictor checkpoint is internally consistent")
    }

    pub fn default_tage_sc_l_branch_predictor_checkpoint_payload(
    ) -> TageScLBranchPredictorCheckpointPayload {
        TageScLBranchPredictorCheckpointPayload::from_snapshot(
            default_riscv_tage_sc_l_branch_predictor().snapshot(),
        )
        .expect("default RISC-V TAGE-SC-L branch predictor checkpoint is valid")
    }

    pub fn restore_tage_sc_l_branch_predictor_checkpoint_payload(
        &self,
        payload: TageScLBranchPredictorCheckpointPayload,
    ) -> Result<(), TageScLBranchPredictorError> {
        let snapshot = payload.into_snapshot();
        let mut state = self.state.lock().expect("riscv core lock");
        let mut restored = state.tage_sc_l_branch_predictor.clone();
        restored.restore(&snapshot)?;
        state.tage_sc_l_branch_predictor = restored;
        state.forget_tage_sc_l_selected_branch_speculations();
        Ok(())
    }

    pub fn validate_tage_sc_l_branch_predictor_checkpoint_payload(
        &self,
        payload: &TageScLBranchPredictorCheckpointPayload,
    ) -> Result<(), TageScLBranchPredictorError> {
        let state = self.state.lock().expect("riscv core lock");
        let mut predictor = state.tage_sc_l_branch_predictor.clone();
        predictor.restore(payload.snapshot())
    }
}

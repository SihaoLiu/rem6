use crate::{
    GShareBranchPredictor, GShareBranchPredictorCheckpointPayload, GShareBranchPredictorConfig,
    GShareBranchPredictorError, RiscvCore, DEFAULT_RISCV_GSHARE_BRANCH_PREDICTOR_ENTRIES,
};

impl RiscvCore {
    pub fn gshare_branch_predictor_checkpoint_payload(
        &self,
    ) -> GShareBranchPredictorCheckpointPayload {
        let state = self.state.lock().expect("riscv core lock");
        GShareBranchPredictorCheckpointPayload::from_snapshot(
            state.committed_gshare_branch_predictor_snapshot(),
        )
        .expect("captured RISC-V gshare branch predictor checkpoint is internally consistent")
    }

    pub fn default_gshare_branch_predictor_checkpoint_payload(
    ) -> GShareBranchPredictorCheckpointPayload {
        GShareBranchPredictorCheckpointPayload::from_snapshot(
            GShareBranchPredictor::new(
                GShareBranchPredictorConfig::new(1, DEFAULT_RISCV_GSHARE_BRANCH_PREDICTOR_ENTRIES)
                    .expect("default RISC-V gshare branch predictor config is valid"),
            )
            .snapshot(),
        )
        .expect("default RISC-V gshare branch predictor checkpoint is valid")
    }

    pub fn restore_gshare_branch_predictor_checkpoint_payload(
        &self,
        payload: GShareBranchPredictorCheckpointPayload,
    ) -> Result<(), GShareBranchPredictorError> {
        let snapshot = payload.into_snapshot();
        let mut state = self.state.lock().expect("riscv core lock");
        let mut restored = state.gshare_branch_predictor.clone();
        restored.restore(&snapshot)?;
        state.gshare_branch_predictor = restored;
        state.forget_gshare_selected_branch_speculations();
        Ok(())
    }

    pub fn validate_gshare_branch_predictor_checkpoint_payload(
        &self,
        payload: &GShareBranchPredictorCheckpointPayload,
    ) -> Result<(), GShareBranchPredictorError> {
        let state = self.state.lock().expect("riscv core lock");
        let mut gshare = state.gshare_branch_predictor.clone();
        gshare.restore(payload.snapshot())
    }
}

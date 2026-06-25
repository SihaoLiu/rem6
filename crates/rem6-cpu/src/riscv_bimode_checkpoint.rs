use crate::{
    BiModeBranchPredictor, BiModeBranchPredictorCheckpointPayload, BiModeBranchPredictorConfig,
    BiModeBranchPredictorError, RiscvCore, DEFAULT_RISCV_BIMODE_CHOICE_ENTRIES,
    DEFAULT_RISCV_BIMODE_GLOBAL_ENTRIES,
};

impl RiscvCore {
    pub fn bimode_branch_predictor_checkpoint_payload(
        &self,
    ) -> BiModeBranchPredictorCheckpointPayload {
        let state = self.state.lock().expect("riscv core lock");
        BiModeBranchPredictorCheckpointPayload::from_snapshot(
            state.bimode_branch_predictor.snapshot(),
        )
        .expect("captured RISC-V bimode branch predictor checkpoint is internally consistent")
    }

    pub fn default_bimode_branch_predictor_checkpoint_payload(
    ) -> BiModeBranchPredictorCheckpointPayload {
        BiModeBranchPredictorCheckpointPayload::from_snapshot(
            BiModeBranchPredictor::new(
                BiModeBranchPredictorConfig::new(
                    1,
                    DEFAULT_RISCV_BIMODE_CHOICE_ENTRIES,
                    DEFAULT_RISCV_BIMODE_GLOBAL_ENTRIES,
                )
                .expect("default RISC-V bimode branch predictor config is valid"),
            )
            .snapshot(),
        )
        .expect("default RISC-V bimode branch predictor checkpoint is valid")
    }

    pub fn restore_bimode_branch_predictor_checkpoint_payload(
        &self,
        payload: BiModeBranchPredictorCheckpointPayload,
    ) -> Result<(), BiModeBranchPredictorError> {
        let snapshot = payload.into_snapshot();
        let mut state = self.state.lock().expect("riscv core lock");
        let mut restored = state.bimode_branch_predictor.clone();
        restored.restore(&snapshot)?;
        state.bimode_branch_predictor = restored;
        Ok(())
    }

    pub fn validate_bimode_branch_predictor_checkpoint_payload(
        &self,
        payload: &BiModeBranchPredictorCheckpointPayload,
    ) -> Result<(), BiModeBranchPredictorError> {
        let state = self.state.lock().expect("riscv core lock");
        let mut bimode = state.bimode_branch_predictor.clone();
        bimode.restore(payload.snapshot())
    }
}

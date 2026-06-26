use crate::{
    default_riscv_multiperspective_perceptron, MultiperspectivePerceptronCheckpointPayload,
    MultiperspectivePerceptronError, RiscvCore,
};

impl RiscvCore {
    pub fn multiperspective_perceptron_checkpoint_payload(
        &self,
    ) -> MultiperspectivePerceptronCheckpointPayload {
        let state = self.state.lock().expect("riscv core lock");
        MultiperspectivePerceptronCheckpointPayload::from_snapshot(
            state.committed_multiperspective_perceptron_snapshot(),
        )
        .expect("captured RISC-V multiperspective perceptron checkpoint is internally consistent")
    }

    pub fn default_multiperspective_perceptron_checkpoint_payload(
    ) -> MultiperspectivePerceptronCheckpointPayload {
        MultiperspectivePerceptronCheckpointPayload::from_snapshot(
            default_riscv_multiperspective_perceptron().snapshot(),
        )
        .expect("default RISC-V multiperspective perceptron checkpoint is valid")
    }

    pub fn restore_multiperspective_perceptron_checkpoint_payload(
        &self,
        payload: MultiperspectivePerceptronCheckpointPayload,
    ) -> Result<(), MultiperspectivePerceptronError> {
        let snapshot = payload.into_snapshot();
        let mut state = self.state.lock().expect("riscv core lock");
        let mut restored = state.multiperspective_perceptron.clone();
        restored.restore(&snapshot)?;
        state.multiperspective_perceptron = restored;
        state.forget_multiperspective_selected_branch_speculations();
        Ok(())
    }

    pub fn validate_multiperspective_perceptron_checkpoint_payload(
        &self,
        payload: &MultiperspectivePerceptronCheckpointPayload,
    ) -> Result<(), MultiperspectivePerceptronError> {
        let state = self.state.lock().expect("riscv core lock");
        let mut predictor = state.multiperspective_perceptron.clone();
        predictor.restore(payload.snapshot())
    }
}

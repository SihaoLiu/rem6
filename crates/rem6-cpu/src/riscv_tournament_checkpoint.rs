use crate::{
    RiscvCore, TournamentBranchPredictor, TournamentBranchPredictorCheckpointPayload,
    TournamentBranchPredictorConfig, TournamentBranchPredictorError,
    DEFAULT_RISCV_TOURNAMENT_CHOICE_ENTRIES, DEFAULT_RISCV_TOURNAMENT_GLOBAL_ENTRIES,
    DEFAULT_RISCV_TOURNAMENT_LOCAL_ENTRIES, DEFAULT_RISCV_TOURNAMENT_LOCAL_HISTORY_ENTRIES,
};

impl RiscvCore {
    pub fn tournament_branch_predictor_checkpoint_payload(
        &self,
    ) -> TournamentBranchPredictorCheckpointPayload {
        let state = self.state.lock().expect("riscv core lock");
        TournamentBranchPredictorCheckpointPayload::from_snapshot(
            state.tournament_branch_predictor.snapshot(),
        )
        .expect("captured RISC-V tournament branch predictor checkpoint is internally consistent")
    }

    pub fn default_tournament_branch_predictor_checkpoint_payload(
    ) -> TournamentBranchPredictorCheckpointPayload {
        TournamentBranchPredictorCheckpointPayload::from_snapshot(
            TournamentBranchPredictor::new(
                TournamentBranchPredictorConfig::new(
                    1,
                    DEFAULT_RISCV_TOURNAMENT_LOCAL_ENTRIES,
                    DEFAULT_RISCV_TOURNAMENT_LOCAL_HISTORY_ENTRIES,
                    DEFAULT_RISCV_TOURNAMENT_GLOBAL_ENTRIES,
                    DEFAULT_RISCV_TOURNAMENT_CHOICE_ENTRIES,
                )
                .expect("default RISC-V tournament branch predictor config is valid"),
            )
            .snapshot(),
        )
        .expect("default RISC-V tournament branch predictor checkpoint is valid")
    }

    pub fn restore_tournament_branch_predictor_checkpoint_payload(
        &self,
        payload: TournamentBranchPredictorCheckpointPayload,
    ) -> Result<(), TournamentBranchPredictorError> {
        let snapshot = payload.into_snapshot();
        let mut state = self.state.lock().expect("riscv core lock");
        let mut restored = state.tournament_branch_predictor.clone();
        restored.restore(&snapshot)?;
        state.tournament_branch_predictor = restored;
        Ok(())
    }

    pub fn validate_tournament_branch_predictor_checkpoint_payload(
        &self,
        payload: &TournamentBranchPredictorCheckpointPayload,
    ) -> Result<(), TournamentBranchPredictorError> {
        let state = self.state.lock().expect("riscv core lock");
        let mut tournament = state.tournament_branch_predictor.clone();
        tournament.restore(payload.snapshot())
    }
}

use std::collections::BTreeSet;

use crate::{
    BiModeBranchPredictorSnapshot, BiModeHistoryUpdate, BiModePrediction, BiModeThreadSnapshot,
    GShareBranchPredictorSnapshot, GShareHistoryUpdate, GSharePrediction, GShareThreadSnapshot,
    RiscvCoreState, RiscvCpuError, TournamentBranchPredictorSnapshot, TournamentHistoryUpdate,
    TournamentPrediction, TournamentThreadSnapshot,
};

impl RiscvCoreState {
    pub(crate) fn rollback_all_selected_branch_speculations(
        &mut self,
    ) -> Result<(), RiscvCpuError> {
        let sequences = self
            .selected_branch_speculations
            .keys()
            .rev()
            .copied()
            .collect::<Vec<_>>();
        for sequence in sequences {
            self.rollback_selected_branch_speculation(sequence)?;
        }
        Ok(())
    }

    pub(crate) fn rollback_inactive_selected_branch_speculations(
        &mut self,
        active_sequences: &BTreeSet<u64>,
    ) -> Result<(), RiscvCpuError> {
        let sequences = self
            .selected_branch_speculations
            .keys()
            .rev()
            .filter(|sequence| !active_sequences.contains(sequence))
            .copied()
            .collect::<Vec<_>>();
        for sequence in sequences {
            self.rollback_selected_branch_speculation(sequence)?;
        }
        Ok(())
    }

    pub(crate) fn rollback_selected_branch_speculation(
        &mut self,
        sequence: u64,
    ) -> Result<(), RiscvCpuError> {
        let Some(speculation) = self.selected_branch_speculations.remove(&sequence) else {
            return Ok(());
        };
        match speculation {
            RiscvSelectedBranchSpeculation::GShare { prediction, .. } => {
                self.gshare_branch_predictor
                    .squash(prediction.history())
                    .map_err(RiscvCpuError::GShareBranchPredictor)?;
            }
            RiscvSelectedBranchSpeculation::BiMode { prediction, .. } => {
                self.bimode_branch_predictor
                    .squash(prediction.history())
                    .map_err(RiscvCpuError::BiModeBranchPredictor)?;
            }
            RiscvSelectedBranchSpeculation::Tournament { prediction, .. } => {
                self.tournament_branch_predictor
                    .squash(prediction.history())
                    .map_err(RiscvCpuError::TournamentBranchPredictor)?;
            }
        }
        Ok(())
    }

    pub(crate) fn committed_gshare_branch_predictor_snapshot(
        &self,
    ) -> GShareBranchPredictorSnapshot {
        let snapshot = self.gshare_branch_predictor.snapshot();
        let mut threads = snapshot.threads().to_vec();
        for speculation in self.selected_branch_speculations.values().rev() {
            if let RiscvSelectedBranchSpeculation::GShare { prediction, .. } = speculation {
                let history = prediction.history();
                threads[history.cpu().get() as usize] =
                    GShareThreadSnapshot::from_global_history(history.global_history_before());
            }
        }
        GShareBranchPredictorSnapshot::from_parts(
            snapshot.config().clone(),
            snapshot.counters().to_vec(),
            threads,
            snapshot.lookup_count(),
            snapshot.history_update_count(),
            snapshot.update_count(),
            snapshot.squash_count(),
        )
    }

    pub(crate) fn committed_bimode_branch_predictor_snapshot(
        &self,
    ) -> BiModeBranchPredictorSnapshot {
        let snapshot = self.bimode_branch_predictor.snapshot();
        let mut threads = snapshot.threads().to_vec();
        for speculation in self.selected_branch_speculations.values().rev() {
            if let RiscvSelectedBranchSpeculation::BiMode { prediction, .. } = speculation {
                let history = prediction.history();
                threads[history.cpu().get() as usize] =
                    BiModeThreadSnapshot::from_global_history(history.global_history_before());
            }
        }
        BiModeBranchPredictorSnapshot::from_parts(
            snapshot.config().clone(),
            snapshot.choice_counters().to_vec(),
            snapshot.taken_counters().to_vec(),
            snapshot.not_taken_counters().to_vec(),
            threads,
            snapshot.lookup_count(),
            snapshot.history_update_count(),
            snapshot.update_count(),
            snapshot.squash_count(),
        )
    }

    pub(crate) fn committed_tournament_branch_predictor_snapshot(
        &self,
    ) -> TournamentBranchPredictorSnapshot {
        let snapshot = self.tournament_branch_predictor.snapshot();
        let mut local_history_table = snapshot.local_history_table().to_vec();
        let mut threads = snapshot.threads().to_vec();
        for speculation in self.selected_branch_speculations.values().rev() {
            if let RiscvSelectedBranchSpeculation::Tournament { prediction, .. } = speculation {
                let history = prediction.history();
                threads[history.cpu().get() as usize] =
                    TournamentThreadSnapshot::from_global_history(history.global_history_before());
                if history.local_history_valid() {
                    local_history_table[history.local_history_index()] =
                        history.local_history_before();
                }
            }
        }
        TournamentBranchPredictorSnapshot::from_parts(
            snapshot.config().clone(),
            snapshot.local_counters().to_vec(),
            local_history_table,
            snapshot.global_counters().to_vec(),
            snapshot.choice_counters().to_vec(),
            threads,
            snapshot.lookup_count(),
            snapshot.history_update_count(),
            snapshot.update_count(),
            snapshot.squash_count(),
        )
    }

    pub(crate) fn forget_gshare_selected_branch_speculations(&mut self) {
        self.selected_branch_speculations.retain(|_, speculation| {
            !matches!(speculation, RiscvSelectedBranchSpeculation::GShare { .. })
        });
    }

    pub(crate) fn forget_bimode_selected_branch_speculations(&mut self) {
        self.selected_branch_speculations.retain(|_, speculation| {
            !matches!(speculation, RiscvSelectedBranchSpeculation::BiMode { .. })
        });
    }

    pub(crate) fn forget_tournament_selected_branch_speculations(&mut self) {
        self.selected_branch_speculations.retain(|_, speculation| {
            !matches!(
                speculation,
                RiscvSelectedBranchSpeculation::Tournament { .. }
            )
        });
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RiscvSelectedBranchSpeculation {
    GShare {
        prediction: GSharePrediction,
        history_update: Option<GShareHistoryUpdate>,
    },
    BiMode {
        prediction: BiModePrediction,
        history_update: Option<BiModeHistoryUpdate>,
    },
    Tournament {
        prediction: TournamentPrediction,
        history_update: Option<TournamentHistoryUpdate>,
    },
}

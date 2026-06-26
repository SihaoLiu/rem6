use rem6_memory::Address;

use crate::BranchTargetPrediction;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RiscvBranchSpeculationSummary {
    predictions: u64,
    repairs: u64,
    removed_youngers: u64,
    max_pending: u64,
    btb_mispredictions: u64,
    predicted_taken_btb_misses: u64,
}

impl RiscvBranchSpeculationSummary {
    pub const fn predictions(self) -> u64 {
        self.predictions
    }

    pub const fn repairs(self) -> u64 {
        self.repairs
    }

    pub const fn removed_youngers(self) -> u64 {
        self.removed_youngers
    }

    pub const fn max_pending(self) -> u64 {
        self.max_pending
    }

    pub const fn btb_mispredictions(self) -> u64 {
        self.btb_mispredictions
    }

    pub const fn predicted_taken_btb_misses(self) -> u64 {
        self.predicted_taken_btb_misses
    }

    pub(crate) fn record_prediction(&mut self, pending: u64) {
        self.predictions = self.predictions.saturating_add(1);
        self.max_pending = self.max_pending.max(pending);
    }

    pub(crate) fn record_repair(&mut self, removed_youngers: u64) {
        self.repairs = self.repairs.saturating_add(1);
        self.removed_youngers = self.removed_youngers.saturating_add(removed_youngers);
    }

    pub(crate) fn record_btb_resolution(
        &mut self,
        predicted_taken: bool,
        actual_target: Option<Address>,
        branch_target_prediction: Option<BranchTargetPrediction>,
    ) {
        let Some(actual_target) = actual_target else {
            return;
        };
        let Some(branch_target_prediction) = branch_target_prediction else {
            return;
        };

        if !branch_target_prediction.hit()
            || branch_target_prediction.target() != Some(actual_target)
        {
            self.btb_mispredictions = self.btb_mispredictions.saturating_add(1);
        }
        if predicted_taken && !branch_target_prediction.hit() {
            self.predicted_taken_btb_misses = self.predicted_taken_btb_misses.saturating_add(1);
        }
    }
}

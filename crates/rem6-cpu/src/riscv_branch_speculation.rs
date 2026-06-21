#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RiscvBranchSpeculationSummary {
    predictions: u64,
    repairs: u64,
    removed_youngers: u64,
    max_pending: u64,
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

    pub(crate) fn record_prediction(&mut self, pending: u64) {
        self.predictions = self.predictions.saturating_add(1);
        self.max_pending = self.max_pending.max(pending);
    }

    pub(crate) fn record_repair(&mut self, removed_youngers: u64) {
        self.repairs = self.repairs.saturating_add(1);
        self.removed_youngers = self.removed_youngers.saturating_add(removed_youngers);
    }
}

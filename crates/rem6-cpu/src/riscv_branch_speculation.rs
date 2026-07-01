use rem6_memory::Address;

use crate::{
    BranchTargetKind, BranchTargetKindCounts, BranchTargetPrediction, BranchTargetProvider,
    BranchTargetProviderCounts, ReturnAddressStackOperationKind,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RiscvBranchSpeculationSummary {
    predictions: u64,
    repairs: u64,
    removed_youngers: u64,
    max_pending: u64,
    lookup_branch_kinds: BranchTargetKindCounts,
    squashed_branch_kinds: BranchTargetKindCounts,
    target_provider: BranchTargetProviderCounts,
    indirect_hits: u64,
    committed_branch_kinds: BranchTargetKindCounts,
    mispredicted_branch_kinds: BranchTargetKindCounts,
    corrected_branch_kinds: BranchTargetKindCounts,
    target_wrong_branch_kinds: BranchTargetKindCounts,
    btb_mispredictions: u64,
    predicted_taken_btb_misses: u64,
    btb_mispredict_due_to_btb_miss: BranchTargetKindCounts,
    mispredict_due_to_predictor: BranchTargetKindCounts,
    return_address_stack: RiscvReturnAddressStackStats,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RiscvReturnAddressStackStats {
    pushes: u64,
    pops: u64,
    squashes: u64,
    used: u64,
    correct: u64,
    incorrect: u64,
}

impl RiscvReturnAddressStackStats {
    pub const fn pushes(self) -> u64 {
        self.pushes
    }

    pub const fn pops(self) -> u64 {
        self.pops
    }

    pub const fn squashes(self) -> u64 {
        self.squashes
    }

    pub const fn used(self) -> u64 {
        self.used
    }

    pub const fn correct(self) -> u64 {
        self.correct
    }

    pub const fn incorrect(self) -> u64 {
        self.incorrect
    }

    fn record_operation(&mut self, kind: ReturnAddressStackOperationKind) {
        match kind {
            ReturnAddressStackOperationKind::Push => {
                self.pushes = self.pushes.saturating_add(1);
            }
            ReturnAddressStackOperationKind::Pop => {
                self.pops = self.pops.saturating_add(1);
            }
            ReturnAddressStackOperationKind::PopThenPush => {
                self.pops = self.pops.saturating_add(1);
                self.pushes = self.pushes.saturating_add(1);
            }
        }
    }

    fn record_squash(&mut self, kind: ReturnAddressStackOperationKind) {
        self.squashes = self.squashes.saturating_add(1);
        match kind {
            ReturnAddressStackOperationKind::Push => {
                self.pops = self.pops.saturating_add(1);
            }
            ReturnAddressStackOperationKind::Pop => {
                self.pushes = self.pushes.saturating_add(1);
            }
            ReturnAddressStackOperationKind::PopThenPush => {
                self.pops = self.pops.saturating_add(1);
                self.pushes = self.pushes.saturating_add(1);
            }
        }
    }

    fn record_commit(&mut self, kind: ReturnAddressStackOperationKind, predicted_correctly: bool) {
        if !matches!(
            kind,
            ReturnAddressStackOperationKind::Pop | ReturnAddressStackOperationKind::PopThenPush
        ) {
            return;
        }
        self.used = self.used.saturating_add(1);
        if predicted_correctly {
            self.correct = self.correct.saturating_add(1);
        } else {
            self.incorrect = self.incorrect.saturating_add(1);
        }
    }
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

    pub const fn lookup_branch_kinds(self) -> BranchTargetKindCounts {
        self.lookup_branch_kinds
    }

    pub const fn squashed_branch_kinds(self) -> BranchTargetKindCounts {
        self.squashed_branch_kinds
    }

    pub const fn target_provider(self) -> BranchTargetProviderCounts {
        self.target_provider
    }

    pub const fn indirect_hits(self) -> u64 {
        self.indirect_hits
    }

    pub const fn committed_branch_kinds(self) -> BranchTargetKindCounts {
        self.committed_branch_kinds
    }

    pub const fn mispredicted_branch_kinds(self) -> BranchTargetKindCounts {
        self.mispredicted_branch_kinds
    }

    pub const fn corrected_branch_kinds(self) -> BranchTargetKindCounts {
        self.corrected_branch_kinds
    }

    pub const fn target_wrong_branch_kinds(self) -> BranchTargetKindCounts {
        self.target_wrong_branch_kinds
    }

    pub const fn btb_mispredictions(self) -> u64 {
        self.btb_mispredictions
    }

    pub const fn predicted_taken_btb_misses(self) -> u64 {
        self.predicted_taken_btb_misses
    }

    pub const fn btb_mispredict_due_to_btb_miss(self) -> BranchTargetKindCounts {
        self.btb_mispredict_due_to_btb_miss
    }

    pub const fn mispredict_due_to_predictor(self) -> BranchTargetKindCounts {
        self.mispredict_due_to_predictor
    }

    pub const fn return_address_stack(self) -> RiscvReturnAddressStackStats {
        self.return_address_stack
    }

    pub(crate) fn record_return_address_stack_operation(
        &mut self,
        kind: ReturnAddressStackOperationKind,
    ) {
        self.return_address_stack.record_operation(kind);
    }

    pub(crate) fn record_return_address_stack_squash(
        &mut self,
        kind: ReturnAddressStackOperationKind,
    ) {
        self.return_address_stack.record_squash(kind);
    }

    pub(crate) fn record_return_address_stack_commit(
        &mut self,
        kind: ReturnAddressStackOperationKind,
        predicted_correctly: bool,
    ) {
        self.return_address_stack
            .record_commit(kind, predicted_correctly);
    }

    pub(crate) fn record_prediction(
        &mut self,
        branch_kind: BranchTargetKind,
        target_provider: BranchTargetProvider,
        pending: u64,
    ) {
        self.predictions = self.predictions.saturating_add(1);
        self.lookup_branch_kinds.increment(branch_kind);
        self.target_provider.increment(target_provider);
        let indirect_hit = target_provider == BranchTargetProvider::Indirect
            && branch_kind.is_indirect_non_return();
        if indirect_hit {
            self.indirect_hits = self.indirect_hits.saturating_add(1);
        }
        self.max_pending = self.max_pending.max(pending);
    }

    pub(crate) fn record_repair(&mut self, removed_youngers: u64) {
        self.repairs = self.repairs.saturating_add(1);
        self.removed_youngers = self.removed_youngers.saturating_add(removed_youngers);
    }

    pub(crate) fn record_squashed_branch_kind(&mut self, branch_kind: BranchTargetKind) {
        self.squashed_branch_kinds.increment(branch_kind);
    }

    pub(crate) fn record_btb_resolution(
        &mut self,
        branch_kind: BranchTargetKind,
        predicted_taken: bool,
        predicted_target: Option<Address>,
        actual_taken: bool,
        actual_target: Option<Address>,
        branch_target_prediction: Option<BranchTargetPrediction>,
    ) {
        let mispredicted = predicted_taken != actual_taken
            || (predicted_taken && predicted_target != actual_target);
        self.committed_branch_kinds.increment(branch_kind);
        if mispredicted {
            self.mispredicted_branch_kinds.increment(branch_kind);
            self.corrected_branch_kinds.increment(branch_kind);
            if predicted_target != actual_target {
                self.target_wrong_branch_kinds.increment(branch_kind);
            }
        }

        let Some(branch_target_prediction) = branch_target_prediction else {
            return;
        };

        if mispredicted {
            if actual_taken && !branch_target_prediction.hit() {
                self.btb_mispredict_due_to_btb_miss.increment(branch_kind);
            } else {
                self.mispredict_due_to_predictor.increment(branch_kind);
            }
        }

        let Some(actual_target) = actual_target else {
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

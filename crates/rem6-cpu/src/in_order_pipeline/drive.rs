use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InOrderPipelineRetirement {
    Any,
    None,
    Sequence(u64),
}

impl InOrderPipelineRetirement {
    pub(super) const fn allows(self, sequence: u64) -> bool {
        match self {
            Self::Any => true,
            Self::None => false,
            Self::Sequence(retire_sequence) => retire_sequence == sequence,
        }
    }
}

impl InOrderPipelineState {
    pub(crate) fn try_advance_cycle_recorded_without_retirement(
        &mut self,
    ) -> Result<InOrderPipelineCycleRecord, InOrderPipelineError> {
        let before = self.snapshot();
        let plan =
            self.advance_cycle_with_redirect_and_retirement(None, InOrderPipelineRetirement::None)?;
        let after = self.snapshot();

        Ok(InOrderPipelineCycleRecord {
            cycle: before.cycle(),
            stall_cycle_count: 0,
            stall_cause: None,
            before,
            plan,
            branch_predictions: Vec::new(),
            after,
        })
    }
}

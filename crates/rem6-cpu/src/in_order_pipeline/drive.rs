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
    pub(crate) fn configure_execute_wait(&mut self, sequence: u64, cycles: u64, key: u64) {
        let instruction = self
            .in_flight
            .iter_mut()
            .find(|instruction| instruction.sequence() == sequence)
            .expect("configured execute-wait instruction is in flight");
        debug_assert!(matches!(
            instruction.stage(),
            InOrderPipelineStage::Execute | InOrderPipelineStage::Commit
        ));
        if cycles > 0 {
            instruction.stage = InOrderPipelineStage::Execute;
            instruction.execute_wait_cycles = Some((cycles, cycles));
            instruction.execute_wait_key = Some(key);
        } else {
            instruction.execute_wait_cycles = None;
            instruction.execute_wait_key = None;
        }
    }

    pub(crate) fn execute_wait_completed(&self, sequence: u64) -> bool {
        self.in_flight.iter().any(|instruction| {
            instruction.sequence() == sequence
                && instruction.execute_wait_remaining_cycles() == Some(0)
        })
    }

    pub(crate) fn bind_execute_wait_key(&mut self, sequence: u64, key: u64) {
        let instruction = self
            .in_flight
            .iter_mut()
            .find(|instruction| instruction.sequence() == sequence)
            .expect("keyed execute-wait instruction is in flight");
        debug_assert!(instruction.execute_wait_cycles.is_some());
        instruction.execute_wait_key = Some(key);
    }

    pub(crate) fn try_record_execute_wait_cycle(
        &mut self,
        sequence: u64,
    ) -> Result<InOrderPipelineCycleRecord, InOrderPipelineError> {
        let before = self.snapshot();
        let plan = InOrderPipelinePlan::resource_stall(self.in_flight.iter().copied())?;
        debug_assert_eq!(
            plan.resource_blocked()
                .first()
                .map(|instruction| instruction.sequence()),
            Some(sequence)
        );
        let instruction = self
            .in_flight
            .iter_mut()
            .find(|instruction| instruction.sequence() == sequence)
            .expect("execute-wait instruction is in flight");
        let (_, remaining) = instruction
            .execute_wait_cycles
            .as_mut()
            .expect("execute-wait instruction has configured latency");
        debug_assert!(*remaining > 0);
        let next_cycle = next_cycle(self.cycle)?;
        *remaining -= 1;
        self.cycle = next_cycle;
        let after = self.snapshot();

        Ok(InOrderPipelineCycleRecord {
            cycle: before.cycle(),
            stall_cycle_count: 1,
            stall_cause: Some(InOrderPipelineStallCause::ExecuteWait),
            before,
            plan,
            branch_predictions: Vec::new(),
            after,
        })
    }

    pub(crate) fn rebind_execute_wait_sequence(&mut self, old_sequence: u64, new_sequence: u64) {
        if old_sequence != new_sequence {
            self.in_flight
                .retain(|instruction| instruction.sequence() != new_sequence);
        }
        let instruction = self
            .in_flight
            .iter_mut()
            .find(|instruction| instruction.sequence() == old_sequence)
            .expect("rebound execute-wait instruction is in flight");
        debug_assert!(instruction.execute_wait_cycles.is_some());
        instruction.sequence = new_sequence;
        self.in_flight
            .sort_by_key(|instruction| instruction.sequence());
    }

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

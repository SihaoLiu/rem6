use super::*;

impl SystemHostController {
    pub fn new(policy: HostEventPolicy, stats: StatsRegistry) -> Self {
        Self {
            run: SystemRunController::new(policy),
            executor: SystemActionExecutor::new(stats),
            action_errors: Vec::new(),
            consumed_stats_reset_outcomes: 0,
        }
    }

    pub const fn run(&self) -> &SystemRunController {
        &self.run
    }

    pub const fn run_mut(&mut self) -> &mut SystemRunController {
        &mut self.run
    }

    pub const fn executor(&self) -> &SystemActionExecutor {
        &self.executor
    }

    pub const fn executor_mut(&mut self) -> &mut SystemActionExecutor {
        &mut self.executor
    }

    pub fn handle_delivery(&mut self, delivery: GuestEventDelivery) -> Vec<SystemActionOutcome> {
        match self.run.execute_delivery(delivery, &mut self.executor) {
            Ok(outcomes) => outcomes,
            Err(error) => {
                self.action_errors.push(error);
                Vec::new()
            }
        }
    }

    pub(crate) fn handle_delivery_with_scheduler_checkpoint(
        &mut self,
        delivery: GuestEventDelivery,
        component: CheckpointComponentId,
        mut scheduler: rem6_kernel::SchedulerCheckpointAccess<'_>,
    ) -> Vec<SystemActionOutcome> {
        let records = self.run.handle_delivery(delivery);
        let mut outcomes = Vec::with_capacity(records.len());
        for record in &records {
            match self.executor.apply_with_scheduler_checkpoint(
                record,
                component.clone(),
                scheduler.reborrow(),
            ) {
                Ok(outcome) => outcomes.push(outcome),
                Err(error) => {
                    self.action_errors.push(error);
                    return Vec::new();
                }
            }
        }
        self.run.outcomes.extend(outcomes.iter().cloned());
        outcomes
    }

    pub fn consume_stats_reset_outcomes(&mut self) -> bool {
        let outcomes = self.run.action_outcomes();
        let first = self.consumed_stats_reset_outcomes.min(outcomes.len());
        let saw_stats_reset = outcomes[first..]
            .iter()
            .any(|outcome| matches!(outcome, SystemActionOutcome::StatsReset(_)));
        self.consumed_stats_reset_outcomes = outcomes.len();
        saw_stats_reset
    }

    pub fn action_errors(&self) -> &[SystemError] {
        &self.action_errors
    }
}

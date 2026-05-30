use crate::Tick;

use super::{
    ConservativeRunSummary, ParallelEpochPlan, ParallelRunProfile, PartitionedScheduler,
    ReadyPartition, RecordedConservativeRunSummary, RecordedRunSummary, RunSummary, SchedulerError,
};

impl PartitionedScheduler {
    pub fn run_next_epoch_parallel(&mut self) -> Result<RunSummary, SchedulerError> {
        self.run_next_epoch_parallel_recorded()
            .map(|recorded| recorded.summary)
    }

    pub fn run_next_epoch_parallel_recorded(
        &mut self,
    ) -> Result<RecordedRunSummary, SchedulerError> {
        let Some(plan) = self.plan_next_parallel_epoch()? else {
            return Ok(self.empty_recorded_parallel_summary());
        };
        self.run_parallel_epoch_plan_recorded(plan)
    }

    pub fn run_next_epoch_parallel_recorded_until(
        &mut self,
        tick_limit: Tick,
    ) -> Result<Option<(ParallelEpochPlan, RecordedRunSummary)>, SchedulerError> {
        let Some(plan) = self.plan_next_parallel_epoch_until(tick_limit)? else {
            return Ok(None);
        };
        let recorded = self.run_parallel_epoch_plan_recorded(plan.clone())?;
        Ok(Some((plan, recorded)))
    }

    fn empty_recorded_parallel_summary(&self) -> RecordedRunSummary {
        RecordedRunSummary {
            summary: RunSummary {
                executed_events: 0,
                final_tick: self.now,
            },
            initial_frontiers: Vec::new(),
            final_frontiers: Vec::new(),
            dispatches: Vec::new(),
            planned_parallel_worker_limit: self.max_parallel_workers,
            planned_batches: Vec::new(),
            batches: Vec::new(),
            profile: ParallelRunProfile::default(),
        }
    }

    fn run_parallel_epoch_plan_recorded(
        &mut self,
        plan: ParallelEpochPlan,
    ) -> Result<RecordedRunSummary, SchedulerError> {
        let horizon = plan.horizon();
        let initial_frontiers = plan.frontiers().to_vec();
        let planned_parallel_worker_limit = plan.parallel_worker_limit();
        let planned_batches = plan.parallel_batches().to_vec();

        if let Some(blocker) = plan.serial_blockers().first() {
            return Err(SchedulerError::SerialEventInParallelEpoch {
                partition: blocker.partition(),
                tick: blocker.tick(),
            });
        }

        let mut ready_partitions = plan
            .ready_partitions()
            .iter()
            .map(|ready| ready.partition)
            .collect::<Vec<_>>();

        let mut executed_events = 0;
        let mut dispatches = Vec::new();
        let mut batches = Vec::new();
        while !ready_partitions.is_empty() {
            let batch = self.run_parallel_batch(
                horizon,
                ready_partitions
                    .iter()
                    .take(self.max_parallel_workers)
                    .copied()
                    .collect(),
            )?;
            executed_events += batch.executed_events;
            dispatches.extend(batch.dispatches);
            batches.push(batch.record);
            self.merge_remote_parallel_events(batch.remote_events)?;

            if let Some((partition, tick)) = self.first_serial_event_at_or_before(horizon) {
                return Err(SchedulerError::SerialEventInParallelEpoch { partition, tick });
            }

            ready_partitions = self.ready_partitions_at_or_before(horizon);
        }

        self.advance_partitions_to(horizon);
        let final_frontiers = self.parallel_epoch_frontiers()?;
        dispatches.sort_by_key(|record| (record.tick(), record.partition(), record.id().local()));
        let profile = ParallelRunProfile::for_epoch(&batches, dispatches.len(), batches.is_empty());

        Ok(RecordedRunSummary {
            summary: RunSummary {
                executed_events,
                final_tick: self.now,
            },
            initial_frontiers,
            final_frontiers,
            dispatches,
            planned_parallel_worker_limit,
            planned_batches,
            batches,
            profile,
        })
    }

    pub fn run_until_idle_parallel(&mut self) -> Result<ConservativeRunSummary, SchedulerError> {
        self.run_until_idle_parallel_recorded()
            .map(|recorded| recorded.summary)
    }

    pub fn run_until_idle_parallel_recorded(
        &mut self,
    ) -> Result<RecordedConservativeRunSummary, SchedulerError> {
        let mut recorded_epochs = Vec::new();
        let mut executed_events = 0;
        let mut profile = ParallelRunProfile::default();

        while self.plan_next_parallel_epoch()?.is_some() {
            let before = self.now;
            let epoch = self.run_next_epoch_parallel_recorded()?;
            let summary = epoch.summary();
            executed_events += summary.executed_events();
            profile = profile.merge(epoch.profile());
            recorded_epochs.push(epoch);

            if summary.final_tick() == before && summary.executed_events() == 0 {
                break;
            }
        }

        Ok(RecordedConservativeRunSummary {
            summary: ConservativeRunSummary {
                epochs: recorded_epochs.len(),
                executed_events,
                final_tick: self.now,
            },
            epochs: recorded_epochs,
            profile,
        })
    }

    pub fn plan_next_parallel_epoch(&self) -> Result<Option<ParallelEpochPlan>, SchedulerError> {
        if self.is_idle() {
            return Ok(None);
        }

        self.plan_next_parallel_epoch_with_limit(None)
    }

    pub fn plan_next_parallel_epoch_until(
        &self,
        tick_limit: Tick,
    ) -> Result<Option<ParallelEpochPlan>, SchedulerError> {
        if self.is_idle() || self.now >= tick_limit {
            return Ok(None);
        }

        self.plan_next_parallel_epoch_with_limit(Some(tick_limit))
    }

    fn plan_next_parallel_epoch_with_limit(
        &self,
        horizon_limit: Option<Tick>,
    ) -> Result<Option<ParallelEpochPlan>, SchedulerError> {
        let frontiers = self.parallel_epoch_frontiers()?;
        let horizon = frontiers
            .iter()
            .map(|frontier| frontier.safe_until())
            .min()
            .expect("non-empty scheduler has a horizon");
        let horizon = horizon_limit
            .map(|limit| horizon.min(limit))
            .unwrap_or(horizon);
        let ready_partitions = frontiers
            .iter()
            .filter_map(|frontier| {
                let next_tick = frontier.next_tick()?;
                (next_tick <= horizon).then_some(ReadyPartition {
                    partition: frontier.partition(),
                    next_tick,
                })
            })
            .collect::<Vec<_>>();
        let ready_partitions = super::sort_ready_partitions(ready_partitions);
        let serial_blockers = self.serial_blockers_at_or_before(horizon);

        Ok(Some(ParallelEpochPlan::new(
            horizon,
            ready_partitions,
            frontiers,
            serial_blockers,
            self.max_parallel_workers,
        )))
    }
}

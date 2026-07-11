use super::*;

#[doc(hidden)]
pub enum MixedEventRun {
    Serial(RunSummary),
    Parallel {
        plan: ParallelEpochPlan,
        recorded: RecordedRunSummary,
    },
}

impl MixedEventRun {
    pub fn summary(&self) -> RunSummary {
        match self {
            Self::Serial(summary) => *summary,
            Self::Parallel { recorded, .. } => recorded.summary(),
        }
    }

    pub fn parallel_recorded(&self) -> Option<&RecordedRunSummary> {
        match self {
            Self::Serial(_) => None,
            Self::Parallel { recorded, .. } => Some(recorded),
        }
    }
}

impl PartitionedScheduler {
    #[doc(hidden)]
    pub fn run_next_mixed_event(
        &mut self,
        plan: ParallelEpochPlan,
    ) -> Result<MixedEventRun, SchedulerError> {
        self.validate_parallel_epoch_plan(&plan)?;
        let Some(partition) = self.next_partition_with_event() else {
            return Ok(MixedEventRun::Serial(RunSummary {
                executed_events: 0,
                final_tick: self.now,
            }));
        };
        let queue = &self.partitions[partition.index() as usize];
        let next_tick = queue
            .peek_tick()
            .expect("selected partition has a pending event");
        if !plan
            .ready_partitions()
            .iter()
            .any(|ready| ready.partition == partition && ready.next_tick == next_tick)
        {
            return Err(SchedulerError::StaleParallelEpochPlan);
        }
        let kind = queue
            .peek_kind()
            .expect("selected partition has a pending event");
        match kind {
            ScheduledEventKind::Serial => {
                self.dispatch_next_in_partition(partition);
                self.advance_partitions_to(self.now);
                Ok(MixedEventRun::Serial(RunSummary {
                    executed_events: 1,
                    final_tick: self.now,
                }))
            }
            ScheduledEventKind::Parallel => {
                let (plan, recorded) = self.dispatch_next_parallel_in_partition(partition, plan)?;
                Ok(MixedEventRun::Parallel { plan, recorded })
            }
        }
    }

    fn dispatch_next_parallel_in_partition(
        &mut self,
        partition: PartitionId,
        plan: ParallelEpochPlan,
    ) -> Result<(ParallelEpochPlan, RecordedRunSummary), SchedulerError> {
        let partition_count = self.partition_count();
        let min_remote_delay = self.min_remote_delay;
        let initial_frontiers = plan.frontiers().to_vec();
        let planned_parallel_worker_limit = plan.parallel_worker_limit();
        let planned_batches = plan.parallel_batches().to_vec();
        let partition_nows = self
            .partitions
            .iter()
            .map(|queue| queue.now)
            .collect::<Vec<_>>();
        let index = partition.index() as usize;
        let queue = &self.partitions[index];
        let next_event = queue.pending.peek().expect("partition has pending event");
        let worker = ParallelWorkerRecord::new(
            0,
            partition,
            queue.now,
            next_event.tick,
            Some(next_event.tick),
            queue.pending_event_count(),
        );
        let mut queue = mem::replace(&mut self.partitions[index], PartitionQueue::new());
        let mut event = queue.pop_next().expect("partition has pending event");
        queue.now = event.tick;
        let callback = event
            .callback
            .take()
            .expect("scheduler callback is present");
        let PartitionEventCallback::Parallel(callback) = callback else {
            unreachable!("serial scheduler event reached parallel dispatcher");
        };
        let rollback_next_id = queue.next_id;
        let rollback_next_order = queue.next_order;
        let rollback_next_remote_order = queue.next_remote_order;
        let rollback_next_progress_order = queue.next_progress_order;
        let mut next_remote_order = queue.next_remote_order;
        let mut next_progress_order = queue.next_progress_order;
        let mut remote_events = Vec::new();
        let mut progress_transitions = Vec::new();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut context = ParallelSchedulerContext {
                queue: &mut queue,
                remote_events: &mut remote_events,
                next_remote_order: &mut next_remote_order,
                progress_transitions: &mut progress_transitions,
                next_progress_order: &mut next_progress_order,
                partition,
                partition_count,
                partition_nows: &partition_nows,
                min_remote_delay,
                now: event.tick,
            };
            callback(&mut context);
        }));
        if result.is_err() {
            queue.rollback_scheduled_events(rollback_next_id, rollback_next_order);
            queue.next_remote_order = rollback_next_remote_order;
            queue.next_progress_order = rollback_next_progress_order;
            self.partitions[index] = queue;
            self.advance_partitions_to(event.tick);
            return Err(SchedulerError::ParallelWorkerPanicked { partition });
        }
        queue.next_remote_order = next_remote_order;
        queue.next_progress_order = next_progress_order;
        self.partitions[index] = queue;
        self.now = event.tick;

        let dispatch =
            SchedulerDispatchRecord::new(event.id, event.tick, ScheduledEventKind::Parallel);
        let remote_sends = remote_events
            .iter()
            .map(|remote| {
                ParallelRemoteSendRecord::with_timing(
                    remote.source,
                    remote.target,
                    remote.source_tick,
                    remote.tick,
                    remote.order,
                )
            })
            .collect();
        let batch = ParallelEpochBatchRecord::new(
            event.tick,
            vec![worker],
            vec![dispatch],
            remote_sends,
            progress_transitions,
        );
        self.merge_remote_parallel_events(remote_events)?;
        self.advance_partitions_to(self.now);
        let final_frontiers = self.parallel_epoch_frontiers()?;
        let profile = ParallelRunProfile::for_epoch(std::slice::from_ref(&batch), 1, false);
        let recorded = RecordedRunSummary {
            summary: RunSummary {
                executed_events: 1,
                final_tick: self.now,
            },
            initial_frontiers,
            final_frontiers,
            dispatches: vec![dispatch],
            planned_parallel_worker_limit,
            planned_batches,
            batches: vec![batch],
            profile,
        };
        Ok((plan, recorded))
    }
}

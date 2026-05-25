use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::error::Error;
use std::fmt;
use std::mem;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::thread;

use crate::Tick;

mod state;

pub use state::{
    EpochPlan, ParallelEpochBatchRecord, ParallelEpochPlan, ParallelPartitionActivity,
    ParallelRunProfile, ParallelWorkerRecord, PartitionFrontier, PartitionSnapshot,
    PendingEventSnapshot, ReadyPartition, RecordedConservativeRunSummary, RecordedRunSummary,
    ScheduledEventKind, SchedulerDispatchRecord, SchedulerSnapshot,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PartitionId(u32);

impl PartitionId {
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PartitionEventId {
    partition: PartitionId,
    local: u64,
}

impl PartitionEventId {
    pub const fn new(partition: PartitionId, local: u64) -> Self {
        Self { partition, local }
    }

    pub const fn partition(self) -> PartitionId {
        self.partition
    }

    pub const fn local(self) -> u64 {
        self.local
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    NoPartitions,
    ZeroLookahead,
    ZeroParallelWorkers,
    UnknownPartition {
        partition: PartitionId,
        partitions: u32,
    },
    InThePast {
        partition: PartitionId,
        now: Tick,
        requested: Tick,
    },
    TickOverflow {
        now: Tick,
        delay: Tick,
    },
    ZeroDelayRemoteMessage {
        source: PartitionId,
        target: PartitionId,
    },
    RemoteDelayBelowLookahead {
        source: PartitionId,
        target: PartitionId,
        delay: Tick,
        minimum: Tick,
    },
    SerialEventInParallelEpoch {
        partition: PartitionId,
        tick: Tick,
    },
    ParallelWorkerPanicked {
        partition: PartitionId,
    },
    EpochHorizonOverflow {
        partition: PartitionId,
        now: Tick,
        delay: Tick,
    },
    SnapshotContainsPendingEvents {
        pending_events: usize,
    },
    RestoreWouldDiscardPendingEvents {
        pending_events: usize,
    },
    SnapshotPartitionCountMismatch {
        snapshot_partitions: u32,
        scheduler_partitions: u32,
    },
    SnapshotLookaheadMismatch {
        snapshot_min_remote_delay: Tick,
        scheduler_min_remote_delay: Tick,
    },
    SnapshotParallelWorkerLimitMismatch {
        snapshot_max_parallel_workers: usize,
        scheduler_max_parallel_workers: usize,
    },
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPartitions => write!(formatter, "scheduler requires at least one partition"),
            Self::ZeroLookahead => write!(formatter, "scheduler lookahead must be positive"),
            Self::ZeroParallelWorkers => {
                write!(formatter, "scheduler parallel worker limit must be positive")
            }
            Self::UnknownPartition {
                partition,
                partitions,
            } => write!(
                formatter,
                "partition {} is outside scheduler partition count {partitions}",
                partition.index()
            ),
            Self::InThePast {
                partition,
                now,
                requested,
            } => write!(
                formatter,
                "cannot schedule partition {} at tick {requested}; current tick is {now}",
                partition.index()
            ),
            Self::TickOverflow { now, delay } => {
                write!(formatter, "tick {now} overflows when adding delay {delay}")
            }
            Self::ZeroDelayRemoteMessage { source, target } => write!(
                formatter,
                "remote message from partition {} to {} requires positive delay",
                source.index(),
                target.index()
            ),
            Self::RemoteDelayBelowLookahead {
                source,
                target,
                delay,
                minimum,
            } => write!(
                formatter,
                "remote message from partition {} to {} has delay {delay}; \
                 configured lookahead is {minimum}",
                source.index(),
                target.index()
            ),
            Self::SerialEventInParallelEpoch { partition, tick } => write!(
                formatter,
                "parallel epoch cannot dispatch serial event in partition {} at tick {tick}",
                partition.index()
            ),
            Self::ParallelWorkerPanicked { partition } => write!(
                formatter,
                "parallel worker for partition {} panicked",
                partition.index()
            ),
            Self::EpochHorizonOverflow {
                partition,
                now,
                delay,
            } => write!(
                formatter,
                "partition {} cannot compute parallel epoch horizon from tick {now} with delay {delay}",
                partition.index()
            ),
            Self::SnapshotContainsPendingEvents { pending_events } => write!(
                formatter,
                "scheduler snapshot contains {pending_events} pending events"
            ),
            Self::RestoreWouldDiscardPendingEvents { pending_events } => write!(
                formatter,
                "scheduler restore would discard {pending_events} pending events"
            ),
            Self::SnapshotPartitionCountMismatch {
                snapshot_partitions,
                scheduler_partitions,
            } => write!(
                formatter,
                "scheduler snapshot has {snapshot_partitions} partitions; scheduler has {scheduler_partitions}"
            ),
            Self::SnapshotLookaheadMismatch {
                snapshot_min_remote_delay,
                scheduler_min_remote_delay,
            } => write!(
                formatter,
                "scheduler snapshot lookahead is {snapshot_min_remote_delay}; scheduler lookahead is {scheduler_min_remote_delay}"
            ),
            Self::SnapshotParallelWorkerLimitMismatch {
                snapshot_max_parallel_workers,
                scheduler_max_parallel_workers,
            } => write!(
                formatter,
                "scheduler snapshot parallel worker limit is {snapshot_max_parallel_workers}; scheduler parallel worker limit is {scheduler_max_parallel_workers}"
            ),
        }
    }
}

impl Error for SchedulerError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunSummary {
    executed_events: usize,
    final_tick: Tick,
}

impl RunSummary {
    pub const fn executed_events(self) -> usize {
        self.executed_events
    }

    pub const fn final_tick(self) -> Tick {
        self.final_tick
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConservativeRunSummary {
    epochs: usize,
    executed_events: usize,
    final_tick: Tick,
}

impl ConservativeRunSummary {
    pub const fn epochs(self) -> usize {
        self.epochs
    }

    pub const fn executed_events(self) -> usize {
        self.executed_events
    }

    pub const fn final_tick(self) -> Tick {
        self.final_tick
    }
}

type SchedulerCallback = Box<dyn FnOnce(&mut SchedulerContext<'_>) + Send + 'static>;
type ParallelSchedulerCallback =
    Box<dyn FnOnce(&mut ParallelSchedulerContext<'_>) + Send + 'static>;

pub struct PartitionedScheduler {
    now: Tick,
    min_remote_delay: Tick,
    max_parallel_workers: usize,
    partitions: Vec<PartitionQueue>,
}

impl PartitionedScheduler {
    pub fn new(partitions: u32) -> Result<Self, SchedulerError> {
        Self::with_min_remote_delay(partitions, 1)
    }

    pub fn with_min_remote_delay(
        partitions: u32,
        min_remote_delay: Tick,
    ) -> Result<Self, SchedulerError> {
        Self::with_parallel_worker_limit(partitions, min_remote_delay, usize::MAX)
    }

    pub fn with_parallel_worker_limit(
        partitions: u32,
        min_remote_delay: Tick,
        max_parallel_workers: usize,
    ) -> Result<Self, SchedulerError> {
        if partitions == 0 {
            return Err(SchedulerError::NoPartitions);
        }
        if min_remote_delay == 0 {
            return Err(SchedulerError::ZeroLookahead);
        }
        if max_parallel_workers == 0 {
            return Err(SchedulerError::ZeroParallelWorkers);
        }

        Ok(Self {
            now: 0,
            min_remote_delay,
            max_parallel_workers,
            partitions: (0..partitions).map(|_| PartitionQueue::new()).collect(),
        })
    }

    pub fn now(&self) -> Tick {
        self.now
    }

    pub fn partition_count(&self) -> u32 {
        self.partitions.len() as u32
    }

    pub fn min_remote_delay(&self) -> Tick {
        self.min_remote_delay
    }

    pub fn max_parallel_workers(&self) -> usize {
        self.max_parallel_workers
    }

    pub fn is_idle(&self) -> bool {
        self.partitions.iter().all(PartitionQueue::is_empty)
    }

    pub fn partition_now(&self, partition: PartitionId) -> Result<Tick, SchedulerError> {
        self.partition(partition)
            .map(|queue| queue.now)
            .ok_or(SchedulerError::UnknownPartition {
                partition,
                partitions: self.partition_count(),
            })
    }

    pub fn next_pending_tick(
        &self,
        partition: PartitionId,
    ) -> Result<Option<Tick>, SchedulerError> {
        self.partition(partition)
            .map(PartitionQueue::peek_tick)
            .ok_or(SchedulerError::UnknownPartition {
                partition,
                partitions: self.partition_count(),
            })
    }

    pub fn schedule_at<F>(
        &mut self,
        partition: PartitionId,
        tick: Tick,
        callback: F,
    ) -> Result<PartitionEventId, SchedulerError>
    where
        F: FnOnce(&mut SchedulerContext<'_>) + Send + 'static,
    {
        let partition_count = self.partition_count();
        let queue = self
            .partition_mut(partition)
            .ok_or(SchedulerError::UnknownPartition {
                partition,
                partitions: partition_count,
            })?;

        queue.schedule_at(partition, tick, Box::new(callback))
    }

    pub fn schedule_after<F>(
        &mut self,
        partition: PartitionId,
        delay: Tick,
        callback: F,
    ) -> Result<PartitionEventId, SchedulerError>
    where
        F: FnOnce(&mut SchedulerContext<'_>) + Send + 'static,
    {
        let tick = self
            .now
            .checked_add(delay)
            .ok_or(SchedulerError::TickOverflow {
                now: self.now,
                delay,
            })?;
        self.schedule_at(partition, tick, callback)
    }

    pub fn schedule_parallel_at<F>(
        &mut self,
        partition: PartitionId,
        tick: Tick,
        callback: F,
    ) -> Result<PartitionEventId, SchedulerError>
    where
        F: FnOnce(&mut ParallelSchedulerContext<'_>) + Send + 'static,
    {
        let partition_count = self.partition_count();
        let queue = self
            .partition_mut(partition)
            .ok_or(SchedulerError::UnknownPartition {
                partition,
                partitions: partition_count,
            })?;

        queue.schedule_parallel_at(partition, tick, Box::new(callback))
    }

    pub fn snapshot(&self) -> SchedulerSnapshot {
        SchedulerSnapshot {
            now: self.now,
            min_remote_delay: self.min_remote_delay,
            max_parallel_workers: self.max_parallel_workers,
            partitions: self
                .partitions
                .iter()
                .enumerate()
                .map(|(index, queue)| queue.snapshot(PartitionId::new(index as u32)))
                .collect(),
        }
    }

    pub fn quiescent_snapshot(&self) -> Result<SchedulerSnapshot, SchedulerError> {
        let snapshot = self.snapshot();
        let pending_events = snapshot.total_pending_events();
        if pending_events != 0 {
            return Err(SchedulerError::SnapshotContainsPendingEvents { pending_events });
        }

        Ok(snapshot)
    }

    pub fn restore_quiescent(
        &mut self,
        snapshot: &SchedulerSnapshot,
    ) -> Result<(), SchedulerError> {
        let snapshot_pending = snapshot.total_pending_events();
        if snapshot_pending != 0 {
            return Err(SchedulerError::SnapshotContainsPendingEvents {
                pending_events: snapshot_pending,
            });
        }

        let current_pending = self.total_pending_events();
        if current_pending != 0 {
            return Err(SchedulerError::RestoreWouldDiscardPendingEvents {
                pending_events: current_pending,
            });
        }

        let scheduler_partitions = self.partition_count();
        let snapshot_partitions = snapshot.partitions.len() as u32;
        if snapshot_partitions != scheduler_partitions {
            return Err(SchedulerError::SnapshotPartitionCountMismatch {
                snapshot_partitions,
                scheduler_partitions,
            });
        }
        if snapshot.min_remote_delay != self.min_remote_delay {
            return Err(SchedulerError::SnapshotLookaheadMismatch {
                snapshot_min_remote_delay: snapshot.min_remote_delay,
                scheduler_min_remote_delay: self.min_remote_delay,
            });
        }
        if snapshot.max_parallel_workers != self.max_parallel_workers {
            return Err(SchedulerError::SnapshotParallelWorkerLimitMismatch {
                snapshot_max_parallel_workers: snapshot.max_parallel_workers,
                scheduler_max_parallel_workers: self.max_parallel_workers,
            });
        }

        self.now = snapshot.now;
        for (queue, partition) in self.partitions.iter_mut().zip(&snapshot.partitions) {
            queue.restore_quiescent(partition);
        }

        Ok(())
    }

    pub fn run_until_idle(&mut self) -> RunSummary {
        let mut executed_events = 0;

        while let Some(partition) = self.next_partition_with_event() {
            self.dispatch_next_in_partition(partition);
            executed_events += 1;
        }

        RunSummary {
            executed_events,
            final_tick: self.now,
        }
    }

    pub fn run_next_epoch(&mut self) -> RunSummary {
        let Some(plan) = self.plan_next_epoch() else {
            return RunSummary {
                executed_events: 0,
                final_tick: self.now,
            };
        };
        let horizon = plan.horizon;

        let mut executed_events = 0;
        while let Some(partition) = self.next_partition_with_event_at_or_before(horizon) {
            self.dispatch_next_in_partition(partition);
            executed_events += 1;
        }

        for queue in &mut self.partitions {
            if queue.now < horizon {
                queue.now = horizon;
            }
        }
        self.now = horizon;

        RunSummary {
            executed_events,
            final_tick: self.now,
        }
    }

    pub fn run_next_epoch_parallel(&mut self) -> Result<RunSummary, SchedulerError> {
        self.run_next_epoch_parallel_recorded()
            .map(|recorded| recorded.summary)
    }

    pub fn run_next_epoch_parallel_recorded(
        &mut self,
    ) -> Result<RecordedRunSummary, SchedulerError> {
        let Some(plan) = self.plan_next_parallel_epoch()? else {
            return Ok(RecordedRunSummary {
                summary: RunSummary {
                    executed_events: 0,
                    final_tick: self.now,
                },
                dispatches: Vec::new(),
                batches: Vec::new(),
                profile: ParallelRunProfile::default(),
            });
        };
        let horizon = plan.horizon();

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
        dispatches.sort_by_key(|record| (record.tick, record.partition(), record.id.local()));
        let profile = ParallelRunProfile::for_epoch(&batches, dispatches.len(), batches.is_empty());

        Ok(RecordedRunSummary {
            summary: RunSummary {
                executed_events,
                final_tick: self.now,
            },
            dispatches,
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

    pub fn run_until_idle_conservative(&mut self) -> ConservativeRunSummary {
        let mut epochs = 0;
        let mut executed_events = 0;

        while self.plan_next_epoch().is_some() {
            let before = self.now;
            let summary = self.run_next_epoch();
            epochs += 1;
            executed_events += summary.executed_events();

            if summary.final_tick() == before && summary.executed_events() == 0 {
                break;
            }
        }

        ConservativeRunSummary {
            epochs,
            executed_events,
            final_tick: self.now,
        }
    }

    pub fn plan_next_epoch(&self) -> Option<EpochPlan> {
        if self.is_idle() {
            return None;
        }

        let horizon = self.next_epoch_horizon()?;
        let ready_partitions = self
            .partitions
            .iter()
            .enumerate()
            .filter_map(|(index, queue)| {
                let next_tick = queue.peek_tick()?;
                (next_tick <= horizon).then_some(ReadyPartition {
                    partition: PartitionId::new(index as u32),
                    next_tick,
                })
            })
            .collect();

        Some(EpochPlan {
            horizon,
            ready_partitions,
        })
    }

    pub fn plan_next_parallel_epoch(&self) -> Result<Option<ParallelEpochPlan>, SchedulerError> {
        if self.is_idle() {
            return Ok(None);
        }

        let mut horizon = None;
        let mut frontiers = Vec::with_capacity(self.partitions.len());
        for (index, queue) in self.partitions.iter().enumerate() {
            let partition = PartitionId::new(index as u32);
            let safe_until = queue.now.checked_add(self.min_remote_delay).ok_or(
                SchedulerError::EpochHorizonOverflow {
                    partition,
                    now: queue.now,
                    delay: self.min_remote_delay,
                },
            )?;
            horizon = Some(horizon.map_or(safe_until, |current: Tick| current.min(safe_until)));
            frontiers.push(PartitionFrontier::new(
                partition,
                queue.now,
                safe_until,
                queue.peek_tick(),
                queue.pending_event_count(),
            ));
        }

        let horizon = horizon.expect("non-empty scheduler has a horizon");
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
        let ready_partitions = sort_ready_partitions(ready_partitions);
        let serial_blockers = self.serial_blockers_at_or_before(horizon);

        Ok(Some(ParallelEpochPlan::new(
            horizon,
            ready_partitions,
            frontiers,
            serial_blockers,
        )))
    }

    fn partition(&self, partition: PartitionId) -> Option<&PartitionQueue> {
        self.partitions.get(partition.index() as usize)
    }

    fn partition_mut(&mut self, partition: PartitionId) -> Option<&mut PartitionQueue> {
        self.partitions.get_mut(partition.index() as usize)
    }

    fn total_pending_events(&self) -> usize {
        self.partitions
            .iter()
            .map(PartitionQueue::pending_event_count)
            .sum()
    }

    fn next_partition_with_event(&self) -> Option<PartitionId> {
        self.partitions
            .iter()
            .enumerate()
            .filter_map(|(index, queue)| {
                queue
                    .peek_tick()
                    .map(|tick| (tick, PartitionId::new(index as u32)))
            })
            .min_by_key(|(tick, partition)| (*tick, *partition))
            .map(|(_, partition)| partition)
    }

    fn next_partition_with_event_at_or_before(&self, horizon: Tick) -> Option<PartitionId> {
        self.partitions
            .iter()
            .enumerate()
            .filter_map(|(index, queue)| {
                let tick = queue.peek_tick()?;
                (tick <= horizon).then_some((tick, PartitionId::new(index as u32)))
            })
            .min_by_key(|(tick, partition)| (*tick, *partition))
            .map(|(_, partition)| partition)
    }

    fn next_epoch_horizon(&self) -> Option<Tick> {
        self.partitions
            .iter()
            .map(|queue| queue.now.checked_add(self.min_remote_delay))
            .collect::<Option<Vec<_>>>()
            .and_then(|horizons| horizons.into_iter().min())
    }

    fn first_serial_event_at_or_before(&self, horizon: Tick) -> Option<(PartitionId, Tick)> {
        self.partitions
            .iter()
            .enumerate()
            .filter_map(|(index, queue)| {
                queue
                    .first_serial_tick_at_or_before(horizon)
                    .map(|tick| (PartitionId::new(index as u32), tick))
            })
            .min_by_key(|(partition, tick)| (*tick, *partition))
    }

    fn serial_blockers_at_or_before(&self, horizon: Tick) -> Vec<SchedulerDispatchRecord> {
        let mut blockers = self
            .partitions
            .iter()
            .enumerate()
            .flat_map(|(index, queue)| {
                queue.serial_blockers_at_or_before(PartitionId::new(index as u32), horizon)
            })
            .collect::<Vec<_>>();
        blockers.sort_by_key(|record| (record.tick(), record.partition(), record.id().local()));
        blockers
    }

    fn ready_partitions_at_or_before(&self, horizon: Tick) -> Vec<PartitionId> {
        sort_ready_partitions(
            self.partitions
                .iter()
                .enumerate()
                .filter_map(|(index, queue)| {
                    let next_tick = queue.peek_tick()?;
                    (next_tick <= horizon).then_some(ReadyPartition {
                        partition: PartitionId::new(index as u32),
                        next_tick,
                    })
                })
                .collect(),
        )
        .into_iter()
        .map(|ready| ready.partition)
        .collect()
    }

    fn run_parallel_batch(
        &mut self,
        horizon: Tick,
        ready_partitions: Vec<PartitionId>,
    ) -> Result<ParallelBatchResult, SchedulerError> {
        let partition_count = self.partition_count();
        let min_remote_delay = self.min_remote_delay;
        let mut partition_queues = Vec::with_capacity(ready_partitions.len());
        let mut workers = Vec::with_capacity(ready_partitions.len());

        for partition in ready_partitions {
            let index = partition.index() as usize;
            let queue = mem::replace(&mut self.partitions[index], PartitionQueue::new());
            let safe_until = queue.now.checked_add(min_remote_delay).ok_or(
                SchedulerError::EpochHorizonOverflow {
                    partition,
                    now: queue.now,
                    delay: min_remote_delay,
                },
            )?;
            workers.push(ParallelWorkerRecord::new(
                partition,
                queue.now,
                safe_until,
                queue.peek_tick(),
                queue.pending_event_count(),
            ));
            partition_queues.push((index, partition, queue));
        }

        let (results, first_error) = thread::scope(|scope| {
            let mut handles = Vec::with_capacity(partition_queues.len());

            for (index, partition, queue) in partition_queues {
                handles.push((
                    partition,
                    scope.spawn(move || {
                        run_parallel_partition(
                            index,
                            partition,
                            queue,
                            horizon,
                            min_remote_delay,
                            partition_count,
                        )
                    }),
                ));
            }

            let mut results = Vec::with_capacity(handles.len());
            let mut first_error = None;
            for (partition, handle) in handles {
                match handle.join() {
                    Ok(result) => {
                        if let Some(error) = result.error.clone() {
                            first_error.get_or_insert(error);
                        }
                        results.push(result);
                    }
                    Err(_) => {
                        first_error
                            .get_or_insert(SchedulerError::ParallelWorkerPanicked { partition });
                    }
                }
            }

            (results, first_error)
        });

        let mut executed_events = 0;
        let mut dispatches = Vec::new();
        let mut latest_partition_tick = self.now;
        let mut remote_events = Vec::new();
        for result in results {
            executed_events += result.executed_events;
            dispatches.extend(result.dispatches);
            latest_partition_tick = latest_partition_tick.max(result.queue.now);
            if result.error.is_none() {
                remote_events.extend(result.remote_events);
            }
            self.partitions[result.index] = result.queue;
        }

        if let Some(error) = first_error {
            self.now = latest_partition_tick;
            self.merge_remote_parallel_events(remote_events)?;
            return Err(error);
        }

        dispatches.sort_by_key(|record| (record.tick(), record.partition(), record.id().local()));
        let record = ParallelEpochBatchRecord::new(horizon, workers, dispatches.clone());

        Ok(ParallelBatchResult {
            executed_events,
            remote_events,
            dispatches,
            record,
        })
    }

    fn merge_remote_parallel_events(
        &mut self,
        mut remote_events: Vec<RemoteScheduledEvent>,
    ) -> Result<(), SchedulerError> {
        remote_events.sort_by_key(|event| (event.target, event.tick, event.source, event.order));
        for event in remote_events {
            self.partitions[event.target.index() as usize].schedule_parallel_at(
                event.target,
                event.tick,
                event.callback,
            )?;
        }

        Ok(())
    }

    fn advance_partitions_to(&mut self, tick: Tick) {
        for queue in &mut self.partitions {
            if queue.now < tick {
                queue.now = tick;
            }
        }
        self.now = tick;
    }

    fn dispatch_next_in_partition(&mut self, partition: PartitionId) {
        let mut event = self.partitions[partition.index() as usize]
            .pop_next()
            .expect("partition has pending event");

        self.now = event.tick;
        self.partitions[partition.index() as usize].now = event.tick;

        let callback = event
            .callback
            .take()
            .expect("scheduler callback is present");
        match callback {
            PartitionEventCallback::Serial(callback) => {
                let mut context = SchedulerContext {
                    scheduler: self,
                    partition,
                    now: event.tick,
                };
                callback(&mut context);
            }
            PartitionEventCallback::Parallel(_) => {
                panic!("parallel scheduler event reached serial dispatcher");
            }
        }
    }
}

impl fmt::Debug for PartitionedScheduler {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PartitionedScheduler")
            .field("now", &self.now)
            .field("min_remote_delay", &self.min_remote_delay)
            .field("max_parallel_workers", &self.max_parallel_workers)
            .field("partition_count", &self.partition_count())
            .finish()
    }
}

pub struct SchedulerContext<'a> {
    scheduler: &'a mut PartitionedScheduler,
    partition: PartitionId,
    now: Tick,
}

impl SchedulerContext<'_> {
    pub fn now(&self) -> Tick {
        self.now
    }

    pub fn partition(&self) -> PartitionId {
        self.partition
    }

    pub fn schedule_local_after<F>(
        &mut self,
        delay: Tick,
        callback: F,
    ) -> Result<PartitionEventId, SchedulerError>
    where
        F: FnOnce(&mut SchedulerContext<'_>) + Send + 'static,
    {
        self.schedule_at_partition_after(self.partition, delay, callback)
    }

    pub fn schedule_remote_after<F>(
        &mut self,
        target: PartitionId,
        delay: Tick,
        callback: F,
    ) -> Result<PartitionEventId, SchedulerError>
    where
        F: FnOnce(&mut SchedulerContext<'_>) + Send + 'static,
    {
        if target != self.partition && delay == 0 {
            return Err(SchedulerError::ZeroDelayRemoteMessage {
                source: self.partition,
                target,
            });
        }
        if target != self.partition && delay < self.scheduler.min_remote_delay {
            return Err(SchedulerError::RemoteDelayBelowLookahead {
                source: self.partition,
                target,
                delay,
                minimum: self.scheduler.min_remote_delay,
            });
        }

        self.schedule_at_partition_after(target, delay, callback)
    }

    fn schedule_at_partition_after<F>(
        &mut self,
        target: PartitionId,
        delay: Tick,
        callback: F,
    ) -> Result<PartitionEventId, SchedulerError>
    where
        F: FnOnce(&mut SchedulerContext<'_>) + Send + 'static,
    {
        let tick = self
            .now
            .checked_add(delay)
            .ok_or(SchedulerError::TickOverflow {
                now: self.now,
                delay,
            })?;
        self.scheduler.schedule_at(target, tick, callback)
    }
}

pub struct ParallelSchedulerContext<'a> {
    queue: &'a mut PartitionQueue,
    remote_events: &'a mut Vec<RemoteScheduledEvent>,
    next_remote_order: &'a mut u64,
    partition: PartitionId,
    partition_count: u32,
    min_remote_delay: Tick,
    now: Tick,
}

impl ParallelSchedulerContext<'_> {
    pub fn now(&self) -> Tick {
        self.now
    }

    pub fn partition(&self) -> PartitionId {
        self.partition
    }

    pub fn schedule_local_after<F>(
        &mut self,
        delay: Tick,
        callback: F,
    ) -> Result<PartitionEventId, SchedulerError>
    where
        F: FnOnce(&mut ParallelSchedulerContext<'_>) + Send + 'static,
    {
        let tick = self
            .now
            .checked_add(delay)
            .ok_or(SchedulerError::TickOverflow {
                now: self.now,
                delay,
            })?;
        self.queue
            .schedule_parallel_at(self.partition, tick, Box::new(callback))
    }

    pub fn schedule_remote_after<F>(
        &mut self,
        target: PartitionId,
        delay: Tick,
        callback: F,
    ) -> Result<PartitionEventId, SchedulerError>
    where
        F: FnOnce(&mut ParallelSchedulerContext<'_>) + Send + 'static,
    {
        if target.index() >= self.partition_count {
            return Err(SchedulerError::UnknownPartition {
                partition: target,
                partitions: self.partition_count,
            });
        }
        if target != self.partition && delay == 0 {
            return Err(SchedulerError::ZeroDelayRemoteMessage {
                source: self.partition,
                target,
            });
        }
        if target != self.partition && delay < self.min_remote_delay {
            return Err(SchedulerError::RemoteDelayBelowLookahead {
                source: self.partition,
                target,
                delay,
                minimum: self.min_remote_delay,
            });
        }

        if target == self.partition {
            return self.schedule_local_after(delay, callback);
        }

        let tick = self
            .now
            .checked_add(delay)
            .ok_or(SchedulerError::TickOverflow {
                now: self.now,
                delay,
            })?;
        let order = *self.next_remote_order;
        *self.next_remote_order += 1;
        self.remote_events.push(RemoteScheduledEvent {
            source: self.partition,
            target,
            tick,
            order,
            callback: Box::new(callback),
        });

        Ok(PartitionEventId {
            partition: target,
            local: order,
        })
    }
}

fn sort_ready_partitions(mut ready_partitions: Vec<ReadyPartition>) -> Vec<ReadyPartition> {
    ready_partitions.sort_by_key(|ready| (ready.next_tick, ready.partition));
    ready_partitions
}

struct ParallelPartitionResult {
    index: usize,
    queue: PartitionQueue,
    executed_events: usize,
    dispatches: Vec<SchedulerDispatchRecord>,
    remote_events: Vec<RemoteScheduledEvent>,
    error: Option<SchedulerError>,
}

struct ParallelBatchResult {
    executed_events: usize,
    remote_events: Vec<RemoteScheduledEvent>,
    dispatches: Vec<SchedulerDispatchRecord>,
    record: ParallelEpochBatchRecord,
}

struct RemoteScheduledEvent {
    source: PartitionId,
    target: PartitionId,
    tick: Tick,
    order: u64,
    callback: ParallelSchedulerCallback,
}

fn run_parallel_partition(
    index: usize,
    partition: PartitionId,
    mut queue: PartitionQueue,
    horizon: Tick,
    min_remote_delay: Tick,
    partition_count: u32,
) -> ParallelPartitionResult {
    let mut executed_events = 0;
    let mut next_remote_order = 0;
    let mut dispatches = Vec::new();
    let mut remote_events = Vec::new();

    while queue.peek_tick().is_some_and(|tick| tick <= horizon) {
        let mut event = queue.pop_next().expect("partition has pending event");
        queue.now = event.tick;
        let callback = event
            .callback
            .take()
            .expect("scheduler callback is present");

        match callback {
            PartitionEventCallback::Serial(_) => {
                return ParallelPartitionResult {
                    index,
                    queue,
                    executed_events,
                    dispatches,
                    remote_events,
                    error: Some(SchedulerError::SerialEventInParallelEpoch {
                        partition,
                        tick: event.tick,
                    }),
                };
            }
            PartitionEventCallback::Parallel(callback) => {
                let rollback_next_id = queue.next_id;
                let rollback_next_order = queue.next_order;
                let result = catch_unwind(AssertUnwindSafe(|| {
                    let mut context = ParallelSchedulerContext {
                        queue: &mut queue,
                        remote_events: &mut remote_events,
                        next_remote_order: &mut next_remote_order,
                        partition,
                        partition_count,
                        min_remote_delay,
                        now: event.tick,
                    };
                    callback(&mut context);
                }));
                if result.is_err() {
                    queue.rollback_scheduled_events(rollback_next_id, rollback_next_order);
                    return ParallelPartitionResult {
                        index,
                        queue,
                        executed_events,
                        dispatches,
                        remote_events,
                        error: Some(SchedulerError::ParallelWorkerPanicked { partition }),
                    };
                }
                executed_events += 1;
                dispatches.push(SchedulerDispatchRecord::new(
                    event.id,
                    event.tick,
                    ScheduledEventKind::Parallel,
                ));
            }
        }
    }

    ParallelPartitionResult {
        index,
        queue,
        executed_events,
        dispatches,
        remote_events,
        error: None,
    }
}

struct PartitionQueue {
    now: Tick,
    next_id: u64,
    next_order: u64,
    pending: BinaryHeap<PartitionEvent>,
}

impl PartitionQueue {
    fn new() -> Self {
        Self {
            now: 0,
            next_id: 0,
            next_order: 0,
            pending: BinaryHeap::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    fn pending_event_count(&self) -> usize {
        self.pending.len()
    }

    fn peek_tick(&self) -> Option<Tick> {
        self.pending.peek().map(|event| event.tick)
    }

    fn pop_next(&mut self) -> Option<PartitionEvent> {
        self.pending.pop()
    }

    fn rollback_scheduled_events(&mut self, next_id: u64, next_order: u64) {
        self.next_id = next_id;
        self.next_order = next_order;
        let pending = mem::take(&mut self.pending)
            .into_vec()
            .into_iter()
            .filter(|event| event.id.local() < next_id && event.order < next_order)
            .collect();
        self.pending = pending;
    }

    fn first_serial_tick_at_or_before(&self, horizon: Tick) -> Option<Tick> {
        self.pending
            .iter()
            .filter(|event| event.tick <= horizon && event.is_serial())
            .map(|event| event.tick)
            .min()
    }

    fn serial_blockers_at_or_before(
        &self,
        partition: PartitionId,
        horizon: Tick,
    ) -> Vec<SchedulerDispatchRecord> {
        let mut blockers = self
            .pending
            .iter()
            .filter(|event| event.tick <= horizon && event.is_serial())
            .map(|event| event.dispatch_record(partition))
            .collect::<Vec<_>>();
        blockers.sort_by_key(|record| (record.tick(), record.id().local()));
        blockers
    }

    fn schedule_at(
        &mut self,
        partition: PartitionId,
        tick: Tick,
        callback: SchedulerCallback,
    ) -> Result<PartitionEventId, SchedulerError> {
        self.schedule_event_at(partition, tick, PartitionEventCallback::Serial(callback))
    }

    fn schedule_parallel_at(
        &mut self,
        partition: PartitionId,
        tick: Tick,
        callback: ParallelSchedulerCallback,
    ) -> Result<PartitionEventId, SchedulerError> {
        self.schedule_event_at(partition, tick, PartitionEventCallback::Parallel(callback))
    }

    fn schedule_event_at(
        &mut self,
        partition: PartitionId,
        tick: Tick,
        callback: PartitionEventCallback,
    ) -> Result<PartitionEventId, SchedulerError> {
        if tick < self.now {
            return Err(SchedulerError::InThePast {
                partition,
                now: self.now,
                requested: tick,
            });
        }

        let id = PartitionEventId {
            partition,
            local: self.next_id,
        };
        self.next_id += 1;

        let order = self.next_order;
        self.next_order += 1;

        self.pending.push(PartitionEvent {
            tick,
            order,
            id,
            callback: Some(callback),
        });

        Ok(id)
    }

    fn snapshot(&self, partition: PartitionId) -> PartitionSnapshot {
        let mut pending_events = self
            .pending
            .iter()
            .map(PartitionEvent::snapshot)
            .collect::<Vec<_>>();
        pending_events.sort_by_key(|event| (event.tick, event.order, event.id.local()));

        PartitionSnapshot {
            partition,
            now: self.now,
            next_event_local: self.next_id,
            next_event_order: self.next_order,
            pending_events,
        }
    }

    fn restore_quiescent(&mut self, snapshot: &PartitionSnapshot) {
        self.now = snapshot.now;
        self.next_id = snapshot.next_event_local;
        self.next_order = snapshot.next_event_order;
        self.pending.clear();
    }
}

enum PartitionEventCallback {
    Serial(SchedulerCallback),
    Parallel(ParallelSchedulerCallback),
}

impl PartitionEventCallback {
    fn kind(&self) -> ScheduledEventKind {
        match self {
            Self::Serial(_) => ScheduledEventKind::Serial,
            Self::Parallel(_) => ScheduledEventKind::Parallel,
        }
    }
}

struct PartitionEvent {
    tick: Tick,
    order: u64,
    id: PartitionEventId,
    callback: Option<PartitionEventCallback>,
}

impl PartitionEvent {
    fn is_serial(&self) -> bool {
        matches!(self.callback, Some(PartitionEventCallback::Serial(_)))
    }

    fn dispatch_record(&self, partition: PartitionId) -> SchedulerDispatchRecord {
        SchedulerDispatchRecord::new(
            PartitionEventId::new(partition, self.id.local()),
            self.tick,
            self.callback
                .as_ref()
                .expect("scheduler callback is present")
                .kind(),
        )
    }

    fn snapshot(&self) -> PendingEventSnapshot {
        PendingEventSnapshot {
            id: self.id,
            tick: self.tick,
            order: self.order,
            kind: self
                .callback
                .as_ref()
                .expect("scheduler callback is present")
                .kind(),
        }
    }
}

impl PartialEq for PartitionEvent {
    fn eq(&self, other: &Self) -> bool {
        self.tick == other.tick && self.order == other.order && self.id == other.id
    }
}

impl Eq for PartitionEvent {}

impl PartialOrd for PartitionEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PartitionEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .tick
            .cmp(&self.tick)
            .then_with(|| other.order.cmp(&self.order))
            .then_with(|| other.id.local.cmp(&self.id.local))
    }
}

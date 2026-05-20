use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::error::Error;
use std::fmt;
use std::mem;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::Tick;

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
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPartitions => write!(formatter, "scheduler requires at least one partition"),
            Self::ZeroLookahead => write!(formatter, "scheduler lookahead must be positive"),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochPlan {
    horizon: Tick,
    ready_partitions: Vec<ReadyPartition>,
}

impl EpochPlan {
    pub fn horizon(&self) -> Tick {
        self.horizon
    }

    pub fn ready_partitions(&self) -> &[ReadyPartition] {
        &self.ready_partitions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadyPartition {
    pub partition: PartitionId,
    pub next_tick: Tick,
}

type SchedulerCallback = Box<dyn FnOnce(&mut SchedulerContext<'_>) + Send + 'static>;
type ParallelSchedulerCallback =
    Box<dyn FnOnce(&mut ParallelSchedulerContext<'_>) + Send + 'static>;

pub struct PartitionedScheduler {
    now: Tick,
    min_remote_delay: Tick,
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
        if partitions == 0 {
            return Err(SchedulerError::NoPartitions);
        }
        if min_remote_delay == 0 {
            return Err(SchedulerError::ZeroLookahead);
        }

        Ok(Self {
            now: 0,
            min_remote_delay,
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
        let Some(plan) = self.plan_next_epoch() else {
            return Ok(RunSummary {
                executed_events: 0,
                final_tick: self.now,
            });
        };
        let horizon = plan.horizon;

        if let Some((partition, tick)) = self.first_serial_event_at_or_before(horizon) {
            return Err(SchedulerError::SerialEventInParallelEpoch { partition, tick });
        }

        let ready_partitions = plan
            .ready_partitions
            .iter()
            .map(|ready| ready.partition)
            .collect::<Vec<_>>();
        let partition_count = self.partition_count();
        let min_remote_delay = self.min_remote_delay;
        let remote_events = Arc::new(Mutex::new(Vec::new()));
        let mut partition_queues = Vec::with_capacity(ready_partitions.len());

        for partition in ready_partitions {
            let index = partition.index() as usize;
            let queue = mem::replace(&mut self.partitions[index], PartitionQueue::new());
            partition_queues.push((index, partition, queue));
        }

        let results = thread::scope(|scope| {
            let mut handles = Vec::with_capacity(partition_queues.len());

            for (index, partition, queue) in partition_queues {
                let remote_events = Arc::clone(&remote_events);
                handles.push(scope.spawn(move || {
                    run_parallel_partition(
                        index,
                        partition,
                        queue,
                        horizon,
                        min_remote_delay,
                        partition_count,
                        remote_events,
                    )
                }));
            }

            handles
                .into_iter()
                .map(|handle| handle.join().expect("parallel scheduler worker panicked"))
                .collect::<Result<Vec<_>, SchedulerError>>()
        })?;

        let mut executed_events = 0;
        for result in results {
            executed_events += result.executed_events;
            self.partitions[result.index] = result.queue;
        }

        for queue in &mut self.partitions {
            if queue.now < horizon {
                queue.now = horizon;
            }
        }
        self.now = horizon;

        let mut remote_events = remote_events.lock().expect("remote event outbox poisoned");
        remote_events.sort_by_key(|event| (event.target, event.tick, event.source, event.order));
        for event in remote_events.drain(..) {
            self.partitions[event.target.index() as usize].schedule_parallel_at(
                event.target,
                event.tick,
                event.callback,
            )?;
        }

        Ok(RunSummary {
            executed_events,
            final_tick: self.now,
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

    fn partition(&self, partition: PartitionId) -> Option<&PartitionQueue> {
        self.partitions.get(partition.index() as usize)
    }

    fn partition_mut(&mut self, partition: PartitionId) -> Option<&mut PartitionQueue> {
        self.partitions.get_mut(partition.index() as usize)
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
    remote_events: &'a Mutex<Vec<RemoteScheduledEvent>>,
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
        self.remote_events
            .lock()
            .expect("remote event outbox poisoned")
            .push(RemoteScheduledEvent {
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

struct ParallelPartitionResult {
    index: usize,
    queue: PartitionQueue,
    executed_events: usize,
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
    remote_events: Arc<Mutex<Vec<RemoteScheduledEvent>>>,
) -> Result<ParallelPartitionResult, SchedulerError> {
    let mut executed_events = 0;
    let mut next_remote_order = 0;

    while queue.peek_tick().is_some_and(|tick| tick <= horizon) {
        let mut event = queue.pop_next().expect("partition has pending event");
        queue.now = event.tick;
        let callback = event
            .callback
            .take()
            .expect("scheduler callback is present");

        match callback {
            PartitionEventCallback::Serial(_) => {
                return Err(SchedulerError::SerialEventInParallelEpoch {
                    partition,
                    tick: event.tick,
                });
            }
            PartitionEventCallback::Parallel(callback) => {
                let mut context = ParallelSchedulerContext {
                    queue: &mut queue,
                    remote_events: &remote_events,
                    next_remote_order: &mut next_remote_order,
                    partition,
                    partition_count,
                    min_remote_delay,
                    now: event.tick,
                };
                callback(&mut context);
                executed_events += 1;
            }
        }
    }

    Ok(ParallelPartitionResult {
        index,
        queue,
        executed_events,
    })
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

    fn peek_tick(&self) -> Option<Tick> {
        self.pending.peek().map(|event| event.tick)
    }

    fn pop_next(&mut self) -> Option<PartitionEvent> {
        self.pending.pop()
    }

    fn first_serial_tick_at_or_before(&self, horizon: Tick) -> Option<Tick> {
        self.pending
            .iter()
            .filter(|event| event.tick <= horizon && event.is_serial())
            .map(|event| event.tick)
            .min()
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
}

enum PartitionEventCallback {
    Serial(SchedulerCallback),
    Parallel(ParallelSchedulerCallback),
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

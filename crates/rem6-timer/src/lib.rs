use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_interrupt::{InterruptError, InterruptLinePort, InterruptSourceId};
use rem6_kernel::{PartitionEventId, PartitionId, SchedulerContext, SchedulerError, Tick};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TimerId(u64);

impl TimerId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerArm {
    generation: u64,
    programmed_tick: Tick,
    deadline: Tick,
}

impl TimerArm {
    pub const fn new(generation: u64, programmed_tick: Tick, deadline: Tick) -> Self {
        Self {
            generation,
            programmed_tick,
            deadline,
        }
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn programmed_tick(self) -> Tick {
        self.programmed_tick
    }

    pub const fn deadline(self) -> Tick {
        self.deadline
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerExpiry {
    generation: u64,
    deadline: Tick,
}

impl TimerExpiry {
    pub const fn new(generation: u64, deadline: Tick) -> Self {
        Self {
            generation,
            deadline,
        }
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn deadline(self) -> Tick {
        self.deadline
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimerSignalError {
    generation: u64,
    tick: Tick,
    error: InterruptError,
}

impl TimerSignalError {
    pub const fn new(generation: u64, tick: Tick, error: InterruptError) -> Self {
        Self {
            generation,
            tick,
            error,
        }
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn error(&self) -> &InterruptError {
        &self.error
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimerSnapshot {
    id: TimerId,
    partition: PartitionId,
    source: InterruptSourceId,
    next_deadline: Option<Tick>,
    arms: Vec<TimerArm>,
    expiries: Vec<TimerExpiry>,
    signal_errors: Vec<TimerSignalError>,
}

impl TimerSnapshot {
    pub fn new(
        id: TimerId,
        partition: PartitionId,
        source: InterruptSourceId,
        next_deadline: Option<Tick>,
        arms: Vec<TimerArm>,
        expiries: Vec<TimerExpiry>,
        signal_errors: Vec<TimerSignalError>,
    ) -> Self {
        Self {
            id,
            partition,
            source,
            next_deadline,
            arms,
            expiries,
            signal_errors,
        }
    }

    pub const fn id(&self) -> TimerId {
        self.id
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn source(&self) -> InterruptSourceId {
        self.source
    }

    pub const fn next_deadline(&self) -> Option<Tick> {
        self.next_deadline
    }

    pub fn arms(&self) -> &[TimerArm] {
        &self.arms
    }

    pub fn expiries(&self) -> &[TimerExpiry] {
        &self.expiries
    }

    pub fn signal_errors(&self) -> &[TimerSignalError] {
        &self.signal_errors
    }
}

#[derive(Clone, Debug)]
pub struct ProgrammableTimer {
    id: TimerId,
    partition: PartitionId,
    source: InterruptSourceId,
    interrupt: InterruptLinePort,
    state: Arc<Mutex<TimerState>>,
}

impl ProgrammableTimer {
    pub fn new(
        id: TimerId,
        partition: PartitionId,
        source: InterruptSourceId,
        interrupt: InterruptLinePort,
    ) -> Self {
        Self {
            id,
            partition,
            source,
            interrupt,
            state: Arc::new(Mutex::new(TimerState::new())),
        }
    }

    pub const fn id(&self) -> TimerId {
        self.id
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn source(&self) -> InterruptSourceId {
        self.source
    }

    pub const fn interrupt(&self) -> &InterruptLinePort {
        &self.interrupt
    }

    pub fn arm_at(
        &self,
        context: &mut SchedulerContext<'_>,
        deadline: Tick,
    ) -> Result<PartitionEventId, TimerError> {
        let now = context.now();
        if deadline < now {
            return Err(TimerError::DeadlineInPast { now, deadline });
        }

        let generation = {
            let mut state = self.state.lock().expect("timer state lock");
            state.arm(now, deadline)
        };
        let delay = deadline - now;
        let state = Arc::clone(&self.state);
        let interrupt = self.interrupt.clone();
        let source = self.source;

        context
            .schedule_remote_after(self.partition, delay, move |context| {
                let should_fire = state
                    .lock()
                    .expect("timer state lock")
                    .expire(generation, context.now());
                if should_fire {
                    if let Err(error) = interrupt.assert(context, source) {
                        state.lock().expect("timer state lock").record_signal_error(
                            generation,
                            context.now(),
                            error,
                        );
                    }
                }
            })
            .map_err(TimerError::Scheduler)
    }

    pub fn snapshot(&self) -> TimerSnapshot {
        let state = self.state.lock().expect("timer state lock");
        TimerSnapshot::new(
            self.id,
            self.partition,
            self.source,
            state.next_deadline,
            state.arms.clone(),
            state.expiries.clone(),
            state.signal_errors.clone(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TimerState {
    generation: u64,
    next_deadline: Option<Tick>,
    arms: Vec<TimerArm>,
    expiries: Vec<TimerExpiry>,
    signal_errors: Vec<TimerSignalError>,
}

impl TimerState {
    const fn new() -> Self {
        Self {
            generation: 0,
            next_deadline: None,
            arms: Vec::new(),
            expiries: Vec::new(),
            signal_errors: Vec::new(),
        }
    }

    fn arm(&mut self, programmed_tick: Tick, deadline: Tick) -> u64 {
        self.generation += 1;
        self.next_deadline = Some(deadline);
        self.arms
            .push(TimerArm::new(self.generation, programmed_tick, deadline));
        self.generation
    }

    fn expire(&mut self, generation: u64, tick: Tick) -> bool {
        if self.generation != generation || self.next_deadline != Some(tick) {
            return false;
        }

        self.next_deadline = None;
        self.expiries.push(TimerExpiry::new(generation, tick));
        true
    }

    fn record_signal_error(&mut self, generation: u64, tick: Tick, error: InterruptError) {
        self.signal_errors
            .push(TimerSignalError::new(generation, tick, error));
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimerError {
    DeadlineInPast { now: Tick, deadline: Tick },
    Scheduler(SchedulerError),
    Interrupt(InterruptError),
}

impl fmt::Display for TimerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeadlineInPast { now, deadline } => {
                write!(
                    formatter,
                    "cannot arm timer for deadline {deadline}; current tick is {now}"
                )
            }
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Interrupt(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for TimerError {}

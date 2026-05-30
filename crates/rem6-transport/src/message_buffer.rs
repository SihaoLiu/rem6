use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportMessageBufferConfig {
    strict_fifo: bool,
    allow_zero_latency: bool,
}

impl TransportMessageBufferConfig {
    pub const fn strict_fifo() -> Self {
        Self {
            strict_fifo: true,
            allow_zero_latency: false,
        }
    }

    pub const fn unordered() -> Self {
        Self {
            strict_fifo: false,
            allow_zero_latency: false,
        }
    }

    pub const fn with_allow_zero_latency(mut self, allow_zero_latency: bool) -> Self {
        self.allow_zero_latency = allow_zero_latency;
        self
    }

    pub const fn strict_fifo_enabled(self) -> bool {
        self.strict_fifo
    }

    pub const fn zero_latency_allowed(self) -> bool {
        self.allow_zero_latency
    }
}

impl Default for TransportMessageBufferConfig {
    fn default() -> Self {
        Self::strict_fifo()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportMessageBufferError {
    ZeroLatency {
        current_tick: Tick,
    },
    TickOverflow {
        current_tick: Tick,
        delta: Tick,
    },
    SequenceOverflow {
        next_sequence: u64,
    },
    StrictFifoArrivalRegression {
        current_tick: Tick,
        delta: Tick,
        arrival_tick: Tick,
        last_arrival_tick: Tick,
    },
    SnapshotQueueOrderRegression {
        previous_arrival_tick: Tick,
        previous_sequence: u64,
        arrival_tick: Tick,
        sequence: u64,
    },
    SnapshotSequenceRegression {
        next_sequence: u64,
        minimum_next_sequence: u64,
    },
}

impl fmt::Display for TransportMessageBufferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLatency { current_tick } => write!(
                formatter,
                "transport message buffer rejected zero latency at tick {current_tick}"
            ),
            Self::TickOverflow {
                current_tick,
                delta,
            } => write!(
                formatter,
                "transport message buffer arrival tick overflow at tick {current_tick} with delta {delta}"
            ),
            Self::SequenceOverflow { next_sequence } => write!(
                formatter,
                "transport message buffer sequence overflow at sequence {next_sequence}"
            ),
            Self::StrictFifoArrivalRegression {
                current_tick,
                delta,
                arrival_tick,
                last_arrival_tick,
            } => write!(
                formatter,
                "strict FIFO arrival regressed at tick {current_tick} with delta {delta}: arrival {arrival_tick} before last arrival {last_arrival_tick}"
            ),
            Self::SnapshotQueueOrderRegression {
                previous_arrival_tick,
                previous_sequence,
                arrival_tick,
                sequence,
            } => write!(
                formatter,
                "transport message buffer snapshot order regressed from arrival {previous_arrival_tick} sequence {previous_sequence} to arrival {arrival_tick} sequence {sequence}"
            ),
            Self::SnapshotSequenceRegression {
                next_sequence,
                minimum_next_sequence,
            } => write!(
                formatter,
                "transport message buffer snapshot next sequence {next_sequence} is before required sequence {minimum_next_sequence}"
            ),
        }
    }
}

impl Error for TransportMessageBufferError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportMessageAdmission {
    arrival_tick: Tick,
    sequence: u64,
    bypassed_strict_fifo: bool,
}

impl TransportMessageAdmission {
    pub const fn arrival_tick(self) -> Tick {
        self.arrival_tick
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn bypassed_strict_fifo(self) -> bool {
        self.bypassed_strict_fifo
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportQueuedMessage<T> {
    arrival_tick: Tick,
    sequence: u64,
    bypassed_strict_fifo: bool,
    payload: T,
}

impl<T> TransportQueuedMessage<T> {
    pub const fn arrival_tick(&self) -> Tick {
        self.arrival_tick
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn bypassed_strict_fifo(&self) -> bool {
        self.bypassed_strict_fifo
    }

    pub const fn payload(&self) -> &T {
        &self.payload
    }

    pub fn into_payload(self) -> T {
        self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportMessageBufferSnapshot<T> {
    config: TransportMessageBufferConfig,
    next_sequence: u64,
    last_arrival_tick: Option<Tick>,
    last_message_bypassed_strict_fifo: bool,
    queue: Vec<TransportQueuedMessage<T>>,
}

impl<T> TransportMessageBufferSnapshot<T> {
    pub const fn config(&self) -> TransportMessageBufferConfig {
        self.config
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub const fn last_arrival_tick(&self) -> Option<Tick> {
        self.last_arrival_tick
    }

    pub const fn last_message_bypassed_strict_fifo(&self) -> bool {
        self.last_message_bypassed_strict_fifo
    }

    pub fn queued_messages(&self) -> &[TransportQueuedMessage<T>] {
        &self.queue
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportMessageBuffer<T> {
    config: TransportMessageBufferConfig,
    next_sequence: u64,
    last_arrival_tick: Option<Tick>,
    last_message_bypassed_strict_fifo: bool,
    queue: Vec<TransportQueuedMessage<T>>,
}

impl<T> TransportMessageBuffer<T> {
    pub fn new(config: TransportMessageBufferConfig) -> Self {
        Self {
            config,
            next_sequence: 0,
            last_arrival_tick: None,
            last_message_bypassed_strict_fifo: false,
            queue: Vec::new(),
        }
    }

    pub const fn config(&self) -> TransportMessageBufferConfig {
        self.config
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub const fn last_arrival_tick(&self) -> Option<Tick> {
        self.last_arrival_tick
    }

    pub const fn last_message_bypassed_strict_fifo(&self) -> bool {
        self.last_message_bypassed_strict_fifo
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn ready_tick(&self) -> Option<Tick> {
        self.queue.first().map(TransportQueuedMessage::arrival_tick)
    }

    pub fn queued_messages(&self) -> &[TransportQueuedMessage<T>] {
        &self.queue
    }

    pub fn enqueue(
        &mut self,
        current_tick: Tick,
        delta: Tick,
        payload: T,
    ) -> Result<TransportMessageAdmission, TransportMessageBufferError> {
        self.enqueue_inner(current_tick, delta, false, payload)
    }

    pub fn enqueue_bypassing_strict_fifo(
        &mut self,
        current_tick: Tick,
        delta: Tick,
        payload: T,
    ) -> Result<TransportMessageAdmission, TransportMessageBufferError> {
        self.enqueue_inner(current_tick, delta, true, payload)
    }

    pub fn pop_ready(&mut self, current_tick: Tick) -> Option<TransportQueuedMessage<T>> {
        if self
            .queue
            .first()
            .is_some_and(|message| message.arrival_tick <= current_tick)
        {
            Some(self.queue.remove(0))
        } else {
            None
        }
    }

    pub fn snapshot(&self) -> TransportMessageBufferSnapshot<T>
    where
        T: Clone,
    {
        TransportMessageBufferSnapshot {
            config: self.config,
            next_sequence: self.next_sequence,
            last_arrival_tick: self.last_arrival_tick,
            last_message_bypassed_strict_fifo: self.last_message_bypassed_strict_fifo,
            queue: self.queue.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: TransportMessageBufferSnapshot<T>,
    ) -> Result<(), TransportMessageBufferError> {
        validate_snapshot(&snapshot)?;
        self.config = snapshot.config;
        self.next_sequence = snapshot.next_sequence;
        self.last_arrival_tick = snapshot.last_arrival_tick;
        self.last_message_bypassed_strict_fifo = snapshot.last_message_bypassed_strict_fifo;
        self.queue = snapshot.queue;
        Ok(())
    }

    fn enqueue_inner(
        &mut self,
        current_tick: Tick,
        delta: Tick,
        bypassed_strict_fifo: bool,
        payload: T,
    ) -> Result<TransportMessageAdmission, TransportMessageBufferError> {
        if delta == 0 && !self.config.allow_zero_latency {
            return Err(TransportMessageBufferError::ZeroLatency { current_tick });
        }
        let arrival_tick =
            current_tick
                .checked_add(delta)
                .ok_or(TransportMessageBufferError::TickOverflow {
                    current_tick,
                    delta,
                })?;
        self.validate_arrival(current_tick, delta, arrival_tick, bypassed_strict_fifo)?;

        let sequence = self.next_sequence;
        let next_sequence =
            sequence
                .checked_add(1)
                .ok_or(TransportMessageBufferError::SequenceOverflow {
                    next_sequence: sequence,
                })?;
        let message = TransportQueuedMessage {
            arrival_tick,
            sequence,
            bypassed_strict_fifo,
            payload,
        };
        self.insert_message(message);
        self.next_sequence = next_sequence;
        self.last_arrival_tick = Some(arrival_tick);
        self.last_message_bypassed_strict_fifo = bypassed_strict_fifo;

        Ok(TransportMessageAdmission {
            arrival_tick,
            sequence,
            bypassed_strict_fifo,
        })
    }

    fn validate_arrival(
        &self,
        current_tick: Tick,
        delta: Tick,
        arrival_tick: Tick,
        bypassed_strict_fifo: bool,
    ) -> Result<(), TransportMessageBufferError> {
        if !self.config.strict_fifo
            || bypassed_strict_fifo
            || self.last_message_bypassed_strict_fifo
        {
            return Ok(());
        }

        if let Some(last_arrival_tick) = self.last_arrival_tick {
            if arrival_tick < last_arrival_tick {
                return Err(TransportMessageBufferError::StrictFifoArrivalRegression {
                    current_tick,
                    delta,
                    arrival_tick,
                    last_arrival_tick,
                });
            }
        }

        Ok(())
    }

    fn insert_message(&mut self, message: TransportQueuedMessage<T>) {
        let index = self
            .queue
            .binary_search_by(|queued| compare_message_order(queued, &message))
            .unwrap_or_else(|index| index);
        self.queue.insert(index, message);
    }
}

impl<T> Default for TransportMessageBuffer<T> {
    fn default() -> Self {
        Self::new(TransportMessageBufferConfig::default())
    }
}

fn validate_snapshot<T>(
    snapshot: &TransportMessageBufferSnapshot<T>,
) -> Result<(), TransportMessageBufferError> {
    for pair in snapshot.queue.windows(2) {
        let previous = &pair[0];
        let current = &pair[1];
        if compare_message_order(previous, current) == Ordering::Greater {
            return Err(TransportMessageBufferError::SnapshotQueueOrderRegression {
                previous_arrival_tick: previous.arrival_tick,
                previous_sequence: previous.sequence,
                arrival_tick: current.arrival_tick,
                sequence: current.sequence,
            });
        }
    }

    if let Some(max_sequence) = snapshot
        .queue
        .iter()
        .map(TransportQueuedMessage::sequence)
        .max()
    {
        let minimum_next_sequence = max_sequence.checked_add(1).ok_or(
            TransportMessageBufferError::SnapshotSequenceRegression {
                next_sequence: snapshot.next_sequence,
                minimum_next_sequence: u64::MAX,
            },
        )?;
        if snapshot.next_sequence < minimum_next_sequence {
            return Err(TransportMessageBufferError::SnapshotSequenceRegression {
                next_sequence: snapshot.next_sequence,
                minimum_next_sequence,
            });
        }
    }

    Ok(())
}

fn compare_message_order<T>(
    left: &TransportQueuedMessage<T>,
    right: &TransportQueuedMessage<T>,
) -> Ordering {
    left.arrival_tick
        .cmp(&right.arrival_tick)
        .then_with(|| left.sequence.cmp(&right.sequence))
}

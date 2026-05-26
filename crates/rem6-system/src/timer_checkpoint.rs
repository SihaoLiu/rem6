use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_interrupt::{
    InterruptError, InterruptEventKind, InterruptLineId, InterruptRoute, InterruptSourceId,
    InterruptTargetId,
};
use rem6_kernel::{PartitionId, SchedulerError};
use rem6_timer::{
    ProgrammableTimer, TimerArm, TimerError, TimerExpiry, TimerId, TimerSignalError, TimerSnapshot,
};

const TIMER_CHUNK: &str = "timer";
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimerCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: TimerSnapshot,
}

impl TimerCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: TimerSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &TimerSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct TimerCheckpointPort {
    component: CheckpointComponentId,
    timer: ProgrammableTimer,
}

impl TimerCheckpointPort {
    pub const fn new(component: CheckpointComponentId, timer: ProgrammableTimer) -> Self {
        Self { component, timer }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn timer(&self) -> ProgrammableTimer {
        self.timer.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<TimerCheckpointRecord, CheckpointError> {
        let snapshot = self.timer.snapshot();
        registry.write_chunk(&self.component, TIMER_CHUNK, encode_timer(&snapshot))?;
        Ok(TimerCheckpointRecord::new(self.component.clone(), snapshot))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<TimerCheckpointRecord, TimerCheckpointError> {
        let record = self.decode_from(registry)?;
        self.validate_snapshot(record.snapshot())?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<TimerCheckpointRecord, TimerCheckpointError> {
        let payload = registry
            .chunk(&self.component, TIMER_CHUNK)
            .ok_or_else(|| TimerCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: TIMER_CHUNK.to_string(),
            })?;
        let snapshot = decode_timer(&self.component, payload)?;
        Ok(TimerCheckpointRecord::new(self.component.clone(), snapshot))
    }

    fn validate_snapshot(&self, snapshot: &TimerSnapshot) -> Result<(), TimerCheckpointError> {
        if self.timer.id() == snapshot.id()
            && self.timer.partition() == snapshot.partition()
            && self.timer.source() == snapshot.source()
        {
            return Ok(());
        }

        Err(TimerCheckpointError::Timer {
            component: self.component.clone(),
            error: TimerError::SnapshotIdentityMismatch {
                expected_id: self.timer.id(),
                actual_id: snapshot.id(),
                expected_partition: self.timer.partition(),
                actual_partition: snapshot.partition(),
                expected_source: self.timer.source(),
                actual_source: snapshot.source(),
            },
        })
    }

    fn restore_snapshot(&self, snapshot: &TimerSnapshot) -> Result<(), TimerCheckpointError> {
        self.timer
            .restore(snapshot)
            .map_err(|error| TimerCheckpointError::Timer {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct TimerCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, TimerCheckpointPort>,
}

impl TimerCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = TimerCheckpointPort>,
    {
        let mut by_component = BTreeMap::new();
        for port in ports {
            let component = port.component().clone();
            if by_component.contains_key(&component) {
                return Err(CheckpointError::DuplicateComponent { component });
            }
            by_component.insert(component, port);
        }

        Ok(Self {
            ports: by_component,
        })
    }

    pub fn component_count(&self) -> usize {
        self.ports.len()
    }

    pub fn components(&self) -> Vec<CheckpointComponentId> {
        self.ports.keys().cloned().collect()
    }

    pub fn register_all(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        for port in self.ports.values() {
            port.register(registry)?;
        }
        Ok(())
    }

    pub fn capture_all_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Vec<TimerCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<TimerCheckpointRecord>, TimerCheckpointError> {
        let mut decoded = Vec::new();
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
            decoded.push((port, record));
        }

        let mut records = Vec::with_capacity(decoded.len());
        for (port, record) in decoded {
            port.restore_snapshot(record.snapshot())?;
            records.push(record);
        }
        Ok(records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimerCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Timer {
        component: CheckpointComponentId,
        error: TimerError,
    },
}

impl TimerCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Timer { component, .. } => component,
        }
    }
}

impl fmt::Display for TimerCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "timer checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "timer checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Timer { component, error } => write!(
                formatter,
                "timer checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for TimerCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Timer { error, .. } => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}

fn encode_timer(snapshot: &TimerSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u64(&mut payload, snapshot.id().get());
    write_u32(&mut payload, snapshot.partition().index());
    write_u32(&mut payload, snapshot.source().get());
    encode_optional_tick(&mut payload, snapshot.next_deadline());
    write_u64(&mut payload, snapshot.arms().len() as u64);
    for arm in snapshot.arms() {
        write_u64(&mut payload, arm.generation());
        write_u64(&mut payload, arm.programmed_tick());
        write_u64(&mut payload, arm.deadline());
    }
    write_u64(&mut payload, snapshot.expiries().len() as u64);
    for expiry in snapshot.expiries() {
        write_u64(&mut payload, expiry.generation());
        write_u64(&mut payload, expiry.deadline());
    }
    write_u64(&mut payload, snapshot.signal_errors().len() as u64);
    for error in snapshot.signal_errors() {
        write_u64(&mut payload, error.generation());
        write_u64(&mut payload, error.tick());
        encode_interrupt_error(&mut payload, error.error());
    }
    payload
}

fn decode_timer(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<TimerSnapshot, TimerCheckpointError> {
    let mut cursor = PayloadCursor::new(component.clone(), payload);
    let id = TimerId::new(cursor.read_u64("timer id")?);
    let partition = PartitionId::new(cursor.read_u32("timer partition")?);
    let source = InterruptSourceId::new(cursor.read_u32("timer source")?);
    let next_deadline = decode_optional_tick(&mut cursor)?;
    let arms = read_timer_arms(&mut cursor)?;
    let expiries = read_timer_expiries(&mut cursor)?;
    let signal_errors = read_timer_signal_errors(&mut cursor)?;
    cursor.finish()?;
    Ok(TimerSnapshot::new(
        id,
        partition,
        source,
        next_deadline,
        arms,
        expiries,
        signal_errors,
    ))
}

fn encode_optional_tick(payload: &mut Vec<u8>, tick: Option<u64>) {
    match tick {
        Some(tick) => {
            write_u64(payload, 1);
            write_u64(payload, tick);
        }
        None => write_u64(payload, 0),
    }
}

fn decode_optional_tick(
    cursor: &mut PayloadCursor<'_>,
) -> Result<Option<u64>, TimerCheckpointError> {
    match cursor.read_u64("timer deadline present")? {
        0 => Ok(None),
        1 => Ok(Some(cursor.read_u64("timer next deadline")?)),
        value => Err(cursor.invalid(format!("timer deadline flag has invalid value {value}"))),
    }
}

fn read_timer_arms(cursor: &mut PayloadCursor<'_>) -> Result<Vec<TimerArm>, TimerCheckpointError> {
    let count = cursor.read_count("timer arm count")?;
    let mut arms = Vec::with_capacity(count);
    for _ in 0..count {
        arms.push(TimerArm::new(
            cursor.read_u64("timer arm generation")?,
            cursor.read_u64("timer arm programmed tick")?,
            cursor.read_u64("timer arm deadline")?,
        ));
    }
    Ok(arms)
}

fn read_timer_expiries(
    cursor: &mut PayloadCursor<'_>,
) -> Result<Vec<TimerExpiry>, TimerCheckpointError> {
    let count = cursor.read_count("timer expiry count")?;
    let mut expiries = Vec::with_capacity(count);
    for _ in 0..count {
        expiries.push(TimerExpiry::new(
            cursor.read_u64("timer expiry generation")?,
            cursor.read_u64("timer expiry deadline")?,
        ));
    }
    Ok(expiries)
}

fn read_timer_signal_errors(
    cursor: &mut PayloadCursor<'_>,
) -> Result<Vec<TimerSignalError>, TimerCheckpointError> {
    let count = cursor.read_count("timer signal error count")?;
    let mut errors = Vec::with_capacity(count);
    for _ in 0..count {
        errors.push(TimerSignalError::new(
            cursor.read_u64("timer signal error generation")?,
            cursor.read_u64("timer signal error tick")?,
            decode_interrupt_error(cursor)?,
        ));
    }
    Ok(errors)
}

fn encode_interrupt_error(payload: &mut Vec<u8>, error: &InterruptError) {
    match error {
        InterruptError::ZeroSignalLatency => write_u64(payload, 0),
        InterruptError::DuplicateLine { line } => {
            write_u64(payload, 1);
            write_u64(payload, line.get());
        }
        InterruptError::UnknownLine { line } => {
            write_u64(payload, 2);
            write_u64(payload, line.get());
        }
        InterruptError::AlreadyPending { line, source } => {
            write_u64(payload, 3);
            write_u64(payload, line.get());
            write_u32(payload, source.get());
        }
        InterruptError::NotPending { line } => {
            write_u64(payload, 4);
            write_u64(payload, line.get());
        }
        InterruptError::SourceMismatch {
            line,
            expected,
            actual,
        } => {
            write_u64(payload, 5);
            write_u64(payload, line.get());
            write_u32(payload, expected.get());
            write_u32(payload, actual.get());
        }
        InterruptError::RouteMismatch {
            line,
            expected,
            actual,
        } => {
            write_u64(payload, 6);
            write_u64(payload, line.get());
            encode_interrupt_route(payload, *expected);
            encode_interrupt_route(payload, *actual);
        }
        InterruptError::NoClaimedInterrupt {
            target,
            target_partition,
        } => {
            write_u64(payload, 7);
            write_u32(payload, target.get());
            write_u32(payload, target_partition.index());
        }
        InterruptError::ClaimMismatch {
            target,
            target_partition,
            expected,
            actual,
        } => {
            write_u64(payload, 8);
            write_u32(payload, target.get());
            write_u32(payload, target_partition.index());
            write_u64(payload, expected.get());
            write_u64(payload, actual.get());
        }
        InterruptError::NonSignalDelivery { kind } => {
            write_u64(payload, 9);
            encode_interrupt_kind(payload, *kind);
        }
        InterruptError::Scheduler(error) => {
            write_u64(payload, 10);
            encode_scheduler_error(payload, error);
        }
    }
}

fn decode_interrupt_error(
    cursor: &mut PayloadCursor<'_>,
) -> Result<InterruptError, TimerCheckpointError> {
    match cursor.read_u64("interrupt error kind")? {
        0 => Ok(InterruptError::ZeroSignalLatency),
        1 => Ok(InterruptError::DuplicateLine {
            line: InterruptLineId::new(cursor.read_u64("interrupt duplicate line")?),
        }),
        2 => Ok(InterruptError::UnknownLine {
            line: InterruptLineId::new(cursor.read_u64("interrupt unknown line")?),
        }),
        3 => Ok(InterruptError::AlreadyPending {
            line: InterruptLineId::new(cursor.read_u64("interrupt pending line")?),
            source: InterruptSourceId::new(cursor.read_u32("interrupt pending source")?),
        }),
        4 => Ok(InterruptError::NotPending {
            line: InterruptLineId::new(cursor.read_u64("interrupt not-pending line")?),
        }),
        5 => Ok(InterruptError::SourceMismatch {
            line: InterruptLineId::new(cursor.read_u64("interrupt source mismatch line")?),
            expected: InterruptSourceId::new(
                cursor.read_u32("interrupt source mismatch expected")?,
            ),
            actual: InterruptSourceId::new(cursor.read_u32("interrupt source mismatch actual")?),
        }),
        6 => Ok(InterruptError::RouteMismatch {
            line: InterruptLineId::new(cursor.read_u64("interrupt route mismatch line")?),
            expected: decode_interrupt_route(cursor)?,
            actual: decode_interrupt_route(cursor)?,
        }),
        7 => Ok(InterruptError::NoClaimedInterrupt {
            target: InterruptTargetId::new(cursor.read_u32("interrupt unclaimed target")?),
            target_partition: PartitionId::new(
                cursor.read_u32("interrupt unclaimed target partition")?,
            ),
        }),
        8 => Ok(InterruptError::ClaimMismatch {
            target: InterruptTargetId::new(cursor.read_u32("interrupt claim target")?),
            target_partition: PartitionId::new(
                cursor.read_u32("interrupt claim target partition")?,
            ),
            expected: InterruptLineId::new(cursor.read_u64("interrupt claim expected line")?),
            actual: InterruptLineId::new(cursor.read_u64("interrupt claim actual line")?),
        }),
        9 => Ok(InterruptError::NonSignalDelivery {
            kind: decode_interrupt_kind(cursor, "interrupt non-signal kind")?,
        }),
        10 => Ok(InterruptError::Scheduler(decode_scheduler_error(cursor)?)),
        value => Err(cursor.invalid(format!("interrupt error has invalid kind {value}"))),
    }
}

fn encode_interrupt_kind(payload: &mut Vec<u8>, kind: InterruptEventKind) {
    write_u64(
        payload,
        match kind {
            InterruptEventKind::Assert => 0,
            InterruptEventKind::Deassert => 1,
            InterruptEventKind::Claim => 2,
            InterruptEventKind::Complete => 3,
        },
    );
}

fn decode_interrupt_kind(
    cursor: &mut PayloadCursor<'_>,
    name: &str,
) -> Result<InterruptEventKind, TimerCheckpointError> {
    match cursor.read_u64(name)? {
        0 => Ok(InterruptEventKind::Assert),
        1 => Ok(InterruptEventKind::Deassert),
        2 => Ok(InterruptEventKind::Claim),
        3 => Ok(InterruptEventKind::Complete),
        value => Err(cursor.invalid(format!("{name} has invalid kind {value}"))),
    }
}

fn encode_interrupt_route(payload: &mut Vec<u8>, route: InterruptRoute) {
    write_u64(payload, route.line().get());
    write_u32(payload, route.target().get());
    write_u32(payload, route.target_partition().index());
}

fn decode_interrupt_route(
    cursor: &mut PayloadCursor<'_>,
) -> Result<InterruptRoute, TimerCheckpointError> {
    Ok(InterruptRoute::new(
        InterruptLineId::new(cursor.read_u64("interrupt route line")?),
        InterruptTargetId::new(cursor.read_u32("interrupt route target")?),
        PartitionId::new(cursor.read_u32("interrupt route target partition")?),
    ))
}

fn encode_scheduler_error(payload: &mut Vec<u8>, error: &SchedulerError) {
    match error {
        SchedulerError::NoPartitions => write_u64(payload, 0),
        SchedulerError::ZeroLookahead => write_u64(payload, 1),
        SchedulerError::ZeroParallelWorkers => write_u64(payload, 13),
        SchedulerError::ParallelWorkerPanicked { partition } => {
            write_u64(payload, 15);
            write_u32(payload, partition.index());
        }
        SchedulerError::UnknownPartition {
            partition,
            partitions,
        } => {
            write_u64(payload, 2);
            write_u32(payload, partition.index());
            write_u32(payload, *partitions);
        }
        SchedulerError::InThePast {
            partition,
            now,
            requested,
        } => {
            write_u64(payload, 3);
            write_u32(payload, partition.index());
            write_u64(payload, *now);
            write_u64(payload, *requested);
        }
        SchedulerError::TickOverflow { now, delay } => {
            write_u64(payload, 4);
            write_u64(payload, *now);
            write_u64(payload, *delay);
        }
        SchedulerError::ZeroDelayRemoteMessage { source, target } => {
            write_u64(payload, 5);
            write_u32(payload, source.index());
            write_u32(payload, target.index());
        }
        SchedulerError::RemoteDelayBelowLookahead {
            source,
            target,
            delay,
            minimum,
        } => {
            write_u64(payload, 6);
            write_u32(payload, source.index());
            write_u32(payload, target.index());
            write_u64(payload, *delay);
            write_u64(payload, *minimum);
        }
        SchedulerError::SerialEventInParallelEpoch { partition, tick } => {
            write_u64(payload, 7);
            write_u32(payload, partition.index());
            write_u64(payload, *tick);
        }
        SchedulerError::EpochHorizonOverflow {
            partition,
            now,
            delay,
        } => {
            write_u64(payload, 12);
            write_u32(payload, partition.index());
            write_u64(payload, *now);
            write_u64(payload, *delay);
        }
        SchedulerError::SnapshotContainsPendingEvents { pending_events } => {
            write_u64(payload, 8);
            write_u64(payload, *pending_events as u64);
        }
        SchedulerError::RestoreWouldDiscardPendingEvents { pending_events } => {
            write_u64(payload, 9);
            write_u64(payload, *pending_events as u64);
        }
        SchedulerError::SnapshotPartitionCountMismatch {
            snapshot_partitions,
            scheduler_partitions,
        } => {
            write_u64(payload, 10);
            write_u32(payload, *snapshot_partitions);
            write_u32(payload, *scheduler_partitions);
        }
        SchedulerError::SnapshotLookaheadMismatch {
            snapshot_min_remote_delay,
            scheduler_min_remote_delay,
        } => {
            write_u64(payload, 11);
            write_u64(payload, *snapshot_min_remote_delay);
            write_u64(payload, *scheduler_min_remote_delay);
        }
        SchedulerError::SnapshotParallelWorkerLimitMismatch {
            snapshot_max_parallel_workers,
            scheduler_max_parallel_workers,
        } => {
            write_u64(payload, 14);
            write_u64(payload, *snapshot_max_parallel_workers as u64);
            write_u64(payload, *scheduler_max_parallel_workers as u64);
        }
    }
}

fn decode_scheduler_error(
    cursor: &mut PayloadCursor<'_>,
) -> Result<SchedulerError, TimerCheckpointError> {
    match cursor.read_u64("scheduler error kind")? {
        0 => Ok(SchedulerError::NoPartitions),
        1 => Ok(SchedulerError::ZeroLookahead),
        13 => Ok(SchedulerError::ZeroParallelWorkers),
        15 => Ok(SchedulerError::ParallelWorkerPanicked {
            partition: PartitionId::new(cursor.read_u32("scheduler panicked worker partition")?),
        }),
        2 => Ok(SchedulerError::UnknownPartition {
            partition: PartitionId::new(cursor.read_u32("scheduler unknown partition")?),
            partitions: cursor.read_u32("scheduler partition count")?,
        }),
        3 => Ok(SchedulerError::InThePast {
            partition: PartitionId::new(cursor.read_u32("scheduler past partition")?),
            now: cursor.read_u64("scheduler past now")?,
            requested: cursor.read_u64("scheduler past requested")?,
        }),
        4 => Ok(SchedulerError::TickOverflow {
            now: cursor.read_u64("scheduler overflow now")?,
            delay: cursor.read_u64("scheduler overflow delay")?,
        }),
        5 => Ok(SchedulerError::ZeroDelayRemoteMessage {
            source: PartitionId::new(cursor.read_u32("scheduler zero-delay source")?),
            target: PartitionId::new(cursor.read_u32("scheduler zero-delay target")?),
        }),
        6 => Ok(SchedulerError::RemoteDelayBelowLookahead {
            source: PartitionId::new(cursor.read_u32("scheduler lookahead source")?),
            target: PartitionId::new(cursor.read_u32("scheduler lookahead target")?),
            delay: cursor.read_u64("scheduler lookahead delay")?,
            minimum: cursor.read_u64("scheduler lookahead minimum")?,
        }),
        7 => Ok(SchedulerError::SerialEventInParallelEpoch {
            partition: PartitionId::new(cursor.read_u32("scheduler serial partition")?),
            tick: cursor.read_u64("scheduler serial tick")?,
        }),
        8 => Ok(SchedulerError::SnapshotContainsPendingEvents {
            pending_events: cursor.read_count("scheduler snapshot pending events")?,
        }),
        9 => Ok(SchedulerError::RestoreWouldDiscardPendingEvents {
            pending_events: cursor.read_count("scheduler restore pending events")?,
        }),
        10 => Ok(SchedulerError::SnapshotPartitionCountMismatch {
            snapshot_partitions: cursor.read_u32("scheduler snapshot partition count")?,
            scheduler_partitions: cursor.read_u32("scheduler live partition count")?,
        }),
        11 => Ok(SchedulerError::SnapshotLookaheadMismatch {
            snapshot_min_remote_delay: cursor.read_u64("scheduler snapshot lookahead")?,
            scheduler_min_remote_delay: cursor.read_u64("scheduler live lookahead")?,
        }),
        12 => Ok(SchedulerError::EpochHorizonOverflow {
            partition: PartitionId::new(cursor.read_u32("scheduler horizon partition")?),
            now: cursor.read_u64("scheduler horizon now")?,
            delay: cursor.read_u64("scheduler horizon delay")?,
        }),
        14 => Ok(SchedulerError::SnapshotParallelWorkerLimitMismatch {
            snapshot_max_parallel_workers: cursor
                .read_count("scheduler snapshot parallel worker limit")?,
            scheduler_max_parallel_workers: cursor
                .read_count("scheduler live parallel worker limit")?,
        }),
        value => Err(cursor.invalid(format!("scheduler error has invalid kind {value}"))),
    }
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

struct PayloadCursor<'a> {
    component: CheckpointComponentId,
    payload: &'a [u8],
    position: usize,
}

impl<'a> PayloadCursor<'a> {
    fn new(component: CheckpointComponentId, payload: &'a [u8]) -> Self {
        Self {
            component,
            payload,
            position: 0,
        }
    }

    fn read_count(&mut self, name: &str) -> Result<usize, TimerCheckpointError> {
        usize::try_from(self.read_u64(name)?)
            .map_err(|_| self.invalid(format!("{name} cannot fit in usize")))
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, TimerCheckpointError> {
        let bytes = self.read_bytes(name, U32_BYTES)?;
        let mut value = [0; U32_BYTES];
        value.copy_from_slice(bytes);
        Ok(u32::from_le_bytes(value))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, TimerCheckpointError> {
        let bytes = self.read_bytes(name, U64_BYTES)?;
        let mut value = [0; U64_BYTES];
        value.copy_from_slice(bytes);
        Ok(u64::from_le_bytes(value))
    }

    fn read_bytes(&mut self, name: &str, count: usize) -> Result<&'a [u8], TimerCheckpointError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or_else(|| self.invalid(format!("{name} at offset {} overflows", self.position)))?;
        if end > self.payload.len() {
            return Err(self.invalid(format!(
                "{name} at offset {} needs {count} bytes but payload has {} remaining",
                self.position,
                self.payload.len().saturating_sub(self.position)
            )));
        }

        let bytes = &self.payload[self.position..end];
        self.position = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), TimerCheckpointError> {
        if self.position == self.payload.len() {
            return Ok(());
        }

        Err(self.invalid(format!(
            "payload has {} trailing bytes",
            self.payload.len() - self.position
        )))
    }

    fn invalid(&self, reason: String) -> TimerCheckpointError {
        TimerCheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}

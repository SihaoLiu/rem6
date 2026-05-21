use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_kernel::{
    PartitionId, PartitionSnapshot, PartitionedScheduler, SchedulerError, SchedulerSnapshot,
};

const SCHEDULER_CHUNK: &str = "scheduler";
const FORMAT_VERSION: u64 = 2;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: SchedulerSnapshot,
}

impl SchedulerCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: SchedulerSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &SchedulerSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct SchedulerCheckpointPort {
    component: CheckpointComponentId,
    scheduler: Arc<Mutex<PartitionedScheduler>>,
}

impl SchedulerCheckpointPort {
    pub fn new(
        component: CheckpointComponentId,
        scheduler: Arc<Mutex<PartitionedScheduler>>,
    ) -> Self {
        Self {
            component,
            scheduler,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn scheduler(&self) -> Arc<Mutex<PartitionedScheduler>> {
        Arc::clone(&self.scheduler)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<SchedulerCheckpointRecord, SchedulerCheckpointError> {
        let snapshot = self
            .scheduler
            .lock()
            .expect("scheduler checkpoint lock")
            .quiescent_snapshot()
            .map_err(|error| SchedulerCheckpointError::Scheduler {
                component: self.component.clone(),
                error,
            })?;
        registry
            .write_chunk(&self.component, SCHEDULER_CHUNK, encode_snapshot(&snapshot))
            .map_err(SchedulerCheckpointError::Checkpoint)?;
        Ok(SchedulerCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<SchedulerCheckpointRecord, SchedulerCheckpointError> {
        let payload = registry
            .chunk(&self.component, SCHEDULER_CHUNK)
            .ok_or_else(|| SchedulerCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: SCHEDULER_CHUNK.to_string(),
            })?;
        let snapshot = decode_snapshot(&self.component, payload)?;
        self.scheduler
            .lock()
            .expect("scheduler checkpoint lock")
            .restore_quiescent(&snapshot)
            .map_err(|error| SchedulerCheckpointError::Scheduler {
                component: self.component.clone(),
                error,
            })?;
        Ok(SchedulerCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }
}

#[derive(Clone, Debug, Default)]
pub struct SchedulerCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, SchedulerCheckpointPort>,
}

impl SchedulerCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = SchedulerCheckpointPort>,
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
    ) -> Result<Vec<SchedulerCheckpointRecord>, SchedulerCheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<SchedulerCheckpointRecord>, SchedulerCheckpointError> {
        self.ports
            .values()
            .map(|port| port.restore_from(registry))
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchedulerCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Checkpoint(CheckpointError),
    Scheduler {
        component: CheckpointComponentId,
        error: SchedulerError,
    },
}

impl SchedulerCheckpointError {
    pub fn component(&self) -> Option<&CheckpointComponentId> {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Scheduler { component, .. } => Some(component),
            Self::Checkpoint(_) => None,
        }
    }
}

impl fmt::Display for SchedulerCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "scheduler checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "scheduler checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Checkpoint(error) => write!(formatter, "{error}"),
            Self::Scheduler { component, error } => write!(
                formatter,
                "scheduler checkpoint component {} cannot capture or restore scheduler: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for SchedulerCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Checkpoint(error) => Some(error),
            Self::Scheduler { error, .. } => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}

fn encode_snapshot(snapshot: &SchedulerSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u64(&mut payload, FORMAT_VERSION);
    write_u64(&mut payload, snapshot.now());
    write_u64(&mut payload, snapshot.min_remote_delay());
    write_u64(&mut payload, snapshot.max_parallel_workers() as u64);
    write_u64(&mut payload, snapshot.partitions().len() as u64);
    for partition in snapshot.partitions() {
        write_u32(&mut payload, partition.partition().index());
        write_u64(&mut payload, partition.now());
        write_u64(&mut payload, partition.next_event_local());
        write_u64(&mut payload, partition.next_event_order());
        write_u64(&mut payload, partition.pending_events().len() as u64);
    }
    payload
}

fn decode_snapshot(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<SchedulerSnapshot, SchedulerCheckpointError> {
    let mut cursor = PayloadCursor::new(component.clone(), payload);
    let version = cursor.read_u64("scheduler checkpoint version")?;
    if version != FORMAT_VERSION {
        return Err(cursor.invalid(format!(
            "scheduler checkpoint version {version} is unsupported"
        )));
    }

    let now = cursor.read_u64("scheduler now")?;
    let min_remote_delay = cursor.read_u64("scheduler lookahead")?;
    let max_parallel_workers = cursor.read_count("scheduler parallel worker limit")?;
    let partition_count = cursor.read_count("scheduler partition count")?;
    let mut partitions = Vec::with_capacity(partition_count);
    for _ in 0..partition_count {
        let partition = PartitionId::new(cursor.read_u32("scheduler partition")?);
        let partition_now = cursor.read_u64("scheduler partition now")?;
        let next_event_local = cursor.read_u64("scheduler next event local")?;
        let next_event_order = cursor.read_u64("scheduler next event order")?;
        let pending_events = cursor.read_count("scheduler pending event count")?;
        if pending_events != 0 {
            return Err(cursor.invalid(format!(
                "quiescent scheduler checkpoint contains {pending_events} pending events"
            )));
        }
        partitions.push(PartitionSnapshot::quiescent(
            partition,
            partition_now,
            next_event_local,
            next_event_order,
        ));
    }
    cursor.finish()?;
    Ok(SchedulerSnapshot::with_parallel_worker_limit(
        now,
        min_remote_delay,
        max_parallel_workers,
        partitions,
    ))
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

    fn read_count(&mut self, name: &str) -> Result<usize, SchedulerCheckpointError> {
        usize::try_from(self.read_u64(name)?)
            .map_err(|_| self.invalid(format!("{name} does not fit host usize")))
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, SchedulerCheckpointError> {
        let bytes = self.read_bytes(name, U32_BYTES)?;
        Ok(u32::from_le_bytes(
            bytes.try_into().expect("u32 byte count checked"),
        ))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, SchedulerCheckpointError> {
        let bytes = self.read_bytes(name, U64_BYTES)?;
        Ok(u64::from_le_bytes(
            bytes.try_into().expect("u64 byte count checked"),
        ))
    }

    fn read_bytes(
        &mut self,
        name: &str,
        count: usize,
    ) -> Result<&'a [u8], SchedulerCheckpointError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or_else(|| self.invalid(format!("{name} offset overflow")))?;
        if end > self.payload.len() {
            return Err(self.invalid(format!(
                "{name} truncated at byte {} while reading {count} bytes",
                self.position
            )));
        }
        let bytes = &self.payload[self.position..end];
        self.position = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), SchedulerCheckpointError> {
        if self.position != self.payload.len() {
            return Err(self.invalid(format!(
                "scheduler checkpoint has {} trailing bytes",
                self.payload.len() - self.position
            )));
        }
        Ok(())
    }

    fn invalid(&self, reason: String) -> SchedulerCheckpointError {
        SchedulerCheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}

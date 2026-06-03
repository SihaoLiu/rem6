use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_kernel::{PartitionId, Tick};

use crate::{
    GuestFutexAddress, GuestFutexError, GuestFutexKey, GuestFutexTable, GuestFutexTableSnapshot,
    GuestFutexWaiter, GuestThreadGroupId, GuestThreadId,
};

const GUEST_FUTEX_CHUNK: &str = "guest-futex";
const GUEST_FUTEX_CHECKPOINT_VERSION: u64 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const WAITER_RECORD_BYTES: usize = U64_BYTES * 4 + U32_BYTES * 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestFutexCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: GuestFutexTableSnapshot,
}

impl GuestFutexCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: GuestFutexTableSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &GuestFutexTableSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct GuestFutexCheckpointPort {
    component: CheckpointComponentId,
    table: Arc<Mutex<GuestFutexTable>>,
}

impl GuestFutexCheckpointPort {
    pub fn new(component: CheckpointComponentId, table: Arc<Mutex<GuestFutexTable>>) -> Self {
        Self { component, table }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn table(&self) -> Arc<Mutex<GuestFutexTable>> {
        Arc::clone(&self.table)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<GuestFutexCheckpointRecord, CheckpointError> {
        let snapshot = self
            .table
            .lock()
            .expect("guest futex checkpoint table lock")
            .snapshot();
        registry.write_chunk(
            &self.component,
            GUEST_FUTEX_CHUNK,
            encode_guest_futex_table(&snapshot),
        )?;
        Ok(GuestFutexCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<GuestFutexCheckpointRecord, GuestFutexCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<GuestFutexCheckpointRecord, GuestFutexCheckpointError> {
        let payload = registry
            .chunk(&self.component, GUEST_FUTEX_CHUNK)
            .ok_or_else(|| GuestFutexCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: GUEST_FUTEX_CHUNK.to_string(),
            })?;
        let snapshot = decode_guest_futex_table(&self.component, payload)?;
        Ok(GuestFutexCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn validate_snapshot(
        &self,
        snapshot: &GuestFutexTableSnapshot,
    ) -> Result<(), GuestFutexCheckpointError> {
        GuestFutexTable::from_snapshot(snapshot.clone())
            .map(|_| ())
            .map_err(|error| invalid_guest_futex_chunk(self.component.clone(), error.to_string()))
    }

    fn restore_snapshot(
        &self,
        snapshot: &GuestFutexTableSnapshot,
    ) -> Result<(), GuestFutexCheckpointError> {
        self.table
            .lock()
            .expect("guest futex checkpoint table lock")
            .restore_snapshot(snapshot.clone())
            .map_err(|error| invalid_guest_futex_chunk(self.component.clone(), error.to_string()))
    }
}

#[derive(Clone, Debug, Default)]
pub struct GuestFutexCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, GuestFutexCheckpointPort>,
}

impl GuestFutexCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = GuestFutexCheckpointPort>,
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
    ) -> Result<Vec<GuestFutexCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<GuestFutexCheckpointRecord>, GuestFutexCheckpointError> {
        self.validate_restore_from(registry)?;
        let mut records = Vec::new();
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
            records.push((port, record));
        }

        let mut restored = Vec::with_capacity(records.len());
        for (port, record) in records {
            port.restore_snapshot(record.snapshot())?;
            restored.push(record);
        }
        Ok(restored)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), GuestFutexCheckpointError> {
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuestFutexCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
}

impl GuestFutexCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. } | Self::InvalidChunk { component, .. } => {
                component
            }
        }
    }
}

impl fmt::Display for GuestFutexCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "guest futex checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "guest futex checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
        }
    }
}

impl Error for GuestFutexCheckpointError {}

fn encode_guest_futex_table(snapshot: &GuestFutexTableSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    push_u64(&mut payload, GUEST_FUTEX_CHECKPOINT_VERSION);
    push_u64(&mut payload, snapshot.waiters().len() as u64);
    for waiter in snapshot.waiters() {
        let key = waiter.key();
        push_u64(&mut payload, key.address().get());
        push_u64(&mut payload, key.thread_group().get());
        push_u64(&mut payload, waiter.thread().get());
        push_u32(&mut payload, waiter.partition().index());
        push_u64(&mut payload, waiter.enqueued_tick());
        push_u32(&mut payload, waiter.bitset());
    }
    payload
}

fn decode_guest_futex_table(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<GuestFutexTableSnapshot, GuestFutexCheckpointError> {
    let mut cursor = PayloadCursor::new(component.clone(), payload);
    let version = cursor.read_u64("guest futex checkpoint version")?;
    if version != GUEST_FUTEX_CHECKPOINT_VERSION {
        return Err(cursor.invalid(format!(
            "unsupported guest futex checkpoint version {version}"
        )));
    }

    let waiter_count = cursor.read_record_count("guest futex waiter count", WAITER_RECORD_BYTES)?;
    let mut waiters = Vec::with_capacity(waiter_count);
    for _ in 0..waiter_count {
        waiters.push(read_waiter(&mut cursor)?);
    }
    cursor.finish()?;

    let snapshot = GuestFutexTableSnapshot::new(waiters);
    let table = GuestFutexTable::from_snapshot(snapshot)
        .map_err(|error| invalid_guest_futex_chunk(component.clone(), error.to_string()))?;
    Ok(table.snapshot())
}

fn read_waiter(
    cursor: &mut PayloadCursor<'_>,
) -> Result<GuestFutexWaiter, GuestFutexCheckpointError> {
    let address = GuestFutexAddress::new(cursor.read_u64("guest futex address")?);
    let thread_group = GuestThreadGroupId::new(cursor.read_u64("guest futex thread group")?);
    let thread = GuestThreadId::new(cursor.read_u64("guest futex thread")?);
    let partition = PartitionId::new(cursor.read_u32("guest futex partition")?);
    let enqueued_tick: Tick = cursor.read_u64("guest futex enqueued tick")?;
    let bitset = cursor.read_u32("guest futex bitset")?;
    GuestFutexWaiter::new(
        GuestFutexKey::new(address, thread_group),
        thread,
        partition,
        enqueued_tick,
        bitset,
    )
    .map_err(|error| cursor.invalid(guest_futex_error_reason(error)))
}

fn guest_futex_error_reason(error: GuestFutexError) -> String {
    error.to_string()
}

fn invalid_guest_futex_chunk(
    component: CheckpointComponentId,
    reason: impl Into<String>,
) -> GuestFutexCheckpointError {
    GuestFutexCheckpointError::InvalidChunk {
        component,
        reason: reason.into(),
    }
}

fn push_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

struct PayloadCursor<'a> {
    component: CheckpointComponentId,
    payload: &'a [u8],
    offset: usize,
}

impl<'a> PayloadCursor<'a> {
    fn new(component: CheckpointComponentId, payload: &'a [u8]) -> Self {
        Self {
            component,
            payload,
            offset: 0,
        }
    }

    fn invalid(&self, reason: impl Into<String>) -> GuestFutexCheckpointError {
        invalid_guest_futex_chunk(self.component.clone(), reason)
    }

    fn read_usize(&mut self, name: &str) -> Result<usize, GuestFutexCheckpointError> {
        usize::try_from(self.read_u64(name)?)
            .map_err(|_| self.invalid(format!("{name} does not fit usize")))
    }

    fn read_record_count(
        &mut self,
        name: &str,
        record_bytes: usize,
    ) -> Result<usize, GuestFutexCheckpointError> {
        let count = self.read_usize(name)?;
        let max_count = self.remaining() / record_bytes;
        if count > max_count {
            return Err(self.invalid(format!(
                "{name} {count} exceeds payload capacity {max_count}"
            )));
        }
        Ok(count)
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, GuestFutexCheckpointError> {
        let bytes = self.read_exact(name, U32_BYTES)?;
        Ok(u32::from_le_bytes(
            bytes
                .try_into()
                .expect("checkpoint u32 slice width is fixed"),
        ))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, GuestFutexCheckpointError> {
        let bytes = self.read_exact(name, U64_BYTES)?;
        Ok(u64::from_le_bytes(
            bytes
                .try_into()
                .expect("checkpoint u64 slice width is fixed"),
        ))
    }

    fn read_exact(
        &mut self,
        name: &str,
        len: usize,
    ) -> Result<&'a [u8], GuestFutexCheckpointError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| self.invalid(format!("{name} cursor overflow")))?;
        let bytes = self
            .payload
            .get(self.offset..end)
            .ok_or_else(|| self.invalid(format!("{name} is truncated")))?;
        self.offset = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), GuestFutexCheckpointError> {
        if self.offset == self.payload.len() {
            return Ok(());
        }

        Err(self.invalid("trailing bytes after guest futex checkpoint payload"))
    }

    fn remaining(&self) -> usize {
        self.payload.len().saturating_sub(self.offset)
    }
}

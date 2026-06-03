use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};

use crate::{
    GuestChildStatus, GuestProcessGroupId, GuestProcessId, GuestSignal, GuestWaitQueue,
    GuestWaitQueueSnapshot, GuestWaitStatus, GuestWaitStatusError,
};

const GUEST_WAIT_CHUNK: &str = "guest-wait";
const GUEST_WAIT_CHECKPOINT_VERSION: u64 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const CHILD_RECORD_BYTES: usize = U32_BYTES * 4 + U64_BYTES;
const WAIT_STATUS_EXITED: u32 = 0;
const WAIT_STATUS_SIGNALED: u32 = 1;
const WAIT_STATUS_STOPPED: u32 = 2;
const WAIT_STATUS_CONTINUED: u32 = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestWaitCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: GuestWaitQueueSnapshot,
}

impl GuestWaitCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: GuestWaitQueueSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &GuestWaitQueueSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct GuestWaitCheckpointPort {
    component: CheckpointComponentId,
    queue: Arc<Mutex<GuestWaitQueue>>,
}

impl GuestWaitCheckpointPort {
    pub fn new(component: CheckpointComponentId, queue: Arc<Mutex<GuestWaitQueue>>) -> Self {
        Self { component, queue }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn queue(&self) -> Arc<Mutex<GuestWaitQueue>> {
        Arc::clone(&self.queue)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<GuestWaitCheckpointRecord, CheckpointError> {
        let snapshot = self
            .queue
            .lock()
            .expect("guest wait checkpoint queue lock")
            .snapshot();
        registry.write_chunk(
            &self.component,
            GUEST_WAIT_CHUNK,
            encode_guest_wait_queue(&snapshot),
        )?;
        Ok(GuestWaitCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<GuestWaitCheckpointRecord, GuestWaitCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_snapshot(record.snapshot());
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<GuestWaitCheckpointRecord, GuestWaitCheckpointError> {
        let payload = registry
            .chunk(&self.component, GUEST_WAIT_CHUNK)
            .ok_or_else(|| GuestWaitCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: GUEST_WAIT_CHUNK.to_string(),
            })?;
        let snapshot = decode_guest_wait_queue(&self.component, payload)?;
        Ok(GuestWaitCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn restore_snapshot(&self, snapshot: &GuestWaitQueueSnapshot) {
        self.queue
            .lock()
            .expect("guest wait checkpoint queue lock")
            .restore_snapshot(snapshot.clone());
    }
}

#[derive(Clone, Debug, Default)]
pub struct GuestWaitCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, GuestWaitCheckpointPort>,
}

impl GuestWaitCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = GuestWaitCheckpointPort>,
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
    ) -> Result<Vec<GuestWaitCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<GuestWaitCheckpointRecord>, GuestWaitCheckpointError> {
        self.validate_restore_from(registry)?;
        let mut records = Vec::new();
        for port in self.ports.values() {
            records.push((port, port.decode_from(registry)?));
        }

        let mut restored = Vec::with_capacity(records.len());
        for (port, record) in records {
            port.restore_snapshot(record.snapshot());
            restored.push(record);
        }
        Ok(restored)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), GuestWaitCheckpointError> {
        for port in self.ports.values() {
            port.decode_from(registry)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuestWaitCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
}

impl GuestWaitCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. } | Self::InvalidChunk { component, .. } => {
                component
            }
        }
    }
}

impl fmt::Display for GuestWaitCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "guest wait checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "guest wait checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
        }
    }
}

impl Error for GuestWaitCheckpointError {}

fn encode_guest_wait_queue(snapshot: &GuestWaitQueueSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    push_u64(&mut payload, GUEST_WAIT_CHECKPOINT_VERSION);
    push_u32(&mut payload, snapshot.current_process_group().get());
    push_u64(&mut payload, snapshot.pending().len() as u64);
    for child in snapshot.pending() {
        push_u32(&mut payload, child.pid().get());
        push_u32(&mut payload, child.process_group().get());
        encode_guest_wait_status(&mut payload, child.status());
    }
    payload
}

fn encode_guest_wait_status(payload: &mut Vec<u8>, status: GuestWaitStatus) {
    match status {
        GuestWaitStatus::Exited { code } => {
            push_u32(payload, WAIT_STATUS_EXITED);
            push_u32(payload, u32::from(code));
            push_u64(payload, 0);
        }
        GuestWaitStatus::Signaled {
            signal,
            core_dumped,
        } => {
            push_u32(payload, WAIT_STATUS_SIGNALED);
            push_u32(payload, u32::from(signal.number()));
            push_u64(payload, u64::from(core_dumped));
        }
        GuestWaitStatus::Stopped { signal } => {
            push_u32(payload, WAIT_STATUS_STOPPED);
            push_u32(payload, u32::from(signal.number()));
            push_u64(payload, 0);
        }
        GuestWaitStatus::Continued => {
            push_u32(payload, WAIT_STATUS_CONTINUED);
            push_u32(payload, 0);
            push_u64(payload, 0);
        }
    }
}

fn decode_guest_wait_queue(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<GuestWaitQueueSnapshot, GuestWaitCheckpointError> {
    let mut cursor = PayloadCursor::new(component.clone(), payload);
    let version = cursor.read_u64("guest wait checkpoint version")?;
    if version != GUEST_WAIT_CHECKPOINT_VERSION {
        return Err(cursor.invalid(format!(
            "unsupported guest wait checkpoint version {version}"
        )));
    }
    let current_process_group =
        GuestProcessGroupId::new(cursor.read_u32("guest wait current process group")?)
            .map_err(|error| cursor.invalid(guest_wait_status_error_reason(error)))?;
    let child_count =
        cursor.read_record_count("guest wait child status count", CHILD_RECORD_BYTES)?;
    let mut pending = Vec::with_capacity(child_count);
    for _ in 0..child_count {
        pending.push(read_child_status(&mut cursor)?);
    }
    cursor.finish()?;

    Ok(
        GuestWaitQueue::from_snapshot(GuestWaitQueueSnapshot::new(current_process_group, pending))
            .snapshot(),
    )
}

fn read_child_status(
    cursor: &mut PayloadCursor<'_>,
) -> Result<GuestChildStatus, GuestWaitCheckpointError> {
    let pid = GuestProcessId::new(cursor.read_u32("guest wait child pid")?)
        .map_err(|error| cursor.invalid(guest_wait_status_error_reason(error)))?;
    let process_group =
        GuestProcessGroupId::new(cursor.read_u32("guest wait child process group")?)
            .map_err(|error| cursor.invalid(guest_wait_status_error_reason(error)))?;
    let status_tag = cursor.read_u32("guest wait child status tag")?;
    let status_value = cursor.read_u32("guest wait child status value")?;
    let core_dumped = cursor.read_bool_flag("guest wait child core-dumped")?;
    let status = decode_guest_wait_status(cursor, status_tag, status_value, core_dumped)?;
    Ok(GuestChildStatus::new(pid, process_group, status))
}

fn decode_guest_wait_status(
    cursor: &PayloadCursor<'_>,
    tag: u32,
    value: u32,
    core_dumped: bool,
) -> Result<GuestWaitStatus, GuestWaitCheckpointError> {
    match tag {
        WAIT_STATUS_EXITED => {
            if core_dumped {
                return Err(cursor.invalid("exited child status cannot set core-dumped flag"));
            }
            let code = u8::try_from(value)
                .map_err(|_| cursor.invalid("guest wait exit code is outside u8 range"))?;
            Ok(GuestWaitStatus::exited(code))
        }
        WAIT_STATUS_SIGNALED => {
            let signal = u8::try_from(value)
                .map_err(|_| cursor.invalid("guest wait signal is outside u8 range"))?;
            GuestSignal::new(signal)
                .map(|signal| GuestWaitStatus::signaled(signal, core_dumped))
                .map_err(|error| cursor.invalid(guest_wait_status_error_reason(error)))
        }
        WAIT_STATUS_STOPPED => {
            if core_dumped {
                return Err(cursor.invalid("stopped child status cannot set core-dumped flag"));
            }
            let signal = u8::try_from(value)
                .map_err(|_| cursor.invalid("guest wait stop signal is outside u8 range"))?;
            GuestSignal::new(signal)
                .map(GuestWaitStatus::stopped)
                .map_err(|error| cursor.invalid(guest_wait_status_error_reason(error)))
        }
        WAIT_STATUS_CONTINUED => {
            if value != 0 {
                return Err(cursor.invalid("continued child status value must be zero"));
            }
            if core_dumped {
                return Err(cursor.invalid("continued child status cannot set core-dumped flag"));
            }
            Ok(GuestWaitStatus::continued())
        }
        value => Err(cursor.invalid(format!("unsupported guest wait status tag {value}"))),
    }
}

fn guest_wait_status_error_reason(error: GuestWaitStatusError) -> String {
    error.to_string()
}

fn invalid_guest_wait_chunk(
    component: CheckpointComponentId,
    reason: impl Into<String>,
) -> GuestWaitCheckpointError {
    GuestWaitCheckpointError::InvalidChunk {
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

    fn invalid(&self, reason: impl Into<String>) -> GuestWaitCheckpointError {
        invalid_guest_wait_chunk(self.component.clone(), reason)
    }

    fn read_bool_flag(&mut self, name: &str) -> Result<bool, GuestWaitCheckpointError> {
        match self.read_u64(name)? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(self.invalid(format!("{name} flag must be 0 or 1, got {value}"))),
        }
    }

    fn read_usize(&mut self, name: &str) -> Result<usize, GuestWaitCheckpointError> {
        usize::try_from(self.read_u64(name)?)
            .map_err(|_| self.invalid(format!("{name} does not fit usize")))
    }

    fn read_record_count(
        &mut self,
        name: &str,
        record_bytes: usize,
    ) -> Result<usize, GuestWaitCheckpointError> {
        let count = self.read_usize(name)?;
        let max_count = self.remaining() / record_bytes;
        if count > max_count {
            return Err(self.invalid(format!(
                "{name} {count} exceeds payload capacity {max_count}"
            )));
        }
        Ok(count)
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, GuestWaitCheckpointError> {
        let bytes = self.read_exact(name, U32_BYTES)?;
        Ok(u32::from_le_bytes(
            bytes
                .try_into()
                .expect("checkpoint u32 slice width is fixed"),
        ))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, GuestWaitCheckpointError> {
        let bytes = self.read_exact(name, U64_BYTES)?;
        Ok(u64::from_le_bytes(
            bytes
                .try_into()
                .expect("checkpoint u64 slice width is fixed"),
        ))
    }

    fn read_exact(&mut self, name: &str, len: usize) -> Result<&'a [u8], GuestWaitCheckpointError> {
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

    fn finish(&self) -> Result<(), GuestWaitCheckpointError> {
        if self.offset == self.payload.len() {
            return Ok(());
        }

        Err(self.invalid("trailing bytes after guest wait checkpoint payload"))
    }

    fn remaining(&self) -> usize {
        self.payload.len().saturating_sub(self.offset)
    }
}

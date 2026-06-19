use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};

use crate::{
    GuestFd, GuestFdEntry, GuestFdError, GuestFdSnapshotEntry, GuestFdTable, GuestFdTableSnapshot,
    GuestFileDescription, GuestFileDescriptionId, GuestFileOffset, GuestFileSignalOwner,
    GuestFileSignalOwnerKind, GuestFileStatusFlags, GuestHostFd,
};

const GUEST_FD_CHUNK: &str = "guest-fd";
const GUEST_FD_CHECKPOINT_VERSION: u64 = 4;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const DESCRIPTION_RECORD_MIN_BYTES: usize = U64_BYTES * 5 + U32_BYTES * 3;
const ENTRY_RECORD_BYTES: usize = U32_BYTES + U64_BYTES * 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestFdCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: GuestFdTableSnapshot,
}

impl GuestFdCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: GuestFdTableSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &GuestFdTableSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct GuestFdCheckpointPort {
    component: CheckpointComponentId,
    table: Arc<Mutex<GuestFdTable>>,
}

impl GuestFdCheckpointPort {
    pub fn new(component: CheckpointComponentId, table: Arc<Mutex<GuestFdTable>>) -> Self {
        Self { component, table }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn table(&self) -> Arc<Mutex<GuestFdTable>> {
        Arc::clone(&self.table)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<GuestFdCheckpointRecord, CheckpointError> {
        let snapshot = self
            .table
            .lock()
            .expect("guest fd checkpoint table lock")
            .snapshot();
        registry.write_chunk(
            &self.component,
            GUEST_FD_CHUNK,
            encode_guest_fd_table(&snapshot),
        )?;
        Ok(GuestFdCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<GuestFdCheckpointRecord, GuestFdCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<GuestFdCheckpointRecord, GuestFdCheckpointError> {
        let payload = registry
            .chunk(&self.component, GUEST_FD_CHUNK)
            .ok_or_else(|| GuestFdCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: GUEST_FD_CHUNK.to_string(),
            })?;
        let snapshot = decode_guest_fd_table(&self.component, payload)?;
        Ok(GuestFdCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn validate_snapshot(
        &self,
        snapshot: &GuestFdTableSnapshot,
    ) -> Result<(), GuestFdCheckpointError> {
        GuestFdTable::from_snapshot(snapshot.clone())
            .map(|_| ())
            .map_err(|error| invalid_guest_fd_chunk(self.component.clone(), error.to_string()))
    }

    fn restore_snapshot(
        &self,
        snapshot: &GuestFdTableSnapshot,
    ) -> Result<(), GuestFdCheckpointError> {
        self.table
            .lock()
            .expect("guest fd checkpoint table lock")
            .restore_snapshot(snapshot.clone())
            .map_err(|error| invalid_guest_fd_chunk(self.component.clone(), error.to_string()))
    }
}

#[derive(Clone, Debug, Default)]
pub struct GuestFdCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, GuestFdCheckpointPort>,
}

impl GuestFdCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = GuestFdCheckpointPort>,
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
    ) -> Result<Vec<GuestFdCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<GuestFdCheckpointRecord>, GuestFdCheckpointError> {
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
    ) -> Result<(), GuestFdCheckpointError> {
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuestFdCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
}

impl GuestFdCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. } | Self::InvalidChunk { component, .. } => {
                component
            }
        }
    }
}

impl fmt::Display for GuestFdCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "guest fd checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "guest fd checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
        }
    }
}

impl Error for GuestFdCheckpointError {}

fn encode_guest_fd_table(snapshot: &GuestFdTableSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    push_u64(&mut payload, GUEST_FD_CHECKPOINT_VERSION);
    push_u64(&mut payload, snapshot.descriptions().len() as u64);
    for description in snapshot.descriptions() {
        push_u64(&mut payload, description.id().get());
        match description.host_fd() {
            Some(host_fd) => {
                push_u64(&mut payload, 1);
                push_u64(
                    &mut payload,
                    u64::try_from(host_fd.get()).expect("guest host fd is nonnegative"),
                );
            }
            None => {
                push_u64(&mut payload, 0);
                push_u64(&mut payload, 0);
            }
        }
        push_u32(&mut payload, description.status_flags().bits());
        push_u64(&mut payload, description.file_offset().get());
        let signal_owner = description.signal_owner();
        push_u64(&mut payload, signal_owner.id() as i64 as u64);
        push_u32(&mut payload, signal_owner.kind().checkpoint_tag());
        push_u32(&mut payload, description.signal_number());
    }
    push_u64(&mut payload, snapshot.entries().len() as u64);
    for entry in snapshot.entries() {
        push_u32(&mut payload, entry.fd().get());
        push_u64(&mut payload, entry.entry().description().get());
        push_u64(&mut payload, u64::from(entry.entry().close_on_exec()));
    }
    payload
}

fn decode_guest_fd_table(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<GuestFdTableSnapshot, GuestFdCheckpointError> {
    let mut cursor = PayloadCursor::new(component.clone(), payload);
    let version = cursor.read_u64("guest fd checkpoint version")?;
    if version != GUEST_FD_CHECKPOINT_VERSION {
        return Err(cursor.invalid(format!("unsupported guest fd checkpoint version {version}")));
    }

    let description_count =
        cursor.read_record_count("guest fd description count", DESCRIPTION_RECORD_MIN_BYTES)?;
    let mut descriptions = Vec::with_capacity(description_count);
    for _ in 0..description_count {
        descriptions.push(read_description(&mut cursor)?);
    }

    let entry_count = cursor.read_record_count("guest fd entry count", ENTRY_RECORD_BYTES)?;
    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        entries.push(read_entry(&mut cursor)?);
    }
    cursor.finish()?;

    let snapshot = GuestFdTableSnapshot::new(entries, descriptions);
    let table = GuestFdTable::from_snapshot(snapshot)
        .map_err(|error| invalid_guest_fd_chunk(component.clone(), error.to_string()))?;
    Ok(table.snapshot())
}

fn read_description(
    cursor: &mut PayloadCursor<'_>,
) -> Result<GuestFileDescription, GuestFdCheckpointError> {
    let id = GuestFileDescriptionId::new(cursor.read_u64("guest fd description id")?);
    let host_present = cursor.read_bool_flag("guest fd host presence")?;
    let host_fd = cursor.read_u64("guest fd host fd")?;
    let status_flags = GuestFileStatusFlags::new(cursor.read_u32("guest fd status flags")?);
    let file_offset = GuestFileOffset::new(cursor.read_u64("guest fd file offset")?);
    let signal_owner_id = i32::try_from(cursor.read_u64("guest fd signal owner")? as i64)
        .map_err(|_| cursor.invalid("guest fd signal owner is outside i32 range"))?;
    let signal_owner_kind = cursor.read_u32("guest fd signal owner kind")?;
    let signal_owner_kind = GuestFileSignalOwnerKind::from_checkpoint_tag(signal_owner_kind)
        .ok_or_else(|| cursor.invalid("guest fd signal owner kind is invalid"))?;
    let signal_number = cursor.read_u32("guest fd signal number")?;
    if signal_owner_id < 0 {
        return Err(cursor.invalid("guest fd signal owner id is negative"));
    }
    let signal_owner = GuestFileSignalOwner::from_kind_and_id(signal_owner_kind, signal_owner_id)
        .map_err(|error| cursor.invalid(error.to_string()))?;

    let mut description = if host_present {
        let host_fd = i32::try_from(host_fd)
            .map_err(|_| cursor.invalid("guest host fd is outside i32 range"))?;
        GuestFileDescription::host_backed(
            id,
            GuestHostFd::new(host_fd)
                .map_err(|error| cursor.invalid(guest_fd_error_reason(error)))?,
            status_flags,
        )
    } else {
        if host_fd != 0 {
            return Err(cursor.invalid("guest host fd must be zero when absent"));
        }
        GuestFileDescription::guest_backed(id, status_flags)
    };
    description.set_file_offset(file_offset);
    description.set_typed_signal_owner(signal_owner);
    description
        .set_signal_number(signal_number)
        .map_err(|error| cursor.invalid(guest_fd_error_reason(error)))?;
    Ok(description)
}

fn read_entry(
    cursor: &mut PayloadCursor<'_>,
) -> Result<GuestFdSnapshotEntry, GuestFdCheckpointError> {
    let fd = cursor.read_u32("guest fd")?;
    let fd =
        i32::try_from(fd).map_err(|_| cursor.invalid("guest fd is outside signed fd range"))?;
    let fd = GuestFd::new(fd).map_err(|error| cursor.invalid(guest_fd_error_reason(error)))?;
    let description = GuestFileDescriptionId::new(cursor.read_u64("guest fd entry description")?);
    let close_on_exec = cursor.read_bool_flag("guest fd close-on-exec")?;
    Ok(GuestFdSnapshotEntry::new(
        fd,
        GuestFdEntry::new(description).with_close_on_exec(close_on_exec),
    ))
}

fn guest_fd_error_reason(error: GuestFdError) -> String {
    error.to_string()
}

fn invalid_guest_fd_chunk(
    component: CheckpointComponentId,
    reason: impl Into<String>,
) -> GuestFdCheckpointError {
    GuestFdCheckpointError::InvalidChunk {
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

    fn invalid(&self, reason: impl Into<String>) -> GuestFdCheckpointError {
        invalid_guest_fd_chunk(self.component.clone(), reason)
    }

    fn read_bool_flag(&mut self, name: &str) -> Result<bool, GuestFdCheckpointError> {
        match self.read_u64(name)? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(self.invalid(format!("{name} flag must be 0 or 1, got {value}"))),
        }
    }

    fn read_usize(&mut self, name: &str) -> Result<usize, GuestFdCheckpointError> {
        usize::try_from(self.read_u64(name)?)
            .map_err(|_| self.invalid(format!("{name} does not fit usize")))
    }

    fn read_record_count(
        &mut self,
        name: &str,
        record_bytes: usize,
    ) -> Result<usize, GuestFdCheckpointError> {
        let count = self.read_usize(name)?;
        let max_count = self.remaining() / record_bytes;
        if count > max_count {
            return Err(self.invalid(format!(
                "{name} {count} exceeds payload capacity {max_count}"
            )));
        }
        Ok(count)
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, GuestFdCheckpointError> {
        let bytes = self.read_exact(name, U32_BYTES)?;
        Ok(u32::from_le_bytes(
            bytes
                .try_into()
                .expect("checkpoint u32 slice width is fixed"),
        ))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, GuestFdCheckpointError> {
        let bytes = self.read_exact(name, U64_BYTES)?;
        Ok(u64::from_le_bytes(
            bytes
                .try_into()
                .expect("checkpoint u64 slice width is fixed"),
        ))
    }

    fn read_exact(&mut self, name: &str, len: usize) -> Result<&'a [u8], GuestFdCheckpointError> {
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

    fn finish(&self) -> Result<(), GuestFdCheckpointError> {
        if self.offset == self.payload.len() {
            return Ok(());
        }

        Err(self.invalid("trailing bytes after guest fd checkpoint payload"))
    }

    fn remaining(&self) -> usize {
        self.payload.len().saturating_sub(self.offset)
    }
}

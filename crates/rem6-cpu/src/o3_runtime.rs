use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use rem6_isa_riscv::{MemoryAccessKind, Register};
use rem6_memory::Address;

use crate::o3_dependency::{O3PhysicalRegisterId, O3RegisterClass};
use crate::o3_pipeline::{
    O3PendingStateCheckpointPayload, O3PendingStateSnapshot, O3PipelineError, O3PipelineStage,
    O3WritebackTransferPolicy, O3WritebackTransferSnapshot,
};
use crate::riscv_execution_event::RiscvCpuExecutionEvent;

const O3_RUNTIME_CHECKPOINT_MAGIC: [u8; 4] = *b"O3RT";
const O3_RUNTIME_CHECKPOINT_VERSION: u8 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const O3_RUNTIME_CHECKPOINT_HEADER_BYTES: usize =
    O3_RUNTIME_CHECKPOINT_MAGIC.len() + 1 + U32_BYTES * 4;
const O3_RUNTIME_ROB_ENTRY_BYTES: usize = U64_BYTES + U64_BYTES + 1 + U32_BYTES + 1;
const O3_RUNTIME_LSQ_ENTRY_BYTES: usize = U64_BYTES + 1 + U64_BYTES + U32_BYTES + 1 + 1;
const O3_RUNTIME_RENAME_ENTRY_BYTES: usize = 1 + U32_BYTES + U32_BYTES;
const O3_RUNTIME_U32_MAX: usize = u32::MAX as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3ReorderBufferEntry {
    sequence: u64,
    pc: Address,
    destination: Option<O3PhysicalRegisterId>,
    ready: bool,
}

impl O3ReorderBufferEntry {
    pub const fn new(
        sequence: u64,
        pc: Address,
        destination: Option<O3PhysicalRegisterId>,
    ) -> Self {
        Self {
            sequence,
            pc,
            destination,
            ready: false,
        }
    }

    pub const fn with_ready(mut self, ready: bool) -> Self {
        self.ready = ready;
        self
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn pc(self) -> Address {
        self.pc
    }

    pub const fn destination(self) -> Option<O3PhysicalRegisterId> {
        self.destination
    }

    pub const fn is_ready(self) -> bool {
        self.ready
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3LoadStoreQueueKind {
    Load,
    Store,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3LoadStoreQueueEntry {
    sequence: u64,
    address: Option<Address>,
    bytes: u32,
    kind: O3LoadStoreQueueKind,
    completed: bool,
}

impl O3LoadStoreQueueEntry {
    pub const fn load(sequence: u64, address: Option<Address>, bytes: u32) -> Self {
        Self {
            sequence,
            address,
            bytes,
            kind: O3LoadStoreQueueKind::Load,
            completed: false,
        }
    }

    pub const fn store(sequence: u64, address: Option<Address>, bytes: u32) -> Self {
        Self {
            sequence,
            address,
            bytes,
            kind: O3LoadStoreQueueKind::Store,
            completed: false,
        }
    }

    pub const fn with_completed(mut self, completed: bool) -> Self {
        self.completed = completed;
        self
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn address(self) -> Option<Address> {
        self.address
    }

    pub const fn bytes(self) -> u32 {
        self.bytes
    }

    pub const fn kind(self) -> O3LoadStoreQueueKind {
        self.kind
    }

    pub const fn is_completed(self) -> bool {
        self.completed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3RenameMapEntry {
    register_class: O3RegisterClass,
    architectural: u32,
    physical: O3PhysicalRegisterId,
}

impl O3RenameMapEntry {
    pub const fn new(
        register_class: O3RegisterClass,
        architectural: u32,
        physical: O3PhysicalRegisterId,
    ) -> Self {
        Self {
            register_class,
            architectural,
            physical,
        }
    }

    pub const fn register_class(self) -> O3RegisterClass {
        self.register_class
    }

    pub const fn architectural(self) -> u32 {
        self.architectural
    }

    pub const fn physical(self) -> O3PhysicalRegisterId {
        self.physical
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3RuntimeSnapshot {
    reorder_buffer: Vec<O3ReorderBufferEntry>,
    load_store_queue: Vec<O3LoadStoreQueueEntry>,
    rename_map: Vec<O3RenameMapEntry>,
    pending_state: O3PendingStateSnapshot,
}

impl O3RuntimeSnapshot {
    pub fn new<R, L, M>(
        reorder_buffer: R,
        load_store_queue: L,
        rename_map: M,
        pending_state: O3PendingStateSnapshot,
    ) -> Result<Self, O3RuntimeError>
    where
        R: IntoIterator<Item = O3ReorderBufferEntry>,
        L: IntoIterator<Item = O3LoadStoreQueueEntry>,
        M: IntoIterator<Item = O3RenameMapEntry>,
    {
        let mut reorder_buffer = reorder_buffer.into_iter().collect::<Vec<_>>();
        reorder_buffer.sort_by_key(|entry| entry.sequence());
        validate_unique(
            "ROB",
            reorder_buffer
                .iter()
                .map(|entry| O3RuntimeUniqueKey::Sequence(entry.sequence())),
        )?;

        let mut load_store_queue = load_store_queue.into_iter().collect::<Vec<_>>();
        load_store_queue.sort_by_key(|entry| entry.sequence());
        validate_unique(
            "LSQ",
            load_store_queue
                .iter()
                .map(|entry| O3RuntimeUniqueKey::Sequence(entry.sequence())),
        )?;

        let mut rename_map = rename_map.into_iter().collect::<Vec<_>>();
        rename_map.sort_by_key(|entry| {
            (
                encode_register_class(entry.register_class()),
                entry.architectural(),
            )
        });
        validate_unique(
            "rename_map",
            rename_map.iter().map(|entry| {
                O3RuntimeUniqueKey::Rename(entry.register_class(), entry.architectural())
            }),
        )?;

        let snapshot = Self {
            reorder_buffer,
            load_store_queue,
            rename_map,
            pending_state,
        };
        validate_runtime_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn reorder_buffer(&self) -> &[O3ReorderBufferEntry] {
        &self.reorder_buffer
    }

    pub fn load_store_queue(&self) -> &[O3LoadStoreQueueEntry] {
        &self.load_store_queue
    }

    pub fn rename_map(&self) -> &[O3RenameMapEntry] {
        &self.rename_map
    }

    pub const fn pending_state(&self) -> &O3PendingStateSnapshot {
        &self.pending_state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3RuntimeState {
    snapshot: O3RuntimeSnapshot,
    stats: O3RuntimeStats,
}

impl O3RuntimeState {
    pub fn restore(&mut self, snapshot: O3RuntimeSnapshot) -> Result<(), O3RuntimeError> {
        validate_runtime_snapshot(&snapshot)?;
        self.snapshot = snapshot;
        Ok(())
    }

    pub fn snapshot(&self) -> O3RuntimeSnapshot {
        self.snapshot.clone()
    }

    pub const fn stats(&self) -> O3RuntimeStats {
        self.stats
    }

    pub fn reset_stats(&mut self) {
        self.stats = O3RuntimeStats::default();
    }

    pub fn record_retired_instruction(&mut self, execution: &RiscvCpuExecutionEvent) {
        self.stats.record_retired_instruction(execution);
    }

    pub fn pending_state_checkpoint_payload(&self) -> O3PendingStateCheckpointPayload {
        O3PendingStateCheckpointPayload::from_snapshot(self.snapshot.pending_state.clone())
            .expect("O3 runtime pending-state snapshot is valid")
    }

    pub fn restore_pending_state_checkpoint_payload(
        &mut self,
        payload: O3PendingStateCheckpointPayload,
    ) -> Result<(), O3PipelineError> {
        let pending_state =
            O3PendingStateCheckpointPayload::from_snapshot(payload.snapshot().clone())?
                .into_snapshot();
        self.snapshot = O3RuntimeSnapshot::new(
            self.snapshot.reorder_buffer.clone(),
            self.snapshot.load_store_queue.clone(),
            self.snapshot.rename_map.clone(),
            pending_state,
        )
        .expect("existing O3 runtime snapshot is valid");
        Ok(())
    }
}

impl Default for O3RuntimeState {
    fn default() -> Self {
        Self {
            snapshot: default_o3_runtime_snapshot(),
            stats: O3RuntimeStats::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct O3RuntimeStats {
    instructions: u64,
    rob_allocations: u64,
    rob_commits: u64,
    rename_writes: u64,
    lsq_loads: u64,
    lsq_stores: u64,
}

impl O3RuntimeStats {
    pub const fn instructions(self) -> u64 {
        self.instructions
    }

    pub const fn rob_allocations(self) -> u64 {
        self.rob_allocations
    }

    pub const fn rob_commits(self) -> u64 {
        self.rob_commits
    }

    pub const fn rename_writes(self) -> u64 {
        self.rename_writes
    }

    pub const fn lsq_loads(self) -> u64 {
        self.lsq_loads
    }

    pub const fn lsq_stores(self) -> u64 {
        self.lsq_stores
    }

    pub const fn has_activity(self) -> bool {
        self.instructions != 0
            || self.rob_allocations != 0
            || self.rob_commits != 0
            || self.rename_writes != 0
            || self.lsq_loads != 0
            || self.lsq_stores != 0
    }

    fn record_retired_instruction(&mut self, execution: &RiscvCpuExecutionEvent) {
        self.instructions = self.instructions.saturating_add(1);
        self.rob_allocations = self.rob_allocations.saturating_add(1);
        self.rob_commits = self.rob_commits.saturating_add(1);

        let record = execution.execution();
        if record.system_event().is_none() {
            let immediate_writes =
                (record.register_writes().len() + record.float_register_writes().len()) as u64;
            let memory_writes = record
                .memory_access()
                .map(o3_memory_destination_writes)
                .unwrap_or(0);
            self.rename_writes = self
                .rename_writes
                .saturating_add(immediate_writes.saturating_add(memory_writes));
        }

        if let Some(access) = record.memory_access() {
            let (loads, stores) = o3_lsq_access_counts(access);
            self.lsq_loads = self.lsq_loads.saturating_add(loads);
            self.lsq_stores = self.lsq_stores.saturating_add(stores);
        }
    }
}

fn o3_memory_destination_writes(access: &MemoryAccessKind) -> u64 {
    match access {
        MemoryAccessKind::Load { rd, .. }
        | MemoryAccessKind::LoadReserved { rd, .. }
        | MemoryAccessKind::StoreConditional { rd, .. }
        | MemoryAccessKind::AtomicMemory { rd, .. } => integer_destination_count(*rd),
        MemoryAccessKind::FloatLoad { .. } => 1,
        MemoryAccessKind::VectorLoadUnitStride {
            group_registers, ..
        }
        | MemoryAccessKind::VectorLoadStrided {
            group_registers, ..
        }
        | MemoryAccessKind::VectorLoadIndexed {
            group_registers, ..
        } => vector_destination_count(*group_registers),
        MemoryAccessKind::VectorLoadSegmentUnitStride {
            fields,
            group_registers,
            ..
        } => vector_destination_count(*fields)
            .saturating_mul(vector_destination_count(*group_registers)),
        MemoryAccessKind::Store { .. }
        | MemoryAccessKind::FloatStore { .. }
        | MemoryAccessKind::VectorStoreUnitStride { .. }
        | MemoryAccessKind::VectorStoreSegmentUnitStride { .. }
        | MemoryAccessKind::VectorStoreStrided { .. }
        | MemoryAccessKind::VectorStoreIndexed { .. } => 0,
    }
}

const fn integer_destination_count(register: Register) -> u64 {
    if register.is_zero() {
        0
    } else {
        1
    }
}

fn vector_destination_count(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}

const fn o3_lsq_access_counts(access: &MemoryAccessKind) -> (u64, u64) {
    match access {
        MemoryAccessKind::Load { .. }
        | MemoryAccessKind::FloatLoad { .. }
        | MemoryAccessKind::VectorLoadUnitStride { .. }
        | MemoryAccessKind::VectorLoadSegmentUnitStride { .. }
        | MemoryAccessKind::VectorLoadStrided { .. }
        | MemoryAccessKind::VectorLoadIndexed { .. }
        | MemoryAccessKind::LoadReserved { .. } => (1, 0),
        MemoryAccessKind::StoreConditional { .. }
        | MemoryAccessKind::Store { .. }
        | MemoryAccessKind::FloatStore { .. }
        | MemoryAccessKind::VectorStoreUnitStride { .. }
        | MemoryAccessKind::VectorStoreSegmentUnitStride { .. }
        | MemoryAccessKind::VectorStoreStrided { .. }
        | MemoryAccessKind::VectorStoreIndexed { .. } => (0, 1),
        MemoryAccessKind::AtomicMemory { .. } => (1, 1),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3RuntimeCheckpointPayload {
    snapshot: O3RuntimeSnapshot,
}

impl O3RuntimeCheckpointPayload {
    pub fn from_snapshot(snapshot: O3RuntimeSnapshot) -> Result<Self, O3RuntimeError> {
        validate_runtime_snapshot(&snapshot)?;
        Ok(Self { snapshot })
    }

    pub fn decode(payload: &[u8]) -> Result<Self, O3RuntimeError> {
        if payload.len() < O3_RUNTIME_CHECKPOINT_HEADER_BYTES {
            return Err(O3RuntimeError::InvalidCheckpointPayloadSize {
                expected: O3_RUNTIME_CHECKPOINT_HEADER_BYTES,
                actual: payload.len(),
            });
        }
        if payload[..O3_RUNTIME_CHECKPOINT_MAGIC.len()] != O3_RUNTIME_CHECKPOINT_MAGIC {
            return Err(O3RuntimeError::InvalidCheckpointMagic);
        }

        let mut offset = O3_RUNTIME_CHECKPOINT_MAGIC.len();
        let version = read_u8(payload, &mut offset)?;
        if version != O3_RUNTIME_CHECKPOINT_VERSION {
            return Err(O3RuntimeError::UnsupportedCheckpointVersion { version });
        }

        let pending_payload_len = read_u32(payload, &mut offset)? as usize;
        let rob_count = read_u32(payload, &mut offset)? as usize;
        let lsq_count = read_u32(payload, &mut offset)? as usize;
        let rename_count = read_u32(payload, &mut offset)? as usize;

        ensure_remaining(payload, offset, pending_payload_len)?;
        let pending_payload_end = offset + pending_payload_len;
        let pending_state =
            O3PendingStateCheckpointPayload::decode(&payload[offset..pending_payload_end])
                .map_err(|error| O3RuntimeError::InvalidPendingState { error })?
                .into_snapshot();
        offset = pending_payload_end;

        ensure_remaining(
            payload,
            offset,
            checked_bytes(rob_count, O3_RUNTIME_ROB_ENTRY_BYTES, payload.len())?,
        )?;
        let mut reorder_buffer = Vec::with_capacity(rob_count);
        for _ in 0..rob_count {
            reorder_buffer.push(read_rob_entry(payload, &mut offset)?);
        }

        ensure_remaining(
            payload,
            offset,
            checked_bytes(lsq_count, O3_RUNTIME_LSQ_ENTRY_BYTES, payload.len())?,
        )?;
        let mut load_store_queue = Vec::with_capacity(lsq_count);
        for _ in 0..lsq_count {
            load_store_queue.push(read_lsq_entry(payload, &mut offset)?);
        }

        ensure_remaining(
            payload,
            offset,
            checked_bytes(rename_count, O3_RUNTIME_RENAME_ENTRY_BYTES, payload.len())?,
        )?;
        let mut rename_map = Vec::with_capacity(rename_count);
        for _ in 0..rename_count {
            rename_map.push(read_rename_entry(payload, &mut offset)?);
        }

        if offset != payload.len() {
            return Err(O3RuntimeError::InvalidCheckpointPayloadSize {
                expected: offset,
                actual: payload.len(),
            });
        }

        Self::from_snapshot(O3RuntimeSnapshot::new(
            reorder_buffer,
            load_store_queue,
            rename_map,
            pending_state,
        )?)
    }

    pub fn encode(&self) -> Vec<u8> {
        let pending_payload =
            O3PendingStateCheckpointPayload::from_snapshot(self.snapshot.pending_state.clone())
                .expect("O3 runtime checkpoint payload was validated before construction")
                .encode();
        let pending_payload_len = encode_u32("pending_payload_length", pending_payload.len())
            .expect("O3 runtime checkpoint payload was validated before construction");
        let rob_count = encode_u32("reorder_buffer_count", self.snapshot.reorder_buffer.len())
            .expect("O3 runtime checkpoint payload was validated before construction");
        let lsq_count = encode_u32(
            "load_store_queue_count",
            self.snapshot.load_store_queue.len(),
        )
        .expect("O3 runtime checkpoint payload was validated before construction");
        let rename_count = encode_u32("rename_map_count", self.snapshot.rename_map.len())
            .expect("O3 runtime checkpoint payload was validated before construction");

        let mut payload = Vec::new();
        payload.extend_from_slice(&O3_RUNTIME_CHECKPOINT_MAGIC);
        payload.push(O3_RUNTIME_CHECKPOINT_VERSION);
        payload.extend_from_slice(&pending_payload_len.to_le_bytes());
        payload.extend_from_slice(&rob_count.to_le_bytes());
        payload.extend_from_slice(&lsq_count.to_le_bytes());
        payload.extend_from_slice(&rename_count.to_le_bytes());
        payload.extend_from_slice(&pending_payload);
        for entry in &self.snapshot.reorder_buffer {
            write_rob_entry(&mut payload, *entry);
        }
        for entry in &self.snapshot.load_store_queue {
            write_lsq_entry(&mut payload, *entry);
        }
        for entry in &self.snapshot.rename_map {
            write_rename_entry(&mut payload, *entry);
        }
        payload
    }

    pub const fn snapshot(&self) -> &O3RuntimeSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> O3RuntimeSnapshot {
        self.snapshot
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum O3RuntimeUniqueKey {
    Sequence(u64),
    Rename(O3RegisterClass, u32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum O3RuntimeError {
    DuplicateReorderBufferSequence {
        sequence: u64,
    },
    DuplicateLoadStoreQueueSequence {
        sequence: u64,
    },
    DuplicateRenameMapEntry {
        register_class: O3RegisterClass,
        architectural: u32,
    },
    InvalidCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidCheckpointMagic,
    UnsupportedCheckpointVersion {
        version: u8,
    },
    InvalidRegisterClassCode {
        code: u8,
    },
    InvalidLoadStoreKindCode {
        code: u8,
    },
    InvalidCheckpointBool {
        field: &'static str,
        value: u8,
    },
    InvalidPendingState {
        error: O3PipelineError,
    },
    CheckpointValueTooLarge {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
}

impl fmt::Display for O3RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateReorderBufferSequence { sequence } => {
                write!(formatter, "O3 runtime ROB repeats sequence {sequence}")
            }
            Self::DuplicateLoadStoreQueueSequence { sequence } => {
                write!(formatter, "O3 runtime LSQ repeats sequence {sequence}")
            }
            Self::DuplicateRenameMapEntry {
                register_class,
                architectural,
            } => write!(
                formatter,
                "O3 runtime rename map repeats {register_class:?} architectural register {architectural}"
            ),
            Self::InvalidCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "O3 runtime checkpoint payload has {actual} bytes but expected {expected}"
            ),
            Self::InvalidCheckpointMagic => {
                write!(formatter, "O3 runtime checkpoint payload has invalid magic")
            }
            Self::UnsupportedCheckpointVersion { version } => write!(
                formatter,
                "O3 runtime checkpoint payload version {version} is not supported"
            ),
            Self::InvalidRegisterClassCode { code } => write!(
                formatter,
                "O3 runtime checkpoint payload has invalid register-class code {code}"
            ),
            Self::InvalidLoadStoreKindCode { code } => write!(
                formatter,
                "O3 runtime checkpoint payload has invalid LSQ kind code {code}"
            ),
            Self::InvalidCheckpointBool { field, value } => write!(
                formatter,
                "O3 runtime checkpoint field {field} boolean has invalid value {value}"
            ),
            Self::InvalidPendingState { error } => {
                write!(formatter, "O3 runtime checkpoint has invalid pending state: {error}")
            }
            Self::CheckpointValueTooLarge {
                field,
                value,
                maximum,
            } => write!(
                formatter,
                "O3 runtime checkpoint field {field} value {value} exceeds maximum {maximum}"
            ),
        }
    }
}

impl Error for O3RuntimeError {}

fn default_o3_runtime_snapshot() -> O3RuntimeSnapshot {
    O3RuntimeSnapshot::new(
        [],
        [],
        [],
        O3PendingStateSnapshot::new(
            [],
            [],
            O3WritebackTransferSnapshot::new(
                O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 1, 0)
                    .expect("default O3 writeback policy is valid"),
                [],
            ),
        )
        .expect("default O3 pending-state snapshot is valid"),
    )
    .expect("default O3 runtime snapshot is valid")
}

fn validate_runtime_snapshot(snapshot: &O3RuntimeSnapshot) -> Result<(), O3RuntimeError> {
    encode_u32("reorder_buffer_count", snapshot.reorder_buffer.len())?;
    encode_u32("load_store_queue_count", snapshot.load_store_queue.len())?;
    encode_u32("rename_map_count", snapshot.rename_map.len())?;
    let pending_payload =
        O3PendingStateCheckpointPayload::from_snapshot(snapshot.pending_state.clone())
            .map_err(|error| O3RuntimeError::InvalidPendingState { error })?
            .encode();
    encode_u32("pending_payload_length", pending_payload.len())?;
    Ok(())
}

fn validate_unique<I>(kind: &'static str, keys: I) -> Result<(), O3RuntimeError>
where
    I: IntoIterator<Item = O3RuntimeUniqueKey>,
{
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key) {
            return match (kind, key) {
                ("ROB", O3RuntimeUniqueKey::Sequence(sequence)) => {
                    Err(O3RuntimeError::DuplicateReorderBufferSequence { sequence })
                }
                ("LSQ", O3RuntimeUniqueKey::Sequence(sequence)) => {
                    Err(O3RuntimeError::DuplicateLoadStoreQueueSequence { sequence })
                }
                ("rename_map", O3RuntimeUniqueKey::Rename(register_class, architectural)) => {
                    Err(O3RuntimeError::DuplicateRenameMapEntry {
                        register_class,
                        architectural,
                    })
                }
                _ => unreachable!("O3 runtime unique key kind is known"),
            };
        }
    }
    Ok(())
}

fn encode_u32(field: &'static str, value: usize) -> Result<u32, O3RuntimeError> {
    u32::try_from(value).map_err(|_| O3RuntimeError::CheckpointValueTooLarge {
        field,
        value,
        maximum: O3_RUNTIME_U32_MAX,
    })
}

fn checked_bytes(
    count: usize,
    bytes_per_item: usize,
    actual: usize,
) -> Result<usize, O3RuntimeError> {
    count
        .checked_mul(bytes_per_item)
        .ok_or(O3RuntimeError::InvalidCheckpointPayloadSize {
            expected: O3_RUNTIME_CHECKPOINT_HEADER_BYTES,
            actual,
        })
}

fn ensure_remaining(payload: &[u8], offset: usize, bytes: usize) -> Result<(), O3RuntimeError> {
    let expected =
        offset
            .checked_add(bytes)
            .ok_or(O3RuntimeError::InvalidCheckpointPayloadSize {
                expected: offset,
                actual: payload.len(),
            })?;
    if payload.len() < expected {
        return Err(O3RuntimeError::InvalidCheckpointPayloadSize {
            expected,
            actual: payload.len(),
        });
    }
    Ok(())
}

fn write_rob_entry(payload: &mut Vec<u8>, entry: O3ReorderBufferEntry) {
    payload.extend_from_slice(&entry.sequence().to_le_bytes());
    payload.extend_from_slice(&entry.pc().get().to_le_bytes());
    if let Some(destination) = entry.destination() {
        payload.push(1);
        payload.extend_from_slice(&destination.get().to_le_bytes());
    } else {
        payload.push(0);
        payload.extend_from_slice(&O3PhysicalRegisterId::invalid().get().to_le_bytes());
    }
    payload.push(bool_flag(entry.is_ready()));
}

fn write_lsq_entry(payload: &mut Vec<u8>, entry: O3LoadStoreQueueEntry) {
    payload.extend_from_slice(&entry.sequence().to_le_bytes());
    if let Some(address) = entry.address() {
        payload.push(1);
        payload.extend_from_slice(&address.get().to_le_bytes());
    } else {
        payload.push(0);
        payload.extend_from_slice(&0_u64.to_le_bytes());
    }
    payload.extend_from_slice(&entry.bytes().to_le_bytes());
    payload.push(encode_lsq_kind(entry.kind()));
    payload.push(bool_flag(entry.is_completed()));
}

fn write_rename_entry(payload: &mut Vec<u8>, entry: O3RenameMapEntry) {
    payload.push(encode_register_class(entry.register_class()));
    payload.extend_from_slice(&entry.architectural().to_le_bytes());
    payload.extend_from_slice(&entry.physical().get().to_le_bytes());
}

fn read_rob_entry(
    payload: &[u8],
    offset: &mut usize,
) -> Result<O3ReorderBufferEntry, O3RuntimeError> {
    let sequence = read_u64(payload, offset)?;
    let pc = Address::new(read_u64(payload, offset)?);
    let destination_present = read_bool("ROB destination-present", payload, offset)?;
    let physical = O3PhysicalRegisterId::new(read_u32(payload, offset)?);
    let ready = read_bool("ROB ready", payload, offset)?;
    Ok(
        O3ReorderBufferEntry::new(sequence, pc, destination_present.then_some(physical))
            .with_ready(ready),
    )
}

fn read_lsq_entry(
    payload: &[u8],
    offset: &mut usize,
) -> Result<O3LoadStoreQueueEntry, O3RuntimeError> {
    let sequence = read_u64(payload, offset)?;
    let address_present = read_bool("LSQ address-present", payload, offset)?;
    let address = Address::new(read_u64(payload, offset)?);
    let bytes = read_u32(payload, offset)?;
    let kind = decode_lsq_kind(read_u8(payload, offset)?)?;
    let completed = read_bool("LSQ completed", payload, offset)?;
    let entry = match kind {
        O3LoadStoreQueueKind::Load => {
            O3LoadStoreQueueEntry::load(sequence, address_present.then_some(address), bytes)
        }
        O3LoadStoreQueueKind::Store => {
            O3LoadStoreQueueEntry::store(sequence, address_present.then_some(address), bytes)
        }
    };
    Ok(entry.with_completed(completed))
}

fn read_rename_entry(
    payload: &[u8],
    offset: &mut usize,
) -> Result<O3RenameMapEntry, O3RuntimeError> {
    let register_class = decode_register_class(read_u8(payload, offset)?)?;
    let architectural = read_u32(payload, offset)?;
    let physical = O3PhysicalRegisterId::new(read_u32(payload, offset)?);
    Ok(O3RenameMapEntry::new(
        register_class,
        architectural,
        physical,
    ))
}

fn read_u8(payload: &[u8], offset: &mut usize) -> Result<u8, O3RuntimeError> {
    ensure_remaining(payload, *offset, 1)?;
    let value = payload[*offset];
    *offset += 1;
    Ok(value)
}

fn read_u32(payload: &[u8], offset: &mut usize) -> Result<u32, O3RuntimeError> {
    ensure_remaining(payload, *offset, U32_BYTES)?;
    let bytes = payload[*offset..*offset + U32_BYTES]
        .try_into()
        .expect("O3 runtime checkpoint u32 slice width is fixed");
    *offset += U32_BYTES;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(payload: &[u8], offset: &mut usize) -> Result<u64, O3RuntimeError> {
    ensure_remaining(payload, *offset, U64_BYTES)?;
    let bytes = payload[*offset..*offset + U64_BYTES]
        .try_into()
        .expect("O3 runtime checkpoint u64 slice width is fixed");
    *offset += U64_BYTES;
    Ok(u64::from_le_bytes(bytes))
}

fn read_bool(
    field: &'static str,
    payload: &[u8],
    offset: &mut usize,
) -> Result<bool, O3RuntimeError> {
    match read_u8(payload, offset)? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(O3RuntimeError::InvalidCheckpointBool { field, value }),
    }
}

fn encode_register_class(register_class: O3RegisterClass) -> u8 {
    match register_class {
        O3RegisterClass::Integer => 0,
        O3RegisterClass::FloatingPoint => 1,
        O3RegisterClass::Vector => 2,
        O3RegisterClass::ConditionCode => 3,
        O3RegisterClass::Misc => 4,
    }
}

fn decode_register_class(code: u8) -> Result<O3RegisterClass, O3RuntimeError> {
    match code {
        0 => Ok(O3RegisterClass::Integer),
        1 => Ok(O3RegisterClass::FloatingPoint),
        2 => Ok(O3RegisterClass::Vector),
        3 => Ok(O3RegisterClass::ConditionCode),
        4 => Ok(O3RegisterClass::Misc),
        _ => Err(O3RuntimeError::InvalidRegisterClassCode { code }),
    }
}

fn encode_lsq_kind(kind: O3LoadStoreQueueKind) -> u8 {
    match kind {
        O3LoadStoreQueueKind::Load => 0,
        O3LoadStoreQueueKind::Store => 1,
    }
}

fn decode_lsq_kind(code: u8) -> Result<O3LoadStoreQueueKind, O3RuntimeError> {
    match code {
        0 => Ok(O3LoadStoreQueueKind::Load),
        1 => Ok(O3LoadStoreQueueKind::Store),
        _ => Err(O3RuntimeError::InvalidLoadStoreKindCode { code }),
    }
}

fn bool_flag(value: bool) -> u8 {
    u8::from(value)
}

impl crate::RiscvCore {
    pub fn o3_runtime_stats(&self) -> O3RuntimeStats {
        self.state
            .lock()
            .expect("riscv core lock")
            .o3_runtime
            .stats()
    }

    pub fn reset_o3_runtime_stats(&self) {
        self.state
            .lock()
            .expect("riscv core lock")
            .o3_runtime
            .reset_stats();
    }

    pub fn record_o3_retired_instruction(&self, execution: &RiscvCpuExecutionEvent) {
        self.state
            .lock()
            .expect("riscv core lock")
            .o3_runtime
            .record_retired_instruction(execution);
    }

    pub fn default_o3_runtime_checkpoint_payload() -> O3RuntimeCheckpointPayload {
        O3RuntimeCheckpointPayload::from_snapshot(default_o3_runtime_snapshot())
            .expect("default O3 runtime checkpoint payload is valid")
    }

    pub fn o3_runtime_checkpoint_payload(&self) -> O3RuntimeCheckpointPayload {
        O3RuntimeCheckpointPayload::from_snapshot(
            self.state
                .lock()
                .expect("riscv core lock")
                .o3_runtime
                .snapshot(),
        )
        .expect("captured RISC-V O3 runtime checkpoint is internally consistent")
    }

    pub fn restore_o3_runtime_checkpoint_payload(
        &self,
        payload: O3RuntimeCheckpointPayload,
    ) -> Result<(), O3RuntimeError> {
        self.validate_o3_runtime_checkpoint_payload(&payload)?;
        self.state
            .lock()
            .expect("riscv core lock")
            .o3_runtime
            .restore(payload.into_snapshot())
    }

    pub fn validate_o3_runtime_checkpoint_payload(
        &self,
        payload: &O3RuntimeCheckpointPayload,
    ) -> Result<(), O3RuntimeError> {
        O3RuntimeCheckpointPayload::from_snapshot(payload.snapshot().clone()).map(|_| ())
    }

    pub fn default_o3_pending_state_checkpoint_payload() -> O3PendingStateCheckpointPayload {
        O3RuntimeState::default().pending_state_checkpoint_payload()
    }

    pub fn o3_pending_state_checkpoint_payload(&self) -> O3PendingStateCheckpointPayload {
        self.state
            .lock()
            .expect("riscv core lock")
            .o3_runtime
            .pending_state_checkpoint_payload()
    }

    pub fn restore_o3_pending_state_checkpoint_payload(
        &self,
        payload: O3PendingStateCheckpointPayload,
    ) -> Result<(), O3PipelineError> {
        self.validate_o3_pending_state_checkpoint_payload(&payload)?;
        self.state
            .lock()
            .expect("riscv core lock")
            .o3_runtime
            .restore_pending_state_checkpoint_payload(payload)
    }

    pub fn validate_o3_pending_state_checkpoint_payload(
        &self,
        payload: &O3PendingStateCheckpointPayload,
    ) -> Result<(), O3PipelineError> {
        O3PendingStateCheckpointPayload::from_snapshot(payload.snapshot().clone()).map(|_| ())
    }
}

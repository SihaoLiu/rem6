use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{AccessSize, Address, AddressRange, AgentId};

mod queue_readiness;

const QUEUE_CHECKPOINT_MAGIC: [u8; 4] = *b"MTLQ";
const QUEUE_CHECKPOINT_VERSION: u32 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const QUEUE_CHECKPOINT_HEADER_BYTES: usize =
    QUEUE_CHECKPOINT_MAGIC.len() + U32_BYTES + U64_BYTES * 3 + U32_BYTES + U32_BYTES;
const QUEUE_CHECKPOINT_ENTRY_BYTES: usize = U32_BYTES * 4 + U64_BYTES * 6;
const QUEUE_CHECKPOINT_U32_MAX: usize = u32::MAX as usize;
const QUEUE_CHECKPOINT_U64_MAX: usize = u64::MAX as usize;
const PAGE_MAP_CHECKPOINT_MAGIC: [u8; 4] = *b"MTPM";
const PAGE_MAP_CHECKPOINT_VERSION: u32 = 1;
const PAGE_MAP_CHECKPOINT_HEADER_BYTES: usize =
    PAGE_MAP_CHECKPOINT_MAGIC.len() + U32_BYTES + U64_BYTES + U32_BYTES + U32_BYTES;
const PAGE_MAP_CHECKPOINT_ENTRY_BYTES: usize = U64_BYTES * 3 + U32_BYTES + U32_BYTES;
const PAGE_MAP_CHECKPOINT_PERMISSION_MASK: u32 = 0x7;
const PAGE_MAP_CHECKPOINT_U32_MAX: usize = u32::MAX as usize;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TranslationRequestId {
    agent: AgentId,
    sequence: u64,
}

impl TranslationRequestId {
    pub const fn new(agent: AgentId, sequence: u64) -> Self {
        Self { agent, sequence }
    }

    pub const fn agent(self) -> AgentId {
        self.agent
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranslationQueueConfig {
    capacity: usize,
    latency: u64,
}

impl TranslationQueueConfig {
    pub fn new(capacity: usize, latency: u64) -> Result<Self, TranslationError> {
        if capacity == 0 {
            return Err(TranslationError::ZeroCapacity);
        }

        Ok(Self { capacity, latency })
    }

    pub const fn capacity(self) -> usize {
        self.capacity
    }

    pub const fn latency(self) -> u64 {
        self.latency
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslationAccessKind {
    InstructionFetch,
    Load,
    Store,
    Atomic,
    Prefetch,
}

impl TranslationAccessKind {
    pub const fn requires_write_permission(self) -> bool {
        matches!(self, Self::Store | Self::Atomic)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationRequest {
    id: TranslationRequestId,
    range: AddressRange,
    access: TranslationAccessKind,
}

impl TranslationRequest {
    pub fn new(
        id: TranslationRequestId,
        virtual_address: Address,
        size: AccessSize,
        access: TranslationAccessKind,
    ) -> Result<Self, TranslationError> {
        let range = AddressRange::new(virtual_address, size).map_err(|_| {
            TranslationError::AddressOverflow {
                start: virtual_address,
                size,
            }
        })?;

        Ok(Self { id, range, access })
    }

    pub const fn id(&self) -> TranslationRequestId {
        self.id
    }

    pub const fn virtual_address(&self) -> Address {
        self.range.start()
    }

    pub const fn range(&self) -> AddressRange {
        self.range
    }

    pub const fn size(&self) -> AccessSize {
        self.range.size()
    }

    pub const fn access(&self) -> TranslationAccessKind {
        self.access
    }

    pub const fn requires_write_permission(&self) -> bool {
        self.access.requires_write_permission()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslationFaultKind {
    PageFault,
    AccessFault,
    PermissionFault,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationFault {
    virtual_address: Address,
    kind: TranslationFaultKind,
}

impl TranslationFault {
    pub const fn new(virtual_address: Address, kind: TranslationFaultKind) -> Self {
        Self {
            virtual_address,
            kind,
        }
    }

    pub const fn virtual_address(&self) -> Address {
        self.virtual_address
    }

    pub const fn kind(&self) -> TranslationFaultKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TranslationResolution {
    Mapped(Address),
    Fault(TranslationFault),
}

impl TranslationResolution {
    pub const fn mapped(physical_address: Address) -> Self {
        Self::Mapped(physical_address)
    }

    pub const fn fault(fault: TranslationFault) -> Self {
        Self::Fault(fault)
    }

    pub const fn physical_address(&self) -> Option<Address> {
        match self {
            Self::Mapped(address) => Some(*address),
            Self::Fault(_) => None,
        }
    }

    pub const fn fault_ref(&self) -> Option<&TranslationFault> {
        match self {
            Self::Mapped(_) => None,
            Self::Fault(fault) => Some(fault),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationSegment {
    virtual_range: AddressRange,
    physical_start: Address,
}

impl TranslationSegment {
    pub fn new(
        virtual_start: Address,
        size: AccessSize,
        physical_start: Address,
    ) -> Result<Self, TranslationError> {
        let virtual_range = AddressRange::new(virtual_start, size).map_err(|_| {
            TranslationError::AddressOverflow {
                start: virtual_start,
                size,
            }
        })?;
        AddressRange::new(physical_start, size).map_err(|_| TranslationError::AddressOverflow {
            start: physical_start,
            size,
        })?;

        Ok(Self {
            virtual_range,
            physical_start,
        })
    }

    pub const fn virtual_start(&self) -> Address {
        self.virtual_range.start()
    }

    pub const fn virtual_range(&self) -> AddressRange {
        self.virtual_range
    }

    pub const fn size(&self) -> AccessSize {
        self.virtual_range.size()
    }

    pub const fn physical_start(&self) -> Address {
        self.physical_start
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TranslationSegmentedResolution {
    Mapped(Vec<TranslationSegment>),
    Fault(TranslationFault),
}

impl TranslationSegmentedResolution {
    pub fn mapped(segments: Vec<TranslationSegment>) -> Self {
        Self::Mapped(segments)
    }

    pub const fn fault(fault: TranslationFault) -> Self {
        Self::Fault(fault)
    }

    pub fn segments(&self) -> Option<&[TranslationSegment]> {
        match self {
            Self::Mapped(segments) => Some(segments),
            Self::Fault(_) => None,
        }
    }

    pub const fn fault_ref(&self) -> Option<&TranslationFault> {
        match self {
            Self::Mapped(_) => None,
            Self::Fault(fault) => Some(fault),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationCompletion {
    request: TranslationRequest,
    resolution: TranslationResolution,
}

impl TranslationCompletion {
    pub fn new(request: TranslationRequest, resolution: TranslationResolution) -> Self {
        Self {
            request,
            resolution,
        }
    }

    pub const fn request(&self) -> &TranslationRequest {
        &self.request
    }

    pub const fn resolution(&self) -> &TranslationResolution {
        &self.resolution
    }

    pub const fn physical_address(&self) -> Option<Address> {
        self.resolution.physical_address()
    }

    pub const fn fault(&self) -> Option<&TranslationFault> {
        self.resolution.fault_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationQueueEntrySnapshot {
    request: TranslationRequest,
    issue_tick: u64,
    ready_tick: u64,
    order: u64,
}

impl TranslationQueueEntrySnapshot {
    pub fn new(request: TranslationRequest, issue_tick: u64, ready_tick: u64, order: u64) -> Self {
        Self {
            request,
            issue_tick,
            ready_tick,
            order,
        }
    }

    pub const fn request(&self) -> &TranslationRequest {
        &self.request
    }

    pub const fn issue_tick(&self) -> u64 {
        self.issue_tick
    }

    pub const fn ready_tick(&self) -> u64 {
        self.ready_tick
    }

    pub const fn order(&self) -> u64 {
        self.order
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationQueueSnapshot {
    config: TranslationQueueConfig,
    entries: Vec<TranslationQueueEntrySnapshot>,
    next_order: u64,
}

impl TranslationQueueSnapshot {
    pub fn new(
        config: TranslationQueueConfig,
        entries: Vec<TranslationQueueEntrySnapshot>,
        next_order: u64,
    ) -> Self {
        Self {
            config,
            entries,
            next_order,
        }
    }

    pub const fn config(&self) -> TranslationQueueConfig {
        self.config
    }

    pub fn entries(&self) -> &[TranslationQueueEntrySnapshot] {
        &self.entries
    }

    pub const fn next_order(&self) -> u64 {
        self.next_order
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationQueueCheckpointPayload {
    snapshot: TranslationQueueSnapshot,
}

impl TranslationQueueCheckpointPayload {
    pub fn from_queue(queue: &TranslationQueue) -> Self {
        Self {
            snapshot: queue.snapshot(),
        }
    }

    pub fn from_snapshot(snapshot: TranslationQueueSnapshot) -> Result<Self, TranslationError> {
        TranslationQueue::from_snapshot(&snapshot)?;
        Ok(Self { snapshot })
    }

    pub fn decode(payload: &[u8]) -> Result<Self, TranslationError> {
        if payload.len() < QUEUE_CHECKPOINT_HEADER_BYTES {
            return Err(TranslationError::InvalidQueueCheckpointPayloadSize {
                expected: QUEUE_CHECKPOINT_HEADER_BYTES,
                actual: payload.len(),
            });
        }
        if payload[0..QUEUE_CHECKPOINT_MAGIC.len()] != QUEUE_CHECKPOINT_MAGIC {
            return Err(TranslationError::InvalidQueueCheckpointMagic);
        }

        let mut offset = QUEUE_CHECKPOINT_MAGIC.len();
        let version = read_queue_checkpoint_u32(payload, &mut offset);
        if version != QUEUE_CHECKPOINT_VERSION {
            return Err(TranslationError::UnsupportedQueueCheckpointVersion { version });
        }

        let capacity =
            decode_queue_checkpoint_usize(read_queue_checkpoint_u64(payload, &mut offset))?;
        let latency = read_queue_checkpoint_u64(payload, &mut offset);
        let next_order = read_queue_checkpoint_u64(payload, &mut offset);
        let entry_count = read_queue_checkpoint_u32(payload, &mut offset) as usize;
        let reserved = read_queue_checkpoint_u32(payload, &mut offset);
        if reserved != 0 {
            return Err(TranslationError::InvalidQueueCheckpointReserved { value: reserved });
        }
        let expected = queue_checkpoint_payload_size(entry_count)?;
        if payload.len() != expected {
            return Err(TranslationError::InvalidQueueCheckpointPayloadSize {
                expected,
                actual: payload.len(),
            });
        }

        let config = TranslationQueueConfig::new(capacity, latency)?;
        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            entries.push(read_queue_checkpoint_entry(payload, &mut offset)?);
        }

        Self::from_snapshot(TranslationQueueSnapshot::new(config, entries, next_order))
    }

    pub fn encode(&self) -> Vec<u8> {
        self.try_encode()
            .expect("translation queue checkpoint values fit the checkpoint encoding")
    }

    pub fn try_encode(&self) -> Result<Vec<u8>, TranslationError> {
        let capacity = encode_queue_checkpoint_u64("capacity", self.snapshot.config().capacity())?;
        let entry_count =
            encode_queue_checkpoint_u32("entry count", self.snapshot.entries().len())?;
        let mut payload = Vec::with_capacity(queue_checkpoint_payload_size(
            self.snapshot.entries().len(),
        )?);
        payload.extend_from_slice(&QUEUE_CHECKPOINT_MAGIC);
        payload.extend_from_slice(&QUEUE_CHECKPOINT_VERSION.to_le_bytes());
        payload.extend_from_slice(&capacity.to_le_bytes());
        payload.extend_from_slice(&self.snapshot.config().latency().to_le_bytes());
        payload.extend_from_slice(&self.snapshot.next_order().to_le_bytes());
        payload.extend_from_slice(&entry_count.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        for entry in self.snapshot.entries() {
            write_queue_checkpoint_entry(&mut payload, entry);
        }
        Ok(payload)
    }

    pub const fn snapshot(&self) -> &TranslationQueueSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> TranslationQueueSnapshot {
        self.snapshot
    }
}

fn write_queue_checkpoint_entry(payload: &mut Vec<u8>, entry: &TranslationQueueEntrySnapshot) {
    payload.extend_from_slice(&entry.request().id().agent().get().to_le_bytes());
    payload
        .extend_from_slice(&encode_queue_checkpoint_access(entry.request().access()).to_le_bytes());
    payload.extend_from_slice(&0_u32.to_le_bytes());
    payload.extend_from_slice(&0_u32.to_le_bytes());
    payload.extend_from_slice(&entry.request().id().sequence().to_le_bytes());
    payload.extend_from_slice(&entry.request().virtual_address().get().to_le_bytes());
    payload.extend_from_slice(&entry.request().size().bytes().to_le_bytes());
    payload.extend_from_slice(&entry.issue_tick().to_le_bytes());
    payload.extend_from_slice(&entry.ready_tick().to_le_bytes());
    payload.extend_from_slice(&entry.order().to_le_bytes());
}

fn read_queue_checkpoint_entry(
    payload: &[u8],
    offset: &mut usize,
) -> Result<TranslationQueueEntrySnapshot, TranslationError> {
    let agent = AgentId::new(read_queue_checkpoint_u32(payload, offset));
    let access = decode_queue_checkpoint_access(read_queue_checkpoint_u32(payload, offset))?;
    let reserved = read_queue_checkpoint_u32(payload, offset);
    let reserved2 = read_queue_checkpoint_u32(payload, offset);
    if reserved != 0 {
        return Err(TranslationError::InvalidQueueCheckpointReserved { value: reserved });
    }
    if reserved2 != 0 {
        return Err(TranslationError::InvalidQueueCheckpointReserved { value: reserved2 });
    }

    let sequence = read_queue_checkpoint_u64(payload, offset);
    let virtual_address = Address::new(read_queue_checkpoint_u64(payload, offset));
    let size = queue_checkpoint_access_size(read_queue_checkpoint_u64(payload, offset))?;
    let issue_tick = read_queue_checkpoint_u64(payload, offset);
    let ready_tick = read_queue_checkpoint_u64(payload, offset);
    let order = read_queue_checkpoint_u64(payload, offset);
    let request = TranslationRequest::new(
        TranslationRequestId::new(agent, sequence),
        virtual_address,
        size,
        access,
    )?;

    Ok(TranslationQueueEntrySnapshot::new(
        request, issue_tick, ready_tick, order,
    ))
}

fn encode_queue_checkpoint_access(access: TranslationAccessKind) -> u32 {
    match access {
        TranslationAccessKind::InstructionFetch => 0,
        TranslationAccessKind::Load => 1,
        TranslationAccessKind::Store => 2,
        TranslationAccessKind::Atomic => 3,
        TranslationAccessKind::Prefetch => 4,
    }
}

fn decode_queue_checkpoint_access(code: u32) -> Result<TranslationAccessKind, TranslationError> {
    match code {
        0 => Ok(TranslationAccessKind::InstructionFetch),
        1 => Ok(TranslationAccessKind::Load),
        2 => Ok(TranslationAccessKind::Store),
        3 => Ok(TranslationAccessKind::Atomic),
        4 => Ok(TranslationAccessKind::Prefetch),
        _ => Err(TranslationError::InvalidQueueCheckpointAccessKind { code }),
    }
}

fn queue_checkpoint_access_size(bytes: u64) -> Result<AccessSize, TranslationError> {
    AccessSize::new(bytes).map_err(|_| TranslationError::InvalidQueueCheckpointAccessSize { bytes })
}

fn decode_queue_checkpoint_usize(value: u64) -> Result<usize, TranslationError> {
    usize::try_from(value).map_err(|_| TranslationError::InvalidQueueCheckpointUsize { value })
}

fn queue_checkpoint_payload_size(entry_count: usize) -> Result<usize, TranslationError> {
    let entry_bytes = entry_count
        .checked_mul(QUEUE_CHECKPOINT_ENTRY_BYTES)
        .ok_or(TranslationError::InvalidQueueCheckpointPayloadSize {
            expected: usize::MAX,
            actual: 0,
        })?;
    QUEUE_CHECKPOINT_HEADER_BYTES
        .checked_add(entry_bytes)
        .ok_or(TranslationError::InvalidQueueCheckpointPayloadSize {
            expected: usize::MAX,
            actual: 0,
        })
}

fn encode_queue_checkpoint_u32(field: &'static str, value: usize) -> Result<u32, TranslationError> {
    u32::try_from(value).map_err(|_| TranslationError::QueueCheckpointValueTooLarge {
        field,
        value,
        maximum: QUEUE_CHECKPOINT_U32_MAX,
    })
}

fn encode_queue_checkpoint_u64(field: &'static str, value: usize) -> Result<u64, TranslationError> {
    u64::try_from(value).map_err(|_| TranslationError::QueueCheckpointValueTooLarge {
        field,
        value,
        maximum: QUEUE_CHECKPOINT_U64_MAX,
    })
}

fn read_queue_checkpoint_u32(payload: &[u8], offset: &mut usize) -> u32 {
    let bytes = payload[*offset..*offset + U32_BYTES]
        .try_into()
        .expect("checkpoint u32 slice width is fixed");
    *offset += U32_BYTES;
    u32::from_le_bytes(bytes)
}

fn read_queue_checkpoint_u64(payload: &[u8], offset: &mut usize) -> u64 {
    let bytes = payload[*offset..*offset + U64_BYTES]
        .try_into()
        .expect("checkpoint u64 slice width is fixed");
    *offset += U64_BYTES;
    u64::from_le_bytes(bytes)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TranslationQueueEntry {
    request: TranslationRequest,
    issue_tick: u64,
    ready_tick: u64,
    order: u64,
}

impl TranslationQueueEntry {
    fn snapshot(&self) -> TranslationQueueEntrySnapshot {
        TranslationQueueEntrySnapshot::new(
            self.request.clone(),
            self.issue_tick,
            self.ready_tick,
            self.order,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationQueue {
    config: TranslationQueueConfig,
    entries: BTreeMap<TranslationRequestId, TranslationQueueEntry>,
    next_order: u64,
}

impl TranslationQueue {
    pub fn new(config: TranslationQueueConfig) -> Self {
        Self {
            config,
            entries: BTreeMap::new(),
            next_order: 0,
        }
    }

    pub fn from_snapshot(snapshot: &TranslationQueueSnapshot) -> Result<Self, TranslationError> {
        if snapshot.entries().len() > snapshot.config().capacity() {
            return Err(TranslationError::QueueFull {
                capacity: snapshot.config().capacity(),
            });
        }

        let mut ids = BTreeSet::new();
        let mut entries = BTreeMap::new();
        for entry in snapshot.entries() {
            let request = entry.request().clone();
            if !ids.insert(request.id()) {
                return Err(TranslationError::DuplicateRequest {
                    request: request.id(),
                });
            }
            if entry.order() >= snapshot.next_order() {
                return Err(TranslationError::SnapshotNextOrderTooSmall {
                    next_order: snapshot.next_order(),
                    request: request.id(),
                    order: entry.order(),
                });
            }
            let expected_ready_tick = entry
                .issue_tick()
                .checked_add(snapshot.config().latency())
                .ok_or(TranslationError::SnapshotReadyTickMismatch {
                    request: request.id(),
                    issue_tick: entry.issue_tick(),
                    latency: snapshot.config().latency(),
                    ready_tick: entry.ready_tick(),
                })?;
            if entry.ready_tick() != expected_ready_tick {
                return Err(TranslationError::SnapshotReadyTickMismatch {
                    request: request.id(),
                    issue_tick: entry.issue_tick(),
                    latency: snapshot.config().latency(),
                    ready_tick: entry.ready_tick(),
                });
            }
            entries.insert(
                request.id(),
                TranslationQueueEntry {
                    request,
                    issue_tick: entry.issue_tick(),
                    ready_tick: entry.ready_tick(),
                    order: entry.order(),
                },
            );
        }

        Ok(Self {
            config: snapshot.config(),
            entries,
            next_order: snapshot.next_order(),
        })
    }

    pub fn restore(&mut self, snapshot: &TranslationQueueSnapshot) -> Result<(), TranslationError> {
        *self = Self::from_snapshot(snapshot)?;
        Ok(())
    }

    pub const fn config(&self) -> TranslationQueueConfig {
        self.config
    }

    pub fn pending_count(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear_pending(&mut self) {
        self.entries.clear();
    }

    pub fn enqueue(
        &mut self,
        issue_tick: u64,
        request: TranslationRequest,
    ) -> Result<(), TranslationError> {
        if self.entries.contains_key(&request.id()) {
            return Err(TranslationError::DuplicateRequest {
                request: request.id(),
            });
        }
        if self.entries.len() >= self.config.capacity() {
            return Err(TranslationError::QueueFull {
                capacity: self.config.capacity(),
            });
        }

        let ready_tick = issue_tick.checked_add(self.config.latency()).ok_or(
            TranslationError::TickOverflow {
                issue_tick,
                latency: self.config.latency(),
            },
        )?;
        let order = self.next_order;
        self.next_order = self
            .next_order
            .checked_add(1)
            .ok_or(TranslationError::QueueOrderOverflow)?;

        self.entries.insert(
            request.id(),
            TranslationQueueEntry {
                request,
                issue_tick,
                ready_tick,
                order,
            },
        );
        Ok(())
    }

    pub fn complete(
        &mut self,
        request: TranslationRequestId,
        resolution: TranslationResolution,
    ) -> Result<TranslationCompletion, TranslationError> {
        let entry = self
            .entries
            .remove(&request)
            .ok_or(TranslationError::UnknownRequest { request })?;
        Ok(TranslationCompletion::new(entry.request, resolution))
    }

    pub fn complete_ready<F>(&mut self, tick: u64, mut resolver: F) -> Vec<TranslationCompletion>
    where
        F: FnMut(&TranslationRequest) -> TranslationResolution,
    {
        self.ready_request_ids(tick)
            .into_iter()
            .filter_map(|request| {
                self.entries.remove(&request).map(|entry| {
                    let resolution = resolver(&entry.request);
                    TranslationCompletion::new(entry.request, resolution)
                })
            })
            .collect()
    }

    pub fn snapshot(&self) -> TranslationQueueSnapshot {
        TranslationQueueSnapshot::new(
            self.config,
            self.ordered_entries()
                .into_iter()
                .map(TranslationQueueEntry::snapshot)
                .collect(),
            self.next_order,
        )
    }

    fn service_order_ids(&self, ready_at: Option<u64>) -> Vec<TranslationRequestId> {
        self.ordered_entries()
            .into_iter()
            .filter(|entry| ready_at.is_none_or(|tick| entry.ready_tick <= tick))
            .map(|entry| entry.request.id())
            .collect()
    }

    fn ordered_entries(&self) -> Vec<&TranslationQueueEntry> {
        let mut entries: Vec<_> = self.entries.values().collect();
        entries.sort_by_key(|entry| (entry.ready_tick, entry.order, entry.request.id()));
        entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TranslationError {
    ZeroCapacity,
    QueueFull {
        capacity: usize,
    },
    DuplicateRequest {
        request: TranslationRequestId,
    },
    UnknownRequest {
        request: TranslationRequestId,
    },
    AddressOverflow {
        start: Address,
        size: AccessSize,
    },
    TickOverflow {
        issue_tick: u64,
        latency: u64,
    },
    SnapshotNextOrderTooSmall {
        next_order: u64,
        request: TranslationRequestId,
        order: u64,
    },
    SnapshotReadyTickMismatch {
        request: TranslationRequestId,
        issue_tick: u64,
        latency: u64,
        ready_tick: u64,
    },
    QueueOrderOverflow,
    InvalidQueueCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidQueueCheckpointMagic,
    UnsupportedQueueCheckpointVersion {
        version: u32,
    },
    InvalidQueueCheckpointReserved {
        value: u32,
    },
    InvalidQueueCheckpointAccessKind {
        code: u32,
    },
    InvalidQueueCheckpointAccessSize {
        bytes: u64,
    },
    InvalidQueueCheckpointUsize {
        value: u64,
    },
    QueueCheckpointValueTooLarge {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
    ZeroTlbCapacity,
    TlbCapacityExceeded {
        capacity: usize,
    },
    DuplicateTlbEntry {
        virtual_page: Address,
    },
    SnapshotNextLruTooSmall {
        next_lru: u64,
        virtual_page: Address,
        last_used: u64,
    },
    OverlappingTlbEntry {
        address_space: crate::TranslationAddressSpaceId,
        existing_start: Address,
        existing_size: AccessSize,
        requested_start: Address,
        requested_size: AccessSize,
    },
    TlbOrderOverflow,
    InvalidTlbCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidTlbCheckpointMagic,
    UnsupportedTlbCheckpointVersion {
        version: u32,
    },
    InvalidTlbCheckpointReserved {
        value: u32,
    },
    InvalidTlbCheckpointScope {
        code: u32,
    },
    InvalidTlbCheckpointPermissions {
        code: u32,
    },
    InvalidTlbCheckpointAddressSpace {
        value: u32,
    },
    InvalidTlbCheckpointUsize {
        value: u64,
    },
    TlbCheckpointValueTooLarge {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
    InvalidPageMapCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidPageMapCheckpointMagic,
    UnsupportedPageMapCheckpointVersion {
        version: u32,
    },
    InvalidPageMapCheckpointReserved {
        value: u32,
    },
    InvalidPageMapCheckpointScope {
        code: u32,
    },
    InvalidPageMapCheckpointPermissions {
        code: u32,
    },
    PageMapCheckpointValueTooLarge {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
    ZeroPageSize,
    NonPowerOfTwoPageSize {
        bytes: u64,
    },
    ZeroPageCount,
    UnalignedVirtualPage {
        address: Address,
        page_size: TranslationPageSize,
    },
    UnalignedPhysicalPage {
        address: Address,
        page_size: TranslationPageSize,
    },
    PageRangeOverflow {
        start: Address,
        page_size: TranslationPageSize,
        page_count: u64,
    },
    OverlappingTranslationMapping {
        existing_start: Address,
        existing_size: AccessSize,
        requested_start: Address,
        requested_size: AccessSize,
    },
}

impl fmt::Display for TranslationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCapacity => write!(formatter, "translation queue capacity must be nonzero"),
            Self::QueueFull { capacity } => {
                write!(
                    formatter,
                    "translation queue is full at capacity {capacity}"
                )
            }
            Self::DuplicateRequest { request } => write!(
                formatter,
                "translation request {} from agent {} is already pending",
                request.sequence(),
                request.agent().get()
            ),
            Self::UnknownRequest { request } => write!(
                formatter,
                "translation request {} from agent {} is not pending",
                request.sequence(),
                request.agent().get()
            ),
            Self::AddressOverflow { start, size } => write!(
                formatter,
                "virtual address {:#x} overflows for {} bytes",
                start.get(),
                size.bytes()
            ),
            Self::TickOverflow {
                issue_tick,
                latency,
            } => write!(
                formatter,
                "translation request issued at tick {issue_tick} overflows latency {latency}"
            ),
            Self::SnapshotNextOrderTooSmall {
                next_order,
                request,
                order,
            } => write!(
                formatter,
                "translation queue snapshot next order {next_order} is not after request {} from agent {} order {order}",
                request.sequence(),
                request.agent().get()
            ),
            Self::SnapshotReadyTickMismatch {
                request,
                issue_tick,
                latency,
                ready_tick,
            } => write!(
                formatter,
                "translation queue snapshot request {} from agent {} has ready tick {ready_tick}; expected issue tick {issue_tick} plus latency {latency}",
                request.sequence(),
                request.agent().get()
            ),
            Self::QueueOrderOverflow => {
                write!(
                    formatter,
                    "translation queue stable order counter overflowed"
                )
            }
            Self::InvalidQueueCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "translation queue checkpoint payload has {actual} bytes; expected {expected}"
            ),
            Self::InvalidQueueCheckpointMagic => {
                write!(formatter, "translation queue checkpoint payload has invalid magic")
            }
            Self::UnsupportedQueueCheckpointVersion { version } => write!(
                formatter,
                "translation queue checkpoint payload version {version} is not supported"
            ),
            Self::InvalidQueueCheckpointReserved { value } => write!(
                formatter,
                "translation queue checkpoint reserved field has nonzero value {value}"
            ),
            Self::InvalidQueueCheckpointAccessKind { code } => write!(
                formatter,
                "translation queue checkpoint payload has invalid access kind {code}"
            ),
            Self::InvalidQueueCheckpointAccessSize { bytes } => write!(
                formatter,
                "translation queue checkpoint payload has invalid access size {bytes}"
            ),
            Self::InvalidQueueCheckpointUsize { value } => write!(
                formatter,
                "translation queue checkpoint usize value {value} cannot fit this target"
            ),
            Self::QueueCheckpointValueTooLarge {
                field,
                value,
                maximum,
            } => write!(
                formatter,
                "translation queue checkpoint {field} value {value} exceeds maximum {maximum}"
            ),
            Self::ZeroTlbCapacity => write!(formatter, "translation TLB capacity must be nonzero"),
            Self::TlbCapacityExceeded { capacity } => {
                write!(
                    formatter,
                    "translation TLB snapshot exceeds capacity {capacity}"
                )
            }
            Self::DuplicateTlbEntry { virtual_page } => write!(
                formatter,
                "translation TLB entry for virtual page {:#x} is duplicated",
                virtual_page.get()
            ),
            Self::SnapshotNextLruTooSmall {
                next_lru,
                virtual_page,
                last_used,
            } => write!(
                formatter,
                "translation TLB snapshot next LRU {next_lru} is not after virtual page {:#x} last-used {last_used}",
                virtual_page.get()
            ),
            Self::OverlappingTlbEntry {
                address_space,
                existing_start,
                existing_size,
                requested_start,
                requested_size,
            } => write!(
                formatter,
                "translation TLB entry for ASID {} range {:#x}+{} overlaps existing range {:#x}+{}",
                address_space.get(),
                requested_start.get(),
                requested_size.bytes(),
                existing_start.get(),
                existing_size.bytes()
            ),
            Self::TlbOrderOverflow => {
                write!(formatter, "translation TLB stable order counter overflowed")
            }
            Self::InvalidTlbCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "translation TLB checkpoint payload has {actual} bytes; expected {expected}"
            ),
            Self::InvalidTlbCheckpointMagic => {
                write!(formatter, "translation TLB checkpoint payload has invalid magic")
            }
            Self::UnsupportedTlbCheckpointVersion { version } => write!(
                formatter,
                "translation TLB checkpoint payload version {version} is not supported"
            ),
            Self::InvalidTlbCheckpointReserved { value } => write!(
                formatter,
                "translation TLB checkpoint reserved field has nonzero value {value}"
            ),
            Self::InvalidTlbCheckpointScope { code } => write!(
                formatter,
                "translation TLB checkpoint payload has invalid scope code {code}"
            ),
            Self::InvalidTlbCheckpointPermissions { code } => write!(
                formatter,
                "translation TLB checkpoint payload has invalid permission bits {code:#x}"
            ),
            Self::InvalidTlbCheckpointAddressSpace { value } => write!(
                formatter,
                "translation TLB checkpoint address-space value {value} exceeds u16 range"
            ),
            Self::InvalidTlbCheckpointUsize { value } => write!(
                formatter,
                "translation TLB checkpoint usize value {value} cannot fit this target"
            ),
            Self::TlbCheckpointValueTooLarge {
                field,
                value,
                maximum,
            } => write!(
                formatter,
                "translation TLB checkpoint {field} value {value} exceeds maximum {maximum}"
            ),
            Self::InvalidPageMapCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "translation page-map checkpoint payload has {actual} bytes; expected {expected}"
            ),
            Self::InvalidPageMapCheckpointMagic => {
                write!(
                    formatter,
                    "translation page-map checkpoint payload has invalid magic"
                )
            }
            Self::UnsupportedPageMapCheckpointVersion { version } => write!(
                formatter,
                "translation page-map checkpoint payload version {version} is not supported"
            ),
            Self::InvalidPageMapCheckpointReserved { value } => write!(
                formatter,
                "translation page-map checkpoint reserved field has nonzero value {value}"
            ),
            Self::InvalidPageMapCheckpointScope { code } => write!(
                formatter,
                "translation page-map checkpoint payload has invalid scope code {code}"
            ),
            Self::InvalidPageMapCheckpointPermissions { code } => write!(
                formatter,
                "translation page-map checkpoint payload has invalid permission bits {code:#x}"
            ),
            Self::PageMapCheckpointValueTooLarge {
                field,
                value,
                maximum,
            } => write!(
                formatter,
                "translation page-map checkpoint {field} value {value} exceeds maximum {maximum}"
            ),
            Self::ZeroPageSize => write!(formatter, "translation page size must be nonzero"),
            Self::NonPowerOfTwoPageSize { bytes } => {
                write!(
                    formatter,
                    "translation page size {bytes} is not a power of two"
                )
            }
            Self::ZeroPageCount => {
                write!(formatter, "translation mapping page count must be nonzero")
            }
            Self::UnalignedVirtualPage { address, page_size } => write!(
                formatter,
                "virtual page address {:#x} is not aligned to translation page size {}",
                address.get(),
                page_size.bytes()
            ),
            Self::UnalignedPhysicalPage { address, page_size } => write!(
                formatter,
                "physical page address {:#x} is not aligned to translation page size {}",
                address.get(),
                page_size.bytes()
            ),
            Self::PageRangeOverflow {
                start,
                page_size,
                page_count,
            } => write!(
                formatter,
                "translation page range at {:#x} overflows for {} pages of {} bytes",
                start.get(),
                page_count,
                page_size.bytes()
            ),
            Self::OverlappingTranslationMapping {
                existing_start,
                existing_size,
                requested_start,
                requested_size,
            } => write!(
                formatter,
                "translation mapping {:#x}+{} overlaps existing mapping {:#x}+{}",
                requested_start.get(),
                requested_size.bytes(),
                existing_start.get(),
                existing_size.bytes()
            ),
        }
    }
}

impl Error for TranslationError {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TranslationPageSize {
    bytes: u64,
}

impl TranslationPageSize {
    pub fn new(bytes: u64) -> Result<Self, TranslationError> {
        if bytes == 0 {
            return Err(TranslationError::ZeroPageSize);
        }
        if !bytes.is_power_of_two() {
            return Err(TranslationError::NonPowerOfTwoPageSize { bytes });
        }

        Ok(Self { bytes })
    }

    pub const fn bytes(self) -> u64 {
        self.bytes
    }

    pub fn page_address(self, address: Address) -> Address {
        Address::new(address.get() & !(self.bytes - 1))
    }

    pub fn page_offset(self, address: Address) -> u64 {
        address.get() - self.page_address(address).get()
    }

    pub fn is_aligned(self, address: Address) -> bool {
        self.page_offset(address) == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranslationPagePermissions {
    read: bool,
    write: bool,
    execute: bool,
}

impl TranslationPagePermissions {
    pub const fn new(read: bool, write: bool, execute: bool) -> Self {
        Self {
            read,
            write,
            execute,
        }
    }

    pub const fn read_only() -> Self {
        Self::new(true, false, false)
    }

    pub const fn read_write() -> Self {
        Self::new(true, true, false)
    }

    pub const fn read_execute() -> Self {
        Self::new(true, false, true)
    }

    pub const fn read_write_execute() -> Self {
        Self::new(true, true, true)
    }

    pub const fn read(self) -> bool {
        self.read
    }

    pub const fn write(self) -> bool {
        self.write
    }

    pub const fn execute(self) -> bool {
        self.execute
    }

    pub const fn allows(self, access: TranslationAccessKind) -> bool {
        match access {
            TranslationAccessKind::InstructionFetch => self.execute,
            TranslationAccessKind::Load | TranslationAccessKind::Prefetch => self.read,
            TranslationAccessKind::Store | TranslationAccessKind::Atomic => self.write,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TranslationPageMappingScope {
    Global,
    NonGlobal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationPageMapping {
    virtual_range: AddressRange,
    physical_start: Address,
    page_count: u64,
    permissions: TranslationPagePermissions,
    scope: TranslationPageMappingScope,
}

impl TranslationPageMapping {
    fn new(
        page_size: TranslationPageSize,
        virtual_start: Address,
        physical_start: Address,
        page_count: u64,
        permissions: TranslationPagePermissions,
        scope: TranslationPageMappingScope,
    ) -> Result<Self, TranslationError> {
        if page_count == 0 {
            return Err(TranslationError::ZeroPageCount);
        }
        if !page_size.is_aligned(virtual_start) {
            return Err(TranslationError::UnalignedVirtualPage {
                address: virtual_start,
                page_size,
            });
        }
        if !page_size.is_aligned(physical_start) {
            return Err(TranslationError::UnalignedPhysicalPage {
                address: physical_start,
                page_size,
            });
        }

        let byte_count = page_size.bytes().checked_mul(page_count).ok_or(
            TranslationError::PageRangeOverflow {
                start: virtual_start,
                page_size,
                page_count,
            },
        )?;
        let size = AccessSize::new(byte_count).map_err(|_| TranslationError::ZeroPageCount)?;
        let virtual_range = AddressRange::new(virtual_start, size).map_err(|_| {
            TranslationError::PageRangeOverflow {
                start: virtual_start,
                page_size,
                page_count,
            }
        })?;
        AddressRange::new(physical_start, size).map_err(|_| {
            TranslationError::PageRangeOverflow {
                start: physical_start,
                page_size,
                page_count,
            }
        })?;

        Ok(Self {
            virtual_range,
            physical_start,
            page_count,
            permissions,
            scope,
        })
    }

    pub const fn virtual_start(&self) -> Address {
        self.virtual_range.start()
    }

    pub const fn physical_start(&self) -> Address {
        self.physical_start
    }

    pub const fn virtual_range(&self) -> AddressRange {
        self.virtual_range
    }

    pub const fn page_count(&self) -> u64 {
        self.page_count
    }

    pub const fn permissions(&self) -> TranslationPagePermissions {
        self.permissions
    }

    pub const fn scope(&self) -> TranslationPageMappingScope {
        self.scope
    }

    fn physical_address(&self, virtual_address: Address) -> Address {
        Address::new(
            self.physical_start.get() + (virtual_address.get() - self.virtual_start().get()),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationPageMapSnapshot {
    page_size: TranslationPageSize,
    mappings: Vec<TranslationPageMapping>,
}

impl TranslationPageMapSnapshot {
    pub fn new(page_size: TranslationPageSize, mappings: Vec<TranslationPageMapping>) -> Self {
        Self {
            page_size,
            mappings,
        }
    }

    pub const fn page_size(&self) -> TranslationPageSize {
        self.page_size
    }

    pub fn mappings(&self) -> &[TranslationPageMapping] {
        &self.mappings
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationPageMapCheckpointPayload {
    snapshot: TranslationPageMapSnapshot,
}

impl TranslationPageMapCheckpointPayload {
    pub fn from_map(map: &TranslationPageMap) -> Self {
        Self {
            snapshot: map.snapshot(),
        }
    }

    pub fn from_snapshot(snapshot: TranslationPageMapSnapshot) -> Result<Self, TranslationError> {
        TranslationPageMap::from_snapshot(&snapshot)?;
        Ok(Self { snapshot })
    }

    pub fn decode(payload: &[u8]) -> Result<Self, TranslationError> {
        if payload.len() < PAGE_MAP_CHECKPOINT_HEADER_BYTES {
            return Err(TranslationError::InvalidPageMapCheckpointPayloadSize {
                expected: PAGE_MAP_CHECKPOINT_HEADER_BYTES,
                actual: payload.len(),
            });
        }
        if payload[0..PAGE_MAP_CHECKPOINT_MAGIC.len()] != PAGE_MAP_CHECKPOINT_MAGIC {
            return Err(TranslationError::InvalidPageMapCheckpointMagic);
        }

        let mut offset = PAGE_MAP_CHECKPOINT_MAGIC.len();
        let version = read_page_map_checkpoint_u32(payload, &mut offset);
        if version != PAGE_MAP_CHECKPOINT_VERSION {
            return Err(TranslationError::UnsupportedPageMapCheckpointVersion { version });
        }

        let page_size =
            TranslationPageSize::new(read_page_map_checkpoint_u64(payload, &mut offset))?;
        let mapping_count = read_page_map_checkpoint_u32(payload, &mut offset) as usize;
        let reserved = read_page_map_checkpoint_u32(payload, &mut offset);
        if reserved != 0 {
            return Err(TranslationError::InvalidPageMapCheckpointReserved { value: reserved });
        }
        let expected = page_map_checkpoint_payload_size(mapping_count)?;
        if payload.len() != expected {
            return Err(TranslationError::InvalidPageMapCheckpointPayloadSize {
                expected,
                actual: payload.len(),
            });
        }

        let mut mappings = Vec::with_capacity(mapping_count);
        for _ in 0..mapping_count {
            mappings.push(read_page_map_checkpoint_mapping(
                page_size,
                payload,
                &mut offset,
            )?);
        }

        Self::from_snapshot(TranslationPageMapSnapshot::new(page_size, mappings))
    }

    pub fn encode(&self) -> Vec<u8> {
        self.try_encode()
            .expect("translation page-map checkpoint values fit the checkpoint encoding")
    }

    pub fn try_encode(&self) -> Result<Vec<u8>, TranslationError> {
        let mapping_count =
            encode_page_map_checkpoint_u32("mapping count", self.snapshot.mappings().len())?;
        let mut payload = Vec::with_capacity(page_map_checkpoint_payload_size(
            self.snapshot.mappings().len(),
        )?);
        payload.extend_from_slice(&PAGE_MAP_CHECKPOINT_MAGIC);
        payload.extend_from_slice(&PAGE_MAP_CHECKPOINT_VERSION.to_le_bytes());
        payload.extend_from_slice(&self.snapshot.page_size().bytes().to_le_bytes());
        payload.extend_from_slice(&mapping_count.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        for mapping in self.snapshot.mappings() {
            write_page_map_checkpoint_mapping(&mut payload, mapping);
        }
        Ok(payload)
    }

    pub const fn snapshot(&self) -> &TranslationPageMapSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> TranslationPageMapSnapshot {
        self.snapshot
    }
}

fn write_page_map_checkpoint_mapping(payload: &mut Vec<u8>, mapping: &TranslationPageMapping) {
    payload.extend_from_slice(&mapping.virtual_start().get().to_le_bytes());
    payload.extend_from_slice(&mapping.physical_start().get().to_le_bytes());
    payload.extend_from_slice(&mapping.page_count().to_le_bytes());
    payload.extend_from_slice(
        &encode_page_map_checkpoint_permissions(mapping.permissions()).to_le_bytes(),
    );
    payload.extend_from_slice(&encode_page_map_checkpoint_scope(mapping.scope()).to_le_bytes());
}

fn read_page_map_checkpoint_mapping(
    page_size: TranslationPageSize,
    payload: &[u8],
    offset: &mut usize,
) -> Result<TranslationPageMapping, TranslationError> {
    let virtual_start = Address::new(read_page_map_checkpoint_u64(payload, offset));
    let physical_start = Address::new(read_page_map_checkpoint_u64(payload, offset));
    let page_count = read_page_map_checkpoint_u64(payload, offset);
    let permissions =
        decode_page_map_checkpoint_permissions(read_page_map_checkpoint_u32(payload, offset))?;
    let scope = decode_page_map_checkpoint_scope(read_page_map_checkpoint_u32(payload, offset))?;

    TranslationPageMapping::new(
        page_size,
        virtual_start,
        physical_start,
        page_count,
        permissions,
        scope,
    )
}

fn encode_page_map_checkpoint_permissions(permissions: TranslationPagePermissions) -> u32 {
    u32::from(permissions.read())
        | (u32::from(permissions.write()) << 1)
        | (u32::from(permissions.execute()) << 2)
}

fn decode_page_map_checkpoint_permissions(
    code: u32,
) -> Result<TranslationPagePermissions, TranslationError> {
    if code & !PAGE_MAP_CHECKPOINT_PERMISSION_MASK != 0 {
        return Err(TranslationError::InvalidPageMapCheckpointPermissions { code });
    }
    Ok(TranslationPagePermissions::new(
        code & 0x1 != 0,
        code & 0x2 != 0,
        code & 0x4 != 0,
    ))
}

fn encode_page_map_checkpoint_scope(scope: TranslationPageMappingScope) -> u32 {
    match scope {
        TranslationPageMappingScope::NonGlobal => 0,
        TranslationPageMappingScope::Global => 1,
    }
}

fn decode_page_map_checkpoint_scope(
    code: u32,
) -> Result<TranslationPageMappingScope, TranslationError> {
    match code {
        0 => Ok(TranslationPageMappingScope::NonGlobal),
        1 => Ok(TranslationPageMappingScope::Global),
        _ => Err(TranslationError::InvalidPageMapCheckpointScope { code }),
    }
}

fn page_map_checkpoint_payload_size(mapping_count: usize) -> Result<usize, TranslationError> {
    let mapping_bytes = mapping_count
        .checked_mul(PAGE_MAP_CHECKPOINT_ENTRY_BYTES)
        .ok_or(TranslationError::InvalidPageMapCheckpointPayloadSize {
            expected: usize::MAX,
            actual: 0,
        })?;
    PAGE_MAP_CHECKPOINT_HEADER_BYTES
        .checked_add(mapping_bytes)
        .ok_or(TranslationError::InvalidPageMapCheckpointPayloadSize {
            expected: usize::MAX,
            actual: 0,
        })
}

fn encode_page_map_checkpoint_u32(
    field: &'static str,
    value: usize,
) -> Result<u32, TranslationError> {
    u32::try_from(value).map_err(|_| TranslationError::PageMapCheckpointValueTooLarge {
        field,
        value,
        maximum: PAGE_MAP_CHECKPOINT_U32_MAX,
    })
}

fn read_page_map_checkpoint_u32(payload: &[u8], offset: &mut usize) -> u32 {
    let bytes = payload[*offset..*offset + U32_BYTES]
        .try_into()
        .expect("checkpoint u32 slice width is fixed");
    *offset += U32_BYTES;
    u32::from_le_bytes(bytes)
}

fn read_page_map_checkpoint_u64(payload: &[u8], offset: &mut usize) -> u64 {
    let bytes = payload[*offset..*offset + U64_BYTES]
        .try_into()
        .expect("checkpoint u64 slice width is fixed");
    *offset += U64_BYTES;
    u64::from_le_bytes(bytes)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationPageMap {
    page_size: TranslationPageSize,
    mappings: Vec<TranslationPageMapping>,
}

impl TranslationPageMap {
    pub fn new(page_size: TranslationPageSize) -> Self {
        Self {
            page_size,
            mappings: Vec::new(),
        }
    }

    pub fn from_snapshot(snapshot: &TranslationPageMapSnapshot) -> Result<Self, TranslationError> {
        let mut map = Self::new(snapshot.page_size());
        for mapping in snapshot.mappings() {
            map.map_with_scope(
                mapping.virtual_start(),
                mapping.physical_start(),
                mapping.page_count(),
                mapping.permissions(),
                mapping.scope(),
            )?;
        }
        Ok(map)
    }

    pub fn restore(
        &mut self,
        snapshot: &TranslationPageMapSnapshot,
    ) -> Result<(), TranslationError> {
        *self = Self::from_snapshot(snapshot)?;
        Ok(())
    }

    pub const fn page_size(&self) -> TranslationPageSize {
        self.page_size
    }

    pub fn mapping_count(&self) -> usize {
        self.mappings.len()
    }

    pub fn mappings(&self) -> &[TranslationPageMapping] {
        &self.mappings
    }

    pub fn map(
        &mut self,
        virtual_start: Address,
        physical_start: Address,
        page_count: u64,
        permissions: TranslationPagePermissions,
    ) -> Result<(), TranslationError> {
        self.map_with_scope(
            virtual_start,
            physical_start,
            page_count,
            permissions,
            TranslationPageMappingScope::NonGlobal,
        )
    }

    pub fn map_with_scope(
        &mut self,
        virtual_start: Address,
        physical_start: Address,
        page_count: u64,
        permissions: TranslationPagePermissions,
        scope: TranslationPageMappingScope,
    ) -> Result<(), TranslationError> {
        let mapping = TranslationPageMapping::new(
            self.page_size,
            virtual_start,
            physical_start,
            page_count,
            permissions,
            scope,
        )?;
        if let Some(existing) = self
            .mappings
            .iter()
            .find(|existing| existing.virtual_range().overlaps(mapping.virtual_range()))
        {
            return Err(TranslationError::OverlappingTranslationMapping {
                existing_start: existing.virtual_start(),
                existing_size: existing.virtual_range().size(),
                requested_start: mapping.virtual_start(),
                requested_size: mapping.virtual_range().size(),
            });
        }

        self.mappings.push(mapping);
        self.mappings
            .sort_by_key(|mapping| (mapping.virtual_start(), mapping.virtual_range().end()));
        Ok(())
    }

    pub fn translate(&self, request: &TranslationRequest) -> TranslationResolution {
        let Some(mapping) = self.mapping_for_range(request.range()) else {
            return TranslationResolution::fault(TranslationFault::new(
                request.virtual_address(),
                TranslationFaultKind::PageFault,
            ));
        };
        if !mapping.permissions().allows(request.access()) {
            return TranslationResolution::fault(TranslationFault::new(
                request.virtual_address(),
                TranslationFaultKind::PermissionFault,
            ));
        }

        TranslationResolution::mapped(mapping.physical_address(request.virtual_address()))
    }

    pub fn translate_segments(
        &self,
        request: &TranslationRequest,
    ) -> TranslationSegmentedResolution {
        let mut segments = Vec::new();
        let mut cursor = request.range().start().get();
        let end = request.range().end().get();

        while cursor < end {
            let virtual_start = Address::new(cursor);
            let page_start = self.page_size.page_address(virtual_start).get();
            let page_end = page_start
                .checked_add(self.page_size.bytes())
                .unwrap_or(end);
            let segment_end = end.min(page_end);
            let segment_size =
                AccessSize::new(segment_end - cursor).expect("translation segment size is nonzero");
            let segment_range = AddressRange::new(virtual_start, segment_size)
                .expect("translation segment is within request range");

            let Some(mapping) = self.mapping_for_range(segment_range) else {
                return TranslationSegmentedResolution::fault(TranslationFault::new(
                    virtual_start,
                    TranslationFaultKind::PageFault,
                ));
            };
            if !mapping.permissions().allows(request.access()) {
                return TranslationSegmentedResolution::fault(TranslationFault::new(
                    virtual_start,
                    TranslationFaultKind::PermissionFault,
                ));
            }

            segments.push(
                TranslationSegment::new(
                    virtual_start,
                    segment_size,
                    mapping.physical_address(virtual_start),
                )
                .expect("translation segment physical range is within mapping"),
            );
            cursor = segment_end;
        }

        TranslationSegmentedResolution::mapped(segments)
    }

    pub fn snapshot(&self) -> TranslationPageMapSnapshot {
        TranslationPageMapSnapshot::new(self.page_size, self.mappings.clone())
    }

    fn mapping_for_range(&self, range: AddressRange) -> Option<&TranslationPageMapping> {
        self.mappings
            .iter()
            .find(|mapping| mapping.virtual_range().contains_range(range))
    }
}

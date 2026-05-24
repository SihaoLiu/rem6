use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{AccessSize, Address, AddressRange, AgentId};

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
        let mut minimum_next_order = 0;
        for entry in snapshot.entries() {
            let request = entry.request().clone();
            if !ids.insert(request.id()) {
                return Err(TranslationError::DuplicateRequest {
                    request: request.id(),
                });
            }
            minimum_next_order = minimum_next_order.max(entry.order().saturating_add(1));
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
            next_order: snapshot.next_order().max(minimum_next_order),
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

    pub fn pending_request_ids(&self) -> Vec<TranslationRequestId> {
        self.service_order_ids(None)
    }

    pub fn ready_request_ids(&self, tick: u64) -> Vec<TranslationRequestId> {
        self.service_order_ids(Some(tick))
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
    QueueFull { capacity: usize },
    DuplicateRequest { request: TranslationRequestId },
    UnknownRequest { request: TranslationRequestId },
    AddressOverflow { start: Address, size: AccessSize },
    TickOverflow { issue_tick: u64, latency: u64 },
    QueueOrderOverflow,
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
            Self::QueueOrderOverflow => {
                write!(
                    formatter,
                    "translation queue stable order counter overflowed"
                )
            }
        }
    }
}

impl Error for TranslationError {}

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
    QueueOrderOverflow,
    ZeroTlbCapacity,
    TlbCapacityExceeded {
        capacity: usize,
    },
    DuplicateTlbEntry {
        virtual_page: Address,
    },
    TlbOrderOverflow,
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
            Self::QueueOrderOverflow => {
                write!(
                    formatter,
                    "translation queue stable order counter overflowed"
                )
            }
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
            Self::TlbOrderOverflow => {
                write!(formatter, "translation TLB stable order counter overflowed")
            }
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationPageMapping {
    virtual_range: AddressRange,
    physical_start: Address,
    page_count: u64,
    permissions: TranslationPagePermissions,
}

impl TranslationPageMapping {
    fn new(
        page_size: TranslationPageSize,
        virtual_start: Address,
        physical_start: Address,
        page_count: u64,
        permissions: TranslationPagePermissions,
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
            map.map(
                mapping.virtual_start(),
                mapping.physical_start(),
                mapping.page_count(),
                mapping.permissions(),
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
        let mapping = TranslationPageMapping::new(
            self.page_size,
            virtual_start,
            physical_start,
            page_count,
            permissions,
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
        let Some(mapping) = self
            .mappings
            .iter()
            .find(|mapping| mapping.virtual_range().contains_range(request.range()))
        else {
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

    pub fn snapshot(&self) -> TranslationPageMapSnapshot {
        TranslationPageMapSnapshot::new(self.page_size, self.mappings.clone())
    }
}

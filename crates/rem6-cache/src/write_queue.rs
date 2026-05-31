use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use rem6_memory::{
    AccessSize, Address, AddressRange, CacheLineLayout, MemoryError, MemoryOperation,
    MemoryRequest, MemoryRequestId,
};

use crate::replacement::ReplacementDecision;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CacheWriteQueueHandle(u64);

impl CacheWriteQueueHandle {
    pub const fn new(index: u64) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheWriteQueueEntryKind {
    WritebackClean,
    WritebackDirty,
    CleanEvict,
    UncacheableWrite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheCleanReplacementPolicy {
    CleanEvict,
    WritebackClean,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheWriteQueueConfig {
    entries: usize,
    reserve: usize,
    total_entries: usize,
}

impl CacheWriteQueueConfig {
    pub fn new(entries: usize, reserve: usize) -> Result<Self, CacheWriteQueueError> {
        if entries == 0 {
            return Err(CacheWriteQueueError::ZeroEntries);
        }
        let total_entries = entries
            .checked_add(reserve)
            .ok_or(CacheWriteQueueError::CapacityOverflow { entries, reserve })?;
        Ok(Self {
            entries,
            reserve,
            total_entries,
        })
    }

    pub const fn entries(&self) -> usize {
        self.entries
    }

    pub const fn reserve(&self) -> usize {
        self.reserve
    }

    pub const fn total_entries(&self) -> usize {
        self.total_entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheWriteQueueError {
    ZeroEntries,
    CapacityOverflow {
        entries: usize,
        reserve: usize,
    },
    EntrySlotsFull {
        entries: usize,
        reserve: usize,
    },
    ReserveSlotsFull {
        total_entries: usize,
    },
    WritebackOperationRequired {
        operation: MemoryOperation,
    },
    UncacheableWriteRequired {
        operation: MemoryOperation,
    },
    UnknownEntry {
        handle: CacheWriteQueueHandle,
    },
    SnapshotConfigMismatch {
        expected: CacheWriteQueueConfig,
        actual: CacheWriteQueueConfig,
    },
    SnapshotTooManyEntries {
        entries: usize,
        total_entries: usize,
    },
    SnapshotDuplicateHandle {
        handle: CacheWriteQueueHandle,
    },
    SnapshotInvalidEntryOperation {
        handle: CacheWriteQueueHandle,
        operation: MemoryOperation,
        uncacheable: bool,
    },
    SnapshotNextHandleTooSmall {
        next_handle: u64,
        handle: CacheWriteQueueHandle,
    },
    ReplacementVictimWayMismatch {
        decision_way: usize,
        victim_way: usize,
    },
    Memory(MemoryError),
}

impl fmt::Display for CacheWriteQueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroEntries => write!(formatter, "cache write queue has no entries"),
            Self::CapacityOverflow { entries, reserve } => write!(
                formatter,
                "cache write queue entries {entries} plus reserve {reserve} overflows"
            ),
            Self::EntrySlotsFull { entries, reserve } => write!(
                formatter,
                "cache write queue has {entries} effective entries allocated with {reserve} reserve entries"
            ),
            Self::ReserveSlotsFull { total_entries } => write!(
                formatter,
                "cache write queue has all {total_entries} physical entries allocated"
            ),
            Self::WritebackOperationRequired { operation } => write!(
                formatter,
                "cache write queue writeback entry cannot use {operation:?}"
            ),
            Self::UncacheableWriteRequired { operation } => write!(
                formatter,
                "cache write queue uncacheable entry cannot use {operation:?}"
            ),
            Self::UnknownEntry { handle } => {
                write!(formatter, "unknown cache write queue entry {handle:?}")
            }
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "cache write queue snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotTooManyEntries {
                entries,
                total_entries,
            } => write!(
                formatter,
                "cache write queue snapshot has {entries} entries for {total_entries} slots"
            ),
            Self::SnapshotDuplicateHandle { handle } => write!(
                formatter,
                "cache write queue snapshot repeats entry {handle:?}"
            ),
            Self::SnapshotInvalidEntryOperation {
                handle,
                operation,
                uncacheable,
            } => write!(
                formatter,
                "cache write queue snapshot entry {handle:?} has operation {operation:?} with uncacheable={uncacheable}"
            ),
            Self::SnapshotNextHandleTooSmall {
                next_handle,
                handle,
            } => write!(
                formatter,
                "cache write queue snapshot next handle {next_handle} is not after {handle:?}"
            ),
            Self::ReplacementVictimWayMismatch {
                decision_way,
                victim_way,
            } => write!(
                formatter,
                "cache write queue replacement decision selected way {decision_way} but victim line came from way {victim_way}"
            ),
            Self::Memory(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for CacheWriteQueueError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            _ => None,
        }
    }
}

impl From<MemoryError> for CacheWriteQueueError {
    fn from(error: MemoryError) -> Self {
        Self::Memory(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheReplacementVictimState {
    Invalid,
    Clean { data: Vec<u8> },
    Dirty { data: Vec<u8> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheReplacementVictim {
    way: usize,
    line: Address,
    line_layout: CacheLineLayout,
    secure: bool,
    state: CacheReplacementVictimState,
}

impl CacheReplacementVictim {
    pub fn invalid(way: usize, line: Address, line_layout: CacheLineLayout, secure: bool) -> Self {
        Self {
            way,
            line: line_layout.line_address(line),
            line_layout,
            secure,
            state: CacheReplacementVictimState::Invalid,
        }
    }

    pub fn clean(
        way: usize,
        line: Address,
        data: Vec<u8>,
        line_layout: CacheLineLayout,
        secure: bool,
    ) -> Self {
        Self {
            way,
            line: line_layout.line_address(line),
            line_layout,
            secure,
            state: CacheReplacementVictimState::Clean { data },
        }
    }

    pub fn dirty(
        way: usize,
        line: Address,
        data: Vec<u8>,
        line_layout: CacheLineLayout,
        secure: bool,
    ) -> Self {
        Self {
            way,
            line: line_layout.line_address(line),
            line_layout,
            secure,
            state: CacheReplacementVictimState::Dirty { data },
        }
    }

    pub const fn way(&self) -> usize {
        self.way
    }

    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn line_layout(&self) -> CacheLineLayout {
        self.line_layout
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn state(&self) -> &CacheReplacementVictimState {
        &self.state
    }

    fn into_writeback_request(
        self,
        request_id: MemoryRequestId,
        clean_policy: CacheCleanReplacementPolicy,
    ) -> Result<Option<MemoryRequest>, CacheWriteQueueError> {
        let request = match self.state {
            CacheReplacementVictimState::Invalid => return Ok(None),
            CacheReplacementVictimState::Clean { data } => match clean_policy {
                CacheCleanReplacementPolicy::CleanEvict => {
                    MemoryRequest::clean_evict(request_id, self.line, self.line_layout)?
                }
                CacheCleanReplacementPolicy::WritebackClean => {
                    MemoryRequest::writeback_clean(request_id, self.line, data, self.line_layout)?
                }
            },
            CacheReplacementVictimState::Dirty { data } => {
                MemoryRequest::writeback_dirty(request_id, self.line, data, self.line_layout)?
            }
        };
        Ok(Some(request))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheWriteQueueEntry {
    handle: CacheWriteQueueHandle,
    request: MemoryRequest,
    secure: bool,
    ready_tick: u64,
    order: u64,
    uncacheable: bool,
}

impl CacheWriteQueueEntry {
    fn new(
        handle: CacheWriteQueueHandle,
        request: MemoryRequest,
        secure: bool,
        ready_tick: u64,
        order: u64,
        uncacheable: bool,
    ) -> Self {
        Self {
            handle,
            request,
            secure,
            ready_tick,
            order,
            uncacheable,
        }
    }

    pub const fn handle(&self) -> CacheWriteQueueHandle {
        self.handle
    }

    pub const fn request(&self) -> &MemoryRequest {
        &self.request
    }

    pub fn line(&self) -> Address {
        self.request.line_address()
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn ready_tick(&self) -> u64 {
        self.ready_tick
    }

    pub const fn order(&self) -> u64 {
        self.order
    }

    pub const fn uncacheable(&self) -> bool {
        self.uncacheable
    }

    pub fn kind(&self) -> CacheWriteQueueEntryKind {
        entry_kind(self.request.operation(), self.uncacheable)
            .expect("entry operation was validated before allocation")
    }

    fn read_from_writeback(&self, range: AddressRange) -> Option<Vec<u8>> {
        let line_layout = self.request.line_layout();
        let line_range = AddressRange::new(
            self.request.line_address(),
            AccessSize::new(line_layout.bytes()).ok()?,
        )
        .ok()?;
        if !line_range.contains_range(range) {
            return None;
        }
        let offset = line_layout.line_offset(range.start()) as usize;
        let size = range.size().bytes() as usize;
        let data = self.request.data()?;
        data.get(offset..offset.checked_add(size)?).map(Vec::from)
    }

    fn read_from_uncacheable(&self, range: AddressRange) -> Option<Vec<u8>> {
        if !self.request.range().contains_range(range) {
            return None;
        }
        let offset = (range.start().get() - self.request.range().start().get()) as usize;
        let size = range.size().bytes() as usize;
        let mask = self.request.byte_mask()?;
        let end = offset.checked_add(size)?;
        if !mask.bits().get(offset..end)?.iter().all(|bit| *bit) {
            return None;
        }
        self.request.data()?.get(offset..end).map(Vec::from)
    }

    fn overlay_uncacheable(&self, range: AddressRange, data: &mut [u8]) {
        if !self.request.range().overlaps(range) {
            return;
        }
        let Some(mask) = self.request.byte_mask() else {
            return;
        };
        let Some(payload) = self.request.data() else {
            return;
        };

        let start = self.request.range().start().get().max(range.start().get());
        let end = self.request.range().end().get().min(range.end().get());
        for address in start..end {
            let request_offset = (address - self.request.range().start().get()) as usize;
            if !mask.bits().get(request_offset).copied().unwrap_or(false) {
                continue;
            }
            let read_offset = (address - range.start().get()) as usize;
            if let (Some(target), Some(source)) =
                (data.get_mut(read_offset), payload.get(request_offset))
            {
                *target = *source;
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheWriteQueueUpdate {
    handle: CacheWriteQueueHandle,
    line: Address,
    allocated_count: usize,
    reserve_used: usize,
}

impl CacheWriteQueueUpdate {
    fn new(entry: &CacheWriteQueueEntry, allocated_count: usize, effective_entries: usize) -> Self {
        Self {
            handle: entry.handle(),
            line: entry.line(),
            allocated_count,
            reserve_used: allocated_count.saturating_sub(effective_entries),
        }
    }

    pub const fn handle(&self) -> CacheWriteQueueHandle {
        self.handle
    }

    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn allocated_count(&self) -> usize {
        self.allocated_count
    }

    pub const fn reserve_used(&self) -> usize {
        self.reserve_used
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheWriteQueueIssue {
    handle: CacheWriteQueueHandle,
    request: MemoryRequest,
    post_issue_downstream_request: Option<MemoryRequest>,
    secure: bool,
    ready_tick: u64,
    order: u64,
    kind: CacheWriteQueueEntryKind,
}

impl CacheWriteQueueIssue {
    fn from_entry(entry: CacheWriteQueueEntry) -> Self {
        Self {
            handle: entry.handle(),
            secure: entry.secure(),
            ready_tick: entry.ready_tick(),
            order: entry.order(),
            kind: entry.kind(),
            request: entry.request,
            post_issue_downstream_request: None,
        }
    }

    pub(crate) fn with_post_issue_downstream_request(mut self, request: MemoryRequest) -> Self {
        self.post_issue_downstream_request = Some(request);
        self
    }

    pub const fn handle(&self) -> CacheWriteQueueHandle {
        self.handle
    }

    pub const fn request(&self) -> &MemoryRequest {
        &self.request
    }

    pub fn post_issue_downstream_request(&self) -> Option<&MemoryRequest> {
        self.post_issue_downstream_request.as_ref()
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn ready_tick(&self) -> u64 {
        self.ready_tick
    }

    pub const fn order(&self) -> u64 {
        self.order
    }

    pub const fn kind(&self) -> CacheWriteQueueEntryKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheWriteQueueSnapshot {
    config: CacheWriteQueueConfig,
    entries: Vec<CacheWriteQueueEntry>,
    next_handle: u64,
    next_order: u64,
}

impl CacheWriteQueueSnapshot {
    pub fn new(
        config: CacheWriteQueueConfig,
        entries: Vec<CacheWriteQueueEntry>,
        next_handle: u64,
        next_order: u64,
    ) -> Self {
        Self {
            config,
            entries,
            next_handle,
            next_order,
        }
    }

    pub const fn config(&self) -> &CacheWriteQueueConfig {
        &self.config
    }

    pub fn entries(&self) -> &[CacheWriteQueueEntry] {
        &self.entries
    }

    pub const fn next_handle(&self) -> u64 {
        self.next_handle
    }

    pub const fn next_order(&self) -> u64 {
        self.next_order
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheWriteQueue {
    config: CacheWriteQueueConfig,
    entries: Vec<CacheWriteQueueEntry>,
    next_handle: u64,
    next_order: u64,
}

impl CacheWriteQueue {
    pub fn new(config: CacheWriteQueueConfig) -> Self {
        Self {
            config,
            entries: Vec::new(),
            next_handle: 0,
            next_order: 0,
        }
    }

    pub const fn config(&self) -> &CacheWriteQueueConfig {
        &self.config
    }

    pub fn allocated_count(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.config.entries()
    }

    pub fn is_reserve_full(&self) -> bool {
        self.entries.len() >= self.config.total_entries()
    }

    pub fn reserve_used(&self) -> usize {
        self.entries.len().saturating_sub(self.config.entries())
    }

    pub fn entry(
        &self,
        handle: CacheWriteQueueHandle,
    ) -> Result<&CacheWriteQueueEntry, CacheWriteQueueError> {
        self.entries
            .iter()
            .find(|entry| entry.handle() == handle)
            .ok_or(CacheWriteQueueError::UnknownEntry { handle })
    }

    pub fn enqueue_writeback(
        &mut self,
        request: MemoryRequest,
        secure: bool,
        ready_tick: u64,
    ) -> Result<CacheWriteQueueUpdate, CacheWriteQueueError> {
        self.enqueue_inner(request, secure, ready_tick, false, false)
    }

    pub fn enqueue_reserved_writeback(
        &mut self,
        request: MemoryRequest,
        secure: bool,
        ready_tick: u64,
    ) -> Result<CacheWriteQueueUpdate, CacheWriteQueueError> {
        self.enqueue_inner(request, secure, ready_tick, false, true)
    }

    pub fn enqueue_uncacheable_write(
        &mut self,
        request: MemoryRequest,
        secure: bool,
        ready_tick: u64,
    ) -> Result<CacheWriteQueueUpdate, CacheWriteQueueError> {
        self.enqueue_inner(request, secure, ready_tick, true, false)
    }

    pub fn enqueue_reserved_uncacheable_write(
        &mut self,
        request: MemoryRequest,
        secure: bool,
        ready_tick: u64,
    ) -> Result<CacheWriteQueueUpdate, CacheWriteQueueError> {
        self.enqueue_inner(request, secure, ready_tick, true, true)
    }

    pub fn enqueue_replacement_writeback(
        &mut self,
        decision: &ReplacementDecision,
        victim: CacheReplacementVictim,
        request_id: MemoryRequestId,
        ready_tick: u64,
        clean_policy: CacheCleanReplacementPolicy,
    ) -> Result<Option<CacheWriteQueueUpdate>, CacheWriteQueueError> {
        if victim.way() != decision.way() {
            return Err(CacheWriteQueueError::ReplacementVictimWayMismatch {
                decision_way: decision.way(),
                victim_way: victim.way(),
            });
        }

        let secure = victim.secure();
        let Some(request) = victim.into_writeback_request(request_id, clean_policy)? else {
            return Ok(None);
        };
        self.enqueue_writeback(request, secure, ready_tick)
            .map(Some)
    }

    fn enqueue_inner(
        &mut self,
        request: MemoryRequest,
        secure: bool,
        ready_tick: u64,
        uncacheable: bool,
        use_reserve: bool,
    ) -> Result<CacheWriteQueueUpdate, CacheWriteQueueError> {
        self.validate_request(request.operation(), uncacheable)?;
        self.can_allocate(use_reserve)?;

        let entry = CacheWriteQueueEntry::new(
            self.next_handle(),
            request,
            secure,
            ready_tick,
            self.next_order(),
            uncacheable,
        );
        self.entries.push(entry);
        let allocated_count = self.entries.len();
        let entry = self.entries.last().expect("entry was just pushed");
        Ok(CacheWriteQueueUpdate::new(
            entry,
            allocated_count,
            self.config.entries(),
        ))
    }

    fn validate_request(
        &self,
        operation: MemoryOperation,
        uncacheable: bool,
    ) -> Result<(), CacheWriteQueueError> {
        match (entry_kind(operation, uncacheable), uncacheable) {
            (Some(CacheWriteQueueEntryKind::UncacheableWrite), true)
            | (Some(CacheWriteQueueEntryKind::WritebackClean), false)
            | (Some(CacheWriteQueueEntryKind::WritebackDirty), false)
            | (Some(CacheWriteQueueEntryKind::CleanEvict), false) => Ok(()),
            (_, true) => Err(CacheWriteQueueError::UncacheableWriteRequired { operation }),
            (_, false) => Err(CacheWriteQueueError::WritebackOperationRequired { operation }),
        }
    }

    fn can_allocate(&self, use_reserve: bool) -> Result<(), CacheWriteQueueError> {
        if use_reserve {
            if self.entries.len() >= self.config.total_entries() {
                return Err(CacheWriteQueueError::ReserveSlotsFull {
                    total_entries: self.config.total_entries(),
                });
            }
            return Ok(());
        }

        if self.entries.len() >= self.config.entries() {
            return Err(CacheWriteQueueError::EntrySlotsFull {
                entries: self.config.entries(),
                reserve: self.config.reserve(),
            });
        }
        Ok(())
    }

    pub fn ready_handles(&self, tick: u64) -> Vec<CacheWriteQueueHandle> {
        let mut entries = self
            .entries
            .iter()
            .filter(|entry| entry.ready_tick() <= tick)
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| (entry.ready_tick(), entry.order(), entry.handle()));
        entries
            .into_iter()
            .map(CacheWriteQueueEntry::handle)
            .collect()
    }

    pub fn next_ready_tick(&self) -> Option<u64> {
        self.entries
            .iter()
            .map(CacheWriteQueueEntry::ready_tick)
            .min()
    }

    pub fn issue_next(
        &mut self,
        tick: u64,
    ) -> Result<Option<CacheWriteQueueIssue>, CacheWriteQueueError> {
        let Some(index) = self.next_ready_index(tick) else {
            return Ok(None);
        };
        Ok(Some(CacheWriteQueueIssue::from_entry(
            self.entries.remove(index),
        )))
    }

    pub fn mark_in_service(
        &mut self,
        handle: CacheWriteQueueHandle,
    ) -> Result<CacheWriteQueueIssue, CacheWriteQueueError> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.handle() == handle)
            .ok_or(CacheWriteQueueError::UnknownEntry { handle })?;
        Ok(CacheWriteQueueIssue::from_entry(self.entries.remove(index)))
    }

    pub fn delay_until(
        &mut self,
        handle: CacheWriteQueueHandle,
        ready_tick: u64,
    ) -> Result<CacheWriteQueueUpdate, CacheWriteQueueError> {
        let allocated_count = self.entries.len();
        let effective_entries = self.config.entries();
        let entry = self.entry_mut(handle)?;
        entry.ready_tick = ready_tick;
        Ok(CacheWriteQueueUpdate::new(
            entry,
            allocated_count,
            effective_entries,
        ))
    }

    pub fn find_match(
        &self,
        line: Address,
        secure: bool,
        ignore_uncacheable: bool,
    ) -> Option<CacheWriteQueueHandle> {
        self.entries
            .iter()
            .find(|entry| {
                entry.line() == line
                    && entry.secure() == secure
                    && !(ignore_uncacheable && entry.uncacheable())
            })
            .map(CacheWriteQueueEntry::handle)
    }

    pub fn pending_conflict(&self, line: Address, secure: bool) -> Option<CacheWriteQueueHandle> {
        self.entries
            .iter()
            .filter(|entry| entry.line() == line && entry.secure() == secure)
            .min_by_key(|entry| (entry.ready_tick(), entry.order(), entry.handle()))
            .map(CacheWriteQueueEntry::handle)
    }

    pub fn satisfy_read(
        &self,
        address: Address,
        size: AccessSize,
        secure: bool,
    ) -> Result<Option<Vec<u8>>, CacheWriteQueueError> {
        let range = AddressRange::new(address, size)?;
        let mut data = None;
        for entry in self.entries.iter().filter(|entry| entry.secure() == secure) {
            match entry.kind() {
                CacheWriteQueueEntryKind::WritebackClean
                | CacheWriteQueueEntryKind::WritebackDirty => {
                    if let Some(writeback) = entry.read_from_writeback(range) {
                        data = Some(writeback);
                    }
                }
                CacheWriteQueueEntryKind::UncacheableWrite => {
                    if let Some(data) = &mut data {
                        entry.overlay_uncacheable(range, data);
                    } else if let Some(uncacheable) = entry.read_from_uncacheable(range) {
                        data = Some(uncacheable);
                    }
                }
                CacheWriteQueueEntryKind::CleanEvict => {}
            }
        }
        Ok(data)
    }

    pub fn snapshot(&self) -> CacheWriteQueueSnapshot {
        CacheWriteQueueSnapshot::new(
            self.config.clone(),
            self.entries.clone(),
            self.next_handle,
            self.next_order,
        )
    }

    pub fn restore(
        &mut self,
        snapshot: &CacheWriteQueueSnapshot,
    ) -> Result<(), CacheWriteQueueError> {
        self.validate_snapshot(snapshot)?;
        self.entries.clone_from(&snapshot.entries);
        self.next_handle = snapshot.next_handle;
        self.next_order = snapshot.next_order;
        Ok(())
    }

    fn validate_snapshot(
        &self,
        snapshot: &CacheWriteQueueSnapshot,
    ) -> Result<(), CacheWriteQueueError> {
        if snapshot.config() != &self.config {
            return Err(CacheWriteQueueError::SnapshotConfigMismatch {
                expected: self.config.clone(),
                actual: snapshot.config().clone(),
            });
        }
        if snapshot.entries().len() > self.config.total_entries() {
            return Err(CacheWriteQueueError::SnapshotTooManyEntries {
                entries: snapshot.entries().len(),
                total_entries: self.config.total_entries(),
            });
        }

        let mut handles = BTreeSet::new();
        for entry in snapshot.entries() {
            if !handles.insert(entry.handle()) {
                return Err(CacheWriteQueueError::SnapshotDuplicateHandle {
                    handle: entry.handle(),
                });
            }
            if entry.handle().index() >= snapshot.next_handle() {
                return Err(CacheWriteQueueError::SnapshotNextHandleTooSmall {
                    next_handle: snapshot.next_handle(),
                    handle: entry.handle(),
                });
            }
            if entry_kind(entry.request().operation(), entry.uncacheable()).is_none() {
                return Err(CacheWriteQueueError::SnapshotInvalidEntryOperation {
                    handle: entry.handle(),
                    operation: entry.request().operation(),
                    uncacheable: entry.uncacheable(),
                });
            }
        }
        Ok(())
    }

    fn entry_mut(
        &mut self,
        handle: CacheWriteQueueHandle,
    ) -> Result<&mut CacheWriteQueueEntry, CacheWriteQueueError> {
        self.entries
            .iter_mut()
            .find(|entry| entry.handle() == handle)
            .ok_or(CacheWriteQueueError::UnknownEntry { handle })
    }

    fn next_ready_index(&self, tick: u64) -> Option<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.ready_tick() <= tick)
            .min_by_key(|(_, entry)| (entry.ready_tick(), entry.order(), entry.handle()))
            .map(|(index, _)| index)
    }

    fn next_handle(&mut self) -> CacheWriteQueueHandle {
        let handle = CacheWriteQueueHandle::new(self.next_handle);
        self.next_handle = self.next_handle.saturating_add(1);
        handle
    }

    fn next_order(&mut self) -> u64 {
        let order = self.next_order;
        self.next_order = self.next_order.saturating_add(1);
        order
    }
}

fn entry_kind(operation: MemoryOperation, uncacheable: bool) -> Option<CacheWriteQueueEntryKind> {
    match (operation, uncacheable) {
        (MemoryOperation::WritebackClean, false) => Some(CacheWriteQueueEntryKind::WritebackClean),
        (MemoryOperation::WritebackDirty, false) => Some(CacheWriteQueueEntryKind::WritebackDirty),
        (MemoryOperation::CleanEvict, false) => Some(CacheWriteQueueEntryKind::CleanEvict),
        (MemoryOperation::Write, true) => Some(CacheWriteQueueEntryKind::UncacheableWrite),
        _ => None,
    }
}

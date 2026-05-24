use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::prefetch_throttle::QueuedPrefetchThrottle;
use rem6_memory::{Address, AgentId};

const MAX_CONFIDENCE: u8 = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedPrefetchConfig {
    capacity: usize,
    latency: u64,
    max_issue_per_tick: usize,
    filter_duplicates: bool,
    line_size: u64,
    page_size: Option<u64>,
    full_policy: QueuedPrefetchFullPolicy,
}

impl QueuedPrefetchConfig {
    pub fn new(
        capacity: usize,
        latency: u64,
        max_issue_per_tick: usize,
        filter_duplicates: bool,
    ) -> Result<Self, QueuedPrefetcherError> {
        Self::with_line_size(capacity, latency, max_issue_per_tick, filter_duplicates, 1)
    }

    pub fn with_line_size(
        capacity: usize,
        latency: u64,
        max_issue_per_tick: usize,
        filter_duplicates: bool,
        line_size: u64,
    ) -> Result<Self, QueuedPrefetcherError> {
        if capacity == 0 {
            return Err(QueuedPrefetcherError::ZeroCapacity);
        }
        if max_issue_per_tick == 0 {
            return Err(QueuedPrefetcherError::ZeroIssueWidth);
        }
        if line_size == 0 {
            return Err(QueuedPrefetcherError::ZeroLineSize);
        }

        Ok(Self {
            capacity,
            latency,
            max_issue_per_tick,
            filter_duplicates,
            line_size,
            page_size: None,
            full_policy: QueuedPrefetchFullPolicy::RejectNew,
        })
    }

    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    pub const fn latency(&self) -> u64 {
        self.latency
    }

    pub const fn max_issue_per_tick(&self) -> usize {
        self.max_issue_per_tick
    }

    pub const fn filter_duplicates(&self) -> bool {
        self.filter_duplicates
    }

    pub const fn line_size(&self) -> u64 {
        self.line_size
    }

    pub const fn page_size(&self) -> Option<u64> {
        self.page_size
    }

    pub const fn full_policy(&self) -> QueuedPrefetchFullPolicy {
        self.full_policy
    }

    pub fn with_page_size(mut self, page_size: u64) -> Result<Self, QueuedPrefetcherError> {
        if page_size == 0 {
            return Err(QueuedPrefetcherError::ZeroPageSize);
        }
        self.page_size = Some(page_size);
        Ok(self)
    }

    pub const fn with_full_policy(mut self, full_policy: QueuedPrefetchFullPolicy) -> Self {
        self.full_policy = full_policy;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueuedPrefetcherError {
    ZeroCapacity,
    ZeroIssueWidth,
    ZeroLineSize,
    ZeroPageSize,
    QueueFull {
        capacity: usize,
    },
    ReadyTickOverflow {
        source_tick: u64,
        latency: u64,
    },
    SnapshotConfigMismatch {
        expected: QueuedPrefetchConfig,
        actual: QueuedPrefetchConfig,
    },
    SnapshotQueueTooLarge {
        pending: usize,
        capacity: usize,
    },
}

impl fmt::Display for QueuedPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCapacity => write!(formatter, "queued prefetch capacity is zero"),
            Self::ZeroIssueWidth => write!(formatter, "queued prefetch issue width is zero"),
            Self::ZeroLineSize => write!(formatter, "queued prefetch line size is zero"),
            Self::ZeroPageSize => write!(formatter, "queued prefetch page size is zero"),
            Self::QueueFull { capacity } => write!(
                formatter,
                "queued prefetch resource is full at capacity {capacity}"
            ),
            Self::ReadyTickOverflow {
                source_tick,
                latency,
            } => write!(
                formatter,
                "queued prefetch ready tick overflows for source tick {source_tick} and latency {latency}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "queued prefetch snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotQueueTooLarge { pending, capacity } => write!(
                formatter,
                "queued prefetch snapshot has {pending} entries for capacity {capacity}"
            ),
        }
    }
}

impl Error for QueuedPrefetcherError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueuedPrefetchFullPolicy {
    RejectNew,
    EvictOldestLowestPriority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueuedPrefetchResidency {
    Cache,
    MissQueue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueuedPrefetchRedundantLine {
    address: Address,
    secure: bool,
    residency: QueuedPrefetchResidency,
}

impl QueuedPrefetchRedundantLine {
    pub const fn in_cache(address: Address, secure: bool) -> Self {
        Self {
            address,
            secure,
            residency: QueuedPrefetchResidency::Cache,
        }
    }

    pub const fn in_miss_queue(address: Address, secure: bool) -> Self {
        Self {
            address,
            secure,
            residency: QueuedPrefetchResidency::MissQueue,
        }
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn residency(&self) -> QueuedPrefetchResidency {
        self.residency
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueuedPrefetchEnqueueResult {
    accepted: usize,
    duplicate_hits: usize,
    updated_priorities: usize,
    dropped_redundant: usize,
    dropped_page_crossing: usize,
    dropped_throttled: usize,
    evicted_full: usize,
}

impl QueuedPrefetchEnqueueResult {
    pub const fn accepted(&self) -> usize {
        self.accepted
    }

    pub const fn duplicate_hits(&self) -> usize {
        self.duplicate_hits
    }

    pub const fn updated_priorities(&self) -> usize {
        self.updated_priorities
    }

    pub const fn dropped_redundant(&self) -> usize {
        self.dropped_redundant
    }

    pub const fn dropped_page_crossing(&self) -> usize {
        self.dropped_page_crossing
    }

    pub const fn dropped_throttled(&self) -> usize {
        self.dropped_throttled
    }

    pub const fn evicted_full(&self) -> usize {
        self.evicted_full
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueuedPrefetchDemandAccess {
    address: Address,
    secure: bool,
}

impl QueuedPrefetchDemandAccess {
    pub const fn new(address: Address, secure: bool) -> Self {
        Self { address, secure }
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedPrefetchEntrySnapshot {
    address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    stride: i64,
    degree_index: u32,
    priority: i32,
    source_tick: u64,
    ready_tick: u64,
    order: u64,
}

impl QueuedPrefetchEntrySnapshot {
    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn context(&self) -> AgentId {
        self.context
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn stride(&self) -> i64 {
        self.stride
    }

    pub const fn degree_index(&self) -> u32 {
        self.degree_index
    }

    pub const fn priority(&self) -> i32 {
        self.priority
    }

    pub const fn source_tick(&self) -> u64 {
        self.source_tick
    }

    pub const fn ready_tick(&self) -> u64 {
        self.ready_tick
    }

    pub const fn order(&self) -> u64 {
        self.order
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedPrefetcherSnapshot {
    config: QueuedPrefetchConfig,
    pending: Vec<QueuedPrefetchEntrySnapshot>,
    next_order: u64,
}

impl QueuedPrefetcherSnapshot {
    pub const fn config(&self) -> &QueuedPrefetchConfig {
        &self.config
    }

    pub fn pending(&self) -> &[QueuedPrefetchEntrySnapshot] {
        &self.pending
    }

    pub const fn next_order(&self) -> u64 {
        self.next_order
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedPrefetchIssue {
    address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    stride: i64,
    degree_index: u32,
    priority: i32,
    source_tick: u64,
    ready_tick: u64,
    order: u64,
}

impl QueuedPrefetchIssue {
    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn context(&self) -> AgentId {
        self.context
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn stride(&self) -> i64 {
        self.stride
    }

    pub const fn degree_index(&self) -> u32 {
        self.degree_index
    }

    pub const fn source_tick(&self) -> u64 {
        self.source_tick
    }

    pub const fn ready_tick(&self) -> u64 {
        self.ready_tick
    }

    pub const fn order(&self) -> u64 {
        self.order
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QueuedPrefetchEntry {
    address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    stride: i64,
    degree_index: u32,
    priority: i32,
    source_tick: u64,
    ready_tick: u64,
    order: u64,
}

impl QueuedPrefetchEntry {
    fn from_candidate<C: PrefetchCandidate>(
        candidate: &C,
        address: Address,
        source_tick: u64,
        ready_tick: u64,
        order: u64,
    ) -> Self {
        Self {
            address,
            context: candidate.context(),
            pc: candidate.pc(),
            secure: candidate.secure(),
            stride: candidate.stride(),
            degree_index: candidate.degree_index(),
            priority: queued_prefetch_priority(candidate),
            source_tick,
            ready_tick,
            order,
        }
    }

    fn from_snapshot(snapshot: &QueuedPrefetchEntrySnapshot) -> Self {
        Self {
            address: snapshot.address,
            context: snapshot.context,
            pc: snapshot.pc,
            secure: snapshot.secure,
            stride: snapshot.stride,
            degree_index: snapshot.degree_index,
            priority: snapshot.priority,
            source_tick: snapshot.source_tick,
            ready_tick: snapshot.ready_tick,
            order: snapshot.order,
        }
    }

    fn snapshot(&self) -> QueuedPrefetchEntrySnapshot {
        QueuedPrefetchEntrySnapshot {
            address: self.address,
            context: self.context,
            pc: self.pc,
            secure: self.secure,
            stride: self.stride,
            degree_index: self.degree_index,
            priority: self.priority,
            source_tick: self.source_tick,
            ready_tick: self.ready_tick,
            order: self.order,
        }
    }

    fn issue(&self) -> QueuedPrefetchIssue {
        QueuedPrefetchIssue {
            address: self.address,
            context: self.context,
            pc: self.pc,
            secure: self.secure,
            stride: self.stride,
            degree_index: self.degree_index,
            priority: self.priority,
            source_tick: self.source_tick,
            ready_tick: self.ready_tick,
            order: self.order,
        }
    }

    fn same_request<C: PrefetchCandidate>(&self, address: Address, candidate: &C) -> bool {
        self.address == address
            && self.context == candidate.context()
            && self.secure == candidate.secure()
    }

    fn update_priority<C: PrefetchCandidate>(&mut self, candidate: &C) -> bool {
        let priority = queued_prefetch_priority(candidate);
        if priority <= self.priority {
            return false;
        }

        self.priority = priority;
        true
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedPrefetcher {
    config: QueuedPrefetchConfig,
    pending: Vec<QueuedPrefetchEntry>,
    next_order: u64,
}

impl QueuedPrefetcher {
    pub fn new(config: QueuedPrefetchConfig) -> Self {
        Self {
            config,
            pending: Vec::new(),
            next_order: 0,
        }
    }

    pub const fn config(&self) -> &QueuedPrefetchConfig {
        &self.config
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn next_ready_tick(&self) -> Option<u64> {
        self.pending.first().map(|entry| entry.ready_tick)
    }

    pub fn enqueue_candidates<C: PrefetchCandidate>(
        &mut self,
        source_tick: u64,
        candidates: &[C],
    ) -> Result<usize, QueuedPrefetcherError> {
        Ok(self
            .enqueue_candidates_filtered(source_tick, candidates, &[])?
            .accepted())
    }

    pub fn enqueue_candidates_filtered<C: PrefetchCandidate>(
        &mut self,
        source_tick: u64,
        candidates: &[C],
        redundant_lines: &[QueuedPrefetchRedundantLine],
    ) -> Result<QueuedPrefetchEnqueueResult, QueuedPrefetcherError> {
        self.enqueue_candidates_filtered_limited(
            source_tick,
            candidates,
            redundant_lines,
            candidates.len(),
        )
    }

    pub fn enqueue_candidates_throttled<C: PrefetchCandidate>(
        &mut self,
        source_tick: u64,
        candidates: &[C],
        redundant_lines: &[QueuedPrefetchRedundantLine],
        throttle: &QueuedPrefetchThrottle,
    ) -> Result<QueuedPrefetchEnqueueResult, QueuedPrefetcherError> {
        self.enqueue_candidates_filtered_limited(
            source_tick,
            candidates,
            redundant_lines,
            throttle.max_permitted(candidates.len()),
        )
    }

    fn enqueue_candidates_filtered_limited<C: PrefetchCandidate>(
        &mut self,
        source_tick: u64,
        candidates: &[C],
        redundant_lines: &[QueuedPrefetchRedundantLine],
        accepted_limit: usize,
    ) -> Result<QueuedPrefetchEnqueueResult, QueuedPrefetcherError> {
        let ready_tick = source_tick.checked_add(self.config.latency()).ok_or(
            QueuedPrefetcherError::ReadyTickOverflow {
                source_tick,
                latency: self.config.latency(),
            },
        )?;
        let accepted_limit = accepted_limit.min(candidates.len());
        let mut accepted = 0;
        let mut duplicate_hits = 0;
        let mut updated_priorities = 0;
        let mut dropped_redundant = 0;
        let mut dropped_page_crossing = 0;
        let mut dropped_throttled = 0;
        let mut evicted_full = 0;
        for (index, candidate) in candidates.iter().enumerate() {
            if accepted == accepted_limit {
                dropped_throttled = candidates.len() - index;
                break;
            }

            let address = self.normalized_address(candidate.address());
            if self.crosses_page(address, candidate.source_address()) {
                dropped_page_crossing += 1;
                continue;
            }
            if self.is_redundant(address, candidate.secure(), redundant_lines) {
                dropped_redundant += 1;
                continue;
            }
            if self.config.filter_duplicates() {
                if let Some(index) = self
                    .pending
                    .iter()
                    .position(|entry| entry.same_request(address, candidate))
                {
                    duplicate_hits += 1;
                    if self.pending[index].update_priority(candidate) {
                        updated_priorities += 1;
                        sort_pending_entries(&mut self.pending);
                    }
                    continue;
                }
            }
            if self.pending.len() == self.config.capacity() {
                match self.config.full_policy() {
                    QueuedPrefetchFullPolicy::RejectNew => {
                        return Err(QueuedPrefetcherError::QueueFull {
                            capacity: self.config.capacity(),
                        });
                    }
                    QueuedPrefetchFullPolicy::EvictOldestLowestPriority => {
                        let victim = oldest_lowest_priority_index(&self.pending);
                        self.pending.remove(victim);
                        evicted_full += 1;
                    }
                }
            }

            let entry = QueuedPrefetchEntry::from_candidate(
                candidate,
                address,
                source_tick,
                ready_tick,
                self.next_order,
            );
            self.next_order = self.next_order.saturating_add(1);
            insert_pending_entry(&mut self.pending, entry);
            accepted += 1;
        }
        Ok(QueuedPrefetchEnqueueResult {
            accepted,
            duplicate_hits,
            updated_priorities,
            dropped_redundant,
            dropped_page_crossing,
            dropped_throttled,
            evicted_full,
        })
    }

    pub fn squash_demand_access(&mut self, access: QueuedPrefetchDemandAccess) -> usize {
        let line_address = self.normalized_address(access.address());
        let original_len = self.pending.len();
        self.pending
            .retain(|entry| !(entry.address == line_address && entry.secure == access.secure()));
        original_len - self.pending.len()
    }

    pub fn issue_ready(&mut self, tick: u64) -> Vec<QueuedPrefetchIssue> {
        let mut issues = Vec::new();
        while issues.len() < self.config.max_issue_per_tick() {
            let Some(issue) = self.issue_one_ready(tick) else {
                break;
            };
            issues.push(issue);
        }
        issues
    }

    pub(crate) fn issue_one_ready(&mut self, tick: u64) -> Option<QueuedPrefetchIssue> {
        if self.pending.first()?.ready_tick > tick {
            return None;
        }
        Some(self.pending.remove(0).issue())
    }

    pub fn snapshot(&self) -> QueuedPrefetcherSnapshot {
        QueuedPrefetcherSnapshot {
            config: self.config.clone(),
            pending: self
                .pending
                .iter()
                .map(QueuedPrefetchEntry::snapshot)
                .collect(),
            next_order: self.next_order,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &QueuedPrefetcherSnapshot,
    ) -> Result<(), QueuedPrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(QueuedPrefetcherError::SnapshotConfigMismatch {
                expected: self.config.clone(),
                actual: snapshot.config().clone(),
            });
        }
        if snapshot.pending().len() > self.config.capacity() {
            return Err(QueuedPrefetcherError::SnapshotQueueTooLarge {
                pending: snapshot.pending().len(),
                capacity: self.config.capacity(),
            });
        }
        let mut pending: Vec<_> = snapshot
            .pending()
            .iter()
            .map(QueuedPrefetchEntry::from_snapshot)
            .collect();
        sort_pending_entries(&mut pending);
        self.pending = pending;
        self.next_order = snapshot.next_order();
        Ok(())
    }

    fn normalized_address(&self, address: Address) -> Address {
        Address::new(address.get() / self.config.line_size() * self.config.line_size())
    }

    fn crosses_page(&self, target: Address, source: Address) -> bool {
        let Some(page_size) = self.config.page_size() else {
            return false;
        };
        page_address(target, page_size) != page_address(source, page_size)
    }

    fn is_redundant(
        &self,
        address: Address,
        secure: bool,
        redundant_lines: &[QueuedPrefetchRedundantLine],
    ) -> bool {
        redundant_lines.iter().any(|line| {
            self.normalized_address(line.address()) == address && line.secure() == secure
        })
    }
}

fn insert_pending_entry(pending: &mut Vec<QueuedPrefetchEntry>, entry: QueuedPrefetchEntry) {
    let index = pending
        .iter()
        .position(|existing| pending_entry_precedes(&entry, existing))
        .unwrap_or(pending.len());
    pending.insert(index, entry);
}

fn sort_pending_entries(pending: &mut [QueuedPrefetchEntry]) {
    pending.sort_by(|left, right| {
        if pending_entry_precedes(left, right) {
            std::cmp::Ordering::Less
        } else if pending_entry_precedes(right, left) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });
}

fn pending_entry_precedes(left: &QueuedPrefetchEntry, right: &QueuedPrefetchEntry) -> bool {
    if left.ready_tick != right.ready_tick {
        return left.ready_tick < right.ready_tick;
    }
    if left.priority != right.priority {
        return left.priority > right.priority;
    }
    left.order < right.order
}

fn oldest_lowest_priority_index(pending: &[QueuedPrefetchEntry]) -> usize {
    pending
        .iter()
        .enumerate()
        .min_by_key(|(_, entry)| (entry.priority, entry.order))
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn queued_prefetch_priority<C: PrefetchCandidate>(candidate: &C) -> i32 {
    let degree = candidate.degree_index().min(i32::MAX as u32) as i32;
    i32::MAX.saturating_sub(degree)
}

fn normalized_address(address: Address, line_size: u64) -> Address {
    Address::new(address.get() / line_size * line_size)
}

fn page_address(address: Address, page_size: u64) -> u64 {
    address.get() / page_size
}

pub trait PrefetchCandidate {
    fn address(&self) -> Address;
    fn source_address(&self) -> Address;
    fn context(&self) -> AgentId;
    fn pc(&self) -> u64;
    fn secure(&self) -> bool;
    fn stride(&self) -> i64;
    fn degree_index(&self) -> u32;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaggedPrefetcherConfig {
    line_size: u64,
    degree: u32,
}

impl TaggedPrefetcherConfig {
    pub fn new(line_size: u64, degree: u32) -> Result<Self, TaggedPrefetcherError> {
        if line_size == 0 {
            return Err(TaggedPrefetcherError::ZeroLineSize);
        }
        if degree == 0 {
            return Err(TaggedPrefetcherError::ZeroDegree);
        }

        Ok(Self { line_size, degree })
    }

    pub const fn line_size(&self) -> u64 {
        self.line_size
    }

    pub const fn degree(&self) -> u32 {
        self.degree
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaggedPrefetcherError {
    ZeroLineSize,
    ZeroDegree,
    SnapshotConfigMismatch {
        expected: TaggedPrefetcherConfig,
        actual: TaggedPrefetcherConfig,
    },
}

impl fmt::Display for TaggedPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLineSize => write!(formatter, "tagged prefetcher line size is zero"),
            Self::ZeroDegree => write!(formatter, "tagged prefetcher degree is zero"),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "tagged prefetcher snapshot config {actual:?} does not match {expected:?}"
            ),
        }
    }
}

impl Error for TaggedPrefetcherError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaggedPrefetchAccess {
    requestor: AgentId,
    pc: u64,
    address: Address,
    secure: bool,
}

impl TaggedPrefetchAccess {
    pub const fn new(requestor: AgentId, pc: u64, address: Address, secure: bool) -> Self {
        Self {
            requestor,
            pc,
            address,
            secure,
        }
    }

    pub const fn requestor(&self) -> AgentId {
        self.requestor
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaggedPrefetchCandidate {
    address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    stride: i64,
    degree_index: u32,
}

impl TaggedPrefetchCandidate {
    fn new(
        address: Address,
        source_address: Address,
        context: AgentId,
        pc: u64,
        secure: bool,
        stride: i64,
        degree_index: u32,
    ) -> Self {
        Self {
            address,
            source_address,
            context,
            pc,
            secure,
            stride,
            degree_index,
        }
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn source_address(&self) -> Address {
        self.source_address
    }

    pub const fn context(&self) -> AgentId {
        self.context
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn stride(&self) -> i64 {
        self.stride
    }

    pub const fn degree_index(&self) -> u32 {
        self.degree_index
    }
}

impl PrefetchCandidate for TaggedPrefetchCandidate {
    fn address(&self) -> Address {
        self.address()
    }

    fn source_address(&self) -> Address {
        self.source_address()
    }

    fn context(&self) -> AgentId {
        self.context()
    }

    fn pc(&self) -> u64 {
        self.pc()
    }

    fn secure(&self) -> bool {
        self.secure()
    }

    fn stride(&self) -> i64 {
        self.stride()
    }

    fn degree_index(&self) -> u32 {
        self.degree_index()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaggedPrefetcherSnapshot {
    config: TaggedPrefetcherConfig,
    last_candidates: Vec<TaggedPrefetchCandidate>,
}

impl TaggedPrefetcherSnapshot {
    pub const fn config(&self) -> &TaggedPrefetcherConfig {
        &self.config
    }

    pub fn last_candidates(&self) -> &[TaggedPrefetchCandidate] {
        &self.last_candidates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaggedPrefetcher {
    config: TaggedPrefetcherConfig,
    last_candidates: Vec<TaggedPrefetchCandidate>,
}

impl TaggedPrefetcher {
    pub fn new(config: TaggedPrefetcherConfig) -> Self {
        Self {
            config,
            last_candidates: Vec::new(),
        }
    }

    pub const fn config(&self) -> &TaggedPrefetcherConfig {
        &self.config
    }

    pub fn last_candidates(&self) -> &[TaggedPrefetchCandidate] {
        &self.last_candidates
    }

    pub fn observe(
        &mut self,
        access: TaggedPrefetchAccess,
    ) -> Result<&[TaggedPrefetchCandidate], TaggedPrefetcherError> {
        self.last_candidates.clear();
        let line_address = normalized_address(access.address(), self.config.line_size());
        let stride = self.config.line_size().min(i64::MAX as u64) as i64;
        for degree_index in 1..=self.config.degree() {
            let offset = u64::from(degree_index).saturating_mul(self.config.line_size());
            if let Some(address) = line_address.get().checked_add(offset) {
                self.last_candidates.push(TaggedPrefetchCandidate::new(
                    Address::new(address),
                    access.address(),
                    access.requestor(),
                    access.pc(),
                    access.secure(),
                    stride,
                    degree_index,
                ));
            }
        }
        Ok(&self.last_candidates)
    }

    pub fn snapshot(&self) -> TaggedPrefetcherSnapshot {
        TaggedPrefetcherSnapshot {
            config: self.config.clone(),
            last_candidates: self.last_candidates.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &TaggedPrefetcherSnapshot,
    ) -> Result<(), TaggedPrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(TaggedPrefetcherError::SnapshotConfigMismatch {
                expected: self.config.clone(),
                actual: snapshot.config().clone(),
            });
        }
        self.last_candidates = snapshot.last_candidates().to_vec();
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StridePrefetcherConfig {
    line_size: u64,
    table_entries: usize,
    confidence_threshold: u8,
    degree: u32,
    distance: u32,
    use_requestor_id: bool,
}

impl StridePrefetcherConfig {
    pub fn new(
        line_size: u64,
        table_entries: usize,
        confidence_threshold: u8,
        degree: u32,
        distance: u32,
        use_requestor_id: bool,
    ) -> Result<Self, StridePrefetcherError> {
        if line_size == 0 {
            return Err(StridePrefetcherError::ZeroLineSize);
        }
        if table_entries == 0 {
            return Err(StridePrefetcherError::ZeroTableEntries);
        }
        if confidence_threshold > MAX_CONFIDENCE {
            return Err(StridePrefetcherError::ConfidenceThresholdOutOfRange {
                threshold: confidence_threshold,
                max: MAX_CONFIDENCE,
            });
        }
        if degree == 0 {
            return Err(StridePrefetcherError::ZeroDegree);
        }

        Ok(Self {
            line_size,
            table_entries,
            confidence_threshold,
            degree,
            distance,
            use_requestor_id,
        })
    }

    pub const fn line_size(&self) -> u64 {
        self.line_size
    }

    pub const fn table_entries(&self) -> usize {
        self.table_entries
    }

    pub const fn confidence_threshold(&self) -> u8 {
        self.confidence_threshold
    }

    pub const fn degree(&self) -> u32 {
        self.degree
    }

    pub const fn distance(&self) -> u32 {
        self.distance
    }

    pub const fn use_requestor_id(&self) -> bool {
        self.use_requestor_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StridePrefetcherError {
    ZeroLineSize,
    ZeroTableEntries,
    ZeroDegree,
    ConfidenceThresholdOutOfRange {
        threshold: u8,
        max: u8,
    },
    SnapshotConfigMismatch {
        expected: StridePrefetcherConfig,
        actual: StridePrefetcherConfig,
    },
    SnapshotContextOutOfRange {
        context: AgentId,
        entries: usize,
        table_entries: usize,
    },
}

impl fmt::Display for StridePrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLineSize => write!(formatter, "stride prefetcher line size is zero"),
            Self::ZeroTableEntries => write!(formatter, "stride prefetcher table has no entries"),
            Self::ZeroDegree => write!(formatter, "stride prefetcher degree is zero"),
            Self::ConfidenceThresholdOutOfRange { threshold, max } => write!(
                formatter,
                "stride prefetcher confidence threshold {threshold} exceeds {max}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "stride prefetcher snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotContextOutOfRange {
                context,
                entries,
                table_entries,
            } => write!(
                formatter,
                "stride prefetcher snapshot context {} has {entries} entries for {table_entries} slots",
                context.get()
            ),
        }
    }
}

impl Error for StridePrefetcherError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StridePrefetchAccess {
    requestor: AgentId,
    pc: u64,
    address: Address,
    secure: bool,
}

impl StridePrefetchAccess {
    pub const fn new(requestor: AgentId, pc: u64, address: Address, secure: bool) -> Self {
        Self {
            requestor,
            pc,
            address,
            secure,
        }
    }

    pub const fn requestor(&self) -> AgentId {
        self.requestor
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StridePrefetchCandidate {
    address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    stride: i64,
    degree_index: u32,
}

impl StridePrefetchCandidate {
    fn new(
        address: Address,
        source_address: Address,
        context: AgentId,
        pc: u64,
        secure: bool,
        stride: i64,
        degree_index: u32,
    ) -> Self {
        Self {
            address,
            source_address,
            context,
            pc,
            secure,
            stride,
            degree_index,
        }
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn source_address(&self) -> Address {
        self.source_address
    }

    pub const fn context(&self) -> AgentId {
        self.context
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn stride(&self) -> i64 {
        self.stride
    }

    pub const fn degree_index(&self) -> u32 {
        self.degree_index
    }
}

impl PrefetchCandidate for StridePrefetchCandidate {
    fn address(&self) -> Address {
        self.address()
    }

    fn source_address(&self) -> Address {
        self.source_address()
    }

    fn context(&self) -> AgentId {
        self.context()
    }

    fn pc(&self) -> u64 {
        self.pc()
    }

    fn secure(&self) -> bool {
        self.secure()
    }

    fn stride(&self) -> i64 {
        self.stride()
    }

    fn degree_index(&self) -> u32 {
        self.degree_index()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StridePrefetchEntrySnapshot {
    pc: u64,
    secure: bool,
    last_address: Address,
    stride: i64,
    confidence: u8,
}

impl StridePrefetchEntrySnapshot {
    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn last_address(&self) -> Address {
        self.last_address
    }

    pub const fn stride(&self) -> i64 {
        self.stride
    }

    pub const fn confidence(&self) -> u8 {
        self.confidence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StridePrefetchContextSnapshot {
    context: AgentId,
    entries: Vec<StridePrefetchEntrySnapshot>,
    next_victim: usize,
}

impl StridePrefetchContextSnapshot {
    pub const fn context(&self) -> AgentId {
        self.context
    }

    pub fn entries(&self) -> &[StridePrefetchEntrySnapshot] {
        &self.entries
    }

    pub const fn next_victim(&self) -> usize {
        self.next_victim
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StridePrefetcherSnapshot {
    config: StridePrefetcherConfig,
    contexts: Vec<StridePrefetchContextSnapshot>,
    last_candidates: Vec<StridePrefetchCandidate>,
}

impl StridePrefetcherSnapshot {
    pub const fn config(&self) -> &StridePrefetcherConfig {
        &self.config
    }

    pub fn contexts(&self) -> &[StridePrefetchContextSnapshot] {
        &self.contexts
    }

    pub fn last_candidates(&self) -> &[StridePrefetchCandidate] {
        &self.last_candidates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StridePrefetchEntry {
    pc: u64,
    secure: bool,
    last_address: Address,
    stride: i64,
    confidence: u8,
}

impl StridePrefetchEntry {
    fn new(pc: u64, secure: bool, last_address: Address) -> Self {
        Self {
            pc,
            secure,
            last_address,
            stride: 0,
            confidence: 0,
        }
    }

    fn snapshot(&self) -> StridePrefetchEntrySnapshot {
        StridePrefetchEntrySnapshot {
            pc: self.pc,
            secure: self.secure,
            last_address: self.last_address,
            stride: self.stride,
            confidence: self.confidence,
        }
    }

    fn from_snapshot(snapshot: &StridePrefetchEntrySnapshot) -> Self {
        Self {
            pc: snapshot.pc,
            secure: snapshot.secure,
            last_address: snapshot.last_address,
            stride: snapshot.stride,
            confidence: snapshot.confidence.min(MAX_CONFIDENCE),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StridePrefetchContext {
    entries: Vec<StridePrefetchEntry>,
    next_victim: usize,
}

impl StridePrefetchContext {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_victim: 0,
        }
    }

    fn snapshot(&self, context: AgentId) -> StridePrefetchContextSnapshot {
        StridePrefetchContextSnapshot {
            context,
            entries: self
                .entries
                .iter()
                .map(StridePrefetchEntry::snapshot)
                .collect(),
            next_victim: self.next_victim,
        }
    }

    fn from_snapshot(snapshot: &StridePrefetchContextSnapshot) -> Self {
        Self {
            entries: snapshot
                .entries
                .iter()
                .map(StridePrefetchEntry::from_snapshot)
                .collect(),
            next_victim: snapshot.next_victim,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StridePrefetcher {
    config: StridePrefetcherConfig,
    contexts: BTreeMap<AgentId, StridePrefetchContext>,
    last_candidates: Vec<StridePrefetchCandidate>,
}

impl StridePrefetcher {
    pub fn new(config: StridePrefetcherConfig) -> Self {
        Self {
            config,
            contexts: BTreeMap::new(),
            last_candidates: Vec::new(),
        }
    }

    pub const fn config(&self) -> &StridePrefetcherConfig {
        &self.config
    }

    pub fn context_count(&self) -> usize {
        self.contexts.len()
    }

    pub fn entry_count(&self, requestor: AgentId) -> usize {
        let context = self.context_key(requestor);
        self.contexts
            .get(&context)
            .map_or(0, |context| context.entries.len())
    }

    pub fn last_candidates(&self) -> &[StridePrefetchCandidate] {
        &self.last_candidates
    }

    pub fn observe(
        &mut self,
        access: StridePrefetchAccess,
    ) -> Result<&[StridePrefetchCandidate], StridePrefetcherError> {
        self.last_candidates.clear();
        let context_key = self.context_key(access.requestor());
        let line_address = self.normalized_address(access.address());
        let config = self.config.clone();
        let context = self
            .contexts
            .entry(context_key)
            .or_insert_with(StridePrefetchContext::new);

        let Some(index) = context
            .entries
            .iter()
            .position(|entry| entry.pc == access.pc() && entry.secure == access.secure())
        else {
            allocate_entry(&config, context, access.pc(), access.secure(), line_address);
            return Ok(&self.last_candidates);
        };

        let entry = &mut context.entries[index];
        let new_stride = line_address.get() as i128 - entry.last_address.get() as i128;
        if new_stride == 0 {
            return Ok(&self.last_candidates);
        }
        let new_stride = new_stride.clamp(i64::MIN as i128, i64::MAX as i128) as i64;
        if new_stride == entry.stride {
            entry.confidence = entry.confidence.saturating_add(1).min(MAX_CONFIDENCE);
        } else {
            entry.confidence = entry.confidence.saturating_sub(1);
            if entry.confidence < config.confidence_threshold() {
                entry.stride = new_stride;
                entry.confidence = 1.min(MAX_CONFIDENCE);
            }
        }
        entry.last_address = line_address;

        if entry.confidence < config.confidence_threshold() {
            return Ok(&self.last_candidates);
        }

        let prefetch_stride = rounded_stride(entry.stride, config.line_size());
        let start = (config.distance() as i128) * (prefetch_stride as i128);
        for degree_index in 1..=config.degree() {
            let offset = start + (degree_index as i128) * (prefetch_stride as i128);
            if let Some(address) = offset_address(line_address, offset) {
                self.last_candidates.push(StridePrefetchCandidate::new(
                    address,
                    access.address(),
                    context_key,
                    access.pc(),
                    access.secure(),
                    prefetch_stride,
                    degree_index,
                ));
            }
        }

        Ok(&self.last_candidates)
    }

    pub fn snapshot(&self) -> StridePrefetcherSnapshot {
        StridePrefetcherSnapshot {
            config: self.config.clone(),
            contexts: self
                .contexts
                .iter()
                .map(|(context, table)| table.snapshot(*context))
                .collect(),
            last_candidates: self.last_candidates.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &StridePrefetcherSnapshot,
    ) -> Result<(), StridePrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(StridePrefetcherError::SnapshotConfigMismatch {
                expected: self.config.clone(),
                actual: snapshot.config().clone(),
            });
        }
        let mut contexts = BTreeMap::new();
        for context in snapshot.contexts() {
            if context.entries().len() > self.config.table_entries() {
                return Err(StridePrefetcherError::SnapshotContextOutOfRange {
                    context: context.context(),
                    entries: context.entries().len(),
                    table_entries: self.config.table_entries(),
                });
            }
            contexts.insert(
                context.context(),
                StridePrefetchContext::from_snapshot(context),
            );
        }

        self.contexts = contexts;
        self.last_candidates = snapshot.last_candidates.clone();
        Ok(())
    }

    fn context_key(&self, requestor: AgentId) -> AgentId {
        if self.config.use_requestor_id() {
            requestor
        } else {
            AgentId::new(0)
        }
    }

    fn normalized_address(&self, address: Address) -> Address {
        Address::new(address.get() / self.config.line_size() * self.config.line_size())
    }
}

fn allocate_entry(
    config: &StridePrefetcherConfig,
    context: &mut StridePrefetchContext,
    pc: u64,
    secure: bool,
    line_address: Address,
) {
    let entry = StridePrefetchEntry::new(pc, secure, line_address);
    if context.entries.len() < config.table_entries() {
        context.entries.push(entry);
        return;
    }

    let victim = context.next_victim % context.entries.len();
    context.entries[victim] = entry;
    context.next_victim = (victim + 1) % config.table_entries();
}

fn rounded_stride(stride: i64, line_size: u64) -> i64 {
    let line_size = line_size.min(i64::MAX as u64) as i64;
    if stride.unsigned_abs() < line_size as u64 {
        if stride.is_negative() {
            -line_size
        } else {
            line_size
        }
    } else {
        stride
    }
}

fn offset_address(base: Address, offset: i128) -> Option<Address> {
    let value = base.get() as i128 + offset;
    if (0..=u64::MAX as i128).contains(&value) {
        Some(Address::new(value as u64))
    } else {
        None
    }
}

use std::error::Error;
use std::fmt;

use crate::allocation::max_vector_len;
use crate::prefetch::PrefetchCandidate;
use crate::prefetch_stats::QueuedPrefetchStatsSnapshot;
use crate::prefetch_throttle::{
    QueuedPrefetchThrottle, QueuedPrefetchThrottleConfig, QueuedPrefetchThrottleError,
    QueuedPrefetchThrottleSnapshot,
};
use rem6_memory::{
    AccessSize, Address, AgentId, TranslationAccessKind, TranslationRequest, TranslationRequestId,
    TranslationResolution,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedPrefetchConfig {
    capacity: usize,
    latency: u64,
    max_issue_per_tick: usize,
    filter_duplicates: bool,
    line_size: u64,
    page_size: u64,
    missing_translation_capacity: usize,
    full_policy: QueuedPrefetchFullPolicy,
    throttle_config: Option<QueuedPrefetchThrottleConfig>,
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
        validate_queued_prefetch_vector_length(
            "capacity",
            capacity,
            maximum_queued_prefetch_entries(),
        )?;
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
            page_size: 0,
            missing_translation_capacity: 0,
            full_policy: QueuedPrefetchFullPolicy::EvictOldestLowestPriority,
            throttle_config: None,
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
        if self.page_size == 0 {
            None
        } else {
            Some(self.page_size)
        }
    }

    pub const fn missing_translation_capacity(&self) -> Option<usize> {
        if self.missing_translation_capacity == 0 {
            None
        } else {
            Some(self.missing_translation_capacity)
        }
    }

    pub const fn full_policy(&self) -> QueuedPrefetchFullPolicy {
        self.full_policy
    }

    pub const fn throttle_config(&self) -> Option<&QueuedPrefetchThrottleConfig> {
        self.throttle_config.as_ref()
    }

    pub fn with_page_size(mut self, page_size: u64) -> Result<Self, QueuedPrefetcherError> {
        if page_size == 0 {
            return Err(QueuedPrefetcherError::ZeroPageSize);
        }
        self.page_size = page_size;
        Ok(self)
    }

    pub fn with_missing_translation_capacity(
        mut self,
        capacity: usize,
    ) -> Result<Self, QueuedPrefetcherError> {
        if capacity == 0 {
            return Err(QueuedPrefetcherError::ZeroMissingTranslationCapacity);
        }
        validate_queued_prefetch_vector_length(
            "missing translation capacity",
            capacity,
            maximum_queued_prefetch_missing_translation_entries(),
        )?;
        self.missing_translation_capacity = capacity;
        Ok(self)
    }

    pub const fn with_full_policy(mut self, full_policy: QueuedPrefetchFullPolicy) -> Self {
        self.full_policy = full_policy;
        self
    }

    pub fn with_throttle_config(mut self, throttle_config: QueuedPrefetchThrottleConfig) -> Self {
        self.throttle_config = Some(throttle_config);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueuedPrefetcherError {
    ZeroCapacity,
    ZeroIssueWidth,
    ZeroLineSize,
    ZeroPageSize,
    ZeroMissingTranslationCapacity,
    QueueFull {
        capacity: usize,
    },
    ReadyTickOverflow {
        source_tick: u64,
        latency: u64,
    },
    VectorLengthTooLarge {
        field: &'static str,
        length: usize,
        maximum: usize,
    },
    SnapshotConfigMismatch {
        expected: QueuedPrefetchConfig,
        actual: QueuedPrefetchConfig,
    },
    SnapshotQueueTooLarge {
        pending: usize,
        capacity: usize,
    },
    SnapshotMissingTranslationQueueTooLarge {
        pending: usize,
        capacity: usize,
    },
    UnknownTranslation {
        request: TranslationRequestId,
    },
    TranslationNotStarted {
        request: TranslationRequestId,
    },
    TranslationRequestAddressOverflow {
        address: Address,
        size: u64,
    },
    ThrottleUsefulCounter {
        source: QueuedPrefetchThrottleError,
    },
    SnapshotThrottleStateMismatch {
        expected_enabled: bool,
        actual_enabled: bool,
    },
    SnapshotThrottleRestore {
        source: QueuedPrefetchThrottleError,
    },
}

impl fmt::Display for QueuedPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroCapacity => write!(formatter, "queued prefetch capacity is zero"),
            Self::ZeroIssueWidth => write!(formatter, "queued prefetch issue width is zero"),
            Self::ZeroLineSize => write!(formatter, "queued prefetch line size is zero"),
            Self::ZeroPageSize => write!(formatter, "queued prefetch page size is zero"),
            Self::ZeroMissingTranslationCapacity => {
                write!(formatter, "queued prefetch missing translation capacity is zero")
            }
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
            Self::VectorLengthTooLarge {
                field,
                length,
                maximum,
            } => write!(
                formatter,
                "queued prefetch {field} length {length} exceeds maximum {maximum}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "queued prefetch snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotQueueTooLarge { pending, capacity } => write!(
                formatter,
                "queued prefetch snapshot has {pending} entries for capacity {capacity}"
            ),
            Self::SnapshotMissingTranslationQueueTooLarge { pending, capacity } => write!(
                formatter,
                "queued prefetch snapshot has {pending} missing translations for capacity {capacity}"
            ),
            Self::UnknownTranslation { request } => write!(
                formatter,
                "queued prefetch translation request {:?} is unknown",
                request
            ),
            Self::TranslationNotStarted { request } => write!(
                formatter,
                "queued prefetch translation request {:?} has not started",
                request
            ),
            Self::TranslationRequestAddressOverflow { address, size } => write!(
                formatter,
                "queued prefetch translation request at {:?} with size {size} overflows",
                address
            ),
            Self::ThrottleUsefulCounter { source } => {
                write!(formatter, "queued prefetch throttle useful counter failed: {source}")
            }
            Self::SnapshotThrottleStateMismatch {
                expected_enabled,
                actual_enabled,
            } => write!(
                formatter,
                "queued prefetch snapshot throttle enabled state {actual_enabled} does not match expected {expected_enabled}"
            ),
            Self::SnapshotThrottleRestore { source } => write!(
                formatter,
                "queued prefetch throttle snapshot restore failed: {source}"
            ),
        }
    }
}

impl Error for QueuedPrefetcherError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ThrottleUsefulCounter { source } | Self::SnapshotThrottleRestore { source } => {
                Some(source)
            }
            Self::ZeroCapacity
            | Self::ZeroIssueWidth
            | Self::ZeroLineSize
            | Self::ZeroPageSize
            | Self::ZeroMissingTranslationCapacity
            | Self::QueueFull { .. }
            | Self::ReadyTickOverflow { .. }
            | Self::VectorLengthTooLarge { .. }
            | Self::SnapshotConfigMismatch { .. }
            | Self::SnapshotQueueTooLarge { .. }
            | Self::SnapshotMissingTranslationQueueTooLarge { .. }
            | Self::UnknownTranslation { .. }
            | Self::TranslationNotStarted { .. }
            | Self::TranslationRequestAddressOverflow { .. }
            | Self::SnapshotThrottleStateMismatch { .. } => None,
        }
    }
}

fn maximum_queued_prefetch_entries() -> usize {
    max_vector_len::<QueuedPrefetchEntry>().min(max_vector_len::<QueuedPrefetchEntrySnapshot>())
}

fn maximum_queued_prefetch_missing_translation_entries() -> usize {
    max_vector_len::<QueuedPrefetchMissingTranslationEntry>().min(max_vector_len::<
        QueuedPrefetchMissingTranslationEntrySnapshot,
    >())
}

fn validate_queued_prefetch_vector_length(
    field: &'static str,
    length: usize,
    maximum: usize,
) -> Result<(), QueuedPrefetcherError> {
    if length > maximum {
        return Err(QueuedPrefetcherError::VectorLengthTooLarge {
            field,
            length,
            maximum,
        });
    }
    Ok(())
}

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
pub struct QueuedPrefetchSourceStatus {
    prefetched: bool,
}

impl QueuedPrefetchSourceStatus {
    pub const fn demand() -> Self {
        Self { prefetched: false }
    }

    pub const fn prefetched() -> Self {
        Self { prefetched: true }
    }

    pub const fn is_prefetched(&self) -> bool {
        self.prefetched
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueuedPrefetchEnqueueResult {
    accepted: usize,
    duplicate_hits: usize,
    updated_priorities: usize,
    dropped_redundant: usize,
    dropped_page_crossing: usize,
    pending_translations: usize,
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

    pub const fn pending_translations(&self) -> usize {
        self.pending_translations
    }

    pub const fn dropped_throttled(&self) -> usize {
        self.dropped_throttled
    }

    pub const fn evicted_full(&self) -> usize {
        self.evicted_full
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueuedPrefetchTranslationOutcome {
    Queued,
    Redundant,
    PrefetchQueueFull,
    TranslationFailed,
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
pub struct QueuedPrefetchMissingTranslationEntrySnapshot {
    virtual_address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    stride: i64,
    degree_index: u32,
    priority: i32,
    source_tick: u64,
    order: u64,
    ongoing_translation: bool,
}

impl QueuedPrefetchMissingTranslationEntrySnapshot {
    pub const fn virtual_address(&self) -> Address {
        self.virtual_address
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

    pub const fn priority(&self) -> i32 {
        self.priority
    }

    pub const fn source_tick(&self) -> u64 {
        self.source_tick
    }

    pub const fn order(&self) -> u64 {
        self.order
    }

    pub const fn ongoing_translation(&self) -> bool {
        self.ongoing_translation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedPrefetchEntrySnapshot {
    address: Address,
    duplicate_address: Address,
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

    pub const fn duplicate_address(&self) -> Address {
        self.duplicate_address
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
    missing_translations: Vec<QueuedPrefetchMissingTranslationEntrySnapshot>,
    next_order: u64,
    stats: QueuedPrefetchStatsSnapshot,
    throttle: Option<QueuedPrefetchThrottleSnapshot>,
}

impl QueuedPrefetcherSnapshot {
    pub const fn config(&self) -> &QueuedPrefetchConfig {
        &self.config
    }

    pub fn pending(&self) -> &[QueuedPrefetchEntrySnapshot] {
        &self.pending
    }

    pub fn missing_translations(&self) -> &[QueuedPrefetchMissingTranslationEntrySnapshot] {
        &self.missing_translations
    }

    pub const fn next_order(&self) -> u64 {
        self.next_order
    }

    pub const fn stats(&self) -> &QueuedPrefetchStatsSnapshot {
        &self.stats
    }

    pub const fn throttle(&self) -> Option<&QueuedPrefetchThrottleSnapshot> {
        self.throttle.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedPrefetchTranslationRequest {
    request: TranslationRequest,
    source_address: Address,
    pc: u64,
    secure: bool,
    stride: i64,
    degree_index: u32,
    priority: i32,
    source_tick: u64,
    order: u64,
}

impl QueuedPrefetchTranslationRequest {
    pub const fn request(&self) -> &TranslationRequest {
        &self.request
    }

    pub const fn source_address(&self) -> Address {
        self.source_address
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

    pub const fn order(&self) -> u64 {
        self.order
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
    duplicate_address: Address,
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
            duplicate_address: address,
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
            duplicate_address: snapshot.duplicate_address,
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
            duplicate_address: self.duplicate_address,
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
        self.duplicate_address == address && self.secure == candidate.secure()
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
struct QueuedPrefetchMissingTranslationEntry {
    virtual_address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    stride: i64,
    degree_index: u32,
    priority: i32,
    source_tick: u64,
    order: u64,
    ongoing_translation: bool,
}

impl QueuedPrefetchMissingTranslationEntry {
    fn from_candidate<C: PrefetchCandidate>(
        candidate: &C,
        virtual_address: Address,
        source_tick: u64,
        order: u64,
    ) -> Self {
        Self {
            virtual_address,
            source_address: candidate.source_address(),
            context: candidate.context(),
            pc: candidate.pc(),
            secure: candidate.secure(),
            stride: candidate.stride(),
            degree_index: candidate.degree_index(),
            priority: queued_prefetch_priority(candidate),
            source_tick,
            order,
            ongoing_translation: false,
        }
    }

    fn from_snapshot(snapshot: &QueuedPrefetchMissingTranslationEntrySnapshot) -> Self {
        Self {
            virtual_address: snapshot.virtual_address,
            source_address: snapshot.source_address,
            context: snapshot.context,
            pc: snapshot.pc,
            secure: snapshot.secure,
            stride: snapshot.stride,
            degree_index: snapshot.degree_index,
            priority: snapshot.priority,
            source_tick: snapshot.source_tick,
            order: snapshot.order,
            ongoing_translation: snapshot.ongoing_translation,
        }
    }

    fn snapshot(&self) -> QueuedPrefetchMissingTranslationEntrySnapshot {
        QueuedPrefetchMissingTranslationEntrySnapshot {
            virtual_address: self.virtual_address,
            source_address: self.source_address,
            context: self.context,
            pc: self.pc,
            secure: self.secure,
            stride: self.stride,
            degree_index: self.degree_index,
            priority: self.priority,
            source_tick: self.source_tick,
            order: self.order,
            ongoing_translation: self.ongoing_translation,
        }
    }

    fn request_id(&self) -> TranslationRequestId {
        TranslationRequestId::new(self.context, self.order)
    }

    fn translation_request(
        &self,
        line_size: u64,
    ) -> Result<QueuedPrefetchTranslationRequest, QueuedPrefetcherError> {
        let size = AccessSize::new(line_size).expect("queued prefetch line size is nonzero");
        let request = TranslationRequest::new(
            self.request_id(),
            self.virtual_address,
            size,
            TranslationAccessKind::Prefetch,
        )
        .map_err(
            |_| QueuedPrefetcherError::TranslationRequestAddressOverflow {
                address: self.virtual_address,
                size: line_size,
            },
        )?;
        Ok(QueuedPrefetchTranslationRequest {
            request,
            source_address: self.source_address,
            pc: self.pc,
            secure: self.secure,
            stride: self.stride,
            degree_index: self.degree_index,
            priority: self.priority,
            source_tick: self.source_tick,
            order: self.order,
        })
    }

    fn same_request<C: PrefetchCandidate>(&self, virtual_address: Address, candidate: &C) -> bool {
        self.virtual_address == virtual_address && self.secure == candidate.secure()
    }

    fn update_priority<C: PrefetchCandidate>(&mut self, candidate: &C) -> bool {
        let priority = queued_prefetch_priority(candidate);
        if priority <= self.priority {
            return false;
        }

        self.priority = priority;
        true
    }

    fn ready_entry(&self, address: Address, ready_tick: u64) -> QueuedPrefetchEntry {
        QueuedPrefetchEntry {
            address,
            duplicate_address: self.virtual_address,
            context: self.context,
            pc: self.pc,
            secure: self.secure,
            stride: self.stride,
            degree_index: self.degree_index,
            priority: self.priority,
            source_tick: self.source_tick,
            ready_tick,
            order: self.order,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedPrefetcher {
    config: QueuedPrefetchConfig,
    pending: Vec<QueuedPrefetchEntry>,
    missing_translations: Vec<QueuedPrefetchMissingTranslationEntry>,
    next_order: u64,
    stats: QueuedPrefetchStatsSnapshot,
    throttle: Option<QueuedPrefetchThrottle>,
}

impl QueuedPrefetcher {
    pub fn new(config: QueuedPrefetchConfig) -> Self {
        let throttle = config
            .throttle_config()
            .cloned()
            .map(QueuedPrefetchThrottle::new);
        Self {
            config,
            pending: Vec::new(),
            missing_translations: Vec::new(),
            next_order: 0,
            stats: QueuedPrefetchStatsSnapshot::default(),
            throttle,
        }
    }

    pub const fn config(&self) -> &QueuedPrefetchConfig {
        &self.config
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn missing_translation_count(&self) -> usize {
        self.missing_translations.len()
    }

    pub fn next_ready_tick(&self) -> Option<u64> {
        self.pending.first().map(|entry| entry.ready_tick)
    }

    pub const fn stats(&self) -> &QueuedPrefetchStatsSnapshot {
        &self.stats
    }

    pub const fn throttle(&self) -> Option<&QueuedPrefetchThrottle> {
        self.throttle.as_ref()
    }

    pub fn record_useful_prefetch(
        &mut self,
        missed_usable_state: bool,
    ) -> Result<(), QueuedPrefetcherError> {
        if let Some(throttle) = &mut self.throttle {
            throttle
                .record_useful(1)
                .map_err(|source| QueuedPrefetcherError::ThrottleUsefulCounter { source })?;
        }
        self.stats.record_useful(missed_usable_state);
        Ok(())
    }

    pub fn record_prefetch_unused(&mut self) {
        self.stats.record_unused();
    }

    pub fn record_demand_mshr_miss(&mut self) {
        self.stats.record_demand_mshr_miss();
    }

    pub fn record_prefetch_hit_in_cache(&mut self) {
        self.stats.record_hit_in_cache();
    }

    pub fn record_prefetch_hit_in_mshr(&mut self) {
        self.stats.record_hit_in_mshr();
    }

    pub fn record_prefetch_hit_in_write_buffer(&mut self) {
        self.stats.record_hit_in_write_buffer();
    }

    pub fn enqueue_candidates<C: PrefetchCandidate>(
        &mut self,
        source_tick: u64,
        candidates: &[C],
    ) -> Result<usize, QueuedPrefetcherError> {
        let result = self.enqueue_candidates_filtered(source_tick, candidates, &[])?;
        Ok(result.accepted() + result.pending_translations())
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
            self.configured_prefetch_limit(candidates.len()),
            QueuedPrefetchSourceStatus::demand(),
        )
    }

    pub fn enqueue_candidates_filtered_with_source<C: PrefetchCandidate>(
        &mut self,
        source_tick: u64,
        candidates: &[C],
        redundant_lines: &[QueuedPrefetchRedundantLine],
        source_status: QueuedPrefetchSourceStatus,
    ) -> Result<QueuedPrefetchEnqueueResult, QueuedPrefetcherError> {
        self.enqueue_candidates_filtered_limited(
            source_tick,
            candidates,
            redundant_lines,
            self.configured_prefetch_limit(candidates.len()),
            source_status,
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
            QueuedPrefetchSourceStatus::demand(),
        )
    }

    pub fn enqueue_candidates_throttled_with_source<C: PrefetchCandidate>(
        &mut self,
        source_tick: u64,
        candidates: &[C],
        redundant_lines: &[QueuedPrefetchRedundantLine],
        throttle: &QueuedPrefetchThrottle,
        source_status: QueuedPrefetchSourceStatus,
    ) -> Result<QueuedPrefetchEnqueueResult, QueuedPrefetcherError> {
        self.enqueue_candidates_filtered_limited(
            source_tick,
            candidates,
            redundant_lines,
            throttle.max_permitted(candidates.len()),
            source_status,
        )
    }

    fn enqueue_candidates_filtered_limited<C: PrefetchCandidate>(
        &mut self,
        source_tick: u64,
        candidates: &[C],
        redundant_lines: &[QueuedPrefetchRedundantLine],
        attempt_limit: usize,
        source_status: QueuedPrefetchSourceStatus,
    ) -> Result<QueuedPrefetchEnqueueResult, QueuedPrefetcherError> {
        let ready_tick = source_tick.checked_add(self.config.latency()).ok_or(
            QueuedPrefetcherError::ReadyTickOverflow {
                source_tick,
                latency: self.config.latency(),
            },
        )?;
        let attempt_limit = attempt_limit.min(candidates.len());
        let mut insertion_attempts = 0;
        let mut accepted = 0;
        let mut duplicate_hits = 0;
        let mut updated_priorities = 0;
        let mut dropped_redundant = 0;
        let mut dropped_page_crossing = 0;
        let mut pending_translations = 0;
        let mut dropped_throttled = 0;
        let mut evicted_full = 0;
        for (index, candidate) in candidates.iter().enumerate() {
            if insertion_attempts == attempt_limit {
                dropped_throttled = candidates.len() - index;
                break;
            }

            let address = self.normalized_address(candidate.address());
            if self.crosses_page(address, candidate.source_address()) {
                self.stats.record_span_page(1);
                if source_status.is_prefetched() {
                    self.stats.record_useful_span_page(1);
                }
                if self.config.missing_translation_capacity().is_none() {
                    dropped_page_crossing += 1;
                    self.stats.record_translation_queue_dropped(1);
                    continue;
                }
                self.stats.record_identified(1);
                insertion_attempts += 1;
                if self.config.filter_duplicates() {
                    if let Some(index) = self
                        .pending
                        .iter()
                        .position(|entry| entry.same_request(address, candidate))
                    {
                        self.stats.record_buffer_hit(1);
                        duplicate_hits += 1;
                        if self.pending[index].update_priority(candidate) {
                            updated_priorities += 1;
                            sort_pending_entries(&mut self.pending);
                        }
                        continue;
                    }
                    if let Some(index) = self
                        .missing_translations
                        .iter()
                        .position(|entry| entry.same_request(address, candidate))
                    {
                        self.stats.record_buffer_hit(1);
                        duplicate_hits += 1;
                        if self.missing_translations[index].update_priority(candidate) {
                            updated_priorities += 1;
                            sort_missing_translation_entries(&mut self.missing_translations);
                        }
                        continue;
                    }
                }
                evicted_full +=
                    self.enqueue_missing_translation(candidate, address, source_tick)?;
                pending_translations += 1;
                continue;
            }
            self.stats.record_identified(1);
            insertion_attempts += 1;
            if self.is_redundant(address, candidate.secure(), redundant_lines) {
                self.stats.record_in_cache_drop(1);
                self.stats.record_prefetch_queue_dropped(1);
                dropped_redundant += 1;
                continue;
            }
            if self.config.filter_duplicates() {
                if let Some(index) = self
                    .pending
                    .iter()
                    .position(|entry| entry.same_request(address, candidate))
                {
                    self.stats.record_buffer_hit(1);
                    duplicate_hits += 1;
                    if self.pending[index].update_priority(candidate) {
                        updated_priorities += 1;
                        sort_pending_entries(&mut self.pending);
                    }
                    continue;
                }
                if let Some(index) = self
                    .missing_translations
                    .iter()
                    .position(|entry| entry.same_request(address, candidate))
                {
                    self.stats.record_buffer_hit(1);
                    duplicate_hits += 1;
                    if self.missing_translations[index].update_priority(candidate) {
                        updated_priorities += 1;
                        sort_missing_translation_entries(&mut self.missing_translations);
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
                        self.stats.record_removed_by_full_queue(1);
                        self.stats.record_prefetch_queue_dropped(1);
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
            self.stats.record_prefetch_queue_enqueued(1);
            accepted += 1;
        }
        Ok(QueuedPrefetchEnqueueResult {
            accepted,
            duplicate_hits,
            updated_priorities,
            dropped_redundant,
            dropped_page_crossing,
            pending_translations,
            dropped_throttled,
            evicted_full,
        })
    }

    pub fn process_missing_translations(
        &mut self,
        max: usize,
    ) -> Result<Vec<QueuedPrefetchTranslationRequest>, QueuedPrefetcherError> {
        let mut pending = Vec::new();
        let line_size = self.config.line_size();
        for (index, entry) in self.missing_translations.iter().enumerate() {
            if pending.len() == max {
                break;
            }
            if entry.ongoing_translation {
                continue;
            }
            let request = entry.translation_request(line_size)?;
            pending.push((index, request));
        }
        for (index, _) in &pending {
            self.missing_translations[*index].ongoing_translation = true;
        }
        self.stats
            .record_translation_queue_issued(pending.len() as u64);
        Ok(pending.into_iter().map(|(_, request)| request).collect())
    }

    pub fn complete_translation(
        &mut self,
        completion_tick: u64,
        request: TranslationRequestId,
        resolution: TranslationResolution,
        redundant_lines: &[QueuedPrefetchRedundantLine],
    ) -> Result<QueuedPrefetchTranslationOutcome, QueuedPrefetcherError> {
        let Some(position) = self
            .missing_translations
            .iter()
            .position(|entry| entry.request_id() == request)
        else {
            return Err(QueuedPrefetcherError::UnknownTranslation { request });
        };
        if !self.missing_translations[position].ongoing_translation {
            return Err(QueuedPrefetcherError::TranslationNotStarted { request });
        }
        let entry = self.missing_translations.remove(position);
        let Some(physical_address) = resolution.physical_address() else {
            self.stats.record_translation_queue_dropped(1);
            return Ok(QueuedPrefetchTranslationOutcome::TranslationFailed);
        };
        self.stats.record_translation_queue_translated(1);
        let address = self.normalized_address(physical_address);
        if self.is_redundant(address, entry.secure, redundant_lines) {
            self.stats.record_in_cache_drop(1);
            self.stats.record_prefetch_queue_dropped(1);
            return Ok(QueuedPrefetchTranslationOutcome::Redundant);
        }
        let ready_tick = completion_tick.checked_add(self.config.latency()).ok_or(
            QueuedPrefetcherError::ReadyTickOverflow {
                source_tick: completion_tick,
                latency: self.config.latency(),
            },
        )?;
        if self.pending.len() == self.config.capacity() {
            match self.config.full_policy() {
                QueuedPrefetchFullPolicy::RejectNew => {
                    self.stats.record_prefetch_queue_dropped(1);
                    return Ok(QueuedPrefetchTranslationOutcome::PrefetchQueueFull);
                }
                QueuedPrefetchFullPolicy::EvictOldestLowestPriority => {
                    let victim = oldest_lowest_priority_index(&self.pending);
                    self.pending.remove(victim);
                    self.stats.record_removed_by_full_queue(1);
                    self.stats.record_prefetch_queue_dropped(1);
                }
            }
        }
        insert_pending_entry(&mut self.pending, entry.ready_entry(address, ready_tick));
        self.stats.record_prefetch_queue_enqueued(1);
        Ok(QueuedPrefetchTranslationOutcome::Queued)
    }

    pub fn squash_demand_access(&mut self, access: QueuedPrefetchDemandAccess) -> usize {
        let line_address = self.normalized_address(access.address());
        let original_len = self.pending.len();
        self.pending
            .retain(|entry| !(entry.address == line_address && entry.secure == access.secure()));
        let removed = original_len - self.pending.len();
        self.stats.record_removed_by_demand(removed as u64);
        self.stats.record_prefetch_queue_dropped(removed as u64);
        removed
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
        let issue = self.pending.remove(0).issue();
        self.stats.record_issued(1);
        self.stats.record_prefetch_queue_issued(1);
        if let Some(throttle) = &mut self.throttle {
            throttle.record_issued_saturating(1);
        }
        Some(issue)
    }

    pub fn snapshot(&self) -> QueuedPrefetcherSnapshot {
        QueuedPrefetcherSnapshot {
            config: self.config.clone(),
            pending: self
                .pending
                .iter()
                .map(QueuedPrefetchEntry::snapshot)
                .collect(),
            missing_translations: self
                .missing_translations
                .iter()
                .map(QueuedPrefetchMissingTranslationEntry::snapshot)
                .collect(),
            next_order: self.next_order,
            stats: self.stats.clone(),
            throttle: self.throttle.as_ref().map(QueuedPrefetchThrottle::snapshot),
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
        let missing_translation_capacity = self.config.missing_translation_capacity().unwrap_or(0);
        if snapshot.missing_translations().len() > missing_translation_capacity {
            return Err(
                QueuedPrefetcherError::SnapshotMissingTranslationQueueTooLarge {
                    pending: snapshot.missing_translations().len(),
                    capacity: missing_translation_capacity,
                },
            );
        }
        let mut pending: Vec<_> = snapshot
            .pending()
            .iter()
            .map(QueuedPrefetchEntry::from_snapshot)
            .collect();
        let mut missing_translations: Vec<_> = snapshot
            .missing_translations()
            .iter()
            .map(QueuedPrefetchMissingTranslationEntry::from_snapshot)
            .collect();
        let throttle = match (self.config.throttle_config(), snapshot.throttle()) {
            (Some(config), Some(snapshot)) => {
                let mut throttle = QueuedPrefetchThrottle::new(config.clone());
                throttle
                    .restore(snapshot)
                    .map_err(|source| QueuedPrefetcherError::SnapshotThrottleRestore { source })?;
                Some(throttle)
            }
            (None, None) => None,
            (expected, actual) => {
                return Err(QueuedPrefetcherError::SnapshotThrottleStateMismatch {
                    expected_enabled: expected.is_some(),
                    actual_enabled: actual.is_some(),
                });
            }
        };
        sort_pending_entries(&mut pending);
        sort_missing_translation_entries(&mut missing_translations);
        self.pending = pending;
        self.missing_translations = missing_translations;
        self.next_order = snapshot.next_order();
        self.stats = snapshot.stats().clone();
        self.throttle = throttle;
        Ok(())
    }

    fn configured_prefetch_limit(&self, total_candidates: usize) -> usize {
        self.throttle.as_ref().map_or(total_candidates, |throttle| {
            throttle.max_permitted(total_candidates)
        })
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

    fn enqueue_missing_translation<C: PrefetchCandidate>(
        &mut self,
        candidate: &C,
        virtual_address: Address,
        source_tick: u64,
    ) -> Result<usize, QueuedPrefetcherError> {
        let capacity = self
            .config
            .missing_translation_capacity()
            .expect("missing translation capacity is checked before enqueue");
        let mut evicted_full = 0;
        if self.missing_translations.len() == capacity {
            match self.config.full_policy() {
                QueuedPrefetchFullPolicy::RejectNew => {
                    return Err(QueuedPrefetcherError::QueueFull { capacity });
                }
                QueuedPrefetchFullPolicy::EvictOldestLowestPriority => {
                    let Some(victim) = oldest_lowest_priority_missing_translation_index(
                        &self.missing_translations,
                    ) else {
                        return Err(QueuedPrefetcherError::QueueFull { capacity });
                    };
                    self.missing_translations.remove(victim);
                    self.stats.record_removed_by_full_queue(1);
                    self.stats.record_translation_queue_dropped(1);
                    evicted_full = 1;
                }
            }
        }
        let entry = QueuedPrefetchMissingTranslationEntry::from_candidate(
            candidate,
            virtual_address,
            source_tick,
            self.next_order,
        );
        self.next_order = self.next_order.saturating_add(1);
        insert_missing_translation_entry(&mut self.missing_translations, entry);
        self.stats.record_translation_queue_enqueued(1);
        Ok(evicted_full)
    }
}

fn insert_pending_entry(pending: &mut Vec<QueuedPrefetchEntry>, entry: QueuedPrefetchEntry) {
    let index = pending
        .iter()
        .position(|existing| pending_entry_precedes(&entry, existing))
        .unwrap_or(pending.len());
    pending.insert(index, entry);
}

fn insert_missing_translation_entry(
    pending: &mut Vec<QueuedPrefetchMissingTranslationEntry>,
    entry: QueuedPrefetchMissingTranslationEntry,
) {
    let index = pending
        .iter()
        .position(|existing| missing_translation_entry_precedes(&entry, existing))
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

fn sort_missing_translation_entries(pending: &mut [QueuedPrefetchMissingTranslationEntry]) {
    pending.sort_by(|left, right| {
        if missing_translation_entry_precedes(left, right) {
            std::cmp::Ordering::Less
        } else if missing_translation_entry_precedes(right, left) {
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

fn missing_translation_entry_precedes(
    left: &QueuedPrefetchMissingTranslationEntry,
    right: &QueuedPrefetchMissingTranslationEntry,
) -> bool {
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

fn oldest_lowest_priority_missing_translation_index(
    pending: &[QueuedPrefetchMissingTranslationEntry],
) -> Option<usize> {
    pending
        .iter()
        .enumerate()
        .filter(|(_, entry)| !entry.ongoing_translation)
        .min_by_key(|(_, entry)| (entry.priority, entry.order))
        .map(|(index, _)| index)
}

fn queued_prefetch_priority<C: PrefetchCandidate>(candidate: &C) -> i32 {
    let degree = candidate.degree_index().min(i32::MAX as u32) as i32;
    i32::MAX.saturating_sub(degree)
}

fn page_address(address: Address, page_size: u64) -> u64 {
    address.get() / page_size
}

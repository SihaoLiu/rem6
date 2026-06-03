use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::allocation::max_vector_len;
use rem6_memory::{Address, AgentId};

const MAX_CONFIDENCE: u8 = 3;

fn normalized_address(address: Address, line_size: u64) -> Address {
    Address::new(address.get() / line_size * line_size)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrefetchAccessKind {
    Read,
    Write,
    Invalidate,
}

impl PrefetchAccessKind {
    const fn is_read(self) -> bool {
        matches!(self, Self::Read)
    }

    const fn is_write(self) -> bool {
        matches!(self, Self::Write)
    }

    const fn is_invalidate(self) -> bool {
        matches!(self, Self::Invalidate)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrefetchObservation {
    access_kind: PrefetchAccessKind,
    miss: bool,
    instruction_fetch: bool,
    prefetched: bool,
    uncacheable: bool,
    software_prefetch: bool,
    cache_maintenance: bool,
    clean_eviction: bool,
    write_coalesced: bool,
}

impl PrefetchObservation {
    pub const fn new(access_kind: PrefetchAccessKind, miss: bool, instruction_fetch: bool) -> Self {
        Self {
            access_kind,
            miss,
            instruction_fetch,
            prefetched: false,
            uncacheable: false,
            software_prefetch: false,
            cache_maintenance: false,
            clean_eviction: false,
            write_coalesced: false,
        }
    }

    pub const fn access_kind(&self) -> PrefetchAccessKind {
        self.access_kind
    }

    pub const fn miss(&self) -> bool {
        self.miss
    }

    pub const fn instruction_fetch(&self) -> bool {
        self.instruction_fetch
    }

    pub const fn prefetched(&self) -> bool {
        self.prefetched
    }

    pub const fn uncacheable(&self) -> bool {
        self.uncacheable
    }

    pub const fn software_prefetch(&self) -> bool {
        self.software_prefetch
    }

    pub const fn cache_maintenance(&self) -> bool {
        self.cache_maintenance
    }

    pub const fn clean_eviction(&self) -> bool {
        self.clean_eviction
    }

    pub const fn write_coalesced(&self) -> bool {
        self.write_coalesced
    }

    pub const fn with_prefetched(mut self, prefetched: bool) -> Self {
        self.prefetched = prefetched;
        self
    }

    pub const fn with_uncacheable(mut self, uncacheable: bool) -> Self {
        self.uncacheable = uncacheable;
        self
    }

    pub const fn with_software_prefetch(mut self, software_prefetch: bool) -> Self {
        self.software_prefetch = software_prefetch;
        self
    }

    pub const fn with_cache_maintenance(mut self, cache_maintenance: bool) -> Self {
        self.cache_maintenance = cache_maintenance;
        self
    }

    pub const fn with_clean_eviction(mut self, clean_eviction: bool) -> Self {
        self.clean_eviction = clean_eviction;
        self
    }

    pub const fn with_write_coalesced(mut self, write_coalesced: bool) -> Self {
        self.write_coalesced = write_coalesced;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrefetchObservationConfigOptions {
    pub on_miss: bool,
    pub on_read: bool,
    pub on_write: bool,
    pub on_data: bool,
    pub on_inst: bool,
    pub prefetch_on_access: bool,
    pub prefetch_on_prefetch_hit: bool,
}

impl Default for PrefetchObservationConfigOptions {
    fn default() -> Self {
        Self {
            on_miss: false,
            on_read: true,
            on_write: true,
            on_data: true,
            on_inst: true,
            prefetch_on_access: false,
            prefetch_on_prefetch_hit: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrefetchObservationConfig {
    on_miss: bool,
    on_read: bool,
    on_write: bool,
    on_data: bool,
    on_inst: bool,
    prefetch_on_access: bool,
    prefetch_on_prefetch_hit: bool,
}

impl PrefetchObservationConfig {
    pub const fn new(options: PrefetchObservationConfigOptions) -> Self {
        Self {
            on_miss: options.on_miss,
            on_read: options.on_read,
            on_write: options.on_write,
            on_data: options.on_data,
            on_inst: options.on_inst,
            prefetch_on_access: options.prefetch_on_access,
            prefetch_on_prefetch_hit: options.prefetch_on_prefetch_hit,
        }
    }

    pub const fn on_miss(&self) -> bool {
        self.on_miss
    }

    pub const fn on_read(&self) -> bool {
        self.on_read
    }

    pub const fn on_write(&self) -> bool {
        self.on_write
    }

    pub const fn on_data(&self) -> bool {
        self.on_data
    }

    pub const fn on_inst(&self) -> bool {
        self.on_inst
    }

    pub const fn prefetch_on_access(&self) -> bool {
        self.prefetch_on_access
    }

    pub const fn prefetch_on_prefetch_hit(&self) -> bool {
        self.prefetch_on_prefetch_hit
    }

    pub const fn should_observe(&self, observation: PrefetchObservation) -> bool {
        if observation.software_prefetch()
            || observation.cache_maintenance()
            || observation.clean_eviction()
        {
            return false;
        }
        if observation.access_kind().is_write() && observation.write_coalesced() {
            return false;
        }
        if !observation.miss() {
            if self.prefetch_on_prefetch_hit() {
                return observation.prefetched();
            }
            if !self.prefetch_on_access() {
                return false;
            }
        }
        if observation.uncacheable() {
            return false;
        }
        if observation.instruction_fetch() {
            if !self.on_inst() {
                return false;
            }
        } else {
            if !self.on_data() {
                return false;
            }
            if observation.access_kind().is_read() && !self.on_read() {
                return false;
            }
            if !observation.access_kind().is_read() && !self.on_write() {
                return false;
            }
            if !observation.access_kind().is_read() && observation.access_kind().is_invalidate() {
                return false;
            }
        }
        if self.on_miss() {
            return observation.miss();
        }
        true
    }
}

impl Default for PrefetchObservationConfig {
    fn default() -> Self {
        Self::new(PrefetchObservationConfigOptions::default())
    }
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
        validate_stride_vector_length(
            "table entries",
            table_entries,
            maximum_stride_table_entries(),
        )?;
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
    VectorLengthTooLarge {
        field: &'static str,
        length: usize,
        maximum: usize,
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
            Self::VectorLengthTooLarge {
                field,
                length,
                maximum,
            } => write!(
                formatter,
                "stride prefetcher {field} length {length} exceeds maximum {maximum}"
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

fn maximum_stride_table_entries() -> usize {
    max_vector_len::<StridePrefetchEntry>().min(max_vector_len::<StridePrefetchEntrySnapshot>())
}

fn validate_stride_vector_length(
    field: &'static str,
    length: usize,
    maximum: usize,
) -> Result<(), StridePrefetcherError> {
    if length > maximum {
        return Err(StridePrefetcherError::VectorLengthTooLarge {
            field,
            length,
            maximum,
        });
    }
    Ok(())
}

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

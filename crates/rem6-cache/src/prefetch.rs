use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

const MAX_CONFIDENCE: u8 = 3;

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
    context: AgentId,
    pc: u64,
    secure: bool,
    stride: i64,
    degree_index: u32,
}

impl StridePrefetchCandidate {
    fn new(
        address: Address,
        context: AgentId,
        pc: u64,
        secure: bool,
        stride: i64,
        degree_index: u32,
    ) -> Self {
        Self {
            address,
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

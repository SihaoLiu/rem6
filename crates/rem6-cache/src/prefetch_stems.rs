use std::collections::{btree_map::Entry, BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::prefetch::PrefetchCandidate;

const SEQUENCE_MAX_COUNTER: u8 = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StemsPrefetcherConfig {
    line_size: u64,
    spatial_region_size: u64,
    spatial_region_bits: u32,
    reconstruction_entries: usize,
    active_generation_entries: usize,
    pattern_sequence_entries: usize,
    region_miss_order_buffer_entries: usize,
    add_duplicate_entries_to_rmob: bool,
    sequence_slots: usize,
}

impl StemsPrefetcherConfig {
    pub fn new(
        line_size: u64,
        spatial_region_size: u64,
        reconstruction_entries: usize,
        active_generation_entries: usize,
        pattern_sequence_entries: usize,
        region_miss_order_buffer_entries: usize,
        add_duplicate_entries_to_rmob: bool,
    ) -> Result<Self, StemsPrefetcherError> {
        if line_size == 0 {
            return Err(StemsPrefetcherError::ZeroLineSize);
        }
        if !line_size.is_power_of_two() {
            return Err(StemsPrefetcherError::LineSizeNotPowerOfTwo { line_size });
        }
        if spatial_region_size == 0 {
            return Err(StemsPrefetcherError::ZeroSpatialRegionSize);
        }
        if !spatial_region_size.is_power_of_two() {
            return Err(StemsPrefetcherError::SpatialRegionSizeNotPowerOfTwo {
                spatial_region_size,
            });
        }
        if spatial_region_size < line_size || !spatial_region_size.is_multiple_of(line_size) {
            return Err(StemsPrefetcherError::SpatialRegionLineMismatch {
                spatial_region_size,
                line_size,
            });
        }
        if reconstruction_entries == 0 {
            return Err(StemsPrefetcherError::ZeroReconstructionEntries);
        }
        if active_generation_entries == 0 {
            return Err(StemsPrefetcherError::ZeroActiveGenerationEntries);
        }
        if pattern_sequence_entries == 0 {
            return Err(StemsPrefetcherError::ZeroPatternSequenceEntries);
        }
        if region_miss_order_buffer_entries == 0 {
            return Err(StemsPrefetcherError::ZeroRegionMissOrderBufferEntries);
        }

        let sequence_slots = (spatial_region_size / line_size) as usize;
        Ok(Self {
            line_size,
            spatial_region_size,
            spatial_region_bits: spatial_region_size.trailing_zeros(),
            reconstruction_entries,
            active_generation_entries,
            pattern_sequence_entries,
            region_miss_order_buffer_entries,
            add_duplicate_entries_to_rmob,
            sequence_slots,
        })
    }

    pub const fn line_size(&self) -> u64 {
        self.line_size
    }

    pub const fn spatial_region_size(&self) -> u64 {
        self.spatial_region_size
    }

    pub const fn spatial_region_bits(&self) -> u32 {
        self.spatial_region_bits
    }

    pub const fn reconstruction_entries(&self) -> usize {
        self.reconstruction_entries
    }

    pub const fn active_generation_entries(&self) -> usize {
        self.active_generation_entries
    }

    pub const fn pattern_sequence_entries(&self) -> usize {
        self.pattern_sequence_entries
    }

    pub const fn region_miss_order_buffer_entries(&self) -> usize {
        self.region_miss_order_buffer_entries
    }

    pub const fn add_duplicate_entries_to_rmob(&self) -> bool {
        self.add_duplicate_entries_to_rmob
    }

    pub const fn sequence_slots(&self) -> usize {
        self.sequence_slots
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StemsPrefetcherError {
    ZeroLineSize,
    LineSizeNotPowerOfTwo {
        line_size: u64,
    },
    ZeroSpatialRegionSize,
    SpatialRegionSizeNotPowerOfTwo {
        spatial_region_size: u64,
    },
    SpatialRegionLineMismatch {
        spatial_region_size: u64,
        line_size: u64,
    },
    ZeroReconstructionEntries,
    ZeroActiveGenerationEntries,
    ZeroPatternSequenceEntries,
    ZeroRegionMissOrderBufferEntries,
    PstAddressOverflow {
        pc: u64,
        spatial_region_size: u64,
        offset: u32,
    },
    RegionBaseOverflow {
        region_address: u64,
        spatial_region_size: u64,
    },
    OffsetAddressOverflow {
        region_base: u64,
        offset: u32,
        line_size: u64,
    },
    SnapshotConfigMismatch {
        expected: Box<StemsPrefetcherConfig>,
        actual: Box<StemsPrefetcherConfig>,
    },
    SnapshotActiveGenerationTooLarge {
        entries: usize,
        max_entries: usize,
    },
    SnapshotPatternSequenceTooLarge {
        entries: usize,
        max_entries: usize,
    },
    SnapshotRegionMissOrderBufferTooLarge {
        entries: usize,
        max_entries: usize,
    },
    SnapshotSequenceShapeMismatch {
        key: u64,
        entries: usize,
        expected: usize,
    },
    SnapshotSequenceCounterOutOfRange {
        key: u64,
        offset: u32,
        counter: u8,
    },
    SnapshotActiveLruShapeMismatch {
        entries: usize,
        table_entries: usize,
    },
    SnapshotPatternLruShapeMismatch {
        entries: usize,
        table_entries: usize,
    },
    SnapshotActiveLruUnknownKey {
        region_address: u64,
        secure: bool,
    },
    SnapshotPatternLruUnknownKey {
        pst_address: u64,
        secure: bool,
    },
}

impl fmt::Display for StemsPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLineSize => write!(formatter, "STeMS line size is zero"),
            Self::LineSizeNotPowerOfTwo { line_size } => {
                write!(formatter, "STeMS line size {line_size} is not a power of two")
            }
            Self::ZeroSpatialRegionSize => {
                write!(formatter, "STeMS spatial region size is zero")
            }
            Self::SpatialRegionSizeNotPowerOfTwo {
                spatial_region_size,
            } => write!(
                formatter,
                "STeMS spatial region size {spatial_region_size} is not a power of two"
            ),
            Self::SpatialRegionLineMismatch {
                spatial_region_size,
                line_size,
            } => write!(
                formatter,
                "STeMS spatial region size {spatial_region_size} is not a multiple of line size {line_size}"
            ),
            Self::ZeroReconstructionEntries => {
                write!(formatter, "STeMS reconstruction buffer has no entries")
            }
            Self::ZeroActiveGenerationEntries => {
                write!(formatter, "STeMS active generation table has no entries")
            }
            Self::ZeroPatternSequenceEntries => {
                write!(formatter, "STeMS pattern sequence table has no entries")
            }
            Self::ZeroRegionMissOrderBufferEntries => {
                write!(formatter, "STeMS region miss order buffer has no entries")
            }
            Self::PstAddressOverflow {
                pc,
                spatial_region_size,
                offset,
            } => write!(
                formatter,
                "STeMS PST address overflows for pc {pc:#x}, region size {spatial_region_size}, offset {offset}"
            ),
            Self::RegionBaseOverflow {
                region_address,
                spatial_region_size,
            } => write!(
                formatter,
                "STeMS region address {region_address:#x} overflows with region size {spatial_region_size}"
            ),
            Self::OffsetAddressOverflow {
                region_base,
                offset,
                line_size,
            } => write!(
                formatter,
                "STeMS offset address overflows for base {region_base:#x}, offset {offset}, line size {line_size}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "STeMS snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotActiveGenerationTooLarge {
                entries,
                max_entries,
            } => write!(
                formatter,
                "STeMS snapshot has {entries} active generations for {max_entries} slots"
            ),
            Self::SnapshotPatternSequenceTooLarge {
                entries,
                max_entries,
            } => write!(
                formatter,
                "STeMS snapshot has {entries} pattern sequences for {max_entries} slots"
            ),
            Self::SnapshotRegionMissOrderBufferTooLarge {
                entries,
                max_entries,
            } => write!(
                formatter,
                "STeMS snapshot has {entries} RMOB entries for {max_entries} slots"
            ),
            Self::SnapshotSequenceShapeMismatch {
                key,
                entries,
                expected,
            } => write!(
                formatter,
                "STeMS snapshot sequence {key:#x} has {entries} entries instead of {expected}"
            ),
            Self::SnapshotSequenceCounterOutOfRange {
                key,
                offset,
                counter,
            } => write!(
                formatter,
                "STeMS snapshot sequence {key:#x} offset {offset} has counter {counter} above {SEQUENCE_MAX_COUNTER}"
            ),
            Self::SnapshotActiveLruShapeMismatch {
                entries,
                table_entries,
            } => write!(
                formatter,
                "STeMS snapshot active LRU has {entries} entries for {table_entries} table rows"
            ),
            Self::SnapshotPatternLruShapeMismatch {
                entries,
                table_entries,
            } => write!(
                formatter,
                "STeMS snapshot pattern LRU has {entries} entries for {table_entries} table rows"
            ),
            Self::SnapshotActiveLruUnknownKey {
                region_address,
                secure,
            } => write!(
                formatter,
                "STeMS snapshot active LRU references missing region {region_address:#x}, secure {secure}"
            ),
            Self::SnapshotPatternLruUnknownKey {
                pst_address,
                secure,
            } => write!(
                formatter,
                "STeMS snapshot pattern LRU references missing PST address {pst_address:#x}, secure {secure}"
            ),
        }
    }
}

impl Error for StemsPrefetcherError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StemsPrefetchAccess {
    requestor: AgentId,
    pc: u64,
    address: Address,
    physical_address: Address,
    secure: bool,
    cache_miss: bool,
}

impl StemsPrefetchAccess {
    pub const fn new(
        requestor: AgentId,
        pc: u64,
        address: Address,
        physical_address: Address,
        secure: bool,
        cache_miss: bool,
    ) -> Self {
        Self {
            requestor,
            pc,
            address,
            physical_address,
            secure,
            cache_miss,
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

    pub const fn physical_address(&self) -> Address {
        self.physical_address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn cache_miss(&self) -> bool {
        self.cache_miss
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LineKey {
    line_address: u64,
    secure: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StemsCacheResidency {
    cache_lines: BTreeSet<LineKey>,
    miss_queue_lines: BTreeSet<LineKey>,
}

impl StemsCacheResidency {
    pub const fn new() -> Self {
        Self {
            cache_lines: BTreeSet::new(),
            miss_queue_lines: BTreeSet::new(),
        }
    }

    pub fn with_cache_line(mut self, address: Address, secure: bool) -> Self {
        self.cache_lines.insert(LineKey {
            line_address: address.get(),
            secure,
        });
        self
    }

    pub fn with_miss_queue_line(mut self, address: Address, secure: bool) -> Self {
        self.miss_queue_lines.insert(LineKey {
            line_address: address.get(),
            secure,
        });
        self
    }

    pub fn cache_lines(&self) -> Vec<(Address, bool)> {
        self.cache_lines
            .iter()
            .map(|key| (Address::new(key.line_address), key.secure))
            .collect()
    }

    pub fn miss_queue_lines(&self) -> Vec<(Address, bool)> {
        self.miss_queue_lines
            .iter()
            .map(|key| (Address::new(key.line_address), key.secure))
            .collect()
    }

    fn contains(&self, address: Address, secure: bool, line_size: u64) -> bool {
        let key = LineKey {
            line_address: normalize_line(address, line_size).get(),
            secure,
        };
        self.cache_lines.contains(&key) || self.miss_queue_lines.contains(&key)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StemsPrefetchCandidate {
    address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    region_address: u64,
    pst_address: u64,
    spatial_offset: u32,
    reconstruction_index: u32,
    stride: i64,
    degree_index: u32,
}

impl StemsPrefetchCandidate {
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

    pub const fn region_address(&self) -> u64 {
        self.region_address
    }

    pub const fn pst_address(&self) -> u64 {
        self.pst_address
    }

    pub const fn spatial_offset(&self) -> u32 {
        self.spatial_offset
    }

    pub const fn reconstruction_index(&self) -> u32 {
        self.reconstruction_index
    }

    pub const fn stride(&self) -> i64 {
        self.stride
    }

    pub const fn degree_index(&self) -> u32 {
        self.degree_index
    }
}

impl PrefetchCandidate for StemsPrefetchCandidate {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StemsSequenceEntrySnapshot {
    counter: u8,
    offset: u32,
    delta: u32,
}

impl StemsSequenceEntrySnapshot {
    pub const fn counter(&self) -> u8 {
        self.counter
    }

    pub const fn offset(&self) -> u32 {
        self.offset
    }

    pub const fn delta(&self) -> u32 {
        self.delta
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StemsGenerationEntrySnapshot {
    region_address: u64,
    secure: bool,
    physical_region_base: Address,
    pc: u64,
    sequence_counter: u32,
    sequence: Vec<StemsSequenceEntrySnapshot>,
}

impl StemsGenerationEntrySnapshot {
    pub const fn region_address(&self) -> u64 {
        self.region_address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn physical_region_base(&self) -> Address {
        self.physical_region_base
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn sequence_counter(&self) -> u32 {
        self.sequence_counter
    }

    pub fn sequence(&self) -> &[StemsSequenceEntrySnapshot] {
        &self.sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StemsPatternSequenceEntrySnapshot {
    pst_address: u64,
    secure: bool,
    physical_region_base: Address,
    pc: u64,
    sequence_counter: u32,
    sequence: Vec<StemsSequenceEntrySnapshot>,
}

impl StemsPatternSequenceEntrySnapshot {
    pub const fn pst_address(&self) -> u64 {
        self.pst_address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn physical_region_base(&self) -> Address {
        self.physical_region_base
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn sequence_counter(&self) -> u32 {
        self.sequence_counter
    }

    pub fn sequence(&self) -> &[StemsSequenceEntrySnapshot] {
        &self.sequence
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StemsRegionMissOrderBufferEntrySnapshot {
    region_address: u64,
    secure: bool,
    pst_address: u64,
    delta: u32,
}

impl StemsRegionMissOrderBufferEntrySnapshot {
    pub const fn region_address(&self) -> u64 {
        self.region_address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn pst_address(&self) -> u64 {
        self.pst_address
    }

    pub const fn delta(&self) -> u32 {
        self.delta
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StemsActiveGenerationKeySnapshot {
    region_address: u64,
    secure: bool,
}

impl StemsActiveGenerationKeySnapshot {
    pub const fn region_address(&self) -> u64 {
        self.region_address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StemsPatternSequenceKeySnapshot {
    pst_address: u64,
    secure: bool,
}

impl StemsPatternSequenceKeySnapshot {
    pub const fn pst_address(&self) -> u64 {
        self.pst_address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StemsPrefetcherSnapshot {
    config: StemsPrefetcherConfig,
    active_generations: Vec<StemsGenerationEntrySnapshot>,
    active_generation_lru: Vec<StemsActiveGenerationKeySnapshot>,
    pattern_sequences: Vec<StemsPatternSequenceEntrySnapshot>,
    pattern_sequence_lru: Vec<StemsPatternSequenceKeySnapshot>,
    region_miss_order_buffer: Vec<StemsRegionMissOrderBufferEntrySnapshot>,
    last_trigger_counter: u32,
    last_candidates: Vec<StemsPrefetchCandidate>,
}

impl StemsPrefetcherSnapshot {
    pub const fn config(&self) -> &StemsPrefetcherConfig {
        &self.config
    }

    pub fn active_generations(&self) -> &[StemsGenerationEntrySnapshot] {
        &self.active_generations
    }

    pub fn active_generation_lru(&self) -> &[StemsActiveGenerationKeySnapshot] {
        &self.active_generation_lru
    }

    pub fn pattern_sequences(&self) -> &[StemsPatternSequenceEntrySnapshot] {
        &self.pattern_sequences
    }

    pub fn pattern_sequence_lru(&self) -> &[StemsPatternSequenceKeySnapshot] {
        &self.pattern_sequence_lru
    }

    pub fn region_miss_order_buffer(&self) -> &[StemsRegionMissOrderBufferEntrySnapshot] {
        &self.region_miss_order_buffer
    }

    pub const fn last_trigger_counter(&self) -> u32 {
        self.last_trigger_counter
    }

    pub fn last_candidates(&self) -> &[StemsPrefetchCandidate] {
        &self.last_candidates
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ActiveGenerationKey {
    region_address: u64,
    secure: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PatternSequenceKey {
    pst_address: u64,
    secure: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SequenceEntry {
    counter: u8,
    offset: u32,
    delta: u32,
}

impl SequenceEntry {
    const fn empty() -> Self {
        Self {
            counter: 0,
            offset: 0,
            delta: 0,
        }
    }

    fn snapshot(&self) -> StemsSequenceEntrySnapshot {
        StemsSequenceEntrySnapshot {
            counter: self.counter,
            offset: self.offset,
            delta: self.delta,
        }
    }

    fn from_snapshot(snapshot: &StemsSequenceEntrySnapshot) -> Self {
        Self {
            counter: snapshot.counter(),
            offset: snapshot.offset(),
            delta: snapshot.delta(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GenerationEntry {
    physical_region_base: Address,
    pc: u64,
    sequence_counter: u32,
    sequence: Vec<SequenceEntry>,
}

impl GenerationEntry {
    fn new(physical_region_base: Address, pc: u64, sequence_slots: usize) -> Self {
        Self {
            physical_region_base,
            pc,
            sequence_counter: 0,
            sequence: vec![SequenceEntry::empty(); sequence_slots],
        }
    }

    fn add_offset(&mut self, offset: u32) {
        for entry in &mut self.sequence {
            if entry.counter > 0 && entry.offset == offset {
                entry.counter = entry.counter.saturating_add(1).min(SEQUENCE_MAX_COUNTER);
                self.sequence_counter = 0;
                return;
            }
        }
        if let Some(entry) = self.sequence.iter_mut().find(|entry| entry.counter == 0) {
            entry.counter = 1;
            entry.offset = offset;
            entry.delta = self.sequence_counter;
        }
        self.sequence_counter = 0;
    }

    fn active_snapshot(&self, key: ActiveGenerationKey) -> StemsGenerationEntrySnapshot {
        StemsGenerationEntrySnapshot {
            region_address: key.region_address,
            secure: key.secure,
            physical_region_base: self.physical_region_base,
            pc: self.pc,
            sequence_counter: self.sequence_counter,
            sequence: self.sequence.iter().map(SequenceEntry::snapshot).collect(),
        }
    }

    fn pattern_snapshot(&self, key: PatternSequenceKey) -> StemsPatternSequenceEntrySnapshot {
        StemsPatternSequenceEntrySnapshot {
            pst_address: key.pst_address,
            secure: key.secure,
            physical_region_base: self.physical_region_base,
            pc: self.pc,
            sequence_counter: self.sequence_counter,
            sequence: self.sequence.iter().map(SequenceEntry::snapshot).collect(),
        }
    }

    fn from_generation_snapshot(snapshot: &StemsGenerationEntrySnapshot) -> Self {
        Self {
            physical_region_base: snapshot.physical_region_base(),
            pc: snapshot.pc(),
            sequence_counter: snapshot.sequence_counter(),
            sequence: snapshot
                .sequence()
                .iter()
                .map(SequenceEntry::from_snapshot)
                .collect(),
        }
    }

    fn from_pattern_snapshot(snapshot: &StemsPatternSequenceEntrySnapshot) -> Self {
        Self {
            physical_region_base: snapshot.physical_region_base(),
            pc: snapshot.pc(),
            sequence_counter: snapshot.sequence_counter(),
            sequence: snapshot
                .sequence()
                .iter()
                .map(SequenceEntry::from_snapshot)
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RegionMissOrderBufferEntry {
    region_address: u64,
    secure: bool,
    pst_address: u64,
    delta: u32,
}

impl RegionMissOrderBufferEntry {
    fn snapshot(&self) -> StemsRegionMissOrderBufferEntrySnapshot {
        StemsRegionMissOrderBufferEntrySnapshot {
            region_address: self.region_address,
            secure: self.secure,
            pst_address: self.pst_address,
            delta: self.delta,
        }
    }

    fn from_snapshot(snapshot: &StemsRegionMissOrderBufferEntrySnapshot) -> Self {
        Self {
            region_address: snapshot.region_address(),
            secure: snapshot.secure(),
            pst_address: snapshot.pst_address(),
            delta: snapshot.delta(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReconstructionEntry {
    address: Address,
    region_address: u64,
    pst_address: u64,
    spatial_offset: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StemsPrefetcher {
    config: StemsPrefetcherConfig,
    active_generation_table: BTreeMap<ActiveGenerationKey, GenerationEntry>,
    active_generation_lru: VecDeque<ActiveGenerationKey>,
    pattern_sequence_table: BTreeMap<PatternSequenceKey, GenerationEntry>,
    pattern_sequence_lru: VecDeque<PatternSequenceKey>,
    region_miss_order_buffer: VecDeque<RegionMissOrderBufferEntry>,
    last_trigger_counter: u32,
    last_candidates: Vec<StemsPrefetchCandidate>,
}

impl StemsPrefetcher {
    pub fn new(config: StemsPrefetcherConfig) -> Self {
        Self {
            config,
            active_generation_table: BTreeMap::new(),
            active_generation_lru: VecDeque::new(),
            pattern_sequence_table: BTreeMap::new(),
            pattern_sequence_lru: VecDeque::new(),
            region_miss_order_buffer: VecDeque::new(),
            last_trigger_counter: 0,
            last_candidates: Vec::new(),
        }
    }

    pub const fn config(&self) -> &StemsPrefetcherConfig {
        &self.config
    }

    pub fn observe(
        &mut self,
        access: StemsPrefetchAccess,
        residency: &StemsCacheResidency,
    ) -> Result<&[StemsPrefetchCandidate], StemsPrefetcherError> {
        self.last_candidates.clear();
        self.check_for_active_generation_end(residency)?;

        let region_address = self.region_address(access.address());
        let offset = self.spatial_offset(access.address());
        let key = ActiveGenerationKey {
            region_address,
            secure: access.secure(),
        };
        if self.active_generation_table.contains_key(&key) {
            self.touch_active_lru(key);
            self.active_generation_table
                .get_mut(&key)
                .expect("active generation key exists")
                .add_offset(offset);
            self.last_trigger_counter = self.last_trigger_counter.saturating_add(1);
        } else {
            let pst_address = self.pst_address(access.pc(), offset)?;
            self.add_to_rmob(
                region_address,
                access.secure(),
                pst_address,
                self.last_trigger_counter,
            );
            self.last_trigger_counter = 0;
            self.insert_active_generation(key, access, offset);
        }

        for (entry_key, entry) in &mut self.active_generation_table {
            if *entry_key != key {
                entry.sequence_counter = entry.sequence_counter.saturating_add(1);
            }
        }

        if access.cache_miss() {
            self.reconstruct_sequence(access, region_address, access.secure())?;
        }
        Ok(&self.last_candidates)
    }

    pub fn snapshot(&self) -> StemsPrefetcherSnapshot {
        StemsPrefetcherSnapshot {
            config: self.config.clone(),
            active_generations: self
                .active_generation_table
                .iter()
                .map(|(key, entry)| entry.active_snapshot(*key))
                .collect(),
            active_generation_lru: self
                .active_generation_lru
                .iter()
                .map(|key| StemsActiveGenerationKeySnapshot {
                    region_address: key.region_address,
                    secure: key.secure,
                })
                .collect(),
            pattern_sequences: self
                .pattern_sequence_table
                .iter()
                .map(|(key, entry)| entry.pattern_snapshot(*key))
                .collect(),
            pattern_sequence_lru: self
                .pattern_sequence_lru
                .iter()
                .map(|key| StemsPatternSequenceKeySnapshot {
                    pst_address: key.pst_address,
                    secure: key.secure,
                })
                .collect(),
            region_miss_order_buffer: self
                .region_miss_order_buffer
                .iter()
                .map(RegionMissOrderBufferEntry::snapshot)
                .collect(),
            last_trigger_counter: self.last_trigger_counter,
            last_candidates: self.last_candidates.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &StemsPrefetcherSnapshot,
    ) -> Result<(), StemsPrefetcherError> {
        self.validate_snapshot(snapshot)?;
        self.active_generation_table = snapshot
            .active_generations()
            .iter()
            .map(|entry| {
                (
                    ActiveGenerationKey {
                        region_address: entry.region_address(),
                        secure: entry.secure(),
                    },
                    GenerationEntry::from_generation_snapshot(entry),
                )
            })
            .collect();
        self.active_generation_lru = snapshot
            .active_generation_lru()
            .iter()
            .map(|key| ActiveGenerationKey {
                region_address: key.region_address(),
                secure: key.secure(),
            })
            .collect();
        self.pattern_sequence_table = snapshot
            .pattern_sequences()
            .iter()
            .map(|entry| {
                (
                    PatternSequenceKey {
                        pst_address: entry.pst_address(),
                        secure: entry.secure(),
                    },
                    GenerationEntry::from_pattern_snapshot(entry),
                )
            })
            .collect();
        self.pattern_sequence_lru = snapshot
            .pattern_sequence_lru()
            .iter()
            .map(|key| PatternSequenceKey {
                pst_address: key.pst_address(),
                secure: key.secure(),
            })
            .collect();
        self.region_miss_order_buffer = snapshot
            .region_miss_order_buffer()
            .iter()
            .map(RegionMissOrderBufferEntry::from_snapshot)
            .collect();
        self.last_trigger_counter = snapshot.last_trigger_counter();
        self.last_candidates = snapshot.last_candidates().to_vec();
        Ok(())
    }

    pub fn active_generation_count(&self) -> usize {
        self.active_generation_table.len()
    }

    pub fn pattern_sequence_count(&self) -> usize {
        self.pattern_sequence_table.len()
    }

    pub fn rmob_entry_count(&self) -> usize {
        self.region_miss_order_buffer.len()
    }

    pub fn active_generation_regions(&self) -> Vec<(u64, bool)> {
        self.active_generation_table
            .keys()
            .map(|key| (key.region_address, key.secure))
            .collect()
    }

    pub fn rmob_entries(&self) -> Vec<StemsRegionMissOrderBufferEntrySnapshot> {
        self.region_miss_order_buffer
            .iter()
            .map(RegionMissOrderBufferEntry::snapshot)
            .collect()
    }

    pub fn last_candidates(&self) -> &[StemsPrefetchCandidate] {
        &self.last_candidates
    }

    fn validate_snapshot(
        &self,
        snapshot: &StemsPrefetcherSnapshot,
    ) -> Result<(), StemsPrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(StemsPrefetcherError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config().clone()),
            });
        }
        if snapshot.active_generations().len() > self.config.active_generation_entries() {
            return Err(StemsPrefetcherError::SnapshotActiveGenerationTooLarge {
                entries: snapshot.active_generations().len(),
                max_entries: self.config.active_generation_entries(),
            });
        }
        if snapshot.pattern_sequences().len() > self.config.pattern_sequence_entries() {
            return Err(StemsPrefetcherError::SnapshotPatternSequenceTooLarge {
                entries: snapshot.pattern_sequences().len(),
                max_entries: self.config.pattern_sequence_entries(),
            });
        }
        if snapshot.region_miss_order_buffer().len()
            > self.config.region_miss_order_buffer_entries()
        {
            return Err(
                StemsPrefetcherError::SnapshotRegionMissOrderBufferTooLarge {
                    entries: snapshot.region_miss_order_buffer().len(),
                    max_entries: self.config.region_miss_order_buffer_entries(),
                },
            );
        }
        self.validate_generation_sequences(snapshot)?;
        self.validate_active_lru(snapshot)?;
        self.validate_pattern_lru(snapshot)?;
        Ok(())
    }

    fn validate_generation_sequences(
        &self,
        snapshot: &StemsPrefetcherSnapshot,
    ) -> Result<(), StemsPrefetcherError> {
        for entry in snapshot.active_generations() {
            validate_sequence_shape(
                entry.region_address(),
                entry.sequence(),
                self.config.sequence_slots(),
            )?;
        }
        for entry in snapshot.pattern_sequences() {
            validate_sequence_shape(
                entry.pst_address(),
                entry.sequence(),
                self.config.sequence_slots(),
            )?;
        }
        Ok(())
    }

    fn validate_active_lru(
        &self,
        snapshot: &StemsPrefetcherSnapshot,
    ) -> Result<(), StemsPrefetcherError> {
        if snapshot.active_generation_lru().len() != snapshot.active_generations().len() {
            return Err(StemsPrefetcherError::SnapshotActiveLruShapeMismatch {
                entries: snapshot.active_generation_lru().len(),
                table_entries: snapshot.active_generations().len(),
            });
        }
        let keys = snapshot
            .active_generations()
            .iter()
            .map(|entry| ActiveGenerationKey {
                region_address: entry.region_address(),
                secure: entry.secure(),
            })
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        for key in snapshot.active_generation_lru() {
            let active_key = ActiveGenerationKey {
                region_address: key.region_address(),
                secure: key.secure(),
            };
            if keys.contains(&active_key) && seen.insert(active_key) {
                continue;
            }
            return Err(StemsPrefetcherError::SnapshotActiveLruUnknownKey {
                region_address: key.region_address(),
                secure: key.secure(),
            });
        }
        Ok(())
    }

    fn validate_pattern_lru(
        &self,
        snapshot: &StemsPrefetcherSnapshot,
    ) -> Result<(), StemsPrefetcherError> {
        if snapshot.pattern_sequence_lru().len() != snapshot.pattern_sequences().len() {
            return Err(StemsPrefetcherError::SnapshotPatternLruShapeMismatch {
                entries: snapshot.pattern_sequence_lru().len(),
                table_entries: snapshot.pattern_sequences().len(),
            });
        }
        let keys = snapshot
            .pattern_sequences()
            .iter()
            .map(|entry| PatternSequenceKey {
                pst_address: entry.pst_address(),
                secure: entry.secure(),
            })
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        for key in snapshot.pattern_sequence_lru() {
            let pattern_key = PatternSequenceKey {
                pst_address: key.pst_address(),
                secure: key.secure(),
            };
            if keys.contains(&pattern_key) && seen.insert(pattern_key) {
                continue;
            }
            return Err(StemsPrefetcherError::SnapshotPatternLruUnknownKey {
                pst_address: key.pst_address(),
                secure: key.secure(),
            });
        }
        Ok(())
    }

    fn check_for_active_generation_end(
        &mut self,
        residency: &StemsCacheResidency,
    ) -> Result<(), StemsPrefetcherError> {
        let entries = self
            .active_generation_table
            .iter()
            .map(|(key, entry)| (*key, entry.clone()))
            .collect::<Vec<_>>();
        for (key, entry) in entries {
            let Some(ending_offset) = entry.sequence.iter().find_map(|sequence| {
                if sequence.counter == 0 {
                    return None;
                }
                let address = offset_address(
                    entry.physical_region_base,
                    sequence.offset,
                    self.config.line_size(),
                )
                .ok()?;
                (!residency.contains(address, key.secure, self.config.line_size()))
                    .then_some(sequence.offset)
            }) else {
                continue;
            };
            let pst_address = self.pst_address(entry.pc, ending_offset)?;
            self.insert_pattern_sequence(
                PatternSequenceKey {
                    pst_address,
                    secure: key.secure,
                },
                entry,
            );
            self.active_generation_table.remove(&key);
            self.active_generation_lru.retain(|entry| *entry != key);
        }
        Ok(())
    }

    fn insert_active_generation(
        &mut self,
        key: ActiveGenerationKey,
        access: StemsPrefetchAccess,
        offset: u32,
    ) {
        while self.active_generation_table.len() >= self.config.active_generation_entries() {
            let Some(victim) = self.active_generation_lru.pop_back() else {
                break;
            };
            self.active_generation_table.remove(&victim);
        }
        let mut entry = GenerationEntry::new(
            self.physical_region_base(access.physical_address()),
            access.pc(),
            self.config.sequence_slots(),
        );
        entry.add_offset(offset);
        self.active_generation_table.insert(key, entry);
        self.touch_active_lru(key);
    }

    fn insert_pattern_sequence(&mut self, key: PatternSequenceKey, entry: GenerationEntry) {
        if let Entry::Occupied(mut occupied) = self.pattern_sequence_table.entry(key) {
            occupied.insert(entry);
            self.touch_pattern_lru(key);
            return;
        }
        while self.pattern_sequence_table.len() >= self.config.pattern_sequence_entries() {
            let Some(victim) = self.pattern_sequence_lru.pop_back() else {
                break;
            };
            self.pattern_sequence_table.remove(&victim);
        }
        self.pattern_sequence_table.insert(key, entry);
        self.touch_pattern_lru(key);
    }

    fn add_to_rmob(&mut self, region_address: u64, secure: bool, pst_address: u64, delta: u32) {
        let entry = RegionMissOrderBufferEntry {
            region_address,
            secure,
            pst_address,
            delta,
        };
        if !self.config.add_duplicate_entries_to_rmob()
            && self
                .region_miss_order_buffer
                .iter()
                .any(|existing| *existing == entry)
        {
            return;
        }
        self.region_miss_order_buffer.push_back(entry);
        while self.region_miss_order_buffer.len() > self.config.region_miss_order_buffer_entries() {
            self.region_miss_order_buffer.pop_front();
        }
    }

    fn reconstruct_sequence(
        &mut self,
        access: StemsPrefetchAccess,
        region_address: u64,
        secure: bool,
    ) -> Result<(), StemsPrefetcherError> {
        let rmob = self
            .region_miss_order_buffer
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let Some(start) = rmob.iter().enumerate().rev().find_map(|(index, entry)| {
            (entry.region_address == region_address && entry.secure == secure).then_some(index)
        }) else {
            return Ok(());
        };

        let mut reconstruction = vec![None; self.config.reconstruction_entries()];
        let mut index = 0;
        for position in start..rmob.len() {
            if index >= reconstruction.len() {
                break;
            }
            let entry = rmob[position];
            let base = self.region_base(entry.region_address)?;
            place_reconstruction(
                &mut reconstruction,
                index,
                ReconstructionEntry {
                    address: Address::new(base),
                    region_address: entry.region_address,
                    pst_address: entry.pst_address,
                    spatial_offset: 0,
                },
            );
            index = advance_reconstruction_index(index, next_delta(&rmob, position));
        }

        index = 0;
        for position in start..rmob.len() {
            if index >= reconstruction.len() {
                break;
            }
            let entry = rmob[position];
            let pattern_key = PatternSequenceKey {
                pst_address: entry.pst_address,
                secure: entry.secure,
            };
            if let Some(pattern) = self.pattern_sequence_table.get(&pattern_key).cloned() {
                self.touch_pattern_lru(pattern_key);
                let region_base = self.region_base(entry.region_address)?;
                for sequence in pattern
                    .sequence
                    .iter()
                    .filter(|sequence| sequence.counter > 1)
                {
                    let ridx = index.saturating_add(sequence.delta as usize);
                    let address = offset_address(
                        Address::new(region_base),
                        sequence.offset,
                        self.config.line_size(),
                    )?;
                    place_reconstruction_near(
                        &mut reconstruction,
                        ridx,
                        ReconstructionEntry {
                            address,
                            region_address: entry.region_address,
                            pst_address: entry.pst_address,
                            spatial_offset: sequence.offset,
                        },
                    );
                }
            }
            index = advance_reconstruction_index(index, next_delta(&rmob, position));
        }

        let source_address = normalize_line(access.address(), self.config.line_size());
        for (index, entry) in reconstruction.into_iter().enumerate() {
            let Some(entry) = entry else {
                continue;
            };
            let degree_index = self
                .last_candidates
                .len()
                .saturating_add(1)
                .min(u32::MAX as usize) as u32;
            self.last_candidates.push(StemsPrefetchCandidate {
                address: entry.address,
                source_address,
                context: access.requestor(),
                pc: access.pc(),
                secure: access.secure(),
                region_address: entry.region_address,
                pst_address: entry.pst_address,
                spatial_offset: entry.spatial_offset,
                reconstruction_index: index.min(u32::MAX as usize) as u32,
                stride: byte_stride(source_address, entry.address),
                degree_index,
            });
        }
        Ok(())
    }

    fn touch_active_lru(&mut self, key: ActiveGenerationKey) {
        self.active_generation_lru.retain(|entry| *entry != key);
        self.active_generation_lru.push_front(key);
    }

    fn touch_pattern_lru(&mut self, key: PatternSequenceKey) {
        self.pattern_sequence_lru.retain(|entry| *entry != key);
        self.pattern_sequence_lru.push_front(key);
    }

    fn region_address(&self, address: Address) -> u64 {
        address.get() / self.config.spatial_region_size()
    }

    fn region_base(&self, region_address: u64) -> Result<u64, StemsPrefetcherError> {
        region_address
            .checked_mul(self.config.spatial_region_size())
            .ok_or(StemsPrefetcherError::RegionBaseOverflow {
                region_address,
                spatial_region_size: self.config.spatial_region_size(),
            })
    }

    fn physical_region_base(&self, physical_address: Address) -> Address {
        Address::new(
            physical_address.get() / self.config.spatial_region_size()
                * self.config.spatial_region_size(),
        )
    }

    fn spatial_offset(&self, address: Address) -> u32 {
        ((address.get() % self.config.spatial_region_size()) / self.config.line_size()) as u32
    }

    fn pst_address(&self, pc: u64, offset: u32) -> Result<u64, StemsPrefetcherError> {
        pc.checked_mul(self.config.spatial_region_size())
            .and_then(|base| base.checked_add(offset as u64))
            .ok_or(StemsPrefetcherError::PstAddressOverflow {
                pc,
                spatial_region_size: self.config.spatial_region_size(),
                offset,
            })
    }
}

fn validate_sequence_shape(
    key: u64,
    sequence: &[StemsSequenceEntrySnapshot],
    expected: usize,
) -> Result<(), StemsPrefetcherError> {
    if sequence.len() != expected {
        return Err(StemsPrefetcherError::SnapshotSequenceShapeMismatch {
            key,
            entries: sequence.len(),
            expected,
        });
    }
    for entry in sequence {
        if entry.counter() > SEQUENCE_MAX_COUNTER {
            return Err(StemsPrefetcherError::SnapshotSequenceCounterOutOfRange {
                key,
                offset: entry.offset(),
                counter: entry.counter(),
            });
        }
    }
    Ok(())
}

fn normalize_line(address: Address, line_size: u64) -> Address {
    Address::new(address.get() / line_size * line_size)
}

fn offset_address(
    region_base: Address,
    offset: u32,
    line_size: u64,
) -> Result<Address, StemsPrefetcherError> {
    let delta = (offset as u64).checked_mul(line_size).ok_or(
        StemsPrefetcherError::OffsetAddressOverflow {
            region_base: region_base.get(),
            offset,
            line_size,
        },
    )?;
    let address = region_base.get().checked_add(delta).ok_or(
        StemsPrefetcherError::OffsetAddressOverflow {
            region_base: region_base.get(),
            offset,
            line_size,
        },
    )?;
    Ok(Address::new(address))
}

fn next_delta(rmob: &[RegionMissOrderBufferEntry], position: usize) -> u32 {
    rmob.get(position.saturating_add(1))
        .map(|entry| entry.delta)
        .unwrap_or(0)
}

fn advance_reconstruction_index(index: usize, delta: u32) -> usize {
    index.saturating_add(delta as usize).saturating_add(1)
}

fn place_reconstruction(
    reconstruction: &mut [Option<ReconstructionEntry>],
    index: usize,
    entry: ReconstructionEntry,
) {
    if let Some(slot) = reconstruction.get_mut(index) {
        *slot = Some(entry);
    }
}

fn place_reconstruction_near(
    reconstruction: &mut [Option<ReconstructionEntry>],
    index: usize,
    entry: ReconstructionEntry,
) {
    for candidate in [
        Some(index),
        index.checked_add(1),
        index.checked_add(2),
        index.checked_sub(1),
        index.checked_sub(2),
    ]
    .into_iter()
    .flatten()
    {
        let Some(slot) = reconstruction.get_mut(candidate) else {
            continue;
        };
        if slot.is_none() {
            *slot = Some(entry);
            return;
        }
    }
}

fn byte_stride(source_address: Address, candidate_address: Address) -> i64 {
    let stride = candidate_address.get() as i128 - source_address.get() as i128;
    stride.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

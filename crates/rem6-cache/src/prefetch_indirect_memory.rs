use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::prefetch::PrefetchCandidate;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectMemoryPrefetcherConfig {
    prefetch_table_entries: usize,
    pattern_detector_entries: usize,
    address_array_len: usize,
    shift_values: Vec<i32>,
    max_prefetch_distance: u32,
    indirect_counter_bits: u32,
    max_indirect_counter: u8,
    prefetch_threshold: u8,
    stream_counter_threshold: u32,
    streaming_distance: u32,
}

impl IndirectMemoryPrefetcherConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        prefetch_table_entries: usize,
        pattern_detector_entries: usize,
        address_array_len: usize,
        shift_values: Vec<i32>,
        max_prefetch_distance: u32,
        indirect_counter_bits: u32,
        prefetch_threshold: u8,
        stream_counter_threshold: u32,
        streaming_distance: u32,
    ) -> Result<Self, IndirectMemoryPrefetcherError> {
        if prefetch_table_entries == 0 {
            return Err(IndirectMemoryPrefetcherError::ZeroPrefetchTableEntries);
        }
        if pattern_detector_entries == 0 {
            return Err(IndirectMemoryPrefetcherError::ZeroPatternDetectorEntries);
        }
        if address_array_len == 0 {
            return Err(IndirectMemoryPrefetcherError::ZeroAddressArrayLen);
        }
        if shift_values.is_empty() {
            return Err(IndirectMemoryPrefetcherError::EmptyShiftValues);
        }
        if let Some(shift) = shift_values
            .iter()
            .copied()
            .find(|shift| !(-63..=63).contains(shift))
        {
            return Err(IndirectMemoryPrefetcherError::ShiftOutOfRange { shift });
        }
        if max_prefetch_distance == 0 {
            return Err(IndirectMemoryPrefetcherError::ZeroMaxPrefetchDistance);
        }
        if !(1..=8).contains(&indirect_counter_bits) {
            return Err(
                IndirectMemoryPrefetcherError::IndirectCounterBitsOutOfRange {
                    indirect_counter_bits,
                },
            );
        }
        if streaming_distance == 0 {
            return Err(IndirectMemoryPrefetcherError::ZeroStreamingDistance);
        }

        let max_indirect_counter = ((1_u16 << indirect_counter_bits) - 1) as u8;
        Ok(Self {
            prefetch_table_entries,
            pattern_detector_entries,
            address_array_len,
            shift_values,
            max_prefetch_distance,
            indirect_counter_bits,
            max_indirect_counter,
            prefetch_threshold,
            stream_counter_threshold,
            streaming_distance,
        })
    }

    pub const fn prefetch_table_entries(&self) -> usize {
        self.prefetch_table_entries
    }

    pub const fn pattern_detector_entries(&self) -> usize {
        self.pattern_detector_entries
    }

    pub const fn address_array_len(&self) -> usize {
        self.address_array_len
    }

    pub fn shift_values(&self) -> &[i32] {
        &self.shift_values
    }

    pub const fn max_prefetch_distance(&self) -> u32 {
        self.max_prefetch_distance
    }

    pub const fn indirect_counter_bits(&self) -> u32 {
        self.indirect_counter_bits
    }

    pub const fn max_indirect_counter(&self) -> u8 {
        self.max_indirect_counter
    }

    pub const fn prefetch_threshold(&self) -> u8 {
        self.prefetch_threshold
    }

    pub const fn stream_counter_threshold(&self) -> u32 {
        self.stream_counter_threshold
    }

    pub const fn streaming_distance(&self) -> u32 {
        self.streaming_distance
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IndirectMemoryPrefetcherError {
    ZeroPrefetchTableEntries,
    ZeroPatternDetectorEntries,
    ZeroAddressArrayLen,
    EmptyShiftValues,
    ShiftOutOfRange {
        shift: i32,
    },
    ZeroMaxPrefetchDistance,
    IndirectCounterBitsOutOfRange {
        indirect_counter_bits: u32,
    },
    ZeroStreamingDistance,
    InvalidIndexReadSize {
        size: u8,
    },
    StreamAddressOverflow {
        address: Address,
        delta: i128,
        degree: u32,
    },
    SnapshotConfigMismatch {
        expected: Box<IndirectMemoryPrefetcherConfig>,
        actual: Box<IndirectMemoryPrefetcherConfig>,
    },
    SnapshotPrefetchTableTooLarge {
        entries: usize,
        max_entries: usize,
    },
    SnapshotPatternDetectorTooLarge {
        entries: usize,
        max_entries: usize,
    },
    SnapshotPatternDetectorShapeMismatch {
        pc: u64,
        secure: bool,
        rows: usize,
        expected_rows: usize,
    },
    SnapshotPatternDetectorShiftShapeMismatch {
        pc: u64,
        secure: bool,
        row: usize,
        columns: usize,
        expected_columns: usize,
    },
    SnapshotIndirectCounterOutOfRange {
        pc: u64,
        secure: bool,
        counter: u8,
        max_counter: u8,
    },
    SnapshotPrefetchLruShapeMismatch {
        entries: usize,
        table_entries: usize,
    },
    SnapshotPatternLruShapeMismatch {
        entries: usize,
        table_entries: usize,
    },
    SnapshotPrefetchLruUnknownKey {
        pc: u64,
        secure: bool,
    },
    SnapshotPatternLruUnknownKey {
        pc: u64,
        secure: bool,
    },
    SnapshotPatternReferencesMissingPrefetchEntry {
        pc: u64,
        secure: bool,
    },
    SnapshotTrackingPatternMissing {
        pc: u64,
        secure: bool,
    },
}

impl fmt::Display for IndirectMemoryPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroPrefetchTableEntries => {
                write!(formatter, "indirect memory prefetch table has no entries")
            }
            Self::ZeroPatternDetectorEntries => {
                write!(formatter, "indirect memory pattern detector has no entries")
            }
            Self::ZeroAddressArrayLen => {
                write!(formatter, "indirect memory pattern detector records no misses")
            }
            Self::EmptyShiftValues => write!(formatter, "indirect memory shift list is empty"),
            Self::ShiftOutOfRange { shift } => {
                write!(formatter, "indirect memory shift {shift} is outside -63..=63")
            }
            Self::ZeroMaxPrefetchDistance => {
                write!(formatter, "indirect memory max prefetch distance is zero")
            }
            Self::IndirectCounterBitsOutOfRange {
                indirect_counter_bits,
            } => write!(
                formatter,
                "indirect memory counter bits {indirect_counter_bits} is outside 1..=8"
            ),
            Self::ZeroStreamingDistance => {
                write!(formatter, "indirect memory streaming distance is zero")
            }
            Self::InvalidIndexReadSize { size } => write!(
                formatter,
                "indirect memory index read size {size} is not one of 1, 2, 4, or 8"
            ),
            Self::StreamAddressOverflow {
                address,
                delta,
                degree,
            } => write!(
                formatter,
                "indirect memory stream address overflows from {:#x} with delta {delta} and degree {degree}",
                address.get()
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "indirect memory snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotPrefetchTableTooLarge {
                entries,
                max_entries,
            } => write!(
                formatter,
                "indirect memory snapshot has {entries} prefetch table entries for {max_entries} slots"
            ),
            Self::SnapshotPatternDetectorTooLarge {
                entries,
                max_entries,
            } => write!(
                formatter,
                "indirect memory snapshot has {entries} pattern detector entries for {max_entries} slots"
            ),
            Self::SnapshotPatternDetectorShapeMismatch {
                pc,
                secure,
                rows,
                expected_rows,
            } => write!(
                formatter,
                "indirect memory snapshot detector pc {pc:#x}, secure {secure} has {rows} rows instead of {expected_rows}"
            ),
            Self::SnapshotPatternDetectorShiftShapeMismatch {
                pc,
                secure,
                row,
                columns,
                expected_columns,
            } => write!(
                formatter,
                "indirect memory snapshot detector pc {pc:#x}, secure {secure}, row {row} has {columns} columns instead of {expected_columns}"
            ),
            Self::SnapshotIndirectCounterOutOfRange {
                pc,
                secure,
                counter,
                max_counter,
            } => write!(
                formatter,
                "indirect memory snapshot pc {pc:#x}, secure {secure} counter {counter} exceeds {max_counter}"
            ),
            Self::SnapshotPrefetchLruShapeMismatch {
                entries,
                table_entries,
            } => write!(
                formatter,
                "indirect memory snapshot prefetch LRU has {entries} entries for {table_entries} table rows"
            ),
            Self::SnapshotPatternLruShapeMismatch {
                entries,
                table_entries,
            } => write!(
                formatter,
                "indirect memory snapshot pattern LRU has {entries} entries for {table_entries} table rows"
            ),
            Self::SnapshotPrefetchLruUnknownKey { pc, secure } => write!(
                formatter,
                "indirect memory snapshot prefetch LRU references missing pc {pc:#x}, secure {secure}"
            ),
            Self::SnapshotPatternLruUnknownKey { pc, secure } => write!(
                formatter,
                "indirect memory snapshot pattern LRU references missing pc {pc:#x}, secure {secure}"
            ),
            Self::SnapshotPatternReferencesMissingPrefetchEntry { pc, secure } => write!(
                formatter,
                "indirect memory snapshot detector references missing prefetch pc {pc:#x}, secure {secure}"
            ),
            Self::SnapshotTrackingPatternMissing { pc, secure } => write!(
                formatter,
                "indirect memory snapshot tracks missing detector pc {pc:#x}, secure {secure}"
            ),
        }
    }
}

impl Error for IndirectMemoryPrefetcherError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndirectMemoryPrefetchAccess {
    context: AgentId,
    pc: u64,
    address: Address,
    secure: bool,
    cache_miss: bool,
    is_write: bool,
    size: u8,
    index_value: Option<i64>,
}

impl IndirectMemoryPrefetchAccess {
    pub const fn new(
        context: AgentId,
        pc: u64,
        address: Address,
        secure: bool,
        cache_miss: bool,
    ) -> Self {
        Self {
            context,
            pc,
            address,
            secure,
            cache_miss,
            is_write: false,
            size: 8,
            index_value: None,
        }
    }

    pub fn with_read_index(
        mut self,
        size: u8,
        value: i64,
    ) -> Result<Self, IndirectMemoryPrefetcherError> {
        if !matches!(size, 1 | 2 | 4 | 8) {
            return Err(IndirectMemoryPrefetcherError::InvalidIndexReadSize { size });
        }
        self.size = size;
        self.index_value = Some(value);
        self.is_write = false;
        Ok(self)
    }

    pub fn with_write(mut self, size: u8) -> Result<Self, IndirectMemoryPrefetcherError> {
        if size == 0 {
            return Err(IndirectMemoryPrefetcherError::InvalidIndexReadSize { size });
        }
        self.size = size;
        self.index_value = None;
        self.is_write = true;
        Ok(self)
    }

    pub const fn context(&self) -> AgentId {
        self.context
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

    pub const fn cache_miss(&self) -> bool {
        self.cache_miss
    }

    pub const fn is_write(&self) -> bool {
        self.is_write
    }

    pub const fn size(&self) -> u8 {
        self.size
    }

    pub const fn index_value(&self) -> Option<i64> {
        self.index_value
    }

    fn readable_index(&self) -> Option<i64> {
        (!self.cache_miss && !self.is_write && self.size <= 8)
            .then_some(self.index_value)
            .flatten()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndirectMemoryPrefetchKind {
    Stream,
    Indirect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectMemoryPrefetchCandidate {
    address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    kind: IndirectMemoryPrefetchKind,
    base_address: Address,
    index: i64,
    shift: i32,
    stream_delta: i64,
    indirect_counter: u8,
    stride: i64,
    degree_index: u32,
}

impl IndirectMemoryPrefetchCandidate {
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

    pub const fn kind(&self) -> IndirectMemoryPrefetchKind {
        self.kind
    }

    pub const fn base_address(&self) -> Address {
        self.base_address
    }

    pub const fn index(&self) -> i64 {
        self.index
    }

    pub const fn shift(&self) -> i32 {
        self.shift
    }

    pub const fn stream_delta(&self) -> i64 {
        self.stream_delta
    }

    pub const fn indirect_counter(&self) -> u8 {
        self.indirect_counter
    }

    pub const fn stride(&self) -> i64 {
        self.stride
    }

    pub const fn degree_index(&self) -> u32 {
        self.degree_index
    }
}

impl PrefetchCandidate for IndirectMemoryPrefetchCandidate {
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PrefetchTableKey {
    pc: u64,
    secure: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PrefetchTableEntry {
    address: Address,
    stream_counter: u32,
    enabled: bool,
    index: i64,
    base_address: Address,
    shift: i32,
    indirect_counter: u8,
    increased_indirect_counter: bool,
}

impl PrefetchTableEntry {
    fn new(address: Address) -> Self {
        Self {
            address,
            stream_counter: 0,
            enabled: false,
            index: 0,
            base_address: Address::new(0),
            shift: 0,
            indirect_counter: 0,
            increased_indirect_counter: false,
        }
    }

    fn snapshot(&self, key: PrefetchTableKey) -> IndirectMemoryPrefetchEntrySnapshot {
        IndirectMemoryPrefetchEntrySnapshot {
            pc: key.pc,
            secure: key.secure,
            address: self.address,
            stream_counter: self.stream_counter,
            enabled: self.enabled,
            index: self.index,
            base_address: self.base_address,
            shift: self.shift,
            indirect_counter: self.indirect_counter,
            increased_indirect_counter: self.increased_indirect_counter,
        }
    }

    fn from_snapshot(snapshot: &IndirectMemoryPrefetchEntrySnapshot) -> Self {
        Self {
            address: snapshot.address(),
            stream_counter: snapshot.stream_counter(),
            enabled: snapshot.enabled(),
            index: snapshot.index(),
            base_address: snapshot.base_address(),
            shift: snapshot.shift(),
            indirect_counter: snapshot.indirect_counter(),
            increased_indirect_counter: snapshot.increased_indirect_counter(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PatternDetectorEntry {
    idx1: i64,
    idx2: i64,
    second_index_set: bool,
    num_misses: usize,
    base_candidates: Vec<Vec<Option<Address>>>,
}

impl PatternDetectorEntry {
    fn new(address_array_len: usize, shift_count: usize, idx1: i64) -> Self {
        Self {
            idx1,
            idx2: 0,
            second_index_set: false,
            num_misses: 0,
            base_candidates: vec![vec![None; shift_count]; address_array_len],
        }
    }

    fn snapshot(&self, key: PrefetchTableKey) -> IndirectMemoryPatternDetectorEntrySnapshot {
        IndirectMemoryPatternDetectorEntrySnapshot {
            pc: key.pc,
            secure: key.secure,
            idx1: self.idx1,
            idx2: self.idx2,
            second_index_set: self.second_index_set,
            num_misses: self.num_misses,
            base_candidates: self.base_candidates.clone(),
        }
    }

    fn from_snapshot(snapshot: &IndirectMemoryPatternDetectorEntrySnapshot) -> Self {
        Self {
            idx1: snapshot.idx1(),
            idx2: snapshot.idx2(),
            second_index_set: snapshot.second_index_set(),
            num_misses: snapshot.num_misses(),
            base_candidates: snapshot.base_candidates().to_vec(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndirectMemoryPrefetchKeySnapshot {
    pc: u64,
    secure: bool,
}

impl IndirectMemoryPrefetchKeySnapshot {
    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectMemoryPrefetchEntrySnapshot {
    pc: u64,
    secure: bool,
    address: Address,
    stream_counter: u32,
    enabled: bool,
    index: i64,
    base_address: Address,
    shift: i32,
    indirect_counter: u8,
    increased_indirect_counter: bool,
}

impl IndirectMemoryPrefetchEntrySnapshot {
    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn stream_counter(&self) -> u32 {
        self.stream_counter
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn index(&self) -> i64 {
        self.index
    }

    pub const fn base_address(&self) -> Address {
        self.base_address
    }

    pub const fn shift(&self) -> i32 {
        self.shift
    }

    pub const fn indirect_counter(&self) -> u8 {
        self.indirect_counter
    }

    pub const fn increased_indirect_counter(&self) -> bool {
        self.increased_indirect_counter
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectMemoryPatternDetectorEntrySnapshot {
    pc: u64,
    secure: bool,
    idx1: i64,
    idx2: i64,
    second_index_set: bool,
    num_misses: usize,
    base_candidates: Vec<Vec<Option<Address>>>,
}

impl IndirectMemoryPatternDetectorEntrySnapshot {
    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn idx1(&self) -> i64 {
        self.idx1
    }

    pub const fn idx2(&self) -> i64 {
        self.idx2
    }

    pub const fn second_index_set(&self) -> bool {
        self.second_index_set
    }

    pub const fn num_misses(&self) -> usize {
        self.num_misses
    }

    pub fn base_candidates(&self) -> &[Vec<Option<Address>>] {
        &self.base_candidates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectMemoryPrefetcherSnapshot {
    config: IndirectMemoryPrefetcherConfig,
    prefetch_entries: Vec<IndirectMemoryPrefetchEntrySnapshot>,
    prefetch_lru: Vec<IndirectMemoryPrefetchKeySnapshot>,
    pattern_detector_entries: Vec<IndirectMemoryPatternDetectorEntrySnapshot>,
    pattern_detector_lru: Vec<IndirectMemoryPrefetchKeySnapshot>,
    tracking_pattern_key: Option<IndirectMemoryPrefetchKeySnapshot>,
    last_candidates: Vec<IndirectMemoryPrefetchCandidate>,
}

impl IndirectMemoryPrefetcherSnapshot {
    pub const fn config(&self) -> &IndirectMemoryPrefetcherConfig {
        &self.config
    }

    pub fn prefetch_entries(&self) -> &[IndirectMemoryPrefetchEntrySnapshot] {
        &self.prefetch_entries
    }

    pub fn prefetch_lru(&self) -> &[IndirectMemoryPrefetchKeySnapshot] {
        &self.prefetch_lru
    }

    pub fn pattern_detector_entries(&self) -> &[IndirectMemoryPatternDetectorEntrySnapshot] {
        &self.pattern_detector_entries
    }

    pub fn pattern_detector_lru(&self) -> &[IndirectMemoryPrefetchKeySnapshot] {
        &self.pattern_detector_lru
    }

    pub const fn tracking_pattern_key(&self) -> Option<IndirectMemoryPrefetchKeySnapshot> {
        self.tracking_pattern_key
    }

    pub fn last_candidates(&self) -> &[IndirectMemoryPrefetchCandidate] {
        &self.last_candidates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndirectMemoryPrefetcher {
    config: IndirectMemoryPrefetcherConfig,
    prefetch_table: BTreeMap<PrefetchTableKey, PrefetchTableEntry>,
    prefetch_lru: VecDeque<PrefetchTableKey>,
    pattern_detector: BTreeMap<PrefetchTableKey, PatternDetectorEntry>,
    pattern_detector_lru: VecDeque<PrefetchTableKey>,
    tracking_pattern_key: Option<PrefetchTableKey>,
    last_candidates: Vec<IndirectMemoryPrefetchCandidate>,
}

impl IndirectMemoryPrefetcher {
    pub fn new(config: IndirectMemoryPrefetcherConfig) -> Self {
        Self {
            config,
            prefetch_table: BTreeMap::new(),
            prefetch_lru: VecDeque::new(),
            pattern_detector: BTreeMap::new(),
            pattern_detector_lru: VecDeque::new(),
            tracking_pattern_key: None,
            last_candidates: Vec::new(),
        }
    }

    pub const fn config(&self) -> &IndirectMemoryPrefetcherConfig {
        &self.config
    }

    pub fn observe(
        &mut self,
        access: IndirectMemoryPrefetchAccess,
    ) -> Result<&[IndirectMemoryPrefetchCandidate], IndirectMemoryPrefetcherError> {
        self.last_candidates.clear();
        self.check_access_match_on_active_entries(access.address());

        if let Some(key) = self.tracking_pattern_key {
            if access.cache_miss() {
                if self
                    .pattern_detector
                    .get(&key)
                    .is_some_and(|entry| entry.second_index_set)
                {
                    self.track_miss_index2(access.address(), key);
                } else {
                    self.track_miss_index1(access.address(), key);
                }
                return Ok(&self.last_candidates);
            }
        }

        let key = PrefetchTableKey {
            pc: access.pc(),
            secure: access.secure(),
        };
        if !self.prefetch_table.contains_key(&key) {
            self.insert_prefetch_entry(key, access.address());
            return Ok(&self.last_candidates);
        }

        self.touch_prefetch_lru(key);
        let previous_address = self
            .prefetch_table
            .get(&key)
            .expect("prefetch entry exists")
            .address;
        if previous_address == access.address() {
            return Ok(&self.last_candidates);
        }

        self.update_stream_state(access, key, previous_address)?;
        if let Some(index) = access.readable_index() {
            if self
                .prefetch_table
                .get(&key)
                .expect("prefetch entry exists")
                .enabled
            {
                self.update_enabled_index(access, key, index);
            } else {
                self.allocate_or_update_pattern_entry(key, index);
            }
        }

        Ok(&self.last_candidates)
    }

    pub fn snapshot(&self) -> IndirectMemoryPrefetcherSnapshot {
        IndirectMemoryPrefetcherSnapshot {
            config: self.config.clone(),
            prefetch_entries: self
                .prefetch_table
                .iter()
                .map(|(key, entry)| entry.snapshot(*key))
                .collect(),
            prefetch_lru: self
                .prefetch_lru
                .iter()
                .map(|key| IndirectMemoryPrefetchKeySnapshot {
                    pc: key.pc,
                    secure: key.secure,
                })
                .collect(),
            pattern_detector_entries: self
                .pattern_detector
                .iter()
                .map(|(key, entry)| entry.snapshot(*key))
                .collect(),
            pattern_detector_lru: self
                .pattern_detector_lru
                .iter()
                .map(|key| IndirectMemoryPrefetchKeySnapshot {
                    pc: key.pc,
                    secure: key.secure,
                })
                .collect(),
            tracking_pattern_key: self.tracking_pattern_key.map(|key| {
                IndirectMemoryPrefetchKeySnapshot {
                    pc: key.pc,
                    secure: key.secure,
                }
            }),
            last_candidates: self.last_candidates.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &IndirectMemoryPrefetcherSnapshot,
    ) -> Result<(), IndirectMemoryPrefetcherError> {
        self.validate_snapshot(snapshot)?;
        self.prefetch_table = snapshot
            .prefetch_entries()
            .iter()
            .map(|entry| {
                (
                    PrefetchTableKey {
                        pc: entry.pc(),
                        secure: entry.secure(),
                    },
                    PrefetchTableEntry::from_snapshot(entry),
                )
            })
            .collect();
        self.prefetch_lru = snapshot.prefetch_lru().iter().map(snapshot_key).collect();
        self.pattern_detector = snapshot
            .pattern_detector_entries()
            .iter()
            .map(|entry| {
                (
                    PrefetchTableKey {
                        pc: entry.pc(),
                        secure: entry.secure(),
                    },
                    PatternDetectorEntry::from_snapshot(entry),
                )
            })
            .collect();
        self.pattern_detector_lru = snapshot
            .pattern_detector_lru()
            .iter()
            .map(snapshot_key)
            .collect();
        self.tracking_pattern_key = snapshot.tracking_pattern_key().map(|key| PrefetchTableKey {
            pc: key.pc(),
            secure: key.secure(),
        });
        self.last_candidates = snapshot.last_candidates().to_vec();
        Ok(())
    }

    pub fn prefetch_table_entry_count(&self) -> usize {
        self.prefetch_table.len()
    }

    pub fn pattern_detector_entry_count(&self) -> usize {
        self.pattern_detector.len()
    }

    pub fn prefetch_table_contains(&self, pc: u64, secure: bool) -> bool {
        self.prefetch_table
            .contains_key(&PrefetchTableKey { pc, secure })
    }

    pub fn tracking_pattern_key(&self) -> Option<(u64, bool)> {
        self.tracking_pattern_key.map(|key| (key.pc, key.secure))
    }

    pub fn indirect_mapping(&self, pc: u64, secure: bool) -> Option<(Address, i32)> {
        let entry = self.prefetch_table.get(&PrefetchTableKey { pc, secure })?;
        entry.enabled.then_some((entry.base_address, entry.shift))
    }

    pub fn indirect_counter(&self, pc: u64, secure: bool) -> Option<u8> {
        self.prefetch_table
            .get(&PrefetchTableKey { pc, secure })
            .map(|entry| entry.indirect_counter)
    }

    pub fn last_candidates(&self) -> &[IndirectMemoryPrefetchCandidate] {
        &self.last_candidates
    }

    fn validate_snapshot(
        &self,
        snapshot: &IndirectMemoryPrefetcherSnapshot,
    ) -> Result<(), IndirectMemoryPrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(IndirectMemoryPrefetcherError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config().clone()),
            });
        }
        if snapshot.prefetch_entries().len() > self.config.prefetch_table_entries() {
            return Err(
                IndirectMemoryPrefetcherError::SnapshotPrefetchTableTooLarge {
                    entries: snapshot.prefetch_entries().len(),
                    max_entries: self.config.prefetch_table_entries(),
                },
            );
        }
        if snapshot.pattern_detector_entries().len() > self.config.pattern_detector_entries() {
            return Err(
                IndirectMemoryPrefetcherError::SnapshotPatternDetectorTooLarge {
                    entries: snapshot.pattern_detector_entries().len(),
                    max_entries: self.config.pattern_detector_entries(),
                },
            );
        }
        for entry in snapshot.prefetch_entries() {
            if entry.indirect_counter() > self.config.max_indirect_counter() {
                return Err(
                    IndirectMemoryPrefetcherError::SnapshotIndirectCounterOutOfRange {
                        pc: entry.pc(),
                        secure: entry.secure(),
                        counter: entry.indirect_counter(),
                        max_counter: self.config.max_indirect_counter(),
                    },
                );
            }
        }
        self.validate_pattern_shapes(snapshot)?;
        self.validate_lru(snapshot.prefetch_lru(), snapshot.prefetch_entries(), true)?;
        self.validate_lru(
            snapshot.pattern_detector_lru(),
            snapshot.pattern_detector_entries(),
            false,
        )?;
        self.validate_pattern_references(snapshot)?;
        Ok(())
    }

    fn validate_pattern_shapes(
        &self,
        snapshot: &IndirectMemoryPrefetcherSnapshot,
    ) -> Result<(), IndirectMemoryPrefetcherError> {
        for entry in snapshot.pattern_detector_entries() {
            if entry.base_candidates().len() != self.config.address_array_len() {
                return Err(
                    IndirectMemoryPrefetcherError::SnapshotPatternDetectorShapeMismatch {
                        pc: entry.pc(),
                        secure: entry.secure(),
                        rows: entry.base_candidates().len(),
                        expected_rows: self.config.address_array_len(),
                    },
                );
            }
            for (row, candidates) in entry.base_candidates().iter().enumerate() {
                if candidates.len() != self.config.shift_values().len() {
                    return Err(
                        IndirectMemoryPrefetcherError::SnapshotPatternDetectorShiftShapeMismatch {
                            pc: entry.pc(),
                            secure: entry.secure(),
                            row,
                            columns: candidates.len(),
                            expected_columns: self.config.shift_values().len(),
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_lru<P>(
        &self,
        lru: &[IndirectMemoryPrefetchKeySnapshot],
        entries: &[P],
        prefetch: bool,
    ) -> Result<(), IndirectMemoryPrefetcherError>
    where
        P: SnapshotKey,
    {
        if lru.len() != entries.len() {
            if prefetch {
                return Err(
                    IndirectMemoryPrefetcherError::SnapshotPrefetchLruShapeMismatch {
                        entries: lru.len(),
                        table_entries: entries.len(),
                    },
                );
            }
            return Err(
                IndirectMemoryPrefetcherError::SnapshotPatternLruShapeMismatch {
                    entries: lru.len(),
                    table_entries: entries.len(),
                },
            );
        }
        let keys = entries
            .iter()
            .map(SnapshotKey::key)
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        for key in lru {
            let lru_key = snapshot_key(key);
            if keys.contains(&lru_key) && seen.insert(lru_key) {
                continue;
            }
            if prefetch {
                return Err(
                    IndirectMemoryPrefetcherError::SnapshotPrefetchLruUnknownKey {
                        pc: key.pc(),
                        secure: key.secure(),
                    },
                );
            }
            return Err(
                IndirectMemoryPrefetcherError::SnapshotPatternLruUnknownKey {
                    pc: key.pc(),
                    secure: key.secure(),
                },
            );
        }
        Ok(())
    }

    fn validate_pattern_references(
        &self,
        snapshot: &IndirectMemoryPrefetcherSnapshot,
    ) -> Result<(), IndirectMemoryPrefetcherError> {
        let prefetch_keys = snapshot
            .prefetch_entries()
            .iter()
            .map(SnapshotKey::key)
            .collect::<BTreeSet<_>>();
        let pattern_keys = snapshot
            .pattern_detector_entries()
            .iter()
            .map(SnapshotKey::key)
            .collect::<BTreeSet<_>>();
        for key in &pattern_keys {
            if !prefetch_keys.contains(key) {
                return Err(
                    IndirectMemoryPrefetcherError::SnapshotPatternReferencesMissingPrefetchEntry {
                        pc: key.pc,
                        secure: key.secure,
                    },
                );
            }
        }
        if let Some(key) = snapshot.tracking_pattern_key().map(|key| PrefetchTableKey {
            pc: key.pc(),
            secure: key.secure(),
        }) {
            if !pattern_keys.contains(&key) {
                return Err(
                    IndirectMemoryPrefetcherError::SnapshotTrackingPatternMissing {
                        pc: key.pc,
                        secure: key.secure,
                    },
                );
            }
        }
        Ok(())
    }

    fn update_stream_state(
        &mut self,
        access: IndirectMemoryPrefetchAccess,
        key: PrefetchTableKey,
        previous_address: Address,
    ) -> Result<(), IndirectMemoryPrefetcherError> {
        let delta = access.address().get() as i128 - previous_address.get() as i128;
        let stream_counter = self
            .prefetch_table
            .get_mut(&key)
            .expect("prefetch entry exists")
            .stream_counter
            .saturating_add(1);
        self.prefetch_table
            .get_mut(&key)
            .expect("prefetch entry exists")
            .stream_counter = stream_counter;

        if stream_counter >= self.config.stream_counter_threshold() {
            for degree in 1..=self.config.streaming_distance() {
                let address = stream_address(access.address(), delta, degree)?;
                self.last_candidates.push(IndirectMemoryPrefetchCandidate {
                    address,
                    source_address: access.address(),
                    context: access.context(),
                    pc: access.pc(),
                    secure: access.secure(),
                    kind: IndirectMemoryPrefetchKind::Stream,
                    base_address: Address::new(0),
                    index: 0,
                    shift: 0,
                    stream_delta: clamp_i128_to_i64(delta),
                    indirect_counter: 0,
                    stride: byte_stride(access.address(), address),
                    degree_index: degree,
                });
            }
        }
        self.prefetch_table
            .get_mut(&key)
            .expect("prefetch entry exists")
            .address = access.address();
        Ok(())
    }

    fn update_enabled_index(
        &mut self,
        access: IndirectMemoryPrefetchAccess,
        key: PrefetchTableKey,
        index: i64,
    ) {
        let entry = self
            .prefetch_table
            .get_mut(&key)
            .expect("prefetch entry exists");
        entry.index = index;
        if entry.increased_indirect_counter {
            entry.increased_indirect_counter = false;
        } else {
            entry.indirect_counter = entry.indirect_counter.saturating_sub(1);
        }
        let entry = self
            .prefetch_table
            .get(&key)
            .expect("prefetch entry exists")
            .clone();
        if entry.indirect_counter <= self.config.prefetch_threshold() {
            return;
        }
        let distance = self.prefetch_distance(entry.indirect_counter);
        for degree in 1..distance {
            let Some(address) = indirect_address(entry.base_address, entry.index, entry.shift)
            else {
                continue;
            };
            self.last_candidates.push(IndirectMemoryPrefetchCandidate {
                address,
                source_address: access.address(),
                context: access.context(),
                pc: access.pc(),
                secure: access.secure(),
                kind: IndirectMemoryPrefetchKind::Indirect,
                base_address: entry.base_address,
                index: entry.index,
                shift: entry.shift,
                stream_delta: 0,
                indirect_counter: entry.indirect_counter,
                stride: byte_stride(access.address(), address),
                degree_index: degree,
            });
        }
    }

    fn prefetch_distance(&self, counter: u8) -> u32 {
        self.config
            .max_prefetch_distance()
            .saturating_mul(counter as u32)
            / self.config.max_indirect_counter() as u32
    }

    fn allocate_or_update_pattern_entry(&mut self, key: PrefetchTableKey, index: i64) {
        if let Some(entry) = self.pattern_detector.get_mut(&key) {
            if !entry.second_index_set {
                entry.idx2 = index;
                entry.second_index_set = true;
                self.tracking_pattern_key = Some(key);
                self.touch_pattern_detector_lru(key);
            } else {
                self.pattern_detector.remove(&key);
                self.pattern_detector_lru.retain(|entry| *entry != key);
                if self.tracking_pattern_key == Some(key) {
                    self.tracking_pattern_key = None;
                }
            }
            return;
        }
        self.insert_pattern_entry(key, index);
        self.tracking_pattern_key = Some(key);
    }

    fn track_miss_index1(&mut self, address: Address, key: PrefetchTableKey) {
        let Some(entry) = self.pattern_detector.get_mut(&key) else {
            self.tracking_pattern_key = None;
            return;
        };
        if entry.num_misses >= self.config.address_array_len() {
            self.tracking_pattern_key = None;
            return;
        }
        for (index, shift) in self.config.shift_values().iter().copied().enumerate() {
            entry.base_candidates[entry.num_misses][index] =
                base_address_from_miss(address, entry.idx1, shift);
        }
        entry.num_misses = entry.num_misses.saturating_add(1);
        if entry.num_misses >= self.config.address_array_len() {
            self.tracking_pattern_key = None;
        }
    }

    fn track_miss_index2(&mut self, address: Address, key: PrefetchTableKey) {
        let Some(entry) = self.pattern_detector.get(&key).cloned() else {
            self.tracking_pattern_key = None;
            return;
        };
        for miss_index in 0..entry.num_misses {
            for (shift_index, shift) in self.config.shift_values().iter().copied().enumerate() {
                let candidate = entry.base_candidates[miss_index][shift_index];
                if candidate == base_address_from_miss(address, entry.idx2, shift) {
                    if let Some(base_address) = candidate {
                        if let Some(prefetch_entry) = self.prefetch_table.get_mut(&key) {
                            prefetch_entry.base_address = base_address;
                            prefetch_entry.shift = shift;
                            prefetch_entry.enabled = true;
                            prefetch_entry.indirect_counter = 0;
                            prefetch_entry.increased_indirect_counter = false;
                        }
                        self.pattern_detector.remove(&key);
                        self.pattern_detector_lru.retain(|entry| *entry != key);
                        self.tracking_pattern_key = None;
                        return;
                    }
                }
            }
        }
    }

    fn check_access_match_on_active_entries(&mut self, address: Address) {
        for entry in self.prefetch_table.values_mut() {
            if !entry.enabled {
                continue;
            }
            if indirect_address(entry.base_address, entry.index, entry.shift) == Some(address) {
                entry.indirect_counter = entry
                    .indirect_counter
                    .saturating_add(1)
                    .min(self.config.max_indirect_counter());
                entry.increased_indirect_counter = true;
            }
        }
    }

    fn insert_prefetch_entry(&mut self, key: PrefetchTableKey, address: Address) {
        while self.prefetch_table.len() >= self.config.prefetch_table_entries() {
            let Some(victim) = self.prefetch_lru.pop_back() else {
                break;
            };
            self.prefetch_table.remove(&victim);
            self.pattern_detector.remove(&victim);
            self.pattern_detector_lru.retain(|entry| *entry != victim);
            if self.tracking_pattern_key == Some(victim) {
                self.tracking_pattern_key = None;
            }
        }
        self.prefetch_table
            .insert(key, PrefetchTableEntry::new(address));
        self.touch_prefetch_lru(key);
    }

    fn insert_pattern_entry(&mut self, key: PrefetchTableKey, idx1: i64) {
        while self.pattern_detector.len() >= self.config.pattern_detector_entries() {
            let Some(victim) = self.pattern_detector_lru.pop_back() else {
                break;
            };
            self.pattern_detector.remove(&victim);
            if self.tracking_pattern_key == Some(victim) {
                self.tracking_pattern_key = None;
            }
        }
        self.pattern_detector.insert(
            key,
            PatternDetectorEntry::new(
                self.config.address_array_len(),
                self.config.shift_values().len(),
                idx1,
            ),
        );
        self.touch_pattern_detector_lru(key);
    }

    fn touch_prefetch_lru(&mut self, key: PrefetchTableKey) {
        self.prefetch_lru.retain(|entry| *entry != key);
        self.prefetch_lru.push_front(key);
    }

    fn touch_pattern_detector_lru(&mut self, key: PrefetchTableKey) {
        self.pattern_detector_lru.retain(|entry| *entry != key);
        self.pattern_detector_lru.push_front(key);
    }
}

trait SnapshotKey {
    fn key(&self) -> PrefetchTableKey;
}

impl SnapshotKey for IndirectMemoryPrefetchEntrySnapshot {
    fn key(&self) -> PrefetchTableKey {
        PrefetchTableKey {
            pc: self.pc(),
            secure: self.secure(),
        }
    }
}

impl SnapshotKey for IndirectMemoryPatternDetectorEntrySnapshot {
    fn key(&self) -> PrefetchTableKey {
        PrefetchTableKey {
            pc: self.pc(),
            secure: self.secure(),
        }
    }
}

fn snapshot_key(key: &IndirectMemoryPrefetchKeySnapshot) -> PrefetchTableKey {
    PrefetchTableKey {
        pc: key.pc(),
        secure: key.secure(),
    }
}

fn shift_index(index: i64, shift: i32) -> Option<i128> {
    if shift >= 0 {
        (index as i128).checked_shl(shift as u32)
    } else {
        Some((index >> (-shift as u32)) as i128)
    }
}

fn base_address_from_miss(address: Address, index: i64, shift: i32) -> Option<Address> {
    let shifted = shift_index(index, shift)?;
    let base = address.get() as i128 - shifted;
    u64::try_from(base).ok().map(Address::new)
}

fn indirect_address(base_address: Address, index: i64, shift: i32) -> Option<Address> {
    let shifted = shift_index(index, shift)?;
    let address = base_address.get() as i128 + shifted;
    u64::try_from(address).ok().map(Address::new)
}

fn stream_address(
    address: Address,
    delta: i128,
    degree: u32,
) -> Result<Address, IndirectMemoryPrefetcherError> {
    let candidate = address.get() as i128 + delta.saturating_mul(degree as i128);
    let candidate = u64::try_from(candidate).map_err(|_| {
        IndirectMemoryPrefetcherError::StreamAddressOverflow {
            address,
            delta,
            degree,
        }
    })?;
    Ok(Address::new(candidate))
}

fn byte_stride(source_address: Address, candidate_address: Address) -> i64 {
    let stride = candidate_address.get() as i128 - source_address.get() as i128;
    clamp_i128_to_i64(stride)
}

fn clamp_i128_to_i64(value: i128) -> i64 {
    value.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

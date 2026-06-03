use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::allocation::max_vector_len;
use crate::prefetch::PrefetchCandidate;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmpmRatio {
    numerator: u64,
    denominator: u64,
}

impl AmpmRatio {
    pub const fn new(numerator: u64, denominator: u64) -> Result<Self, AmpmPrefetcherError> {
        if denominator == 0 {
            return Err(AmpmPrefetcherError::ZeroRatioDenominator);
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    pub const fn numerator(&self) -> u64 {
        self.numerator
    }

    pub const fn denominator(&self) -> u64 {
        self.denominator
    }

    fn count_ratio_exceeds(&self, numerator: u64, denominator: u64) -> bool {
        denominator != 0
            && (numerator as u128) * (self.denominator as u128)
                > (self.numerator as u128) * (denominator as u128)
    }

    fn count_ratio_below(&self, numerator: u64, denominator: u64) -> bool {
        denominator != 0
            && (numerator as u128) * (self.denominator as u128)
                < (self.numerator as u128) * (denominator as u128)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmpmEpochConfig {
    high_coverage_threshold: AmpmRatio,
    low_coverage_threshold: AmpmRatio,
    high_accuracy_threshold: AmpmRatio,
    low_accuracy_threshold: AmpmRatio,
    high_cache_hit_threshold: AmpmRatio,
    low_cache_hit_threshold: AmpmRatio,
    epoch_cycles: u64,
    offchip_memory_latency_cycles: u64,
}

impl AmpmEpochConfig {
    pub fn gem5_defaults(
        epoch_cycles: u64,
        offchip_memory_latency_cycles: u64,
    ) -> Result<Self, AmpmPrefetcherError> {
        if epoch_cycles == 0 {
            return Err(AmpmPrefetcherError::ZeroEpochCycles);
        }
        if offchip_memory_latency_cycles == 0 {
            return Err(AmpmPrefetcherError::ZeroOffchipMemoryLatency);
        }
        Ok(Self {
            high_coverage_threshold: AmpmRatio::new(1, 4)?,
            low_coverage_threshold: AmpmRatio::new(1, 8)?,
            high_accuracy_threshold: AmpmRatio::new(1, 2)?,
            low_accuracy_threshold: AmpmRatio::new(1, 4)?,
            high_cache_hit_threshold: AmpmRatio::new(7, 8)?,
            low_cache_hit_threshold: AmpmRatio::new(3, 4)?,
            epoch_cycles,
            offchip_memory_latency_cycles,
        })
    }

    pub const fn high_coverage_threshold(&self) -> AmpmRatio {
        self.high_coverage_threshold
    }

    pub const fn low_coverage_threshold(&self) -> AmpmRatio {
        self.low_coverage_threshold
    }

    pub const fn high_accuracy_threshold(&self) -> AmpmRatio {
        self.high_accuracy_threshold
    }

    pub const fn low_accuracy_threshold(&self) -> AmpmRatio {
        self.low_accuracy_threshold
    }

    pub const fn high_cache_hit_threshold(&self) -> AmpmRatio {
        self.high_cache_hit_threshold
    }

    pub const fn low_cache_hit_threshold(&self) -> AmpmRatio {
        self.low_cache_hit_threshold
    }

    pub const fn epoch_cycles(&self) -> u64 {
        self.epoch_cycles
    }

    pub const fn offchip_memory_latency_cycles(&self) -> u64 {
        self.offchip_memory_latency_cycles
    }

    fn memory_bandwidth_degree(&self, stats: AmpmEpochStats) -> u32 {
        let requests = stats
            .raw_cache_misses()
            .saturating_sub(stats.useful_prefetches())
            .saturating_add(stats.issued_prefetches());
        let degree = (requests as u128) * (self.offchip_memory_latency_cycles as u128)
            / (self.epoch_cycles as u128);
        degree.min(u32::MAX as u128) as u32
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmpmPrefetcherConfig {
    line_size: u64,
    hot_zone_size: u64,
    degree: u32,
    table_entries: usize,
    table_assoc: usize,
    limit_stride: Option<u64>,
    epoch_control: Option<AmpmEpochConfig>,
}

impl AmpmPrefetcherConfig {
    pub fn new(
        line_size: u64,
        hot_zone_size: u64,
        degree: u32,
        table_entries: usize,
    ) -> Result<Self, AmpmPrefetcherError> {
        if line_size == 0 {
            return Err(AmpmPrefetcherError::ZeroLineSize);
        }
        if hot_zone_size == 0 {
            return Err(AmpmPrefetcherError::ZeroHotZoneSize);
        }
        if !hot_zone_size.is_power_of_two() {
            return Err(AmpmPrefetcherError::HotZoneNotPowerOfTwo { hot_zone_size });
        }
        if !hot_zone_size.is_multiple_of(line_size) {
            return Err(AmpmPrefetcherError::HotZoneLineMismatch {
                hot_zone_size,
                line_size,
            });
        }
        if hot_zone_size / line_size < 2 {
            return Err(AmpmPrefetcherError::HotZoneTooSmall {
                hot_zone_size,
                line_size,
            });
        }
        let hot_zone_lines = usize::try_from(hot_zone_size / line_size).unwrap_or(usize::MAX);
        validate_ampm_vector_length(
            "hot zone lines",
            hot_zone_lines,
            maximum_ampm_hot_zone_lines(),
        )?;
        if degree == 0 {
            return Err(AmpmPrefetcherError::ZeroDegree);
        }
        if table_entries == 0 {
            return Err(AmpmPrefetcherError::ZeroTableEntries);
        }
        if table_entries < 3 {
            return Err(AmpmPrefetcherError::TableTooSmall { table_entries });
        }
        validate_ampm_vector_length("table entries", table_entries, maximum_ampm_table_entries())?;

        Ok(Self {
            line_size,
            hot_zone_size,
            degree,
            table_entries,
            table_assoc: table_entries,
            limit_stride: None,
            epoch_control: None,
        })
    }

    pub const fn line_size(&self) -> u64 {
        self.line_size
    }

    pub const fn hot_zone_size(&self) -> u64 {
        self.hot_zone_size
    }

    pub const fn degree(&self) -> u32 {
        self.degree
    }

    pub const fn table_entries(&self) -> usize {
        self.table_entries
    }

    pub const fn table_assoc(&self) -> usize {
        self.table_assoc
    }

    pub const fn table_sets(&self) -> usize {
        self.table_entries / self.table_assoc
    }

    pub const fn limit_stride(&self) -> Option<u64> {
        self.limit_stride
    }

    pub const fn epoch_control(&self) -> Option<AmpmEpochConfig> {
        self.epoch_control
    }

    pub const fn with_limit_stride(
        mut self,
        limit_stride: u64,
    ) -> Result<Self, AmpmPrefetcherError> {
        if limit_stride == 0 {
            return Err(AmpmPrefetcherError::ZeroLimitStride);
        }
        self.limit_stride = Some(limit_stride);
        Ok(self)
    }

    pub fn with_table_assoc(mut self, table_assoc: usize) -> Result<Self, AmpmPrefetcherError> {
        validate_table_assoc(self.table_entries, table_assoc)?;
        self.table_assoc = table_assoc;
        Ok(self)
    }

    pub const fn with_epoch_control(mut self, epoch_control: AmpmEpochConfig) -> Self {
        self.epoch_control = Some(epoch_control);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AmpmPrefetcherError {
    ZeroLineSize,
    ZeroHotZoneSize,
    ZeroDegree,
    ZeroTableEntries,
    ZeroTableAssoc,
    ZeroLimitStride,
    ZeroEpochCycles,
    ZeroOffchipMemoryLatency,
    ZeroRatioDenominator,
    EpochControlDisabled,
    HotZoneNotPowerOfTwo {
        hot_zone_size: u64,
    },
    HotZoneLineMismatch {
        hot_zone_size: u64,
        line_size: u64,
    },
    HotZoneTooSmall {
        hot_zone_size: u64,
        line_size: u64,
    },
    TableTooSmall {
        table_entries: usize,
    },
    TableAssocExceedsEntries {
        table_entries: usize,
        table_assoc: usize,
    },
    TableEntriesNotMultipleOfAssoc {
        table_entries: usize,
        table_assoc: usize,
    },
    TableSetCountNotPowerOfTwo {
        table_entries: usize,
        table_assoc: usize,
        table_sets: usize,
    },
    VectorLengthTooLarge {
        field: &'static str,
        length: usize,
        maximum: usize,
    },
    SnapshotConfigMismatch {
        expected: Box<AmpmPrefetcherConfig>,
        actual: Box<AmpmPrefetcherConfig>,
    },
    SnapshotEntryCountOutOfRange {
        entries: usize,
        table_entries: usize,
    },
    SnapshotEntryShapeMismatch {
        zone: u64,
        states: usize,
        expected: usize,
    },
    SnapshotSetEntryCountOutOfRange {
        set: usize,
        entries: usize,
        table_assoc: usize,
    },
}

impl fmt::Display for AmpmPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLineSize => write!(formatter, "AMPM prefetcher line size is zero"),
            Self::ZeroHotZoneSize => write!(formatter, "AMPM prefetcher hot zone size is zero"),
            Self::ZeroDegree => write!(formatter, "AMPM prefetcher degree is zero"),
            Self::ZeroTableEntries => write!(formatter, "AMPM prefetcher table has no entries"),
            Self::ZeroTableAssoc => {
                write!(formatter, "AMPM prefetcher table associativity is zero")
            }
            Self::ZeroLimitStride => write!(formatter, "AMPM prefetcher stride limit is zero"),
            Self::ZeroEpochCycles => write!(formatter, "AMPM prefetcher epoch cycle count is zero"),
            Self::ZeroOffchipMemoryLatency => write!(
                formatter,
                "AMPM prefetcher offchip memory latency is zero"
            ),
            Self::ZeroRatioDenominator => {
                write!(formatter, "AMPM prefetcher ratio denominator is zero")
            }
            Self::EpochControlDisabled => {
                write!(formatter, "AMPM prefetcher epoch control is disabled")
            }
            Self::HotZoneNotPowerOfTwo { hot_zone_size } => write!(
                formatter,
                "AMPM prefetcher hot zone size {hot_zone_size} is not a power of two"
            ),
            Self::HotZoneLineMismatch {
                hot_zone_size,
                line_size,
            } => write!(
                formatter,
                "AMPM prefetcher hot zone size {hot_zone_size} is not a multiple of line size {line_size}"
            ),
            Self::HotZoneTooSmall {
                hot_zone_size,
                line_size,
            } => write!(
                formatter,
                "AMPM prefetcher hot zone size {hot_zone_size} has fewer than two {line_size}-byte lines"
            ),
            Self::TableTooSmall { table_entries } => write!(
                formatter,
                "AMPM prefetcher table has {table_entries} entries but needs at least three"
            ),
            Self::TableAssocExceedsEntries {
                table_entries,
                table_assoc,
            } => write!(
                formatter,
                "AMPM prefetcher table associativity {table_assoc} exceeds {table_entries} entries"
            ),
            Self::TableEntriesNotMultipleOfAssoc {
                table_entries,
                table_assoc,
            } => write!(
                formatter,
                "AMPM prefetcher table entries {table_entries} are not a multiple of associativity {table_assoc}"
            ),
            Self::TableSetCountNotPowerOfTwo {
                table_entries,
                table_assoc,
                table_sets,
            } => write!(
                formatter,
                "AMPM prefetcher table with {table_entries} entries and associativity {table_assoc} has {table_sets} non-power-of-two sets"
            ),
            Self::VectorLengthTooLarge {
                field,
                length,
                maximum,
            } => write!(
                formatter,
                "AMPM prefetcher {field} length {length} exceeds maximum {maximum}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "AMPM prefetcher snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotEntryCountOutOfRange {
                entries,
                table_entries,
            } => write!(
                formatter,
                "AMPM prefetcher snapshot has {entries} entries for {table_entries} slots"
            ),
            Self::SnapshotEntryShapeMismatch {
                zone,
                states,
                expected,
            } => write!(
                formatter,
                "AMPM prefetcher snapshot zone {zone} has {states} states instead of {expected}"
            ),
            Self::SnapshotSetEntryCountOutOfRange {
                set,
                entries,
                table_assoc,
            } => write!(
                formatter,
                "AMPM prefetcher snapshot set {set} has {entries} entries for associativity {table_assoc}"
            ),
        }
    }
}

impl Error for AmpmPrefetcherError {}

fn maximum_ampm_hot_zone_lines() -> usize {
    max_vector_len::<AmpmAccessMapState>()
        .min(max_vector_len::<AmpmWindowState>() / 3)
        .min(usize::MAX / 3)
}

fn maximum_ampm_table_entries() -> usize {
    max_vector_len::<AmpmZoneKey>().min(max_vector_len::<AmpmAccessMapEntrySnapshot>())
}

fn validate_ampm_vector_length(
    field: &'static str,
    length: usize,
    maximum: usize,
) -> Result<(), AmpmPrefetcherError> {
    if length > maximum {
        return Err(AmpmPrefetcherError::VectorLengthTooLarge {
            field,
            length,
            maximum,
        });
    }
    Ok(())
}

fn validate_table_assoc(
    table_entries: usize,
    table_assoc: usize,
) -> Result<(), AmpmPrefetcherError> {
    if table_assoc == 0 {
        return Err(AmpmPrefetcherError::ZeroTableAssoc);
    }
    if table_assoc > table_entries {
        return Err(AmpmPrefetcherError::TableAssocExceedsEntries {
            table_entries,
            table_assoc,
        });
    }
    if !table_entries.is_multiple_of(table_assoc) {
        return Err(AmpmPrefetcherError::TableEntriesNotMultipleOfAssoc {
            table_entries,
            table_assoc,
        });
    }
    let table_sets = table_entries / table_assoc;
    if !table_sets.is_power_of_two() {
        return Err(AmpmPrefetcherError::TableSetCountNotPowerOfTwo {
            table_entries,
            table_assoc,
            table_sets,
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmpmPrefetchAccess {
    requestor: AgentId,
    pc: u64,
    address: Address,
    secure: bool,
}

impl AmpmPrefetchAccess {
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
pub struct AmpmPrefetchCandidate {
    address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    stride: i64,
    degree_index: u32,
}

impl AmpmPrefetchCandidate {
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

impl PrefetchCandidate for AmpmPrefetchCandidate {
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
pub enum AmpmAccessMapState {
    Init,
    Prefetch,
    Access,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AmpmEpochStats {
    issued_prefetches: u64,
    useful_prefetches: u64,
    raw_cache_misses: u64,
    raw_cache_hits: u64,
}

impl AmpmEpochStats {
    pub const fn issued_prefetches(&self) -> u64 {
        self.issued_prefetches
    }

    pub const fn useful_prefetches(&self) -> u64 {
        self.useful_prefetches
    }

    pub const fn raw_cache_misses(&self) -> u64 {
        self.raw_cache_misses
    }

    pub const fn raw_cache_hits(&self) -> u64 {
        self.raw_cache_hits
    }

    const fn cache_accesses(&self) -> u64 {
        self.raw_cache_hits.saturating_add(self.raw_cache_misses)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmpmEpochReport {
    stats: AmpmEpochStats,
    previous_degree: u32,
    previous_useful_degree: u32,
    next_degree: u32,
    next_useful_degree: u32,
    memory_bandwidth_degree: u32,
}

impl AmpmEpochReport {
    pub const fn stats(&self) -> &AmpmEpochStats {
        &self.stats
    }

    pub const fn previous_degree(&self) -> u32 {
        self.previous_degree
    }

    pub const fn previous_useful_degree(&self) -> u32 {
        self.previous_useful_degree
    }

    pub const fn next_degree(&self) -> u32 {
        self.next_degree
    }

    pub const fn next_useful_degree(&self) -> u32 {
        self.next_useful_degree
    }

    pub const fn memory_bandwidth_degree(&self) -> u32 {
        self.memory_bandwidth_degree
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmpmAccessMapEntrySnapshot {
    zone: u64,
    secure: bool,
    states: Vec<AmpmAccessMapState>,
}

impl AmpmAccessMapEntrySnapshot {
    pub const fn zone(&self) -> u64 {
        self.zone
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub fn states(&self) -> &[AmpmAccessMapState] {
        &self.states
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmpmPrefetcherSnapshot {
    config: AmpmPrefetcherConfig,
    entries: Vec<AmpmAccessMapEntrySnapshot>,
    lru_order: Vec<AmpmZoneKey>,
    last_candidates: Vec<AmpmPrefetchCandidate>,
    current_degree: u32,
    useful_degree: u32,
    issued_prefetches: u64,
    useful_prefetches: u64,
    raw_cache_misses: u64,
    raw_cache_hits: u64,
    epoch_stats: AmpmEpochStats,
    last_epoch_report: Option<AmpmEpochReport>,
}

impl AmpmPrefetcherSnapshot {
    pub const fn config(&self) -> &AmpmPrefetcherConfig {
        &self.config
    }

    pub fn entries(&self) -> &[AmpmAccessMapEntrySnapshot] {
        &self.entries
    }

    pub fn last_candidates(&self) -> &[AmpmPrefetchCandidate] {
        &self.last_candidates
    }

    pub const fn current_degree(&self) -> u32 {
        self.current_degree
    }

    pub const fn useful_degree(&self) -> u32 {
        self.useful_degree
    }

    pub const fn epoch_stats(&self) -> &AmpmEpochStats {
        &self.epoch_stats
    }

    pub const fn last_epoch_report(&self) -> Option<&AmpmEpochReport> {
        self.last_epoch_report.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct AmpmZoneKey {
    zone: u64,
    secure: bool,
}

impl AmpmZoneKey {
    const fn new(zone: u64, secure: bool) -> Self {
        Self { zone, secure }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AmpmAccessMapEntry {
    states: Vec<AmpmAccessMapState>,
}

impl AmpmAccessMapEntry {
    fn new(lines_per_zone: usize) -> Self {
        Self {
            states: vec![AmpmAccessMapState::Init; lines_per_zone],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AmpmWindowState {
    Init,
    Prefetch,
    Access,
    Invalid,
}

impl From<AmpmAccessMapState> for AmpmWindowState {
    fn from(state: AmpmAccessMapState) -> Self {
        match state {
            AmpmAccessMapState::Init => Self::Init,
            AmpmAccessMapState::Prefetch => Self::Prefetch,
            AmpmAccessMapState::Access => Self::Access,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmpmPrefetcher {
    config: AmpmPrefetcherConfig,
    entries: BTreeMap<AmpmZoneKey, AmpmAccessMapEntry>,
    lru_order: Vec<AmpmZoneKey>,
    last_candidates: Vec<AmpmPrefetchCandidate>,
    current_degree: u32,
    useful_degree: u32,
    issued_prefetches: u64,
    useful_prefetches: u64,
    raw_cache_misses: u64,
    raw_cache_hits: u64,
    epoch_stats: AmpmEpochStats,
    last_epoch_report: Option<AmpmEpochReport>,
}

impl AmpmPrefetcher {
    pub fn new(config: AmpmPrefetcherConfig) -> Self {
        let current_degree = config.degree();
        Self {
            config,
            entries: BTreeMap::new(),
            lru_order: Vec::new(),
            last_candidates: Vec::new(),
            current_degree,
            useful_degree: current_degree,
            issued_prefetches: 0,
            useful_prefetches: 0,
            raw_cache_misses: 0,
            raw_cache_hits: 0,
            epoch_stats: AmpmEpochStats::default(),
            last_epoch_report: None,
        }
    }

    pub const fn config(&self) -> &AmpmPrefetcherConfig {
        &self.config
    }

    pub fn zone_count(&self) -> usize {
        self.entries.len()
    }

    pub fn last_candidates(&self) -> &[AmpmPrefetchCandidate] {
        &self.last_candidates
    }

    pub const fn issued_prefetch_count(&self) -> u64 {
        self.issued_prefetches
    }

    pub const fn useful_prefetch_count(&self) -> u64 {
        self.useful_prefetches
    }

    pub const fn raw_cache_miss_count(&self) -> u64 {
        self.raw_cache_misses
    }

    pub const fn raw_cache_hit_count(&self) -> u64 {
        self.raw_cache_hits
    }

    pub const fn current_degree(&self) -> u32 {
        self.current_degree
    }

    pub const fn useful_degree(&self) -> u32 {
        self.useful_degree
    }

    pub const fn epoch_issued_prefetch_count(&self) -> u64 {
        self.epoch_stats.issued_prefetches()
    }

    pub const fn epoch_useful_prefetch_count(&self) -> u64 {
        self.epoch_stats.useful_prefetches()
    }

    pub const fn epoch_raw_cache_miss_count(&self) -> u64 {
        self.epoch_stats.raw_cache_misses()
    }

    pub const fn epoch_raw_cache_hit_count(&self) -> u64 {
        self.epoch_stats.raw_cache_hits()
    }

    pub const fn last_epoch_report(&self) -> Option<&AmpmEpochReport> {
        self.last_epoch_report.as_ref()
    }

    pub fn observe(
        &mut self,
        access: AmpmPrefetchAccess,
    ) -> Result<&[AmpmPrefetchCandidate], AmpmPrefetcherError> {
        self.last_candidates.clear();
        let zone = access.address().get() / self.config.hot_zone_size();
        let block =
            (access.address().get() % self.config.hot_zone_size()) / self.config.line_size();
        self.ensure_neighbor_zones(zone, access.secure());
        self.set_entry_state(zone, access.secure(), block, AmpmAccessMapState::Access);

        if self.current_degree() == 0 {
            return Ok(&self.last_candidates);
        }

        let states = self.window_states(zone, access.secure());
        let lines_per_zone = self.lines_per_zone() as i64;
        let current = lines_per_zone + block as i64;
        for stride in 1..self.max_stride_bound() {
            if self.check_candidate(&states, current, stride as i64) {
                self.push_candidate(access, zone, current - stride as i64);
            }
            if self.last_candidates.len() == self.current_degree() as usize {
                break;
            }

            if self.check_candidate(&states, current, -(stride as i64)) {
                self.push_candidate(access, zone, current + stride as i64);
            }
            if self.last_candidates.len() == self.current_degree() as usize {
                break;
            }
        }

        Ok(&self.last_candidates)
    }

    pub fn process_epoch(&mut self) -> Result<AmpmEpochReport, AmpmPrefetcherError> {
        let Some(epoch_control) = self.config.epoch_control() else {
            return Err(AmpmPrefetcherError::EpochControlDisabled);
        };

        let stats = self.epoch_stats;
        let prefetch_coverage_high = epoch_control
            .high_coverage_threshold()
            .count_ratio_exceeds(stats.useful_prefetches(), stats.raw_cache_misses());
        let prefetch_coverage_low = epoch_control
            .low_coverage_threshold()
            .count_ratio_below(stats.useful_prefetches(), stats.raw_cache_misses());
        let prefetch_accuracy_high = epoch_control
            .high_accuracy_threshold()
            .count_ratio_exceeds(stats.useful_prefetches(), stats.issued_prefetches());
        let prefetch_accuracy_low = epoch_control
            .low_accuracy_threshold()
            .count_ratio_below(stats.useful_prefetches(), stats.issued_prefetches());
        let cache_hit_ratio_high = epoch_control
            .high_cache_hit_threshold()
            .count_ratio_exceeds(stats.raw_cache_hits(), stats.cache_accesses());
        let cache_hit_ratio_low = epoch_control
            .low_cache_hit_threshold()
            .count_ratio_below(stats.raw_cache_hits(), stats.cache_accesses());

        let previous_degree = self.current_degree;
        let previous_useful_degree = self.useful_degree;
        let next_useful_degree =
            if prefetch_coverage_high && (prefetch_accuracy_high || cache_hit_ratio_low) {
                previous_useful_degree.saturating_add(1)
            } else if (prefetch_coverage_low && (prefetch_accuracy_low || cache_hit_ratio_high))
                || (prefetch_accuracy_low && cache_hit_ratio_high)
            {
                previous_useful_degree.saturating_sub(1)
            } else {
                previous_useful_degree
            };
        let memory_bandwidth_degree = epoch_control.memory_bandwidth_degree(stats);
        let next_degree = memory_bandwidth_degree.min(next_useful_degree);
        let report = AmpmEpochReport {
            stats,
            previous_degree,
            previous_useful_degree,
            next_degree,
            next_useful_degree,
            memory_bandwidth_degree,
        };

        self.current_degree = next_degree;
        self.useful_degree = next_useful_degree;
        self.epoch_stats = AmpmEpochStats::default();
        self.last_epoch_report = Some(report);
        Ok(report)
    }

    pub fn snapshot(&self) -> AmpmPrefetcherSnapshot {
        AmpmPrefetcherSnapshot {
            config: self.config.clone(),
            entries: self
                .entries
                .iter()
                .map(|(key, entry)| AmpmAccessMapEntrySnapshot {
                    zone: key.zone,
                    secure: key.secure,
                    states: entry.states.clone(),
                })
                .collect(),
            lru_order: self.lru_order.clone(),
            last_candidates: self.last_candidates.clone(),
            current_degree: self.current_degree,
            useful_degree: self.useful_degree,
            issued_prefetches: self.issued_prefetches,
            useful_prefetches: self.useful_prefetches,
            raw_cache_misses: self.raw_cache_misses,
            raw_cache_hits: self.raw_cache_hits,
            epoch_stats: self.epoch_stats,
            last_epoch_report: self.last_epoch_report,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &AmpmPrefetcherSnapshot,
    ) -> Result<(), AmpmPrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(AmpmPrefetcherError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config().clone()),
            });
        }
        if snapshot.entries().len() > self.config.table_entries() {
            return Err(AmpmPrefetcherError::SnapshotEntryCountOutOfRange {
                entries: snapshot.entries().len(),
                table_entries: self.config.table_entries(),
            });
        }

        let expected_states = self.lines_per_zone();
        let mut entries = BTreeMap::new();
        for entry in snapshot.entries() {
            if entry.states().len() != expected_states {
                return Err(AmpmPrefetcherError::SnapshotEntryShapeMismatch {
                    zone: entry.zone(),
                    states: entry.states().len(),
                    expected: expected_states,
                });
            }
            entries.insert(
                AmpmZoneKey::new(entry.zone(), entry.secure()),
                AmpmAccessMapEntry {
                    states: entry.states().to_vec(),
                },
            );
        }

        self.entries = entries;
        self.lru_order = snapshot
            .lru_order
            .iter()
            .copied()
            .filter(|key| self.entries.contains_key(key))
            .collect();
        for key in self.entries.keys() {
            if !self.lru_order.contains(key) {
                self.lru_order.push(*key);
            }
        }
        self.validate_restored_set_counts()?;
        self.last_candidates = snapshot.last_candidates().to_vec();
        self.current_degree = snapshot.current_degree;
        self.useful_degree = snapshot.useful_degree;
        self.issued_prefetches = snapshot.issued_prefetches;
        self.useful_prefetches = snapshot.useful_prefetches;
        self.raw_cache_misses = snapshot.raw_cache_misses;
        self.raw_cache_hits = snapshot.raw_cache_hits;
        self.epoch_stats = snapshot.epoch_stats;
        self.last_epoch_report = snapshot.last_epoch_report;
        Ok(())
    }

    fn lines_per_zone(&self) -> usize {
        (self.config.hot_zone_size() / self.config.line_size()) as usize
    }

    fn max_stride_bound(&self) -> u64 {
        let default_bound = self.config.hot_zone_size() / self.config.line_size() / 2;
        match self.config.limit_stride() {
            Some(limit) => default_bound.min(limit.saturating_add(1)),
            None => default_bound,
        }
    }

    fn ensure_neighbor_zones(&mut self, zone: u64, secure: bool) {
        self.ensure_zone(zone, secure);
        if let Some(previous) = zone.checked_sub(1) {
            self.ensure_zone(previous, secure);
        }
        if zone < u64::MAX / self.config.hot_zone_size() {
            self.ensure_zone(zone + 1, secure);
        }
    }

    fn ensure_zone(&mut self, zone: u64, secure: bool) {
        let key = AmpmZoneKey::new(zone, secure);
        if self.entries.contains_key(&key) {
            self.touch_zone(key);
            return;
        }
        let set = self.table_set(key);
        let set_entries = self
            .lru_order
            .iter()
            .filter(|candidate| self.table_set(**candidate) == set)
            .count();
        if set_entries == self.config.table_assoc() {
            if let Some(victim_index) = self
                .lru_order
                .iter()
                .position(|candidate| self.table_set(*candidate) == set)
            {
                let victim = self.lru_order.remove(victim_index);
                self.entries.remove(&victim);
            }
        }
        self.lru_order.push(key);
        self.entries
            .insert(key, AmpmAccessMapEntry::new(self.lines_per_zone()));
    }

    fn touch_zone(&mut self, key: AmpmZoneKey) {
        if let Some(index) = self
            .lru_order
            .iter()
            .position(|candidate| *candidate == key)
        {
            self.lru_order.remove(index);
        }
        self.lru_order.push(key);
    }

    fn table_set(&self, key: AmpmZoneKey) -> usize {
        (key.zone as usize) & (self.config.table_sets() - 1)
    }

    fn validate_restored_set_counts(&self) -> Result<(), AmpmPrefetcherError> {
        let mut set_counts = vec![0_usize; self.config.table_sets()];
        for key in self.entries.keys() {
            set_counts[self.table_set(*key)] += 1;
        }
        for (set, entries) in set_counts.into_iter().enumerate() {
            if entries > self.config.table_assoc() {
                return Err(AmpmPrefetcherError::SnapshotSetEntryCountOutOfRange {
                    set,
                    entries,
                    table_assoc: self.config.table_assoc(),
                });
            }
        }
        Ok(())
    }

    fn set_entry_state(&mut self, zone: u64, secure: bool, block: u64, state: AmpmAccessMapState) {
        let key = AmpmZoneKey::new(zone, secure);
        let Some(entry) = self.entries.get_mut(&key) else {
            return;
        };
        let Some(slot) = entry.states.get_mut(block as usize) else {
            return;
        };
        let old = *slot;
        *slot = state;
        match (old, state) {
            (AmpmAccessMapState::Init, AmpmAccessMapState::Prefetch) => {
                self.issued_prefetches = self.issued_prefetches.saturating_add(1);
                self.epoch_stats.issued_prefetches =
                    self.epoch_stats.issued_prefetches.saturating_add(1);
            }
            (AmpmAccessMapState::Init, AmpmAccessMapState::Access) => {
                self.raw_cache_misses = self.raw_cache_misses.saturating_add(1);
                self.epoch_stats.raw_cache_misses =
                    self.epoch_stats.raw_cache_misses.saturating_add(1);
            }
            (AmpmAccessMapState::Prefetch, AmpmAccessMapState::Access) => {
                self.useful_prefetches = self.useful_prefetches.saturating_add(1);
                self.raw_cache_misses = self.raw_cache_misses.saturating_add(1);
                self.epoch_stats.useful_prefetches =
                    self.epoch_stats.useful_prefetches.saturating_add(1);
                self.epoch_stats.raw_cache_misses =
                    self.epoch_stats.raw_cache_misses.saturating_add(1);
            }
            (AmpmAccessMapState::Access, AmpmAccessMapState::Access) => {
                self.raw_cache_hits = self.raw_cache_hits.saturating_add(1);
                self.epoch_stats.raw_cache_hits = self.epoch_stats.raw_cache_hits.saturating_add(1);
            }
            _ => {}
        }
    }

    fn window_states(&self, zone: u64, secure: bool) -> Vec<AmpmWindowState> {
        let mut states = Vec::with_capacity(self.lines_per_zone() * 3);
        for window_zone in [
            zone.checked_sub(1),
            Some(zone),
            zone.checked_add(1)
                .filter(|next| *next <= u64::MAX / self.config.hot_zone_size()),
        ] {
            for block in 0..self.lines_per_zone() {
                let state = window_zone
                    .and_then(|zone| self.entry_state(zone, secure, block))
                    .map(AmpmWindowState::from)
                    .unwrap_or(AmpmWindowState::Invalid);
                states.push(state);
            }
        }
        states
    }

    fn entry_state(&self, zone: u64, secure: bool, block: usize) -> Option<AmpmAccessMapState> {
        self.entries
            .get(&AmpmZoneKey::new(zone, secure))
            .and_then(|entry| entry.states.get(block).copied())
    }

    fn check_candidate(&self, states: &[AmpmWindowState], current: i64, stride: i64) -> bool {
        let target = window_state(states, current - stride);
        let first = window_state(states, current + stride);
        let second = window_state(states, current + 2 * stride);
        let second_plus_one = window_state(states, current + 2 * stride + 1);

        target != AmpmWindowState::Invalid
            && first == AmpmWindowState::Access
            && (second == AmpmWindowState::Access || second_plus_one == AmpmWindowState::Access)
    }

    fn push_candidate(&mut self, access: AmpmPrefetchAccess, current_zone: u64, window_index: i64) {
        let Some((zone, block)) = self.zone_block_from_window(current_zone, window_index) else {
            return;
        };
        let Some(address) = zone_block_address(&self.config, zone, block) else {
            return;
        };
        let line_address = normalize_address(access.address(), self.config.line_size());
        let stride = (address.get() as i128 - line_address.get() as i128)
            .clamp(i64::MIN as i128, i64::MAX as i128) as i64;
        let degree_index = self
            .last_candidates
            .len()
            .saturating_add(1)
            .min(u32::MAX as usize) as u32;
        self.set_entry_state(zone, access.secure(), block, AmpmAccessMapState::Prefetch);
        self.last_candidates.push(AmpmPrefetchCandidate::new(
            address,
            access.address(),
            access.requestor(),
            access.pc(),
            access.secure(),
            stride,
            degree_index,
        ));
    }

    fn zone_block_from_window(&self, current_zone: u64, window_index: i64) -> Option<(u64, u64)> {
        let lines = self.lines_per_zone() as i64;
        if !(0..3 * lines).contains(&window_index) {
            return None;
        }
        if window_index < lines {
            Some((current_zone.checked_sub(1)?, window_index as u64))
        } else if window_index < 2 * lines {
            Some((current_zone, (window_index - lines) as u64))
        } else {
            Some((
                current_zone.checked_add(1)?,
                (window_index - 2 * lines) as u64,
            ))
        }
    }
}

fn window_state(states: &[AmpmWindowState], index: i64) -> AmpmWindowState {
    if index.is_negative() {
        return AmpmWindowState::Invalid;
    }
    states
        .get(index as usize)
        .copied()
        .unwrap_or(AmpmWindowState::Invalid)
}

fn normalize_address(address: Address, line_size: u64) -> Address {
    Address::new(address.get() / line_size * line_size)
}

fn zone_block_address(config: &AmpmPrefetcherConfig, zone: u64, block: u64) -> Option<Address> {
    let zone_base = zone.checked_mul(config.hot_zone_size())?;
    let block_offset = block.checked_mul(config.line_size())?;
    zone_base.checked_add(block_offset).map(Address::new)
}

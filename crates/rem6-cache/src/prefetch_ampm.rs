use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::prefetch::PrefetchCandidate;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmpmPrefetcherConfig {
    line_size: u64,
    hot_zone_size: u64,
    degree: u32,
    table_entries: usize,
    limit_stride: Option<u64>,
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
        if degree == 0 {
            return Err(AmpmPrefetcherError::ZeroDegree);
        }
        if table_entries == 0 {
            return Err(AmpmPrefetcherError::ZeroTableEntries);
        }
        if table_entries < 3 {
            return Err(AmpmPrefetcherError::TableTooSmall { table_entries });
        }

        Ok(Self {
            line_size,
            hot_zone_size,
            degree,
            table_entries,
            limit_stride: None,
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

    pub const fn limit_stride(&self) -> Option<u64> {
        self.limit_stride
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AmpmPrefetcherError {
    ZeroLineSize,
    ZeroHotZoneSize,
    ZeroDegree,
    ZeroTableEntries,
    ZeroLimitStride,
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
    SnapshotConfigMismatch {
        expected: AmpmPrefetcherConfig,
        actual: AmpmPrefetcherConfig,
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
}

impl fmt::Display for AmpmPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLineSize => write!(formatter, "AMPM prefetcher line size is zero"),
            Self::ZeroHotZoneSize => write!(formatter, "AMPM prefetcher hot zone size is zero"),
            Self::ZeroDegree => write!(formatter, "AMPM prefetcher degree is zero"),
            Self::ZeroTableEntries => write!(formatter, "AMPM prefetcher table has no entries"),
            Self::ZeroLimitStride => write!(formatter, "AMPM prefetcher stride limit is zero"),
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
        }
    }
}

impl Error for AmpmPrefetcherError {}

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
    insertion_order: Vec<AmpmZoneKey>,
    next_victim: usize,
    last_candidates: Vec<AmpmPrefetchCandidate>,
    issued_prefetches: u64,
    useful_prefetches: u64,
    raw_cache_misses: u64,
    raw_cache_hits: u64,
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
    insertion_order: Vec<AmpmZoneKey>,
    next_victim: usize,
    last_candidates: Vec<AmpmPrefetchCandidate>,
    issued_prefetches: u64,
    useful_prefetches: u64,
    raw_cache_misses: u64,
    raw_cache_hits: u64,
}

impl AmpmPrefetcher {
    pub fn new(config: AmpmPrefetcherConfig) -> Self {
        Self {
            config,
            entries: BTreeMap::new(),
            insertion_order: Vec::new(),
            next_victim: 0,
            last_candidates: Vec::new(),
            issued_prefetches: 0,
            useful_prefetches: 0,
            raw_cache_misses: 0,
            raw_cache_hits: 0,
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

        let states = self.window_states(zone, access.secure());
        let lines_per_zone = self.lines_per_zone() as i64;
        let current = lines_per_zone + block as i64;
        for stride in 1..self.max_stride_bound() {
            if self.check_candidate(&states, current, stride as i64) {
                self.push_candidate(access, zone, current - stride as i64);
            }
            if self.last_candidates.len() == self.config.degree() as usize {
                break;
            }

            if self.check_candidate(&states, current, -(stride as i64)) {
                self.push_candidate(access, zone, current + stride as i64);
            }
            if self.last_candidates.len() == self.config.degree() as usize {
                break;
            }
        }

        Ok(&self.last_candidates)
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
            insertion_order: self.insertion_order.clone(),
            next_victim: self.next_victim,
            last_candidates: self.last_candidates.clone(),
            issued_prefetches: self.issued_prefetches,
            useful_prefetches: self.useful_prefetches,
            raw_cache_misses: self.raw_cache_misses,
            raw_cache_hits: self.raw_cache_hits,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &AmpmPrefetcherSnapshot,
    ) -> Result<(), AmpmPrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(AmpmPrefetcherError::SnapshotConfigMismatch {
                expected: self.config.clone(),
                actual: snapshot.config().clone(),
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
        self.insertion_order = snapshot
            .insertion_order
            .iter()
            .copied()
            .filter(|key| self.entries.contains_key(key))
            .collect();
        for key in self.entries.keys() {
            if !self.insertion_order.contains(key) {
                self.insertion_order.push(*key);
            }
        }
        self.next_victim = if self.insertion_order.is_empty() {
            0
        } else {
            snapshot.next_victim % self.insertion_order.len()
        };
        self.last_candidates = snapshot.last_candidates().to_vec();
        self.issued_prefetches = snapshot.issued_prefetches;
        self.useful_prefetches = snapshot.useful_prefetches;
        self.raw_cache_misses = snapshot.raw_cache_misses;
        self.raw_cache_hits = snapshot.raw_cache_hits;
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
            return;
        }
        if self.entries.len() == self.config.table_entries() {
            let victim_index = self.next_victim % self.insertion_order.len();
            let victim = self.insertion_order[victim_index];
            self.entries.remove(&victim);
            self.insertion_order[victim_index] = key;
            self.next_victim = (victim_index + 1) % self.config.table_entries();
        } else {
            self.insertion_order.push(key);
        }
        self.entries
            .insert(key, AmpmAccessMapEntry::new(self.lines_per_zone()));
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
            }
            (AmpmAccessMapState::Init, AmpmAccessMapState::Access) => {
                self.raw_cache_misses = self.raw_cache_misses.saturating_add(1);
            }
            (AmpmAccessMapState::Prefetch, AmpmAccessMapState::Access) => {
                self.useful_prefetches = self.useful_prefetches.saturating_add(1);
                self.raw_cache_misses = self.raw_cache_misses.saturating_add(1);
            }
            (AmpmAccessMapState::Access, AmpmAccessMapState::Access) => {
                self.raw_cache_hits = self.raw_cache_hits.saturating_add(1);
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

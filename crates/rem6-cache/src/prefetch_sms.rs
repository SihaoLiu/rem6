use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::prefetch::PrefetchCandidate;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmsPrefetcherConfig {
    line_size: u64,
    region_size: u64,
    max_contexts: usize,
    pattern_history_entries: usize,
}

impl SmsPrefetcherConfig {
    pub const fn new(
        line_size: u64,
        region_size: u64,
        max_contexts: usize,
        pattern_history_entries: usize,
    ) -> Result<Self, SmsPrefetcherError> {
        if line_size == 0 {
            return Err(SmsPrefetcherError::ZeroLineSize);
        }
        if region_size == 0 {
            return Err(SmsPrefetcherError::ZeroRegionSize);
        }
        if region_size < line_size || !region_size.is_multiple_of(line_size) {
            return Err(SmsPrefetcherError::RegionLineMismatch {
                region_size,
                line_size,
            });
        }
        if max_contexts == 0 {
            return Err(SmsPrefetcherError::ZeroMaxContexts);
        }
        if pattern_history_entries == 0 {
            return Err(SmsPrefetcherError::ZeroPatternHistoryEntries);
        }

        Ok(Self {
            line_size,
            region_size,
            max_contexts,
            pattern_history_entries,
        })
    }

    pub const fn line_size(&self) -> u64 {
        self.line_size
    }

    pub const fn region_size(&self) -> u64 {
        self.region_size
    }

    pub const fn max_contexts(&self) -> usize {
        self.max_contexts
    }

    pub const fn pattern_history_entries(&self) -> usize {
        self.pattern_history_entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SmsPrefetcherError {
    ZeroLineSize,
    ZeroRegionSize,
    ZeroMaxContexts,
    ZeroPatternHistoryEntries,
    RegionLineMismatch {
        region_size: u64,
        line_size: u64,
    },
    SnapshotConfigMismatch {
        expected: Box<SmsPrefetcherConfig>,
        actual: Box<SmsPrefetcherConfig>,
    },
    SnapshotFilterTableTooLarge {
        entries: usize,
        max_contexts: usize,
    },
    SnapshotActiveTableTooLarge {
        entries: usize,
        max_contexts: usize,
    },
    SnapshotPatternHistoryTooLarge {
        entries: usize,
        max_entries: usize,
    },
    SnapshotFilterQueueShapeMismatch {
        entries: usize,
        table_entries: usize,
    },
    SnapshotActiveQueueShapeMismatch {
        entries: usize,
        table_entries: usize,
    },
    SnapshotPatternQueueShapeMismatch {
        entries: usize,
        table_entries: usize,
    },
}

impl fmt::Display for SmsPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLineSize => write!(formatter, "SMS line size is zero"),
            Self::ZeroRegionSize => write!(formatter, "SMS region size is zero"),
            Self::ZeroMaxContexts => write!(formatter, "SMS context capacity is zero"),
            Self::ZeroPatternHistoryEntries => {
                write!(formatter, "SMS pattern history capacity is zero")
            }
            Self::RegionLineMismatch {
                region_size,
                line_size,
            } => write!(
                formatter,
                "SMS region size {region_size} is not a positive multiple of line size {line_size}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "SMS snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotFilterTableTooLarge {
                entries,
                max_contexts,
            } => write!(
                formatter,
                "SMS snapshot filter table has {entries} entries for {max_contexts} slots"
            ),
            Self::SnapshotActiveTableTooLarge {
                entries,
                max_contexts,
            } => write!(
                formatter,
                "SMS snapshot active table has {entries} entries for {max_contexts} slots"
            ),
            Self::SnapshotPatternHistoryTooLarge {
                entries,
                max_entries,
            } => write!(
                formatter,
                "SMS snapshot pattern history has {entries} entries for {max_entries} slots"
            ),
            Self::SnapshotFilterQueueShapeMismatch {
                entries,
                table_entries,
            } => write!(
                formatter,
                "SMS snapshot filter queue has {entries} entries for {table_entries} table rows"
            ),
            Self::SnapshotActiveQueueShapeMismatch {
                entries,
                table_entries,
            } => write!(
                formatter,
                "SMS snapshot active queue has {entries} entries for {table_entries} table rows"
            ),
            Self::SnapshotPatternQueueShapeMismatch {
                entries,
                table_entries,
            } => write!(
                formatter,
                "SMS snapshot pattern queue has {entries} entries for {table_entries} table rows"
            ),
        }
    }
}

impl Error for SmsPrefetcherError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SmsPrefetchAccess {
    requestor: AgentId,
    pc: u64,
    address: Address,
    secure: bool,
}

impl SmsPrefetchAccess {
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
pub struct SmsPrefetchCandidate {
    address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    region_base: Address,
    pattern_pc: u64,
    trigger_offset: u64,
    pattern_offset: u64,
    stride: i64,
    degree_index: u32,
}

impl SmsPrefetchCandidate {
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

    pub const fn region_base(&self) -> Address {
        self.region_base
    }

    pub const fn pattern_pc(&self) -> u64 {
        self.pattern_pc
    }

    pub const fn trigger_offset(&self) -> u64 {
        self.trigger_offset
    }

    pub const fn pattern_offset(&self) -> u64 {
        self.pattern_offset
    }

    pub const fn stride(&self) -> i64 {
        self.stride
    }

    pub const fn degree_index(&self) -> u32 {
        self.degree_index
    }
}

impl PrefetchCandidate for SmsPrefetchCandidate {
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
pub struct SmsFilterEntrySnapshot {
    region_base: u64,
    pc: u64,
    trigger_offset: u64,
}

impl SmsFilterEntrySnapshot {
    pub const fn region_base(&self) -> u64 {
        self.region_base
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn trigger_offset(&self) -> u64 {
        self.trigger_offset
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmsActiveEntrySnapshot {
    region_base: u64,
    pc: u64,
    trigger_offset: u64,
    offsets: Vec<u64>,
}

impl SmsActiveEntrySnapshot {
    pub const fn region_base(&self) -> u64 {
        self.region_base
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn trigger_offset(&self) -> u64 {
        self.trigger_offset
    }

    pub fn offsets(&self) -> &[u64] {
        &self.offsets
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmsPatternEntrySnapshot {
    pc: u64,
    trigger_offset: u64,
    offsets: Vec<u64>,
}

impl SmsPatternEntrySnapshot {
    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn trigger_offset(&self) -> u64 {
        self.trigger_offset
    }

    pub fn offsets(&self) -> &[u64] {
        &self.offsets
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmsPrefetcherSnapshot {
    config: SmsPrefetcherConfig,
    filter_entries: Vec<SmsFilterEntrySnapshot>,
    filter_fifo: Vec<u64>,
    active_entries: Vec<SmsActiveEntrySnapshot>,
    active_lru: Vec<u64>,
    pattern_entries: Vec<SmsPatternEntrySnapshot>,
    pattern_lru: Vec<(u64, u64)>,
    last_candidates: Vec<SmsPrefetchCandidate>,
}

impl SmsPrefetcherSnapshot {
    pub const fn config(&self) -> &SmsPrefetcherConfig {
        &self.config
    }

    pub fn filter_entries(&self) -> &[SmsFilterEntrySnapshot] {
        &self.filter_entries
    }

    pub fn filter_fifo(&self) -> &[u64] {
        &self.filter_fifo
    }

    pub fn active_entries(&self) -> &[SmsActiveEntrySnapshot] {
        &self.active_entries
    }

    pub fn active_lru(&self) -> &[u64] {
        &self.active_lru
    }

    pub fn pattern_entries(&self) -> &[SmsPatternEntrySnapshot] {
        &self.pattern_entries
    }

    pub fn pattern_lru(&self) -> &[(u64, u64)] {
        &self.pattern_lru
    }

    pub fn last_candidates(&self) -> &[SmsPrefetchCandidate] {
        &self.last_candidates
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SmsFilterEntry {
    pc: u64,
    trigger_offset: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct SmsPatternKey {
    pc: u64,
    trigger_offset: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SmsAccessRegion {
    block_address: u64,
    region_base: u64,
    offset: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmsPrefetcher {
    config: SmsPrefetcherConfig,
    filter_table: BTreeMap<u64, SmsFilterEntry>,
    filter_fifo: VecDeque<u64>,
    active_generation_table: BTreeMap<u64, BTreeSet<u64>>,
    active_pc_offsets: BTreeMap<u64, SmsFilterEntry>,
    active_lru: VecDeque<u64>,
    pattern_history_table: BTreeMap<SmsPatternKey, BTreeSet<u64>>,
    pattern_lru: VecDeque<SmsPatternKey>,
    last_candidates: Vec<SmsPrefetchCandidate>,
}

impl SmsPrefetcher {
    pub fn new(config: SmsPrefetcherConfig) -> Self {
        Self {
            config,
            filter_table: BTreeMap::new(),
            filter_fifo: VecDeque::new(),
            active_generation_table: BTreeMap::new(),
            active_pc_offsets: BTreeMap::new(),
            active_lru: VecDeque::new(),
            pattern_history_table: BTreeMap::new(),
            pattern_lru: VecDeque::new(),
            last_candidates: Vec::new(),
        }
    }

    pub const fn config(&self) -> &SmsPrefetcherConfig {
        &self.config
    }

    pub fn observe(
        &mut self,
        access: SmsPrefetchAccess,
    ) -> Result<&[SmsPrefetchCandidate], SmsPrefetcherError> {
        self.last_candidates.clear();
        let region = self.access_region(access.address());
        self.train(access.pc(), region);
        self.predict(access, region);
        Ok(&self.last_candidates)
    }

    pub fn observe_evict(&mut self, address: Address) {
        let region_base = self.access_region(address).region_base;
        let Some(offsets) = self.active_generation_table.remove(&region_base) else {
            return;
        };
        self.remove_active_lru(region_base);
        let Some(pc_offset) = self.active_pc_offsets.remove(&region_base) else {
            return;
        };
        let key = SmsPatternKey {
            pc: pc_offset.pc,
            trigger_offset: pc_offset.trigger_offset,
        };
        self.pattern_history_table.insert(key, offsets);
        self.touch_pattern_lru(key);
        self.trim_pattern_history();
    }

    pub fn snapshot(&self) -> SmsPrefetcherSnapshot {
        SmsPrefetcherSnapshot {
            config: self.config.clone(),
            filter_entries: self
                .filter_table
                .iter()
                .map(|(region_base, entry)| SmsFilterEntrySnapshot {
                    region_base: *region_base,
                    pc: entry.pc,
                    trigger_offset: entry.trigger_offset,
                })
                .collect(),
            filter_fifo: self.filter_fifo.iter().copied().collect(),
            active_entries: self
                .active_generation_table
                .iter()
                .filter_map(|(region_base, offsets)| {
                    self.active_pc_offsets.get(region_base).map(|pc_offset| {
                        SmsActiveEntrySnapshot {
                            region_base: *region_base,
                            pc: pc_offset.pc,
                            trigger_offset: pc_offset.trigger_offset,
                            offsets: offsets.iter().copied().collect(),
                        }
                    })
                })
                .collect(),
            active_lru: self.active_lru.iter().copied().collect(),
            pattern_entries: self
                .pattern_history_table
                .iter()
                .map(|(key, offsets)| SmsPatternEntrySnapshot {
                    pc: key.pc,
                    trigger_offset: key.trigger_offset,
                    offsets: offsets.iter().copied().collect(),
                })
                .collect(),
            pattern_lru: self
                .pattern_lru
                .iter()
                .map(|key| (key.pc, key.trigger_offset))
                .collect(),
            last_candidates: self.last_candidates.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &SmsPrefetcherSnapshot) -> Result<(), SmsPrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(SmsPrefetcherError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config().clone()),
            });
        }
        if snapshot.filter_entries().len() > self.config.max_contexts() {
            return Err(SmsPrefetcherError::SnapshotFilterTableTooLarge {
                entries: snapshot.filter_entries().len(),
                max_contexts: self.config.max_contexts(),
            });
        }
        if snapshot.active_entries().len() > self.config.max_contexts() {
            return Err(SmsPrefetcherError::SnapshotActiveTableTooLarge {
                entries: snapshot.active_entries().len(),
                max_contexts: self.config.max_contexts(),
            });
        }
        if snapshot.pattern_entries().len() > self.config.pattern_history_entries() {
            return Err(SmsPrefetcherError::SnapshotPatternHistoryTooLarge {
                entries: snapshot.pattern_entries().len(),
                max_entries: self.config.pattern_history_entries(),
            });
        }
        if snapshot.filter_fifo().len() != snapshot.filter_entries().len() {
            return Err(SmsPrefetcherError::SnapshotFilterQueueShapeMismatch {
                entries: snapshot.filter_fifo().len(),
                table_entries: snapshot.filter_entries().len(),
            });
        }
        if snapshot.active_lru().len() != snapshot.active_entries().len() {
            return Err(SmsPrefetcherError::SnapshotActiveQueueShapeMismatch {
                entries: snapshot.active_lru().len(),
                table_entries: snapshot.active_entries().len(),
            });
        }
        if snapshot.pattern_lru().len() != snapshot.pattern_entries().len() {
            return Err(SmsPrefetcherError::SnapshotPatternQueueShapeMismatch {
                entries: snapshot.pattern_lru().len(),
                table_entries: snapshot.pattern_entries().len(),
            });
        }

        self.filter_table = snapshot
            .filter_entries()
            .iter()
            .map(|entry| {
                (
                    entry.region_base(),
                    SmsFilterEntry {
                        pc: entry.pc(),
                        trigger_offset: entry.trigger_offset(),
                    },
                )
            })
            .collect();
        self.filter_fifo = snapshot.filter_fifo().iter().copied().collect();
        self.active_generation_table = snapshot
            .active_entries()
            .iter()
            .map(|entry| {
                (
                    entry.region_base(),
                    entry.offsets().iter().copied().collect::<BTreeSet<_>>(),
                )
            })
            .collect();
        self.active_pc_offsets = snapshot
            .active_entries()
            .iter()
            .map(|entry| {
                (
                    entry.region_base(),
                    SmsFilterEntry {
                        pc: entry.pc(),
                        trigger_offset: entry.trigger_offset(),
                    },
                )
            })
            .collect();
        self.active_lru = snapshot.active_lru().iter().copied().collect();
        self.pattern_history_table = snapshot
            .pattern_entries()
            .iter()
            .map(|entry| {
                (
                    SmsPatternKey {
                        pc: entry.pc(),
                        trigger_offset: entry.trigger_offset(),
                    },
                    entry.offsets().iter().copied().collect::<BTreeSet<_>>(),
                )
            })
            .collect();
        self.pattern_lru = snapshot
            .pattern_lru()
            .iter()
            .map(|(pc, trigger_offset)| SmsPatternKey {
                pc: *pc,
                trigger_offset: *trigger_offset,
            })
            .collect();
        self.last_candidates = snapshot.last_candidates().to_vec();
        Ok(())
    }

    pub fn filter_entry_count(&self) -> usize {
        self.filter_table.len()
    }

    pub fn active_entry_count(&self) -> usize {
        self.active_generation_table.len()
    }

    pub fn pattern_entry_count(&self) -> usize {
        self.pattern_history_table.len()
    }

    pub fn filter_trigger(&self, region_base: u64) -> Option<(u64, u64)> {
        self.filter_table
            .get(&region_base)
            .map(|entry| (entry.pc, entry.trigger_offset))
    }

    pub fn active_offsets(&self, region_base: u64) -> Vec<u64> {
        self.active_generation_table
            .get(&region_base)
            .map(|offsets| offsets.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn pattern_offsets(&self, pc: u64, trigger_offset: u64) -> Vec<u64> {
        self.pattern_history_table
            .get(&SmsPatternKey { pc, trigger_offset })
            .map(|offsets| offsets.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn last_candidates(&self) -> &[SmsPrefetchCandidate] {
        &self.last_candidates
    }

    fn train(&mut self, pc: u64, region: SmsAccessRegion) {
        if let Some(offsets) = self.active_generation_table.get_mut(&region.region_base) {
            offsets.insert(region.offset);
            self.touch_active_lru(region.region_base);
            return;
        }

        if let Some(filter_entry) = self.filter_table.remove(&region.region_base) {
            self.remove_filter_fifo(region.region_base);
            let mut offsets = BTreeSet::new();
            offsets.insert(filter_entry.trigger_offset);
            offsets.insert(region.offset);
            self.active_generation_table
                .insert(region.region_base, offsets);
            self.active_pc_offsets
                .insert(region.region_base, filter_entry);
            self.touch_active_lru(region.region_base);
            self.trim_active_table();
            return;
        }

        self.filter_table.insert(
            region.region_base,
            SmsFilterEntry {
                pc,
                trigger_offset: region.offset,
            },
        );
        self.touch_filter_fifo(region.region_base);
        self.trim_filter_table();
    }

    fn predict(&mut self, access: SmsPrefetchAccess, region: SmsAccessRegion) {
        let key = SmsPatternKey {
            pc: access.pc(),
            trigger_offset: region.offset,
        };
        let Some(offsets) = self.pattern_history_table.get(&key).cloned() else {
            return;
        };
        self.touch_pattern_lru(key);
        for offset in offsets {
            let candidate_address = self.block_address(Address::new(region.region_base + offset));
            let degree_index = self
                .last_candidates
                .len()
                .saturating_add(1)
                .min(u32::MAX as usize) as u32;
            let stride = candidate_address.get() as i128 - access.address().get() as i128;
            self.last_candidates.push(SmsPrefetchCandidate {
                address: candidate_address,
                source_address: access.address(),
                context: access.requestor(),
                pc: access.pc(),
                secure: access.secure(),
                region_base: Address::new(region.region_base),
                pattern_pc: key.pc,
                trigger_offset: key.trigger_offset,
                pattern_offset: offset,
                stride: stride.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
                degree_index,
            });
        }
    }

    fn trim_filter_table(&mut self) {
        while self.filter_table.len() > self.config.max_contexts() {
            let Some(region_base) = self.filter_fifo.pop_back() else {
                break;
            };
            self.filter_table.remove(&region_base);
        }
    }

    fn trim_active_table(&mut self) {
        while self.active_generation_table.len() > self.config.max_contexts() {
            let Some(region_base) = self.active_lru.pop_back() else {
                break;
            };
            self.active_generation_table.remove(&region_base);
            self.active_pc_offsets.remove(&region_base);
        }
    }

    fn trim_pattern_history(&mut self) {
        while self.pattern_history_table.len() > self.config.pattern_history_entries() {
            let Some(key) = self.pattern_lru.pop_back() else {
                break;
            };
            self.pattern_history_table.remove(&key);
        }
    }

    fn touch_filter_fifo(&mut self, region_base: u64) {
        self.remove_filter_fifo(region_base);
        self.filter_fifo.push_front(region_base);
    }

    fn remove_filter_fifo(&mut self, region_base: u64) {
        self.filter_fifo.retain(|entry| *entry != region_base);
    }

    fn touch_active_lru(&mut self, region_base: u64) {
        self.remove_active_lru(region_base);
        self.active_lru.push_front(region_base);
    }

    fn remove_active_lru(&mut self, region_base: u64) {
        self.active_lru.retain(|entry| *entry != region_base);
    }

    fn touch_pattern_lru(&mut self, key: SmsPatternKey) {
        self.pattern_lru.retain(|entry| *entry != key);
        self.pattern_lru.push_front(key);
    }

    fn access_region(&self, address: Address) -> SmsAccessRegion {
        let block_address = self.block_address(address).get();
        let region_base = block_address / self.config.region_size() * self.config.region_size();
        SmsAccessRegion {
            block_address,
            region_base,
            offset: block_address - region_base,
        }
    }

    fn block_address(&self, address: Address) -> Address {
        Address::new(address.get() / self.config.line_size() * self.config.line_size())
    }
}

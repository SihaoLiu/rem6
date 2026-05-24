use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::prefetch::PrefetchCandidate;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PifPrefetcherConfig {
    line_size: u64,
    preceding_blocks: usize,
    succeeding_blocks: usize,
    temporal_compactor_entries: usize,
    stream_address_buffer_entries: usize,
    history_buffer_entries: usize,
    index_entries: usize,
}

impl PifPrefetcherConfig {
    pub fn new(
        line_size: u64,
        preceding_blocks: usize,
        succeeding_blocks: usize,
        temporal_compactor_entries: usize,
        stream_address_buffer_entries: usize,
        history_buffer_entries: usize,
        index_entries: usize,
    ) -> Result<Self, PifPrefetcherError> {
        if line_size == 0 {
            return Err(PifPrefetcherError::ZeroLineSize);
        }
        if !line_size.is_power_of_two() {
            return Err(PifPrefetcherError::LineSizeNotPowerOfTwo { line_size });
        }
        if preceding_blocks == 0 && succeeding_blocks == 0 {
            return Err(PifPrefetcherError::EmptySpatialWindow);
        }
        if temporal_compactor_entries == 0 {
            return Err(PifPrefetcherError::ZeroTemporalCompactorEntries);
        }
        if stream_address_buffer_entries == 0 {
            return Err(PifPrefetcherError::ZeroStreamAddressBufferEntries);
        }
        if history_buffer_entries == 0 {
            return Err(PifPrefetcherError::ZeroHistoryBufferEntries);
        }
        if index_entries == 0 {
            return Err(PifPrefetcherError::ZeroIndexEntries);
        }

        Ok(Self {
            line_size,
            preceding_blocks,
            succeeding_blocks,
            temporal_compactor_entries,
            stream_address_buffer_entries,
            history_buffer_entries,
            index_entries,
        })
    }

    pub const fn line_size(&self) -> u64 {
        self.line_size
    }

    pub const fn preceding_blocks(&self) -> usize {
        self.preceding_blocks
    }

    pub const fn succeeding_blocks(&self) -> usize {
        self.succeeding_blocks
    }

    pub const fn temporal_compactor_entries(&self) -> usize {
        self.temporal_compactor_entries
    }

    pub const fn stream_address_buffer_entries(&self) -> usize {
        self.stream_address_buffer_entries
    }

    pub const fn history_buffer_entries(&self) -> usize {
        self.history_buffer_entries
    }

    pub const fn index_entries(&self) -> usize {
        self.index_entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PifPrefetcherError {
    ZeroLineSize,
    LineSizeNotPowerOfTwo {
        line_size: u64,
    },
    EmptySpatialWindow,
    ZeroTemporalCompactorEntries,
    ZeroStreamAddressBufferEntries,
    ZeroHistoryBufferEntries,
    ZeroIndexEntries,
    SnapshotConfigMismatch {
        expected: Box<PifPrefetcherConfig>,
        actual: Box<PifPrefetcherConfig>,
    },
    SnapshotTemporalCompactorTooLarge {
        entries: usize,
        max_entries: usize,
    },
    SnapshotHistoryTooLarge {
        entries: usize,
        max_entries: usize,
    },
    SnapshotIndexTooLarge {
        entries: usize,
        max_entries: usize,
    },
    SnapshotStreamAddressBufferTooLarge {
        entries: usize,
        max_entries: usize,
    },
    SnapshotIndexReferencesMissingHistory {
        history_id: u64,
    },
    SnapshotStreamReferencesMissingHistory {
        history_id: u64,
    },
}

impl fmt::Display for PifPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLineSize => write!(formatter, "PIF line size is zero"),
            Self::LineSizeNotPowerOfTwo { line_size } => {
                write!(formatter, "PIF line size {line_size} is not a power of two")
            }
            Self::EmptySpatialWindow => write!(formatter, "PIF spatial window is empty"),
            Self::ZeroTemporalCompactorEntries => {
                write!(formatter, "PIF temporal compactor has no entries")
            }
            Self::ZeroStreamAddressBufferEntries => {
                write!(formatter, "PIF stream address buffer has no entries")
            }
            Self::ZeroHistoryBufferEntries => {
                write!(formatter, "PIF history buffer has no entries")
            }
            Self::ZeroIndexEntries => write!(formatter, "PIF index has no entries"),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "PIF snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotTemporalCompactorTooLarge {
                entries,
                max_entries,
            } => write!(
                formatter,
                "PIF snapshot temporal compactor has {entries} entries for {max_entries} slots"
            ),
            Self::SnapshotHistoryTooLarge {
                entries,
                max_entries,
            } => write!(
                formatter,
                "PIF snapshot history has {entries} entries for {max_entries} slots"
            ),
            Self::SnapshotIndexTooLarge {
                entries,
                max_entries,
            } => write!(
                formatter,
                "PIF snapshot index has {entries} entries for {max_entries} slots"
            ),
            Self::SnapshotStreamAddressBufferTooLarge {
                entries,
                max_entries,
            } => write!(
                formatter,
                "PIF snapshot stream address buffer has {entries} entries for {max_entries} slots"
            ),
            Self::SnapshotIndexReferencesMissingHistory { history_id } => write!(
                formatter,
                "PIF snapshot index references missing history id {history_id}"
            ),
            Self::SnapshotStreamReferencesMissingHistory { history_id } => write!(
                formatter,
                "PIF snapshot stream address buffer references missing history id {history_id}"
            ),
        }
    }
}

impl Error for PifPrefetcherError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PifPrefetchAccess {
    context: AgentId,
    pc: Address,
    secure: bool,
}

impl PifPrefetchAccess {
    pub const fn new(context: AgentId, pc: Address, secure: bool) -> Self {
        Self {
            context,
            pc,
            secure,
        }
    }

    pub const fn context(&self) -> AgentId {
        self.context
    }

    pub const fn pc_address(&self) -> Address {
        self.pc
    }

    pub const fn pc(&self) -> u64 {
        self.pc.get()
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PifPrefetchCandidate {
    address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    trigger: Address,
    block_offset: i64,
    stride: i64,
    degree_index: u32,
}

impl PifPrefetchCandidate {
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

    pub const fn trigger(&self) -> Address {
        self.trigger
    }

    pub const fn block_offset(&self) -> i64 {
        self.block_offset
    }

    pub const fn stride(&self) -> i64 {
        self.stride
    }

    pub const fn degree_index(&self) -> u32 {
        self.degree_index
    }
}

impl PrefetchCandidate for PifPrefetchCandidate {
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
pub struct PifCompactorEntrySnapshot {
    trigger: Address,
    block_offsets: Vec<i64>,
}

impl PifCompactorEntrySnapshot {
    pub const fn trigger(&self) -> Address {
        self.trigger
    }

    pub fn block_offsets(&self) -> &[i64] {
        &self.block_offsets
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PifHistoryEntrySnapshot {
    history_id: u64,
    entry: PifCompactorEntrySnapshot,
}

impl PifHistoryEntrySnapshot {
    pub const fn history_id(&self) -> u64 {
        self.history_id
    }

    pub const fn entry(&self) -> &PifCompactorEntrySnapshot {
        &self.entry
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PifIndexEntrySnapshot {
    trigger: Address,
    secure: bool,
    history_id: u64,
}

impl PifIndexEntrySnapshot {
    pub const fn trigger(&self) -> Address {
        self.trigger
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn history_id(&self) -> u64 {
        self.history_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PifPrefetcherSnapshot {
    config: PifPrefetcherConfig,
    spatial_compactor: Option<PifCompactorEntrySnapshot>,
    temporal_compactor: Vec<PifCompactorEntrySnapshot>,
    history_entries: Vec<PifHistoryEntrySnapshot>,
    index_entries: Vec<PifIndexEntrySnapshot>,
    index_lru: Vec<(Address, bool)>,
    stream_address_buffer: Vec<u64>,
    next_history_id: u64,
    last_candidates: Vec<PifPrefetchCandidate>,
}

impl PifPrefetcherSnapshot {
    pub const fn config(&self) -> &PifPrefetcherConfig {
        &self.config
    }

    pub const fn spatial_compactor(&self) -> Option<&PifCompactorEntrySnapshot> {
        self.spatial_compactor.as_ref()
    }

    pub fn temporal_compactor(&self) -> &[PifCompactorEntrySnapshot] {
        &self.temporal_compactor
    }

    pub fn history_entries(&self) -> &[PifHistoryEntrySnapshot] {
        &self.history_entries
    }

    pub fn index_entries(&self) -> &[PifIndexEntrySnapshot] {
        &self.index_entries
    }

    pub fn index_lru(&self) -> &[(Address, bool)] {
        &self.index_lru
    }

    pub fn stream_address_buffer(&self) -> &[u64] {
        &self.stream_address_buffer
    }

    pub const fn next_history_id(&self) -> u64 {
        self.next_history_id
    }

    pub fn last_candidates(&self) -> &[PifPrefetchCandidate] {
        &self.last_candidates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PifCompactorEntry {
    trigger: Address,
    block_offsets: BTreeSet<i64>,
}

impl PifCompactorEntry {
    fn new(trigger: Address) -> Self {
        Self {
            trigger,
            block_offsets: BTreeSet::new(),
        }
    }

    fn from_snapshot(snapshot: &PifCompactorEntrySnapshot) -> Self {
        Self {
            trigger: snapshot.trigger(),
            block_offsets: snapshot.block_offsets().iter().copied().collect(),
        }
    }

    fn snapshot(&self) -> PifCompactorEntrySnapshot {
        PifCompactorEntrySnapshot {
            trigger: self.trigger,
            block_offsets: self.block_offsets.iter().copied().collect(),
        }
    }

    fn is_in_spatial_window(&self, block: Address, config: &PifPrefetcherConfig) -> bool {
        let offset = block_offset(self.trigger, block, config.line_size());
        if offset < 0 {
            offset.unsigned_abs() as usize <= config.preceding_blocks()
        } else {
            offset as usize <= config.succeeding_blocks()
        }
    }

    fn record_if_in_window(&mut self, block: Address, config: &PifPrefetcherConfig) -> bool {
        if !self.is_in_spatial_window(block, config) {
            return false;
        }
        let offset = block_offset(self.trigger, block, config.line_size());
        if offset != 0 {
            self.block_offsets.insert(offset);
        }
        true
    }

    fn has_address(&self, block: Address, config: &PifPrefetcherConfig) -> bool {
        let offset = block_offset(self.trigger, block, config.line_size());
        offset == 0 || self.block_offsets.contains(&offset)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PifHistoryEntry {
    history_id: u64,
    entry: PifCompactorEntry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct PifIndexKey {
    trigger: u64,
    secure: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PifPrefetcher {
    config: PifPrefetcherConfig,
    spatial_compactor: Option<PifCompactorEntry>,
    temporal_compactor: VecDeque<PifCompactorEntry>,
    history_buffer: VecDeque<PifHistoryEntry>,
    index: BTreeMap<PifIndexKey, u64>,
    index_lru: VecDeque<PifIndexKey>,
    stream_address_buffer: VecDeque<u64>,
    next_history_id: u64,
    last_candidates: Vec<PifPrefetchCandidate>,
}

impl PifPrefetcher {
    pub fn new(config: PifPrefetcherConfig) -> Self {
        Self {
            config,
            spatial_compactor: None,
            temporal_compactor: VecDeque::new(),
            history_buffer: VecDeque::new(),
            index: BTreeMap::new(),
            index_lru: VecDeque::new(),
            stream_address_buffer: VecDeque::new(),
            next_history_id: 0,
            last_candidates: Vec::new(),
        }
    }

    pub const fn config(&self) -> &PifPrefetcherConfig {
        &self.config
    }

    pub fn observe_retired_instruction(&mut self, pc: Address) {
        let block = self.block_address(pc);
        if self.temporal_compactor.is_empty() {
            let entry = PifCompactorEntry::new(block);
            self.spatial_compactor = Some(entry.clone());
            self.temporal_compactor.push_back(entry);
            return;
        }

        if self
            .spatial_compactor
            .as_mut()
            .is_some_and(|entry| entry.record_if_in_window(block, &self.config))
        {
            return;
        }

        let mut found_temporal = None;
        for (position, entry) in self.temporal_compactor.iter().enumerate() {
            if entry.is_in_spatial_window(block, &self.config) {
                found_temporal = Some(position);
                break;
            }
        }

        if let Some(position) = found_temporal {
            let entry = self
                .temporal_compactor
                .remove(position)
                .expect("temporal compactor position was found");
            self.trim_temporal_for_insert();
            self.temporal_compactor.push_back(entry.clone());
            self.spatial_compactor = Some(entry);
            return;
        }

        if let Some(entry) = self.spatial_compactor.clone() {
            self.trim_temporal_for_insert();
            self.temporal_compactor.push_back(entry.clone());
            self.commit_history_entry(entry);
        }
        self.spatial_compactor = Some(PifCompactorEntry::new(block));
    }

    pub fn observe(&mut self, access: PifPrefetchAccess) -> &[PifPrefetchCandidate] {
        self.last_candidates.clear();
        let block = self.block_address(access.pc_address());

        for position in 0..self.stream_address_buffer.len() {
            let history_id = self.stream_address_buffer[position];
            let Some(entry) = self.history_entry(history_id) else {
                continue;
            };
            if !entry.entry.has_address(block, &self.config) {
                continue;
            }
            let prediction_id = self
                .next_stream_history_id(history_id)
                .unwrap_or(history_id);
            self.stream_address_buffer[position] = prediction_id;
            if let Some(prediction_entry) = self.history_entry(prediction_id).cloned() {
                self.emit_predictions(access, &prediction_entry.entry);
            }
            return &self.last_candidates;
        }

        let key = PifIndexKey {
            trigger: block.get(),
            secure: access.secure(),
        };
        let Some(history_id) = self.index.get(&key).copied() else {
            return &self.last_candidates;
        };
        let Some(history_entry) = self.history_entry(history_id).cloned() else {
            self.index.remove(&key);
            self.remove_index_lru(key);
            return &self.last_candidates;
        };
        self.touch_index_lru(key);
        self.push_stream_address_buffer(history_id);
        self.emit_predictions(access, &history_entry.entry);
        &self.last_candidates
    }

    pub fn snapshot(&self) -> PifPrefetcherSnapshot {
        PifPrefetcherSnapshot {
            config: self.config.clone(),
            spatial_compactor: self
                .spatial_compactor
                .as_ref()
                .map(PifCompactorEntry::snapshot),
            temporal_compactor: self
                .temporal_compactor
                .iter()
                .map(PifCompactorEntry::snapshot)
                .collect(),
            history_entries: self
                .history_buffer
                .iter()
                .map(|entry| PifHistoryEntrySnapshot {
                    history_id: entry.history_id,
                    entry: entry.entry.snapshot(),
                })
                .collect(),
            index_entries: self
                .index
                .iter()
                .map(|(key, history_id)| PifIndexEntrySnapshot {
                    trigger: Address::new(key.trigger),
                    secure: key.secure,
                    history_id: *history_id,
                })
                .collect(),
            index_lru: self
                .index_lru
                .iter()
                .map(|key| (Address::new(key.trigger), key.secure))
                .collect(),
            stream_address_buffer: self.stream_address_buffer.iter().copied().collect(),
            next_history_id: self.next_history_id,
            last_candidates: self.last_candidates.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &PifPrefetcherSnapshot) -> Result<(), PifPrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(PifPrefetcherError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config().clone()),
            });
        }
        if snapshot.temporal_compactor().len() > self.config.temporal_compactor_entries() {
            return Err(PifPrefetcherError::SnapshotTemporalCompactorTooLarge {
                entries: snapshot.temporal_compactor().len(),
                max_entries: self.config.temporal_compactor_entries(),
            });
        }
        if snapshot.history_entries().len() > self.config.history_buffer_entries() {
            return Err(PifPrefetcherError::SnapshotHistoryTooLarge {
                entries: snapshot.history_entries().len(),
                max_entries: self.config.history_buffer_entries(),
            });
        }
        if snapshot.index_entries().len() > self.config.index_entries() {
            return Err(PifPrefetcherError::SnapshotIndexTooLarge {
                entries: snapshot.index_entries().len(),
                max_entries: self.config.index_entries(),
            });
        }
        if snapshot.stream_address_buffer().len() > self.config.stream_address_buffer_entries() {
            return Err(PifPrefetcherError::SnapshotStreamAddressBufferTooLarge {
                entries: snapshot.stream_address_buffer().len(),
                max_entries: self.config.stream_address_buffer_entries(),
            });
        }

        let history_ids = snapshot
            .history_entries()
            .iter()
            .map(PifHistoryEntrySnapshot::history_id)
            .collect::<BTreeSet<_>>();
        for entry in snapshot.index_entries() {
            if !history_ids.contains(&entry.history_id()) {
                return Err(PifPrefetcherError::SnapshotIndexReferencesMissingHistory {
                    history_id: entry.history_id(),
                });
            }
        }
        for history_id in snapshot.stream_address_buffer() {
            if !history_ids.contains(history_id) {
                return Err(PifPrefetcherError::SnapshotStreamReferencesMissingHistory {
                    history_id: *history_id,
                });
            }
        }

        self.spatial_compactor = snapshot
            .spatial_compactor()
            .map(PifCompactorEntry::from_snapshot);
        self.temporal_compactor = snapshot
            .temporal_compactor()
            .iter()
            .map(PifCompactorEntry::from_snapshot)
            .collect();
        self.history_buffer = snapshot
            .history_entries()
            .iter()
            .map(|entry| PifHistoryEntry {
                history_id: entry.history_id(),
                entry: PifCompactorEntry::from_snapshot(entry.entry()),
            })
            .collect();
        self.index = snapshot
            .index_entries()
            .iter()
            .map(|entry| {
                (
                    PifIndexKey {
                        trigger: entry.trigger().get(),
                        secure: entry.secure(),
                    },
                    entry.history_id(),
                )
            })
            .collect();
        self.index_lru = snapshot
            .index_lru()
            .iter()
            .map(|(trigger, secure)| PifIndexKey {
                trigger: trigger.get(),
                secure: *secure,
            })
            .collect();
        self.stream_address_buffer = snapshot.stream_address_buffer().iter().copied().collect();
        self.next_history_id = snapshot.next_history_id();
        self.last_candidates = snapshot.last_candidates().to_vec();
        Ok(())
    }

    pub fn history_entry_count(&self) -> usize {
        self.history_buffer.len()
    }

    pub fn index_entry_count(&self) -> usize {
        self.index.len()
    }

    pub fn temporal_compactor_count(&self) -> usize {
        self.temporal_compactor.len()
    }

    pub fn stream_address_buffer_count(&self) -> usize {
        self.stream_address_buffer.len()
    }

    pub fn history_triggers(&self) -> Vec<Address> {
        self.history_buffer
            .iter()
            .map(|entry| entry.entry.trigger)
            .collect()
    }

    pub fn index_contains(&self, trigger: Address, secure: bool) -> bool {
        self.index.contains_key(&PifIndexKey {
            trigger: self.block_address(trigger).get(),
            secure,
        })
    }

    pub fn last_candidates(&self) -> &[PifPrefetchCandidate] {
        &self.last_candidates
    }

    fn commit_history_entry(&mut self, entry: PifCompactorEntry) {
        let history_id = self.next_history_id;
        self.next_history_id = self.next_history_id.saturating_add(1);
        self.history_buffer
            .push_back(PifHistoryEntry { history_id, entry });
        let trigger = self
            .history_buffer
            .back()
            .expect("history entry was just pushed")
            .entry
            .trigger;
        let key = PifIndexKey {
            trigger: trigger.get(),
            secure: false,
        };
        self.index.insert(key, history_id);
        self.touch_index_lru(key);
        self.trim_history_buffer();
        self.trim_index();
    }

    fn emit_predictions(&mut self, access: PifPrefetchAccess, entry: &PifCompactorEntry) {
        for offset in &entry.block_offsets {
            let Some(address) = offset_address(entry.trigger, *offset, self.config.line_size())
            else {
                continue;
            };
            let degree_index = self
                .last_candidates
                .len()
                .saturating_add(1)
                .min(u32::MAX as usize) as u32;
            let stride = address.get() as i128 - access.pc_address().get() as i128;
            self.last_candidates.push(PifPrefetchCandidate {
                address,
                source_address: access.pc_address(),
                context: access.context(),
                pc: access.pc(),
                secure: access.secure(),
                trigger: entry.trigger,
                block_offset: *offset,
                stride: stride.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
                degree_index,
            });
        }
    }

    fn trim_temporal_for_insert(&mut self) {
        while self.temporal_compactor.len() >= self.config.temporal_compactor_entries() {
            self.temporal_compactor.pop_front();
        }
    }

    fn trim_history_buffer(&mut self) {
        while self.history_buffer.len() > self.config.history_buffer_entries() {
            let Some(entry) = self.history_buffer.pop_front() else {
                break;
            };
            self.index
                .retain(|_, history_id| *history_id != entry.history_id);
            self.index_lru.retain(|key| self.index.contains_key(key));
            self.stream_address_buffer
                .retain(|history_id| *history_id != entry.history_id);
        }
    }

    fn trim_index(&mut self) {
        while self.index.len() > self.config.index_entries() {
            let Some(key) = self.index_lru.pop_back() else {
                break;
            };
            self.index.remove(&key);
        }
    }

    fn touch_index_lru(&mut self, key: PifIndexKey) {
        self.remove_index_lru(key);
        self.index_lru.push_front(key);
    }

    fn remove_index_lru(&mut self, key: PifIndexKey) {
        self.index_lru.retain(|entry| *entry != key);
    }

    fn push_stream_address_buffer(&mut self, history_id: u64) {
        self.stream_address_buffer.push_back(history_id);
        while self.stream_address_buffer.len() > self.config.stream_address_buffer_entries() {
            self.stream_address_buffer.pop_front();
        }
    }

    fn history_entry(&self, history_id: u64) -> Option<&PifHistoryEntry> {
        self.history_buffer
            .iter()
            .find(|entry| entry.history_id == history_id)
    }

    fn next_stream_history_id(&self, history_id: u64) -> Option<u64> {
        let position = self
            .history_buffer
            .iter()
            .position(|entry| entry.history_id == history_id)?;
        self.history_buffer
            .get(position.saturating_add(1))
            .map(|entry| entry.history_id)
    }

    fn block_address(&self, address: Address) -> Address {
        Address::new(address.get() / self.config.line_size() * self.config.line_size())
    }
}

fn block_offset(trigger: Address, block: Address, line_size: u64) -> i64 {
    let trigger_block = trigger.get() / line_size;
    let target_block = block.get() / line_size;
    let offset = target_block as i128 - trigger_block as i128;
    offset.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

fn offset_address(trigger: Address, offset: i64, line_size: u64) -> Option<Address> {
    let signed = trigger.get() as i128 + offset as i128 * line_size as i128;
    if !(0..=u64::MAX as i128).contains(&signed) {
        return None;
    }
    Some(Address::new(signed as u64))
}

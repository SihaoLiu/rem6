use std::collections::BTreeSet;

use rem6_memory::{Address, CacheLineLayout};

use crate::allocation::{max_vector_len, MAX_VECTOR_ALLOCATION_BYTES};
use crate::indexing::{CacheIndexingLocation, CacheIndexingPolicyConfig, CacheIndexingPolicyKind};
use crate::replacement::{
    CacheReplacementPolicyConfig, CacheReplacementPolicyError, CacheReplacementPolicyKind,
    ReplacementDecision, ReplacementSet, ReplacementUpdate,
};

mod error;
mod snapshot;

pub use error::CacheCompressedTagsError;
pub use snapshot::{
    CacheCompressedTagEntrySnapshot, CacheCompressedTagSetSnapshot, CacheCompressedTagsSnapshot,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheCompressedTagsConfig {
    kind: CacheReplacementPolicyKind,
    line_layout: CacheLineLayout,
    superblock_layout: CacheLineLayout,
    sets: usize,
    ways: usize,
    max_compression_ratio: usize,
    indexing_config: CacheIndexingPolicyConfig,
    policy_config: CacheReplacementPolicyConfig,
}

impl CacheCompressedTagsConfig {
    pub fn new(
        kind: CacheReplacementPolicyKind,
        line_layout: CacheLineLayout,
        sets: usize,
        ways: usize,
        max_compression_ratio: usize,
    ) -> Result<Self, CacheCompressedTagsError> {
        Self::new_with_indexing(
            kind,
            CacheIndexingPolicyKind::SetAssociative,
            line_layout,
            sets,
            ways,
            max_compression_ratio,
        )
    }

    pub fn new_with_indexing(
        kind: CacheReplacementPolicyKind,
        indexing_kind: CacheIndexingPolicyKind,
        line_layout: CacheLineLayout,
        sets: usize,
        ways: usize,
        max_compression_ratio: usize,
    ) -> Result<Self, CacheCompressedTagsError> {
        if line_layout.bytes() < 4 {
            return Err(CacheCompressedTagsError::LineSizeTooSmall {
                bytes: line_layout.bytes(),
            });
        }
        if max_compression_ratio == 0 {
            return Err(CacheCompressedTagsError::ZeroMaxCompressionRatio);
        }
        if !max_compression_ratio.is_power_of_two() {
            return Err(CacheCompressedTagsError::MaxCompressionRatioNotPowerOfTwo {
                ratio: max_compression_ratio,
            });
        }
        validate_vector_length::<CacheCompressedTagSet>("sets", sets)?;
        validate_vector_length::<CacheCompressedTagEntry>("ways", ways)?;
        validate_vector_length::<Option<CacheCompressedTagLine>>(
            "max compression ratio",
            max_compression_ratio,
        )?;
        if kind == CacheReplacementPolicyKind::WeightedLru {
            return Err(CacheCompressedTagsError::UnsupportedReplacementPolicy { kind });
        }

        let superblock_layout = superblock_layout(line_layout, max_compression_ratio)?;
        let indexing_config =
            CacheIndexingPolicyConfig::new(indexing_kind, superblock_layout, sets, ways)
                .map_err(|source| CacheCompressedTagsError::IndexingPolicyConfig { source })?;
        let policy_config = CacheReplacementPolicyConfig::new(kind, ways)
            .map_err(|source| CacheCompressedTagsError::ReplacementPolicyConfig { source })?;

        Ok(Self {
            kind,
            line_layout,
            superblock_layout,
            sets,
            ways,
            max_compression_ratio,
            indexing_config,
            policy_config,
        })
    }

    pub const fn kind(&self) -> CacheReplacementPolicyKind {
        self.kind
    }

    pub const fn line_layout(&self) -> CacheLineLayout {
        self.line_layout
    }

    pub const fn superblock_layout(&self) -> CacheLineLayout {
        self.superblock_layout
    }

    pub const fn sets(&self) -> usize {
        self.sets
    }

    pub const fn ways(&self) -> usize {
        self.ways
    }

    pub const fn max_compression_ratio(&self) -> usize {
        self.max_compression_ratio
    }

    pub const fn indexing_config(&self) -> &CacheIndexingPolicyConfig {
        &self.indexing_config
    }

    pub const fn policy_config(&self) -> &CacheReplacementPolicyConfig {
        &self.policy_config
    }

    pub fn superblock_base(&self, address: Address) -> Address {
        self.superblock_layout.line_address(address)
    }

    pub fn superblock_offset(&self, address: Address) -> usize {
        let line = self.line_layout.line_address(address);
        ((line.get() - self.superblock_base(address).get()) / self.line_layout.bytes()) as usize
    }

    pub fn compression_factor_for_size(&self, compressed_size_bits: usize) -> usize {
        compression_factor(
            self.line_layout.bytes(),
            compressed_size_bits,
            self.max_compression_ratio,
        )
    }

    fn line_address(&self, address: Address) -> Address {
        self.line_layout.line_address(address)
    }

    fn locations_for_superblock(&self, superblock_base: Address) -> Vec<CacheIndexingLocation> {
        self.indexing_config.candidate_locations(superblock_base)
    }

    fn expected_set_for_way(&self, superblock_base: Address, way: usize) -> Option<usize> {
        self.locations_for_superblock(superblock_base)
            .into_iter()
            .find(|location| location.way() == way)
            .map(CacheIndexingLocation::set)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheCompressedTags {
    config: CacheCompressedTagsConfig,
    tick: u64,
    sets: Vec<CacheCompressedTagSet>,
}

impl CacheCompressedTags {
    pub fn new(config: CacheCompressedTagsConfig) -> Self {
        let sets = (0..config.sets())
            .map(|_| CacheCompressedTagSet::new(&config))
            .collect();
        Self {
            config,
            tick: 0,
            sets,
        }
    }

    pub const fn config(&self) -> &CacheCompressedTagsConfig {
        &self.config
    }

    pub fn find(&self, address: Address) -> Option<CacheCompressedTagLookup> {
        let superblock_base = self.config.superblock_base(address);
        let line = self.config.line_address(address);
        let offset = self.config.superblock_offset(address);
        self.superblock_location(superblock_base)
            .and_then(|(set, way)| {
                self.sets[set].entries[way].blocks[offset].map(|block| {
                    (block.line == line).then_some(CacheCompressedTagLookup {
                        line,
                        superblock_base,
                        set,
                        way,
                        offset,
                        compressed_size_bits: block.compressed_size_bits,
                        compressed: block.compressed,
                        compression_factor: self.sets[set].entries[way].compression_factor,
                    })
                })?
            })
    }

    pub fn insert(
        &mut self,
        address: Address,
        compressed_size_bits: usize,
    ) -> Result<CacheCompressedTagInsert, CacheCompressedTagsError> {
        self.insert_inner(address, compressed_size_bits, None)
    }

    pub fn insert_with_signature(
        &mut self,
        address: Address,
        compressed_size_bits: usize,
        signature: u64,
    ) -> Result<CacheCompressedTagInsert, CacheCompressedTagsError> {
        self.insert_inner(address, compressed_size_bits, Some(signature))
    }

    fn insert_inner(
        &mut self,
        address: Address,
        compressed_size_bits: usize,
        signature: Option<u64>,
    ) -> Result<CacheCompressedTagInsert, CacheCompressedTagsError> {
        self.require_signature(signature)?;
        let superblock_base = self.config.superblock_base(address);
        let line = self.config.line_address(address);
        let offset = self.config.superblock_offset(address);

        if let Some((set, way)) = self.superblock_location(superblock_base) {
            let can_coallocate = {
                let entry = &self.sets[set].entries[way];
                entry.blocks[offset].is_none()
                    && entry.can_coallocate(&self.config, compressed_size_bits)
            };
            if can_coallocate {
                let update = self.touch_replacement(set, way, signature)?;
                let entry = &mut self.sets[set].entries[way];
                let compression_factor = entry.compression_factor;
                entry.blocks[offset] = Some(CacheCompressedTagLine {
                    line,
                    compressed_size_bits,
                    compressed: entry.is_compressed(),
                });
                return Ok(CacheCompressedTagInsert {
                    line,
                    superblock_base,
                    set,
                    way,
                    offset,
                    compressed_size_bits,
                    compression_factor,
                    compressed: true,
                    co_allocated: true,
                    new_superblock: false,
                    evicted_lines: Vec::new(),
                    decision: None,
                    update,
                });
            }

            let decision = self.decision_for_superblock_location(superblock_base, set, way)?;
            return self.replace_superblock(
                (set, way, decision),
                address,
                compressed_size_bits,
                signature,
            );
        }

        let (set, way, decision) = self.victim_location(superblock_base)?;
        self.replace_superblock(
            (set, way, decision),
            address,
            compressed_size_bits,
            signature,
        )
    }

    fn replace_superblock(
        &mut self,
        victim: (usize, usize, ReplacementDecision),
        address: Address,
        compressed_size_bits: usize,
        signature: Option<u64>,
    ) -> Result<CacheCompressedTagInsert, CacheCompressedTagsError> {
        let (set, way, decision) = victim;
        let superblock_base = self.config.superblock_base(address);
        let line = self.config.line_address(address);
        let offset = self.config.superblock_offset(address);
        let update = self.reset_replacement(set, way, signature)?;
        let tick = self.next_tick();
        let entry = &mut self.sets[set].entries[way];
        let evicted_lines = entry.valid_lines();
        let compression_factor = self
            .config
            .compression_factor_for_size(compressed_size_bits);
        entry.superblock_base = Some(superblock_base);
        entry.blocks.fill(None);
        entry.compression_factor = compression_factor;
        entry.replacement_state.reset(self.config.kind(), tick);
        entry.blocks[offset] = Some(CacheCompressedTagLine {
            line,
            compressed_size_bits,
            compressed: compression_factor != 1,
        });

        Ok(CacheCompressedTagInsert {
            line,
            superblock_base,
            set,
            way,
            offset,
            compressed_size_bits,
            compression_factor,
            compressed: compression_factor != 1,
            co_allocated: false,
            new_superblock: true,
            evicted_lines,
            decision: Some(decision),
            update,
        })
    }

    pub fn access(
        &mut self,
        address: Address,
    ) -> Result<Option<CacheCompressedTagAccess>, CacheCompressedTagsError> {
        self.access_inner(address, None)
    }

    pub fn access_with_signature(
        &mut self,
        address: Address,
        signature: u64,
    ) -> Result<Option<CacheCompressedTagAccess>, CacheCompressedTagsError> {
        self.access_inner(address, Some(signature))
    }

    fn access_inner(
        &mut self,
        address: Address,
        signature: Option<u64>,
    ) -> Result<Option<CacheCompressedTagAccess>, CacheCompressedTagsError> {
        let Some(hit) = self.find(address) else {
            return Ok(None);
        };
        self.require_signature(signature)?;
        let update = self.touch_replacement(hit.set, hit.way, signature)?;
        Ok(Some(CacheCompressedTagAccess {
            line: hit.line,
            superblock_base: hit.superblock_base,
            set: hit.set,
            way: hit.way,
            offset: hit.offset,
            compressed_size_bits: hit.compressed_size_bits,
            compression_factor: hit.compression_factor,
            compressed: hit.compressed,
            update,
        }))
    }

    pub fn invalidate(
        &mut self,
        address: Address,
    ) -> Result<Option<CacheCompressedTagInvalidate>, CacheCompressedTagsError> {
        let superblock_base = self.config.superblock_base(address);
        let line = self.config.line_address(address);
        let offset = self.config.superblock_offset(address);
        let Some((set, way)) = self.superblock_location(superblock_base) else {
            return Ok(None);
        };
        let Some(block) = self.sets[set].entries[way].blocks[offset] else {
            return Ok(None);
        };
        if block.line != line {
            return Ok(None);
        }

        self.sets[set].entries[way].blocks[offset] = None;
        let superblock_still_valid = self.sets[set].entries[way].is_valid();
        let update = if superblock_still_valid {
            None
        } else {
            let entry = &mut self.sets[set].entries[way];
            entry.superblock_base = None;
            entry.compression_factor = 1;
            entry.replacement_state.invalidate();
            Some(
                self.sets[set]
                    .replacement
                    .invalidate(way)
                    .map_err(|source| CacheCompressedTagsError::ReplacementPolicyState {
                        source,
                    })?,
            )
        };
        let compression_factor = self.sets[set].entries[way].compression_factor;

        Ok(Some(CacheCompressedTagInvalidate {
            line,
            superblock_base,
            set,
            way,
            offset,
            compressed_size_bits: block.compressed_size_bits,
            compression_factor,
            compressed: block.compressed,
            superblock_still_valid,
            update,
        }))
    }

    pub fn superblock_lines(
        &self,
        set: usize,
        way: usize,
    ) -> Result<Vec<Option<Address>>, CacheCompressedTagsError> {
        self.check_set(set)?;
        self.check_way(way)?;
        Ok(self.sets[set].entries[way]
            .blocks
            .iter()
            .map(|block| block.map(|block| block.line))
            .collect())
    }

    pub fn resident_lines(&self) -> Vec<Address> {
        let mut lines = self
            .sets
            .iter()
            .flat_map(|set| set.entries.iter())
            .flat_map(CacheCompressedTagEntry::valid_lines)
            .collect::<Vec<_>>();
        lines.sort();
        lines
    }

    pub fn valid_superblock_count(&self) -> usize {
        self.sets
            .iter()
            .flat_map(|set| set.entries.iter())
            .filter(|entry| entry.is_valid())
            .count()
    }

    pub fn valid_line_count(&self) -> usize {
        self.sets
            .iter()
            .flat_map(|set| set.entries.iter())
            .map(CacheCompressedTagEntry::valid_count)
            .sum()
    }

    pub fn snapshot(&self) -> CacheCompressedTagsSnapshot {
        CacheCompressedTagsSnapshot {
            config: self.config.clone(),
            tick: self.tick,
            sets: self
                .sets
                .iter()
                .map(CacheCompressedTagSet::snapshot)
                .collect(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &CacheCompressedTagsSnapshot,
    ) -> Result<(), CacheCompressedTagsError> {
        self.validate_snapshot(snapshot)?;

        let mut restored = Vec::with_capacity(snapshot.sets.len());
        for set_snapshot in &snapshot.sets {
            let mut replacement = ReplacementSet::new(self.config.policy_config().clone());
            replacement
                .restore(&set_snapshot.replacement)
                .map_err(|source| CacheCompressedTagsError::ReplacementPolicyState { source })?;
            restored.push(CacheCompressedTagSet {
                entries: set_snapshot
                    .entries
                    .iter()
                    .map(CacheCompressedTagEntry::from_snapshot)
                    .collect(),
                replacement,
            });
        }
        self.tick = snapshot.tick;
        self.sets = restored;
        Ok(())
    }

    fn require_signature(&self, signature: Option<u64>) -> Result<(), CacheCompressedTagsError> {
        if matches!(self.config.kind(), CacheReplacementPolicyKind::Ship { .. })
            && signature.is_none()
        {
            return Err(CacheCompressedTagsError::ReplacementPolicyState {
                source: CacheReplacementPolicyError::SignatureRequired,
            });
        }
        Ok(())
    }

    fn touch_replacement(
        &mut self,
        set: usize,
        way: usize,
        signature: Option<u64>,
    ) -> Result<ReplacementUpdate, CacheCompressedTagsError> {
        let update = match signature {
            Some(signature) => self.sets[set]
                .replacement
                .touch_with_signature(way, signature),
            None => self.sets[set].replacement.touch(way),
        }
        .map_err(|source| CacheCompressedTagsError::ReplacementPolicyState { source })?;
        let tick = self.next_tick();
        self.sets[set].entries[way]
            .replacement_state
            .touch(self.config.kind(), tick);
        Ok(update)
    }

    fn reset_replacement(
        &mut self,
        set: usize,
        way: usize,
        signature: Option<u64>,
    ) -> Result<ReplacementUpdate, CacheCompressedTagsError> {
        match signature {
            Some(signature) => self.sets[set]
                .replacement
                .reset_with_signature(way, signature),
            None => self.sets[set].replacement.reset(way),
        }
        .map_err(|source| CacheCompressedTagsError::ReplacementPolicyState { source })
    }

    fn superblock_location(&self, superblock_base: Address) -> Option<(usize, usize)> {
        self.config
            .locations_for_superblock(superblock_base)
            .into_iter()
            .find(|location| {
                self.sets[location.set()].entries[location.way()].superblock_base
                    == Some(superblock_base)
            })
            .map(|location| (location.set(), location.way()))
    }

    fn victim_location(
        &mut self,
        superblock_base: Address,
    ) -> Result<(usize, usize, ReplacementDecision), CacheCompressedTagsError> {
        let locations = self.config.locations_for_superblock(superblock_base);
        if self.config.indexing_config().kind() == CacheIndexingPolicyKind::SetAssociative {
            let set = locations
                .first()
                .ok_or(CacheCompressedTagsError::ReplacementPolicyState {
                    source: CacheReplacementPolicyError::NoCandidates,
                })?
                .set();
            let decision = self.sets[set]
                .replacement
                .victim(0..self.config.ways())
                .map_err(|source| CacheCompressedTagsError::ReplacementPolicyState { source })?;
            return Ok((set, decision.way(), decision));
        }

        let selected = self.select_cross_set_victim(&locations)?;
        let decision =
            self.decision_for_superblock_location(superblock_base, selected.set(), selected.way())?;
        Ok((selected.set(), selected.way(), decision))
    }

    fn decision_for_superblock_location(
        &mut self,
        superblock_base: Address,
        set: usize,
        way: usize,
    ) -> Result<ReplacementDecision, CacheCompressedTagsError> {
        let candidates = self
            .config
            .locations_for_superblock(superblock_base)
            .iter()
            .map(|location| location.way())
            .collect::<Vec<_>>();
        self.sets[set]
            .replacement
            .decision_for_selected_victim(way, candidates)
            .map_err(|source| CacheCompressedTagsError::ReplacementPolicyState { source })
    }

    fn select_cross_set_victim(
        &mut self,
        locations: &[CacheIndexingLocation],
    ) -> Result<CacheIndexingLocation, CacheCompressedTagsError> {
        match self.config.kind() {
            CacheReplacementPolicyKind::Brrip { rrpv_bits, .. }
            | CacheReplacementPolicyKind::Ship { rrpv_bits, .. } => {
                self.select_cross_set_brrip_victim(locations, rrpv_bits)
            }
            CacheReplacementPolicyKind::Lru
            | CacheReplacementPolicyKind::WeightedLru
            | CacheReplacementPolicyKind::Fifo
            | CacheReplacementPolicyKind::Mru
            | CacheReplacementPolicyKind::Lfu
            | CacheReplacementPolicyKind::Bip { .. }
            | CacheReplacementPolicyKind::TreePlru => {
                self.select_cross_set_metadata_victim(locations)
            }
            CacheReplacementPolicyKind::SecondChance => {
                self.select_cross_set_second_chance_victim(locations)
            }
        }
    }

    fn select_cross_set_second_chance_victim(
        &mut self,
        locations: &[CacheIndexingLocation],
    ) -> Result<CacheIndexingLocation, CacheCompressedTagsError> {
        if locations.is_empty() {
            return Err(CacheCompressedTagsError::ReplacementPolicyState {
                source: CacheReplacementPolicyError::NoCandidates,
            });
        }

        for location in locations {
            if !self.sets[location.set()].entries[location.way()]
                .replacement_state
                .valid
            {
                return Ok(*location);
            }
        }

        loop {
            let selected = self.select_cross_set_metadata_victim(locations)?;
            let state = self.sets[selected.set()].entries[selected.way()].replacement_state;
            if !state.second_chance {
                return Ok(selected);
            }

            let tick = self.next_tick();
            let state = &mut self.sets[selected.set()].entries[selected.way()].replacement_state;
            state.insertion_tick = tick;
            state.second_chance = false;
        }
    }

    fn select_cross_set_brrip_victim(
        &mut self,
        locations: &[CacheIndexingLocation],
        rrpv_bits: u8,
    ) -> Result<CacheIndexingLocation, CacheCompressedTagsError> {
        if locations.is_empty() {
            return Err(CacheCompressedTagsError::ReplacementPolicyState {
                source: CacheReplacementPolicyError::NoCandidates,
            });
        }

        for location in locations {
            if !self.sets[location.set()]
                .replacement
                .entry(location.way())
                .map_err(|source| CacheCompressedTagsError::ReplacementPolicyState { source })?
                .valid()
            {
                return Ok(*location);
            }
        }

        let max = (1u64 << rrpv_bits) - 1;
        let mut highest = 0;
        for location in locations {
            highest = highest.max(
                self.sets[location.set()]
                    .replacement
                    .entry(location.way())
                    .map_err(|source| CacheCompressedTagsError::ReplacementPolicyState { source })?
                    .rrpv(),
            );
        }
        if highest < max {
            let increment = max - highest;
            for location in locations {
                self.sets[location.set()]
                    .replacement
                    .age_rrpv_candidate(location.way(), increment, max)
                    .map_err(|source| CacheCompressedTagsError::ReplacementPolicyState {
                        source,
                    })?;
            }
        }

        self.select_cross_set_metadata_victim(locations)
    }

    fn select_cross_set_metadata_victim(
        &self,
        locations: &[CacheIndexingLocation],
    ) -> Result<CacheIndexingLocation, CacheCompressedTagsError> {
        let Some(first) = locations.first().copied() else {
            return Err(CacheCompressedTagsError::ReplacementPolicyState {
                source: CacheReplacementPolicyError::NoCandidates,
            });
        };

        let mut selected = first;
        for location in &locations[1..] {
            if self.location_precedes_for_victim(*location, selected)? {
                selected = *location;
            }
        }
        Ok(selected)
    }

    fn location_precedes_for_victim(
        &self,
        current: CacheIndexingLocation,
        selected: CacheIndexingLocation,
    ) -> Result<bool, CacheCompressedTagsError> {
        let current_entry = &self.sets[current.set()].entries[current.way()];
        let selected_entry = &self.sets[selected.set()].entries[selected.way()];
        let current_state = current_entry.replacement_state;
        let selected_state = selected_entry.replacement_state;
        if !selected_state.valid {
            return Ok(false);
        }
        if !current_state.valid {
            return Ok(true);
        }
        let precedes = match self.config.kind() {
            CacheReplacementPolicyKind::Lru
            | CacheReplacementPolicyKind::WeightedLru
            | CacheReplacementPolicyKind::Bip { .. } => {
                current_state.last_touch_tick < selected_state.last_touch_tick
            }
            CacheReplacementPolicyKind::Fifo | CacheReplacementPolicyKind::SecondChance => {
                current_state.insertion_tick < selected_state.insertion_tick
            }
            CacheReplacementPolicyKind::Mru => {
                current_state.last_touch_tick > selected_state.last_touch_tick
            }
            CacheReplacementPolicyKind::Lfu => {
                current_state.reference_count < selected_state.reference_count
            }
            CacheReplacementPolicyKind::Brrip { .. } | CacheReplacementPolicyKind::Ship { .. } => {
                let current_entry = self.sets[current.set()]
                    .replacement
                    .entry(current.way())
                    .map_err(|source| CacheCompressedTagsError::ReplacementPolicyState {
                        source,
                    })?;
                let selected_entry = self.sets[selected.set()]
                    .replacement
                    .entry(selected.way())
                    .map_err(|source| CacheCompressedTagsError::ReplacementPolicyState {
                        source,
                    })?;
                if !selected_entry.valid() {
                    false
                } else if !current_entry.valid() {
                    true
                } else {
                    current_entry.rrpv() > selected_entry.rrpv()
                }
            }
            CacheReplacementPolicyKind::TreePlru => false,
        };
        Ok(precedes)
    }

    fn validate_snapshot(
        &self,
        snapshot: &CacheCompressedTagsSnapshot,
    ) -> Result<(), CacheCompressedTagsError> {
        if self.config != snapshot.config {
            return Err(CacheCompressedTagsError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config.clone()),
            });
        }
        if snapshot.sets.len() != self.config.sets() {
            return Err(CacheCompressedTagsError::SnapshotSetCountMismatch {
                sets: snapshot.sets.len(),
                expected_sets: self.config.sets(),
            });
        }

        let mut seen = BTreeSet::new();
        let mut seen_superblocks = BTreeSet::new();
        for (set_index, set_snapshot) in snapshot.sets.iter().enumerate() {
            if set_snapshot.entries.len() != self.config.ways() {
                return Err(CacheCompressedTagsError::SnapshotWayCountMismatch {
                    set: set_index,
                    ways: set_snapshot.entries.len(),
                    expected_ways: self.config.ways(),
                });
            }
            for (way_index, entry_snapshot) in set_snapshot.entries.iter().enumerate() {
                self.validate_entry_snapshot(
                    set_index,
                    way_index,
                    entry_snapshot,
                    &mut seen,
                    &mut seen_superblocks,
                )?;
            }
        }
        Ok(())
    }

    fn validate_entry_snapshot(
        &self,
        set: usize,
        way: usize,
        snapshot: &CacheCompressedTagEntrySnapshot,
        seen: &mut BTreeSet<Address>,
        seen_superblocks: &mut BTreeSet<Address>,
    ) -> Result<(), CacheCompressedTagsError> {
        if snapshot.blocks.len() != self.config.max_compression_ratio() {
            return Err(CacheCompressedTagsError::SnapshotBlockCountMismatch {
                set,
                way,
                blocks: snapshot.blocks.len(),
                expected_blocks: self.config.max_compression_ratio(),
            });
        }
        if snapshot.compression_factor == 0
            || snapshot.compression_factor > self.config.max_compression_ratio()
            || !snapshot.compression_factor.is_power_of_two()
        {
            return Err(CacheCompressedTagsError::SnapshotInvalidCompressionFactor {
                set,
                way,
                factor: snapshot.compression_factor,
                max_factor: self.config.max_compression_ratio(),
            });
        }
        let valid_blocks = snapshot
            .blocks
            .iter()
            .filter(|block| block.is_some())
            .count();
        let superblock_base = snapshot.superblock_base;
        if valid_blocks == 0 && (superblock_base.is_some() || snapshot.compression_factor != 1) {
            return Err(CacheCompressedTagsError::SnapshotEmptySuperblock {
                set,
                way,
                compression_factor: snapshot.compression_factor,
                has_superblock_base: superblock_base.is_some(),
            });
        }
        if valid_blocks > snapshot.compression_factor {
            return Err(
                CacheCompressedTagsError::SnapshotSuperblockCapacityExceeded {
                    set,
                    way,
                    valid_blocks,
                    compression_factor: snapshot.compression_factor,
                },
            );
        }
        let expected_compressed = snapshot.compression_factor != 1;
        let maximum_compressed_size_bits =
            block_bits(self.config.line_layout()) / snapshot.compression_factor.max(1);
        let mut oversized_blocks = 0;

        if let Some(superblock_base) = superblock_base {
            if !seen_superblocks.insert(superblock_base) {
                return Err(CacheCompressedTagsError::SnapshotDuplicateSuperblock {
                    superblock_base,
                });
            }
            if self.config.superblock_base(superblock_base) != superblock_base {
                return Err(CacheCompressedTagsError::SnapshotMisalignedSuperblock {
                    superblock_base,
                });
            }
            let expected_set = self
                .config
                .expected_set_for_way(superblock_base, way)
                .ok_or(CacheCompressedTagsError::UnknownWay {
                    way,
                    ways: self.config.ways(),
                })?;
            if set != expected_set {
                return Err(CacheCompressedTagsError::SnapshotSuperblockSetMismatch {
                    superblock_base,
                    set,
                    expected_set,
                });
            }
        }

        for (offset, block) in snapshot.blocks.iter().enumerate() {
            let Some(block) = *block else {
                continue;
            };
            let Some(superblock_base) = superblock_base else {
                return Err(CacheCompressedTagsError::SnapshotLineWithoutSuperblock {
                    set,
                    way,
                    offset,
                });
            };
            if self.config.line_address(block.line) != block.line {
                return Err(CacheCompressedTagsError::SnapshotMisalignedLine { line: block.line });
            }
            let expected_superblock_base = self.config.superblock_base(block.line);
            if expected_superblock_base != superblock_base {
                return Err(CacheCompressedTagsError::SnapshotLineSuperblockMismatch {
                    line: block.line,
                    superblock_base,
                    expected_superblock_base,
                });
            }
            let expected_offset = self.config.superblock_offset(block.line);
            if offset != expected_offset {
                return Err(CacheCompressedTagsError::SnapshotLineOffsetMismatch {
                    line: block.line,
                    offset,
                    expected_offset,
                });
            }
            if !seen.insert(block.line) {
                return Err(CacheCompressedTagsError::SnapshotDuplicateLine { line: block.line });
            }
            if snapshot.compression_factor != 1
                && block.compressed_size_bits > maximum_compressed_size_bits
            {
                oversized_blocks += 1;
                if oversized_blocks > 1
                    || self
                        .config
                        .compression_factor_for_size(block.compressed_size_bits)
                        != snapshot.compression_factor
                {
                    return Err(CacheCompressedTagsError::SnapshotCompressedSizeTooLarge {
                        set,
                        way,
                        offset,
                        compressed_size_bits: block.compressed_size_bits,
                        maximum_bits: maximum_compressed_size_bits,
                    });
                }
            }
            if block.compressed != expected_compressed {
                return Err(CacheCompressedTagsError::SnapshotCompressedFlagMismatch {
                    set,
                    way,
                    offset,
                    compressed: block.compressed,
                    expected_compressed,
                });
            }
        }
        Ok(())
    }

    fn check_set(&self, set: usize) -> Result<(), CacheCompressedTagsError> {
        if set >= self.config.sets() {
            return Err(CacheCompressedTagsError::UnknownSet {
                set,
                sets: self.config.sets(),
            });
        }
        Ok(())
    }

    fn check_way(&self, way: usize) -> Result<(), CacheCompressedTagsError> {
        if way >= self.config.ways() {
            return Err(CacheCompressedTagsError::UnknownWay {
                way,
                ways: self.config.ways(),
            });
        }
        Ok(())
    }

    fn next_tick(&mut self) -> u64 {
        self.tick = self.tick.saturating_add(1);
        self.tick
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheCompressedTagLookup {
    line: Address,
    superblock_base: Address,
    set: usize,
    way: usize,
    offset: usize,
    compressed_size_bits: usize,
    compression_factor: usize,
    compressed: bool,
}

impl CacheCompressedTagLookup {
    pub const fn line(self) -> Address {
        self.line
    }

    pub const fn superblock_base(self) -> Address {
        self.superblock_base
    }

    pub const fn set(self) -> usize {
        self.set
    }

    pub const fn way(self) -> usize {
        self.way
    }

    pub const fn offset(self) -> usize {
        self.offset
    }

    pub const fn compressed_size_bits(self) -> usize {
        self.compressed_size_bits
    }

    pub const fn compression_factor(self) -> usize {
        self.compression_factor
    }

    pub const fn compressed(self) -> bool {
        self.compressed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheCompressedTagInsert {
    line: Address,
    superblock_base: Address,
    set: usize,
    way: usize,
    offset: usize,
    compressed_size_bits: usize,
    compression_factor: usize,
    compressed: bool,
    co_allocated: bool,
    new_superblock: bool,
    evicted_lines: Vec<Address>,
    decision: Option<ReplacementDecision>,
    update: ReplacementUpdate,
}

impl CacheCompressedTagInsert {
    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn superblock_base(&self) -> Address {
        self.superblock_base
    }

    pub const fn set(&self) -> usize {
        self.set
    }

    pub const fn way(&self) -> usize {
        self.way
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }

    pub const fn compressed_size_bits(&self) -> usize {
        self.compressed_size_bits
    }

    pub const fn compression_factor(&self) -> usize {
        self.compression_factor
    }

    pub const fn compressed(&self) -> bool {
        self.compressed
    }

    pub const fn co_allocated(&self) -> bool {
        self.co_allocated
    }

    pub const fn new_superblock(&self) -> bool {
        self.new_superblock
    }

    pub fn evicted_lines(&self) -> &[Address] {
        &self.evicted_lines
    }

    pub const fn decision(&self) -> Option<&ReplacementDecision> {
        self.decision.as_ref()
    }

    pub const fn update(&self) -> &ReplacementUpdate {
        &self.update
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheCompressedTagInvalidate {
    line: Address,
    superblock_base: Address,
    set: usize,
    way: usize,
    offset: usize,
    compressed_size_bits: usize,
    compression_factor: usize,
    compressed: bool,
    superblock_still_valid: bool,
    update: Option<ReplacementUpdate>,
}

impl CacheCompressedTagInvalidate {
    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn superblock_base(&self) -> Address {
        self.superblock_base
    }

    pub const fn set(&self) -> usize {
        self.set
    }

    pub const fn way(&self) -> usize {
        self.way
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }

    pub const fn compressed_size_bits(&self) -> usize {
        self.compressed_size_bits
    }

    pub const fn compression_factor(&self) -> usize {
        self.compression_factor
    }

    pub const fn compressed(&self) -> bool {
        self.compressed
    }

    pub const fn superblock_still_valid(&self) -> bool {
        self.superblock_still_valid
    }

    pub const fn update(&self) -> Option<&ReplacementUpdate> {
        self.update.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheCompressedTagAccess {
    line: Address,
    superblock_base: Address,
    set: usize,
    way: usize,
    offset: usize,
    compressed_size_bits: usize,
    compression_factor: usize,
    compressed: bool,
    update: ReplacementUpdate,
}

impl CacheCompressedTagAccess {
    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn superblock_base(&self) -> Address {
        self.superblock_base
    }

    pub const fn set(&self) -> usize {
        self.set
    }

    pub const fn way(&self) -> usize {
        self.way
    }

    pub const fn offset(&self) -> usize {
        self.offset
    }

    pub const fn compressed_size_bits(&self) -> usize {
        self.compressed_size_bits
    }

    pub const fn compression_factor(&self) -> usize {
        self.compression_factor
    }

    pub const fn compressed(&self) -> bool {
        self.compressed
    }

    pub const fn update(&self) -> &ReplacementUpdate {
        &self.update
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CacheCompressedTagSet {
    entries: Vec<CacheCompressedTagEntry>,
    replacement: ReplacementSet,
}

impl CacheCompressedTagSet {
    fn new(config: &CacheCompressedTagsConfig) -> Self {
        Self {
            entries: (0..config.ways())
                .map(|_| CacheCompressedTagEntry::new(config.max_compression_ratio()))
                .collect(),
            replacement: ReplacementSet::new(config.policy_config().clone()),
        }
    }

    fn snapshot(&self) -> CacheCompressedTagSetSnapshot {
        CacheCompressedTagSetSnapshot {
            entries: self
                .entries
                .iter()
                .map(CacheCompressedTagEntry::snapshot)
                .collect(),
            replacement: self.replacement.snapshot(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CacheCompressedTagEntry {
    superblock_base: Option<Address>,
    blocks: Vec<Option<CacheCompressedTagLine>>,
    compression_factor: usize,
    replacement_state: CacheCompressedTagReplacementState,
}

impl CacheCompressedTagEntry {
    fn new(max_compression_ratio: usize) -> Self {
        Self {
            superblock_base: None,
            blocks: vec![None; max_compression_ratio],
            compression_factor: 1,
            replacement_state: CacheCompressedTagReplacementState::new(),
        }
    }

    fn from_snapshot(snapshot: &CacheCompressedTagEntrySnapshot) -> Self {
        Self {
            superblock_base: snapshot.superblock_base,
            blocks: snapshot.blocks.clone(),
            compression_factor: snapshot.compression_factor,
            replacement_state: snapshot.replacement_state,
        }
    }

    fn is_valid(&self) -> bool {
        self.blocks.iter().any(Option::is_some)
    }

    fn valid_count(&self) -> usize {
        self.blocks.iter().filter(|block| block.is_some()).count()
    }

    fn valid_lines(&self) -> Vec<Address> {
        self.blocks
            .iter()
            .flatten()
            .map(|block| block.line)
            .collect()
    }

    fn is_compressed(&self) -> bool {
        self.blocks
            .iter()
            .flatten()
            .next()
            .is_none_or(|block| block.compressed)
    }

    fn can_coallocate(
        &self,
        config: &CacheCompressedTagsConfig,
        compressed_size_bits: usize,
    ) -> bool {
        self.is_compressed()
            && self.valid_count() < self.compression_factor
            && compressed_size_bits
                <= block_bits(config.line_layout()) / self.compression_factor.max(1)
    }

    fn snapshot(&self) -> CacheCompressedTagEntrySnapshot {
        CacheCompressedTagEntrySnapshot {
            superblock_base: self.superblock_base,
            blocks: self.blocks.clone(),
            compression_factor: self.compression_factor,
            replacement_state: self.replacement_state,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheCompressedTagLine {
    line: Address,
    compressed_size_bits: usize,
    compressed: bool,
}

impl CacheCompressedTagLine {
    pub const fn new(line: Address, compressed_size_bits: usize, compressed: bool) -> Self {
        Self {
            line,
            compressed_size_bits,
            compressed,
        }
    }

    pub const fn line(self) -> Address {
        self.line
    }

    pub const fn compressed_size_bits(self) -> usize {
        self.compressed_size_bits
    }

    pub const fn compressed(self) -> bool {
        self.compressed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CacheCompressedTagReplacementState {
    valid: bool,
    last_touch_tick: u64,
    insertion_tick: u64,
    reference_count: u64,
    second_chance: bool,
}

impl CacheCompressedTagReplacementState {
    const fn new() -> Self {
        Self {
            valid: false,
            last_touch_tick: 0,
            insertion_tick: 0,
            reference_count: 0,
            second_chance: false,
        }
    }

    fn from_blocks(blocks: &[Option<CacheCompressedTagLine>]) -> Self {
        let mut state = Self::new();
        state.valid = blocks.iter().any(Option::is_some);
        state
    }

    fn reset(&mut self, kind: CacheReplacementPolicyKind, tick: u64) {
        self.valid = true;
        match kind {
            CacheReplacementPolicyKind::Lru
            | CacheReplacementPolicyKind::WeightedLru
            | CacheReplacementPolicyKind::Mru
            | CacheReplacementPolicyKind::Bip { .. }
            | CacheReplacementPolicyKind::TreePlru
            | CacheReplacementPolicyKind::Brrip { .. }
            | CacheReplacementPolicyKind::Ship { .. } => {
                self.last_touch_tick = tick;
            }
            CacheReplacementPolicyKind::Fifo | CacheReplacementPolicyKind::SecondChance => {
                self.insertion_tick = tick;
                self.second_chance = false;
            }
            CacheReplacementPolicyKind::Lfu => {
                self.reference_count = 1;
            }
        }
    }

    fn touch(&mut self, kind: CacheReplacementPolicyKind, tick: u64) {
        self.valid = true;
        match kind {
            CacheReplacementPolicyKind::Lru
            | CacheReplacementPolicyKind::WeightedLru
            | CacheReplacementPolicyKind::Mru
            | CacheReplacementPolicyKind::Bip { .. }
            | CacheReplacementPolicyKind::TreePlru
            | CacheReplacementPolicyKind::Brrip { .. }
            | CacheReplacementPolicyKind::Ship { .. } => {
                self.last_touch_tick = tick;
            }
            CacheReplacementPolicyKind::Fifo => {}
            CacheReplacementPolicyKind::SecondChance => {
                self.second_chance = true;
            }
            CacheReplacementPolicyKind::Lfu => {
                self.reference_count = self.reference_count.saturating_add(1);
            }
        }
    }

    fn invalidate(&mut self) {
        *self = Self::new();
    }
}

fn validate_vector_length<T>(
    field: &'static str,
    length: usize,
) -> Result<(), CacheCompressedTagsError> {
    let maximum = max_vector_len::<T>();
    if length > maximum {
        return Err(CacheCompressedTagsError::VectorLengthTooLarge {
            field,
            length,
            maximum,
        });
    }
    Ok(())
}

fn superblock_layout(
    line_layout: CacheLineLayout,
    max_compression_ratio: usize,
) -> Result<CacheLineLayout, CacheCompressedTagsError> {
    let line_bytes = line_layout.bytes();
    let Some(ratio) = u64::try_from(max_compression_ratio).ok() else {
        return Err(CacheCompressedTagsError::SuperblockSpanTooLarge {
            line_bytes,
            max_compression_ratio,
        });
    };
    let Some(superblock_bytes) = line_bytes.checked_mul(ratio) else {
        return Err(CacheCompressedTagsError::SuperblockSpanTooLarge {
            line_bytes,
            max_compression_ratio,
        });
    };
    if superblock_bytes > MAX_VECTOR_ALLOCATION_BYTES as u64 {
        return Err(CacheCompressedTagsError::SuperblockSpanTooLarge {
            line_bytes,
            max_compression_ratio,
        });
    }
    CacheLineLayout::new(superblock_bytes).map_err(|_| {
        CacheCompressedTagsError::SuperblockSpanTooLarge {
            line_bytes,
            max_compression_ratio,
        }
    })
}

fn compression_factor(
    line_bytes: u64,
    compressed_size_bits: usize,
    max_compression_ratio: usize,
) -> usize {
    let bits = line_bytes as u128 * 8;
    if compressed_size_bits == 0 {
        return max_compression_ratio;
    }
    if compressed_size_bits as u128 > bits {
        return 1;
    }
    let ratio = bits / compressed_size_bits as u128;
    next_power_of_two_capped(ratio, max_compression_ratio)
}

fn next_power_of_two_capped(value: u128, cap: usize) -> usize {
    if value == 0 {
        return 1;
    }
    let mut power = 1usize;
    while (power as u128) < value && power < cap {
        power = power.saturating_mul(2).min(cap);
    }
    power.min(cap).max(1)
}

fn block_bits(line_layout: CacheLineLayout) -> usize {
    let bits = line_layout.bytes() as u128 * 8;
    bits.min(usize::MAX as u128) as usize
}

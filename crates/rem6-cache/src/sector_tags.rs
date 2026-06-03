use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use rem6_memory::{Address, CacheLineLayout};

use crate::allocation::{max_vector_len, MAX_VECTOR_ALLOCATION_BYTES};
use crate::indexing::{
    CacheIndexingLocation, CacheIndexingPolicyConfig, CacheIndexingPolicyError,
    CacheIndexingPolicyKind,
};
use crate::replacement::{
    CacheReplacementPolicyConfig, CacheReplacementPolicyError, CacheReplacementPolicyKind,
    ReplacementDecision, ReplacementSet, ReplacementSetSnapshot, ReplacementUpdate,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheSectorTagsConfig {
    kind: CacheReplacementPolicyKind,
    line_layout: CacheLineLayout,
    sector_layout: CacheLineLayout,
    sets: usize,
    ways: usize,
    blocks_per_sector: usize,
    indexing_config: CacheIndexingPolicyConfig,
    policy_config: CacheReplacementPolicyConfig,
}

impl CacheSectorTagsConfig {
    pub fn new(
        kind: CacheReplacementPolicyKind,
        line_layout: CacheLineLayout,
        sets: usize,
        ways: usize,
        blocks_per_sector: usize,
    ) -> Result<Self, CacheSectorTagsError> {
        Self::new_with_indexing(
            kind,
            CacheIndexingPolicyKind::SetAssociative,
            line_layout,
            sets,
            ways,
            blocks_per_sector,
        )
    }

    pub fn new_with_indexing(
        kind: CacheReplacementPolicyKind,
        indexing_kind: CacheIndexingPolicyKind,
        line_layout: CacheLineLayout,
        sets: usize,
        ways: usize,
        blocks_per_sector: usize,
    ) -> Result<Self, CacheSectorTagsError> {
        if line_layout.bytes() < 4 {
            return Err(CacheSectorTagsError::LineSizeTooSmall {
                bytes: line_layout.bytes(),
            });
        }
        if blocks_per_sector == 0 {
            return Err(CacheSectorTagsError::ZeroBlocksPerSector);
        }
        if !blocks_per_sector.is_power_of_two() {
            return Err(CacheSectorTagsError::BlocksPerSectorNotPowerOfTwo {
                blocks: blocks_per_sector,
            });
        }
        validate_vector_length::<CacheSectorTagSet>("sets", sets)?;
        validate_vector_length::<CacheSectorTagEntry>("ways", ways)?;
        validate_vector_length::<Option<Address>>("blocks per sector", blocks_per_sector)?;

        let sector_layout = sector_layout(line_layout, blocks_per_sector)?;
        let indexing_config =
            CacheIndexingPolicyConfig::new(indexing_kind, sector_layout, sets, ways)
                .map_err(|source| CacheSectorTagsError::IndexingPolicyConfig { source })?;
        let policy_config = CacheReplacementPolicyConfig::new(kind, ways)
            .map_err(|source| CacheSectorTagsError::ReplacementPolicyConfig { source })?;

        Ok(Self {
            kind,
            line_layout,
            sector_layout,
            sets,
            ways,
            blocks_per_sector,
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

    pub const fn sector_layout(&self) -> CacheLineLayout {
        self.sector_layout
    }

    pub const fn sets(&self) -> usize {
        self.sets
    }

    pub const fn ways(&self) -> usize {
        self.ways
    }

    pub const fn blocks_per_sector(&self) -> usize {
        self.blocks_per_sector
    }

    pub const fn indexing_config(&self) -> &CacheIndexingPolicyConfig {
        &self.indexing_config
    }

    pub const fn policy_config(&self) -> &CacheReplacementPolicyConfig {
        &self.policy_config
    }

    pub fn sector_base(&self, address: Address) -> Address {
        self.sector_layout.line_address(address)
    }

    pub fn sector_offset(&self, address: Address) -> usize {
        let line = self.line_layout.line_address(address);
        ((line.get() - self.sector_base(address).get()) / self.line_layout.bytes()) as usize
    }

    fn line_address(&self, address: Address) -> Address {
        self.line_layout.line_address(address)
    }

    fn locations_for_sector(&self, sector_base: Address) -> Vec<CacheIndexingLocation> {
        self.indexing_config.candidate_locations(sector_base)
    }

    fn expected_set_for_way(&self, sector_base: Address, way: usize) -> Option<usize> {
        self.locations_for_sector(sector_base)
            .into_iter()
            .find(|location| location.way() == way)
            .map(CacheIndexingLocation::set)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheSectorTagsError {
    ZeroBlocksPerSector,
    BlocksPerSectorNotPowerOfTwo {
        blocks: usize,
    },
    LineSizeTooSmall {
        bytes: u64,
    },
    SectorSpanTooLarge {
        line_bytes: u64,
        blocks_per_sector: usize,
    },
    VectorLengthTooLarge {
        field: &'static str,
        length: usize,
        maximum: usize,
    },
    IndexingPolicyConfig {
        source: CacheIndexingPolicyError,
    },
    ReplacementPolicyConfig {
        source: CacheReplacementPolicyError,
    },
    ReplacementPolicyState {
        source: CacheReplacementPolicyError,
    },
    UnknownSet {
        set: usize,
        sets: usize,
    },
    UnknownWay {
        way: usize,
        ways: usize,
    },
    SnapshotConfigMismatch {
        expected: Box<CacheSectorTagsConfig>,
        actual: Box<CacheSectorTagsConfig>,
    },
    SnapshotSetCountMismatch {
        sets: usize,
        expected_sets: usize,
    },
    SnapshotWayCountMismatch {
        set: usize,
        ways: usize,
        expected_ways: usize,
    },
    SnapshotBlocksPerSectorMismatch {
        set: usize,
        way: usize,
        blocks: usize,
        expected_blocks: usize,
    },
    SnapshotLineWithoutSector {
        set: usize,
        way: usize,
        offset: usize,
    },
    SnapshotMisalignedSector {
        sector_base: Address,
    },
    SnapshotSectorSetMismatch {
        sector_base: Address,
        set: usize,
        expected_set: usize,
    },
    SnapshotMisalignedLine {
        line: Address,
    },
    SnapshotLineSectorMismatch {
        line: Address,
        sector_base: Address,
        expected_sector_base: Address,
    },
    SnapshotLineOffsetMismatch {
        line: Address,
        offset: usize,
        expected_offset: usize,
    },
    SnapshotDuplicateLine {
        line: Address,
    },
    SnapshotDuplicateSector {
        sector_base: Address,
    },
}

impl fmt::Display for CacheSectorTagsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroBlocksPerSector => write!(formatter, "cache sector tags have no blocks per sector"),
            Self::BlocksPerSectorNotPowerOfTwo { blocks } => write!(
                formatter,
                "cache sector tags need a power-of-two blocks-per-sector count, got {blocks}"
            ),
            Self::LineSizeTooSmall { bytes } => write!(
                formatter,
                "cache sector tags need a cache line size of at least 4 bytes, got {bytes}"
            ),
            Self::SectorSpanTooLarge {
                line_bytes,
                blocks_per_sector,
            } => write!(
                formatter,
                "cache sector span with line size {line_bytes} and {blocks_per_sector} blocks per sector is too large"
            ),
            Self::VectorLengthTooLarge {
                field,
                length,
                maximum,
            } => write!(
                formatter,
                "cache sector tags {field} length {length} exceeds vector allocation limit {maximum}"
            ),
            Self::IndexingPolicyConfig { source } => {
                write!(formatter, "cache sector tag indexing config is invalid: {source}")
            }
            Self::ReplacementPolicyConfig { source } => {
                write!(formatter, "cache sector tag replacement config is invalid: {source}")
            }
            Self::ReplacementPolicyState { source } => {
                write!(formatter, "cache sector tag replacement state is invalid: {source}")
            }
            Self::UnknownSet { set, sets } => {
                write!(formatter, "cache sector tag set {set} is outside {sets} sets")
            }
            Self::UnknownWay { way, ways } => {
                write!(formatter, "cache sector tag way {way} is outside {ways} ways")
            }
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "cache sector tag snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotSetCountMismatch {
                sets,
                expected_sets,
            } => write!(
                formatter,
                "cache sector tag snapshot has {sets} sets instead of {expected_sets}"
            ),
            Self::SnapshotWayCountMismatch {
                set,
                ways,
                expected_ways,
            } => write!(
                formatter,
                "cache sector tag snapshot set {set} has {ways} ways instead of {expected_ways}"
            ),
            Self::SnapshotBlocksPerSectorMismatch {
                set,
                way,
                blocks,
                expected_blocks,
            } => write!(
                formatter,
                "cache sector tag snapshot set {set} way {way} has {blocks} blocks instead of {expected_blocks}"
            ),
            Self::SnapshotLineWithoutSector { set, way, offset } => write!(
                formatter,
                "cache sector tag snapshot has a line in set {set} way {way} offset {offset} without a sector"
            ),
            Self::SnapshotMisalignedSector { sector_base } => write!(
                formatter,
                "cache sector tag snapshot sector {:#x} is not sector-aligned",
                sector_base.get()
            ),
            Self::SnapshotSectorSetMismatch {
                sector_base,
                set,
                expected_set,
            } => write!(
                formatter,
                "cache sector tag snapshot sector {:#x} is in set {set} instead of {expected_set}",
                sector_base.get()
            ),
            Self::SnapshotMisalignedLine { line } => write!(
                formatter,
                "cache sector tag snapshot line {:#x} is not cache-line aligned",
                line.get()
            ),
            Self::SnapshotLineSectorMismatch {
                line,
                sector_base,
                expected_sector_base,
            } => write!(
                formatter,
                "cache sector tag snapshot line {:#x} is in sector {:#x} instead of {:#x}",
                line.get(),
                sector_base.get(),
                expected_sector_base.get()
            ),
            Self::SnapshotLineOffsetMismatch {
                line,
                offset,
                expected_offset,
            } => write!(
                formatter,
                "cache sector tag snapshot line {:#x} is at offset {offset} instead of {expected_offset}",
                line.get()
            ),
            Self::SnapshotDuplicateLine { line } => write!(
                formatter,
                "cache sector tag snapshot repeats line {:#x}",
                line.get()
            ),
            Self::SnapshotDuplicateSector { sector_base } => write!(
                formatter,
                "cache sector tag snapshot repeats sector {:#x}",
                sector_base.get()
            ),
        }
    }
}

impl Error for CacheSectorTagsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::IndexingPolicyConfig { source } => Some(source),
            Self::ReplacementPolicyConfig { source } | Self::ReplacementPolicyState { source } => {
                Some(source)
            }
            Self::ZeroBlocksPerSector
            | Self::BlocksPerSectorNotPowerOfTwo { .. }
            | Self::LineSizeTooSmall { .. }
            | Self::SectorSpanTooLarge { .. }
            | Self::VectorLengthTooLarge { .. }
            | Self::UnknownSet { .. }
            | Self::UnknownWay { .. }
            | Self::SnapshotConfigMismatch { .. }
            | Self::SnapshotSetCountMismatch { .. }
            | Self::SnapshotWayCountMismatch { .. }
            | Self::SnapshotBlocksPerSectorMismatch { .. }
            | Self::SnapshotLineWithoutSector { .. }
            | Self::SnapshotMisalignedSector { .. }
            | Self::SnapshotSectorSetMismatch { .. }
            | Self::SnapshotMisalignedLine { .. }
            | Self::SnapshotLineSectorMismatch { .. }
            | Self::SnapshotLineOffsetMismatch { .. }
            | Self::SnapshotDuplicateLine { .. }
            | Self::SnapshotDuplicateSector { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheSectorTags {
    config: CacheSectorTagsConfig,
    sets: Vec<CacheSectorTagSet>,
    tick: u64,
}

impl CacheSectorTags {
    pub fn new(config: CacheSectorTagsConfig) -> Self {
        let sets = (0..config.sets())
            .map(|_| CacheSectorTagSet::new(&config))
            .collect();
        Self {
            config,
            sets,
            tick: 0,
        }
    }

    pub const fn config(&self) -> &CacheSectorTagsConfig {
        &self.config
    }

    pub fn find(&self, address: Address) -> Option<CacheSectorTagLookup> {
        let sector_base = self.config.sector_base(address);
        let line = self.config.line_address(address);
        let offset = self.config.sector_offset(address);
        self.sector_location(sector_base).and_then(|(set, way)| {
            let sector = &self.sets[set].sectors[way];
            (sector.lines[offset] == Some(line)).then_some(CacheSectorTagLookup {
                line,
                sector_base,
                set,
                way,
                offset,
            })
        })
    }

    pub fn insert(
        &mut self,
        address: Address,
    ) -> Result<CacheSectorTagInsert, CacheSectorTagsError> {
        self.insert_inner(address, None)
    }

    pub fn insert_with_signature(
        &mut self,
        address: Address,
        signature: u64,
    ) -> Result<CacheSectorTagInsert, CacheSectorTagsError> {
        self.insert_inner(address, Some(signature))
    }

    fn insert_inner(
        &mut self,
        address: Address,
        signature: Option<u64>,
    ) -> Result<CacheSectorTagInsert, CacheSectorTagsError> {
        self.require_signature(signature)?;
        let sector_base = self.config.sector_base(address);
        let line = self.config.line_address(address);
        let offset = self.config.sector_offset(address);

        if let Some((set, way)) = self.sector_location(sector_base) {
            let update = self.touch_replacement(set, way, signature)?;
            self.sets[set].sectors[way].lines[offset] = Some(line);
            return Ok(CacheSectorTagInsert {
                line,
                sector_base,
                set,
                way,
                offset,
                new_sector: false,
                evicted_lines: Vec::new(),
                decision: None,
                update,
            });
        }

        let (set, way, decision) = self.victim_location(sector_base)?;
        let update = self.reset_replacement(set, way, signature)?;
        let sector = &mut self.sets[set].sectors[way];
        let evicted_lines = sector.valid_lines();
        sector.sector_base = Some(sector_base);
        sector.lines.fill(None);
        sector.lines[offset] = Some(line);

        Ok(CacheSectorTagInsert {
            line,
            sector_base,
            set,
            way,
            offset,
            new_sector: true,
            evicted_lines,
            decision: Some(decision),
            update,
        })
    }

    pub fn access(
        &mut self,
        address: Address,
    ) -> Result<Option<CacheSectorTagAccess>, CacheSectorTagsError> {
        self.access_inner(address, None)
    }

    pub fn access_with_signature(
        &mut self,
        address: Address,
        signature: u64,
    ) -> Result<Option<CacheSectorTagAccess>, CacheSectorTagsError> {
        self.access_inner(address, Some(signature))
    }

    fn access_inner(
        &mut self,
        address: Address,
        signature: Option<u64>,
    ) -> Result<Option<CacheSectorTagAccess>, CacheSectorTagsError> {
        let Some(hit) = self.find(address) else {
            return Ok(None);
        };
        self.require_signature(signature)?;
        let update = self.touch_replacement(hit.set, hit.way, signature)?;
        Ok(Some(CacheSectorTagAccess {
            line: hit.line,
            sector_base: hit.sector_base,
            set: hit.set,
            way: hit.way,
            offset: hit.offset,
            update,
        }))
    }

    pub fn invalidate(
        &mut self,
        address: Address,
    ) -> Result<Option<CacheSectorTagInvalidate>, CacheSectorTagsError> {
        let sector_base = self.config.sector_base(address);
        let line = self.config.line_address(address);
        let offset = self.config.sector_offset(address);
        let Some((set, way)) = self.sector_location(sector_base) else {
            return Ok(None);
        };
        if self.sets[set].sectors[way].lines[offset] != Some(line) {
            return Ok(None);
        }

        let sector = &mut self.sets[set].sectors[way];
        sector.lines[offset] = None;
        let sector_still_valid = sector.is_valid();
        let update = if sector_still_valid {
            None
        } else {
            sector.sector_base = None;
            Some(self.invalidate_replacement(set, way)?)
        };

        Ok(Some(CacheSectorTagInvalidate {
            line,
            sector_base,
            set,
            way,
            offset,
            sector_still_valid,
            update,
        }))
    }

    pub fn sector_lines(
        &self,
        set: usize,
        way: usize,
    ) -> Result<Vec<Option<Address>>, CacheSectorTagsError> {
        self.check_set(set)?;
        self.check_way(way)?;
        Ok(self.sets[set].sectors[way].lines.clone())
    }

    pub fn resident_lines(&self) -> Vec<Address> {
        let mut lines = self
            .sets
            .iter()
            .flat_map(|set| set.sectors.iter())
            .flat_map(CacheSectorTagEntry::valid_lines)
            .collect::<Vec<_>>();
        lines.sort();
        lines
    }

    pub fn valid_sector_count(&self) -> usize {
        self.sets
            .iter()
            .flat_map(|set| set.sectors.iter())
            .filter(|sector| sector.is_valid())
            .count()
    }

    pub fn valid_line_count(&self) -> usize {
        self.sets
            .iter()
            .flat_map(|set| set.sectors.iter())
            .map(CacheSectorTagEntry::valid_count)
            .sum()
    }

    pub fn snapshot(&self) -> CacheSectorTagsSnapshot {
        CacheSectorTagsSnapshot {
            config: self.config.clone(),
            tick: self.tick,
            sets: self.sets.iter().map(CacheSectorTagSet::snapshot).collect(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &CacheSectorTagsSnapshot,
    ) -> Result<(), CacheSectorTagsError> {
        self.validate_snapshot(snapshot)?;

        let mut restored = Vec::with_capacity(snapshot.sets.len());
        for set_snapshot in &snapshot.sets {
            let mut replacement = ReplacementSet::new(self.config.policy_config().clone());
            replacement
                .restore(&set_snapshot.replacement)
                .map_err(|source| CacheSectorTagsError::ReplacementPolicyState { source })?;
            restored.push(CacheSectorTagSet {
                sectors: set_snapshot
                    .sectors
                    .iter()
                    .map(CacheSectorTagEntry::from_snapshot)
                    .collect(),
                replacement,
            });
        }
        self.sets = restored;
        self.tick = snapshot.tick;
        Ok(())
    }

    fn require_signature(&self, signature: Option<u64>) -> Result<(), CacheSectorTagsError> {
        if matches!(self.config.kind(), CacheReplacementPolicyKind::Ship { .. })
            && signature.is_none()
        {
            return Err(CacheSectorTagsError::ReplacementPolicyState {
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
    ) -> Result<ReplacementUpdate, CacheSectorTagsError> {
        let update = match signature {
            Some(signature) => self.sets[set]
                .replacement
                .touch_with_signature(way, signature),
            None => self.sets[set].replacement.touch(way),
        }
        .map_err(|source| CacheSectorTagsError::ReplacementPolicyState { source })?;
        let tick = self.next_tick();
        self.sets[set].sectors[way]
            .replacement_state
            .touch(self.config.kind(), tick);
        Ok(update)
    }

    fn reset_replacement(
        &mut self,
        set: usize,
        way: usize,
        signature: Option<u64>,
    ) -> Result<ReplacementUpdate, CacheSectorTagsError> {
        let update = match signature {
            Some(signature) => self.sets[set]
                .replacement
                .reset_with_signature(way, signature),
            None => self.sets[set].replacement.reset(way),
        }
        .map_err(|source| CacheSectorTagsError::ReplacementPolicyState { source })?;
        let tick = self.next_tick();
        self.sets[set].sectors[way]
            .replacement_state
            .reset(self.config.kind(), tick);
        Ok(update)
    }

    fn invalidate_replacement(
        &mut self,
        set: usize,
        way: usize,
    ) -> Result<ReplacementUpdate, CacheSectorTagsError> {
        let update = self.sets[set]
            .replacement
            .invalidate(way)
            .map_err(|source| CacheSectorTagsError::ReplacementPolicyState { source })?;
        self.sets[set].sectors[way].replacement_state.invalidate();
        Ok(update)
    }

    fn sector_location(&self, sector_base: Address) -> Option<(usize, usize)> {
        self.config
            .locations_for_sector(sector_base)
            .into_iter()
            .find(|location| {
                self.sets[location.set()].sectors[location.way()].sector_base == Some(sector_base)
            })
            .map(|location| (location.set(), location.way()))
    }

    fn victim_location(
        &mut self,
        sector_base: Address,
    ) -> Result<(usize, usize, ReplacementDecision), CacheSectorTagsError> {
        let locations = self.config.locations_for_sector(sector_base);
        if self.config.indexing_config().kind() == CacheIndexingPolicyKind::SetAssociative {
            let set = locations
                .first()
                .ok_or(CacheSectorTagsError::ReplacementPolicyState {
                    source: CacheReplacementPolicyError::NoCandidates,
                })?
                .set();
            let decision = self.sets[set]
                .replacement
                .victim(0..self.config.ways())
                .map_err(|source| CacheSectorTagsError::ReplacementPolicyState { source })?;
            return Ok((set, decision.way(), decision));
        }

        let selected = self.select_cross_set_victim(&locations)?;
        let candidates = locations
            .iter()
            .map(|location| location.way())
            .collect::<Vec<_>>();
        let decision = self.sets[selected.set()]
            .replacement
            .decision_for_selected_victim(selected.way(), candidates)
            .map_err(|source| CacheSectorTagsError::ReplacementPolicyState { source })?;
        Ok((selected.set(), selected.way(), decision))
    }

    fn select_cross_set_victim(
        &mut self,
        locations: &[CacheIndexingLocation],
    ) -> Result<CacheIndexingLocation, CacheSectorTagsError> {
        match self.config.kind() {
            CacheReplacementPolicyKind::Brrip { rrpv_bits, .. }
            | CacheReplacementPolicyKind::Ship { rrpv_bits, .. } => {
                self.select_cross_set_brrip_victim(locations, rrpv_bits)
            }
            CacheReplacementPolicyKind::Lru
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
    ) -> Result<CacheIndexingLocation, CacheSectorTagsError> {
        if locations.is_empty() {
            return Err(CacheSectorTagsError::ReplacementPolicyState {
                source: CacheReplacementPolicyError::NoCandidates,
            });
        }

        for location in locations {
            if !self.sets[location.set()].sectors[location.way()]
                .replacement_state
                .valid
            {
                return Ok(*location);
            }
        }

        loop {
            let selected = self.select_cross_set_metadata_victim(locations)?;
            let state = self.sets[selected.set()].sectors[selected.way()].replacement_state;
            if !state.second_chance {
                return Ok(selected);
            }

            let tick = self.next_tick();
            let state = &mut self.sets[selected.set()].sectors[selected.way()].replacement_state;
            state.insertion_tick = tick;
            state.second_chance = false;
        }
    }

    fn select_cross_set_brrip_victim(
        &mut self,
        locations: &[CacheIndexingLocation],
        rrpv_bits: u8,
    ) -> Result<CacheIndexingLocation, CacheSectorTagsError> {
        if locations.is_empty() {
            return Err(CacheSectorTagsError::ReplacementPolicyState {
                source: CacheReplacementPolicyError::NoCandidates,
            });
        }

        for location in locations {
            if !self.sets[location.set()]
                .replacement
                .entry(location.way())
                .map_err(|source| CacheSectorTagsError::ReplacementPolicyState { source })?
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
                    .map_err(|source| CacheSectorTagsError::ReplacementPolicyState { source })?
                    .rrpv(),
            );
        }
        if highest < max {
            let increment = max - highest;
            for location in locations {
                self.sets[location.set()]
                    .replacement
                    .age_rrpv_candidate(location.way(), increment, max)
                    .map_err(|source| CacheSectorTagsError::ReplacementPolicyState { source })?;
            }
        }

        self.select_cross_set_metadata_victim(locations)
    }

    fn select_cross_set_metadata_victim(
        &self,
        locations: &[CacheIndexingLocation],
    ) -> Result<CacheIndexingLocation, CacheSectorTagsError> {
        let Some(first) = locations.first().copied() else {
            return Err(CacheSectorTagsError::ReplacementPolicyState {
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
    ) -> Result<bool, CacheSectorTagsError> {
        let current_sector = &self.sets[current.set()].sectors[current.way()];
        let selected_sector = &self.sets[selected.set()].sectors[selected.way()];
        let current_state = current_sector.replacement_state;
        let selected_state = selected_sector.replacement_state;
        if !selected_state.valid {
            return Ok(false);
        }
        if !current_state.valid {
            return Ok(true);
        }
        let precedes = match self.config.kind() {
            CacheReplacementPolicyKind::Lru | CacheReplacementPolicyKind::Bip { .. } => {
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
                    .map_err(|source| CacheSectorTagsError::ReplacementPolicyState { source })?;
                let selected_entry = self.sets[selected.set()]
                    .replacement
                    .entry(selected.way())
                    .map_err(|source| CacheSectorTagsError::ReplacementPolicyState { source })?;
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
        snapshot: &CacheSectorTagsSnapshot,
    ) -> Result<(), CacheSectorTagsError> {
        if self.config != snapshot.config {
            return Err(CacheSectorTagsError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config.clone()),
            });
        }
        if snapshot.sets.len() != self.config.sets() {
            return Err(CacheSectorTagsError::SnapshotSetCountMismatch {
                sets: snapshot.sets.len(),
                expected_sets: self.config.sets(),
            });
        }

        let mut seen = BTreeSet::new();
        let mut seen_sectors = BTreeSet::new();
        for (set_index, set_snapshot) in snapshot.sets.iter().enumerate() {
            if set_snapshot.sectors.len() != self.config.ways() {
                return Err(CacheSectorTagsError::SnapshotWayCountMismatch {
                    set: set_index,
                    ways: set_snapshot.sectors.len(),
                    expected_ways: self.config.ways(),
                });
            }
            for (way_index, sector_snapshot) in set_snapshot.sectors.iter().enumerate() {
                self.validate_sector_snapshot(
                    set_index,
                    way_index,
                    sector_snapshot,
                    &mut seen,
                    &mut seen_sectors,
                )?;
            }
        }
        Ok(())
    }

    fn validate_sector_snapshot(
        &self,
        set: usize,
        way: usize,
        snapshot: &CacheSectorTagEntrySnapshot,
        seen: &mut BTreeSet<Address>,
        seen_sectors: &mut BTreeSet<Address>,
    ) -> Result<(), CacheSectorTagsError> {
        if snapshot.lines.len() != self.config.blocks_per_sector() {
            return Err(CacheSectorTagsError::SnapshotBlocksPerSectorMismatch {
                set,
                way,
                blocks: snapshot.lines.len(),
                expected_blocks: self.config.blocks_per_sector(),
            });
        }

        let sector_base = snapshot.sector_base;
        if let Some(sector_base) = sector_base {
            if !seen_sectors.insert(sector_base) {
                return Err(CacheSectorTagsError::SnapshotDuplicateSector { sector_base });
            }
            if self.config.sector_base(sector_base) != sector_base {
                return Err(CacheSectorTagsError::SnapshotMisalignedSector { sector_base });
            }
            let expected_set = self.config.expected_set_for_way(sector_base, way).ok_or(
                CacheSectorTagsError::UnknownWay {
                    way,
                    ways: self.config.ways(),
                },
            )?;
            if set != expected_set {
                return Err(CacheSectorTagsError::SnapshotSectorSetMismatch {
                    sector_base,
                    set,
                    expected_set,
                });
            }
        }

        for (offset, line) in snapshot.lines.iter().enumerate() {
            let Some(line) = *line else {
                continue;
            };
            let Some(sector_base) = sector_base else {
                return Err(CacheSectorTagsError::SnapshotLineWithoutSector { set, way, offset });
            };
            if self.config.line_address(line) != line {
                return Err(CacheSectorTagsError::SnapshotMisalignedLine { line });
            }
            let expected_sector_base = self.config.sector_base(line);
            if expected_sector_base != sector_base {
                return Err(CacheSectorTagsError::SnapshotLineSectorMismatch {
                    line,
                    sector_base,
                    expected_sector_base,
                });
            }
            let expected_offset = self.config.sector_offset(line);
            if offset != expected_offset {
                return Err(CacheSectorTagsError::SnapshotLineOffsetMismatch {
                    line,
                    offset,
                    expected_offset,
                });
            }
            if !seen.insert(line) {
                return Err(CacheSectorTagsError::SnapshotDuplicateLine { line });
            }
        }
        Ok(())
    }

    fn next_tick(&mut self) -> u64 {
        self.tick = self.tick.saturating_add(1);
        self.tick
    }

    fn check_set(&self, set: usize) -> Result<(), CacheSectorTagsError> {
        if set >= self.config.sets() {
            return Err(CacheSectorTagsError::UnknownSet {
                set,
                sets: self.config.sets(),
            });
        }
        Ok(())
    }

    fn check_way(&self, way: usize) -> Result<(), CacheSectorTagsError> {
        if way >= self.config.ways() {
            return Err(CacheSectorTagsError::UnknownWay {
                way,
                ways: self.config.ways(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheSectorTagLookup {
    line: Address,
    sector_base: Address,
    set: usize,
    way: usize,
    offset: usize,
}

impl CacheSectorTagLookup {
    pub const fn line(self) -> Address {
        self.line
    }

    pub const fn sector_base(self) -> Address {
        self.sector_base
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheSectorTagInsert {
    line: Address,
    sector_base: Address,
    set: usize,
    way: usize,
    offset: usize,
    new_sector: bool,
    evicted_lines: Vec<Address>,
    decision: Option<ReplacementDecision>,
    update: ReplacementUpdate,
}

impl CacheSectorTagInsert {
    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn sector_base(&self) -> Address {
        self.sector_base
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

    pub const fn new_sector(&self) -> bool {
        self.new_sector
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
pub struct CacheSectorTagInvalidate {
    line: Address,
    sector_base: Address,
    set: usize,
    way: usize,
    offset: usize,
    sector_still_valid: bool,
    update: Option<ReplacementUpdate>,
}

impl CacheSectorTagInvalidate {
    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn sector_base(&self) -> Address {
        self.sector_base
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

    pub const fn sector_still_valid(&self) -> bool {
        self.sector_still_valid
    }

    pub const fn update(&self) -> Option<&ReplacementUpdate> {
        self.update.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheSectorTagAccess {
    line: Address,
    sector_base: Address,
    set: usize,
    way: usize,
    offset: usize,
    update: ReplacementUpdate,
}

impl CacheSectorTagAccess {
    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn sector_base(&self) -> Address {
        self.sector_base
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

    pub const fn update(&self) -> &ReplacementUpdate {
        &self.update
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CacheSectorTagSet {
    sectors: Vec<CacheSectorTagEntry>,
    replacement: ReplacementSet,
}

impl CacheSectorTagSet {
    fn new(config: &CacheSectorTagsConfig) -> Self {
        Self {
            sectors: (0..config.ways())
                .map(|_| CacheSectorTagEntry::new(config.blocks_per_sector()))
                .collect(),
            replacement: ReplacementSet::new(config.policy_config().clone()),
        }
    }

    fn snapshot(&self) -> CacheSectorTagSetSnapshot {
        CacheSectorTagSetSnapshot {
            sectors: self
                .sectors
                .iter()
                .map(CacheSectorTagEntry::snapshot)
                .collect(),
            replacement: self.replacement.snapshot(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CacheSectorTagEntry {
    sector_base: Option<Address>,
    lines: Vec<Option<Address>>,
    replacement_state: CacheSectorTagReplacementState,
}

impl CacheSectorTagEntry {
    fn new(blocks_per_sector: usize) -> Self {
        Self {
            sector_base: None,
            lines: vec![None; blocks_per_sector],
            replacement_state: CacheSectorTagReplacementState::new(),
        }
    }

    fn from_snapshot(snapshot: &CacheSectorTagEntrySnapshot) -> Self {
        Self {
            sector_base: snapshot.sector_base,
            lines: snapshot.lines.clone(),
            replacement_state: snapshot.replacement_state,
        }
    }

    fn is_valid(&self) -> bool {
        self.lines.iter().any(Option::is_some)
    }

    fn valid_count(&self) -> usize {
        self.lines.iter().filter(|line| line.is_some()).count()
    }

    fn valid_lines(&self) -> Vec<Address> {
        self.lines.iter().flatten().copied().collect()
    }

    fn snapshot(&self) -> CacheSectorTagEntrySnapshot {
        CacheSectorTagEntrySnapshot {
            sector_base: self.sector_base,
            lines: self.lines.clone(),
            replacement_state: self.replacement_state,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CacheSectorTagReplacementState {
    valid: bool,
    last_touch_tick: u64,
    insertion_tick: u64,
    reference_count: u64,
    second_chance: bool,
}

impl CacheSectorTagReplacementState {
    const fn new() -> Self {
        Self {
            valid: false,
            last_touch_tick: 0,
            insertion_tick: 0,
            reference_count: 0,
            second_chance: false,
        }
    }

    fn from_lines(lines: &[Option<Address>]) -> Self {
        let mut state = Self::new();
        state.valid = lines.iter().any(Option::is_some);
        state
    }

    fn reset(&mut self, kind: CacheReplacementPolicyKind, tick: u64) {
        self.valid = true;
        match kind {
            CacheReplacementPolicyKind::Lru
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheSectorTagsSnapshot {
    config: CacheSectorTagsConfig,
    tick: u64,
    sets: Vec<CacheSectorTagSetSnapshot>,
}

impl CacheSectorTagsSnapshot {
    pub fn new(config: CacheSectorTagsConfig, sets: Vec<CacheSectorTagSetSnapshot>) -> Self {
        Self {
            config,
            tick: 0,
            sets,
        }
    }

    pub const fn config(&self) -> &CacheSectorTagsConfig {
        &self.config
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub fn sets(&self) -> &[CacheSectorTagSetSnapshot] {
        &self.sets
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheSectorTagSetSnapshot {
    sectors: Vec<CacheSectorTagEntrySnapshot>,
    replacement: ReplacementSetSnapshot,
}

impl CacheSectorTagSetSnapshot {
    pub fn new(
        sectors: Vec<CacheSectorTagEntrySnapshot>,
        replacement: ReplacementSetSnapshot,
    ) -> Self {
        Self {
            sectors,
            replacement,
        }
    }

    pub fn sectors(&self) -> &[CacheSectorTagEntrySnapshot] {
        &self.sectors
    }

    pub const fn replacement(&self) -> &ReplacementSetSnapshot {
        &self.replacement
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheSectorTagEntrySnapshot {
    sector_base: Option<Address>,
    lines: Vec<Option<Address>>,
    replacement_state: CacheSectorTagReplacementState,
}

impl CacheSectorTagEntrySnapshot {
    pub fn new(sector_base: Option<Address>, lines: Vec<Option<Address>>) -> Self {
        let replacement_state = CacheSectorTagReplacementState::from_lines(&lines);
        Self {
            sector_base,
            lines,
            replacement_state,
        }
    }

    pub const fn sector_base(&self) -> Option<Address> {
        self.sector_base
    }

    pub fn lines(&self) -> &[Option<Address>] {
        &self.lines
    }
}

fn validate_vector_length<T>(
    field: &'static str,
    length: usize,
) -> Result<(), CacheSectorTagsError> {
    let maximum = max_vector_len::<T>();
    if length > maximum {
        return Err(CacheSectorTagsError::VectorLengthTooLarge {
            field,
            length,
            maximum,
        });
    }
    Ok(())
}

fn sector_layout(
    line_layout: CacheLineLayout,
    blocks_per_sector: usize,
) -> Result<CacheLineLayout, CacheSectorTagsError> {
    let line_bytes = line_layout.bytes();
    let Some(blocks) = u64::try_from(blocks_per_sector).ok() else {
        return Err(CacheSectorTagsError::SectorSpanTooLarge {
            line_bytes,
            blocks_per_sector,
        });
    };
    let Some(sector_bytes) = line_bytes.checked_mul(blocks) else {
        return Err(CacheSectorTagsError::SectorSpanTooLarge {
            line_bytes,
            blocks_per_sector,
        });
    };
    if sector_bytes > MAX_VECTOR_ALLOCATION_BYTES as u64 {
        return Err(CacheSectorTagsError::SectorSpanTooLarge {
            line_bytes,
            blocks_per_sector,
        });
    }
    CacheLineLayout::new(sector_bytes).map_err(|_| CacheSectorTagsError::SectorSpanTooLarge {
        line_bytes,
        blocks_per_sector,
    })
}

use std::collections::BTreeSet;

use rem6_memory::{Address, CacheLineLayout};

use crate::indexing::{CacheIndexingLocation, CacheIndexingPolicyConfig, CacheIndexingPolicyKind};
use crate::replacement::{
    validate_replacement_vector_length, weighted_lru_precedes, CacheReplacementPolicyConfig,
    CacheReplacementPolicyError, CacheReplacementPolicyKind, ReplacementDecision, ReplacementSet,
    ReplacementSetSnapshot, ReplacementUpdate,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheReplacementDirectoryConfig {
    kind: CacheReplacementPolicyKind,
    line_layout: CacheLineLayout,
    sets: usize,
    ways: usize,
    indexing_config: CacheIndexingPolicyConfig,
    policy_config: CacheReplacementPolicyConfig,
}

impl CacheReplacementDirectoryConfig {
    pub fn new(
        kind: CacheReplacementPolicyKind,
        line_layout: CacheLineLayout,
        sets: usize,
        ways: usize,
    ) -> Result<Self, CacheReplacementPolicyError> {
        if sets == 0 {
            return Err(CacheReplacementPolicyError::ZeroSets);
        }
        validate_replacement_vector_length::<ReplacementDirectorySet>("sets", sets)?;
        validate_replacement_vector_length::<Option<Address>>("ways", ways)?;
        let policy_config = CacheReplacementPolicyConfig::new(kind, ways)?;
        let indexing_config =
            CacheIndexingPolicyConfig::new_set_associative_for_directory(line_layout, sets, ways)?;
        Ok(Self {
            kind,
            line_layout,
            sets,
            ways,
            indexing_config,
            policy_config,
        })
    }

    pub fn new_with_indexing(
        kind: CacheReplacementPolicyKind,
        indexing_kind: CacheIndexingPolicyKind,
        line_layout: CacheLineLayout,
        sets: usize,
        ways: usize,
    ) -> Result<Self, CacheReplacementPolicyError> {
        if sets == 0 {
            return Err(CacheReplacementPolicyError::ZeroSets);
        }
        validate_replacement_vector_length::<ReplacementDirectorySet>("sets", sets)?;
        validate_replacement_vector_length::<Option<Address>>("ways", ways)?;
        let policy_config = CacheReplacementPolicyConfig::new(kind, ways)?;
        let indexing_config =
            CacheIndexingPolicyConfig::new(indexing_kind, line_layout, sets, ways)?;
        Ok(Self {
            kind,
            line_layout,
            sets,
            ways,
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

    pub const fn sets(&self) -> usize {
        self.sets
    }

    pub const fn ways(&self) -> usize {
        self.ways
    }

    pub const fn indexing_config(&self) -> &CacheIndexingPolicyConfig {
        &self.indexing_config
    }

    pub const fn policy_config(&self) -> &CacheReplacementPolicyConfig {
        &self.policy_config
    }

    fn line_address(&self, line: Address) -> Address {
        self.line_layout.line_address(line)
    }

    fn locations_for_line(&self, line: Address) -> Vec<CacheIndexingLocation> {
        self.indexing_config
            .candidate_locations(self.line_address(line))
    }

    fn expected_set_for_way(&self, line: Address, way: usize) -> Option<usize> {
        self.locations_for_line(line)
            .into_iter()
            .find(|location| location.way() == way)
            .map(CacheIndexingLocation::set)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheReplacementDirectory {
    config: CacheReplacementDirectoryConfig,
    sets: Vec<ReplacementDirectorySet>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReplacementDirectoryAccess {
    Default,
    Signature(u64),
    Occupancy(i64),
}

impl ReplacementDirectoryAccess {
    fn touch(
        self,
        replacement: &mut ReplacementSet,
        way: usize,
    ) -> Result<ReplacementUpdate, CacheReplacementPolicyError> {
        match self {
            Self::Default => replacement.touch(way),
            Self::Signature(signature) => replacement.touch_with_signature(way, signature),
            Self::Occupancy(occupancy) => replacement.touch_with_occupancy(way, occupancy),
        }
    }

    fn reset(
        self,
        replacement: &mut ReplacementSet,
        way: usize,
    ) -> Result<ReplacementUpdate, CacheReplacementPolicyError> {
        match self {
            Self::Default => replacement.reset(way),
            Self::Signature(signature) => replacement.reset_with_signature(way, signature),
            Self::Occupancy(occupancy) => replacement.reset_with_occupancy(way, occupancy),
        }
    }
}

impl CacheReplacementDirectory {
    pub fn new(config: CacheReplacementDirectoryConfig) -> Self {
        let sets = (0..config.sets())
            .map(|_| ReplacementDirectorySet::new(config.policy_config().clone()))
            .collect();
        Self { config, sets }
    }

    pub const fn config(&self) -> &CacheReplacementDirectoryConfig {
        &self.config
    }

    pub fn resident_lines(&self) -> Vec<Address> {
        let mut lines = self
            .sets
            .iter()
            .flat_map(|set| set.lines.iter().flatten().copied())
            .collect::<Vec<_>>();
        lines.sort();
        lines
    }

    pub fn way_for(&self, line: Address) -> Option<(usize, usize)> {
        let line = self.config.line_address(line);
        self.config
            .locations_for_line(line)
            .into_iter()
            .find(|location| self.sets[location.set()].lines[location.way()] == Some(line))
            .map(|location| (location.set(), location.way()))
    }

    pub fn set_lines(
        &self,
        set: usize,
    ) -> Result<Vec<Option<Address>>, CacheReplacementPolicyError> {
        self.check_set(set)?;
        Ok(self.sets[set].lines.clone())
    }

    pub fn install(
        &mut self,
        line: Address,
    ) -> Result<ReplacementDirectoryInstall, CacheReplacementPolicyError> {
        self.install_inner(line, ReplacementDirectoryAccess::Default)
    }

    pub fn install_with_signature(
        &mut self,
        line: Address,
        signature: u64,
    ) -> Result<ReplacementDirectoryInstall, CacheReplacementPolicyError> {
        self.install_inner(line, ReplacementDirectoryAccess::Signature(signature))
    }

    pub fn install_with_occupancy(
        &mut self,
        line: Address,
        occupancy: i64,
    ) -> Result<ReplacementDirectoryInstall, CacheReplacementPolicyError> {
        self.install_inner(line, ReplacementDirectoryAccess::Occupancy(occupancy))
    }

    pub fn touch(
        &mut self,
        line: Address,
    ) -> Result<ReplacementUpdate, CacheReplacementPolicyError> {
        self.touch_inner(line, ReplacementDirectoryAccess::Default)
    }

    pub fn touch_with_signature(
        &mut self,
        line: Address,
        signature: u64,
    ) -> Result<ReplacementUpdate, CacheReplacementPolicyError> {
        self.touch_inner(line, ReplacementDirectoryAccess::Signature(signature))
    }

    pub fn touch_with_occupancy(
        &mut self,
        line: Address,
        occupancy: i64,
    ) -> Result<ReplacementUpdate, CacheReplacementPolicyError> {
        self.touch_inner(line, ReplacementDirectoryAccess::Occupancy(occupancy))
    }

    pub fn remove_resident_line(
        &mut self,
        line: Address,
    ) -> Result<Option<ReplacementUpdate>, CacheReplacementPolicyError> {
        let line = self.config.line_address(line);
        let Some((set, way)) = self.way_for(line) else {
            return Ok(None);
        };

        let directory_set = &mut self.sets[set];
        let update = directory_set.replacement.invalidate(way)?;
        directory_set.lines[way] = None;
        Ok(Some(update))
    }

    pub fn move_resident_line(
        &mut self,
        line: Address,
        destination_set: usize,
        destination_way: usize,
    ) -> Result<ReplacementDirectoryMove, CacheReplacementPolicyError> {
        let line = self.config.line_address(line);
        self.check_set(destination_set)?;
        self.check_way(destination_way)?;

        let expected_set = self
            .config
            .expected_set_for_way(line, destination_way)
            .ok_or(CacheReplacementPolicyError::UnknownWay {
                way: destination_way,
                ways: self.config.ways(),
            })?;
        if destination_set != expected_set {
            return Err(CacheReplacementPolicyError::LineSetMismatch {
                line,
                set: destination_set,
                expected_set,
            });
        }

        let (source_set, source_way) = self
            .way_for(line)
            .ok_or(CacheReplacementPolicyError::UnknownResidentLine { line })?;
        if self.sets[destination_set].lines[destination_way].is_some() {
            return Err(CacheReplacementPolicyError::OccupiedDestinationWay {
                set: destination_set,
                way: destination_way,
            });
        }

        if source_set == destination_set {
            let directory_set = &mut self.sets[source_set];
            directory_set
                .replacement
                .relocate_way(source_way, destination_way)?;
            directory_set.lines[source_way] = None;
            directory_set.lines[destination_way] = Some(line);
        } else {
            let (source, destination) =
                two_directory_sets_mut(&mut self.sets, source_set, destination_set);
            source.replacement.relocate_way_to_set(
                source_way,
                &mut destination.replacement,
                destination_way,
            )?;
            source.lines[source_way] = None;
            destination.lines[destination_way] = Some(line);
        }

        Ok(ReplacementDirectoryMove {
            line,
            source_set,
            source_way,
            destination_set,
            destination_way,
        })
    }

    pub fn snapshot(&self) -> CacheReplacementDirectorySnapshot {
        CacheReplacementDirectorySnapshot {
            config: self.config.clone(),
            sets: self
                .sets
                .iter()
                .map(ReplacementDirectorySet::snapshot)
                .collect(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &CacheReplacementDirectorySnapshot,
    ) -> Result<(), CacheReplacementPolicyError> {
        if self.config != snapshot.config {
            return Err(
                CacheReplacementPolicyError::SnapshotDirectoryConfigMismatch {
                    expected: Box::new(self.config.clone()),
                    actual: Box::new(snapshot.config.clone()),
                },
            );
        }
        if snapshot.sets.len() != self.config.sets() {
            return Err(
                CacheReplacementPolicyError::SnapshotDirectorySetCountMismatch {
                    sets: snapshot.sets.len(),
                    expected_sets: self.config.sets(),
                },
            );
        }

        let mut seen = BTreeSet::new();
        let mut restored = Vec::with_capacity(snapshot.sets.len());
        for (set_index, set_snapshot) in snapshot.sets.iter().enumerate() {
            if set_snapshot.lines.len() != self.config.ways() {
                return Err(
                    CacheReplacementPolicyError::SnapshotDirectoryWayCountMismatch {
                        set: set_index,
                        ways: set_snapshot.lines.len(),
                        expected_ways: self.config.ways(),
                    },
                );
            }
            for (way_index, line) in set_snapshot.lines.iter().enumerate() {
                let Some(line) = *line else {
                    continue;
                };
                let line = self.config.line_address(line);
                let expected_set = self.config.expected_set_for_way(line, way_index).ok_or(
                    CacheReplacementPolicyError::UnknownWay {
                        way: way_index,
                        ways: self.config.ways(),
                    },
                )?;
                if expected_set != set_index {
                    return Err(CacheReplacementPolicyError::SnapshotLineSetMismatch {
                        line,
                        set: set_index,
                        expected_set,
                    });
                }
                if !seen.insert(line) {
                    return Err(CacheReplacementPolicyError::SnapshotDuplicateLine { line });
                }
            }

            let mut replacement = ReplacementSet::new(self.config.policy_config().clone());
            replacement.restore(&set_snapshot.replacement)?;
            restored.push(ReplacementDirectorySet {
                lines: set_snapshot
                    .lines
                    .iter()
                    .map(|line| line.map(|line| self.config.line_address(line)))
                    .collect(),
                replacement,
            });
        }

        self.sets = restored;
        Ok(())
    }

    fn install_inner(
        &mut self,
        line: Address,
        access: ReplacementDirectoryAccess,
    ) -> Result<ReplacementDirectoryInstall, CacheReplacementPolicyError> {
        let line = self.config.line_address(line);
        if let Some((set, way)) = self.way_for(line) {
            let update = access.touch(&mut self.sets[set].replacement, way)?;
            return Ok(ReplacementDirectoryInstall {
                line,
                set,
                way,
                evicted_line: None,
                decision: None,
                update,
            });
        }

        let (set, way, decision) = self.victim_location(line)?;
        let directory_set = &mut self.sets[set];
        let evicted_line = directory_set.lines[way].replace(line);
        let update = access.reset(&mut directory_set.replacement, way)?;
        Ok(ReplacementDirectoryInstall {
            line,
            set,
            way,
            evicted_line,
            decision: Some(decision),
            update,
        })
    }

    fn touch_inner(
        &mut self,
        line: Address,
        access: ReplacementDirectoryAccess,
    ) -> Result<ReplacementUpdate, CacheReplacementPolicyError> {
        let line = self.config.line_address(line);
        let (set, way) = self
            .way_for(line)
            .ok_or(CacheReplacementPolicyError::UnknownResidentLine { line })?;
        access.touch(&mut self.sets[set].replacement, way)
    }

    fn victim_location(
        &mut self,
        line: Address,
    ) -> Result<(usize, usize, ReplacementDecision), CacheReplacementPolicyError> {
        let locations = self.config.locations_for_line(line);
        if self.config.indexing_config().kind() == CacheIndexingPolicyKind::SetAssociative {
            let set = locations
                .first()
                .ok_or(CacheReplacementPolicyError::NoCandidates)?
                .set();
            let decision = self.sets[set].replacement.victim(0..self.config.ways())?;
            return Ok((set, decision.way(), decision));
        }

        let selected = self.select_cross_set_victim(&locations)?;
        let candidates = locations
            .iter()
            .map(|location| location.way())
            .collect::<Vec<_>>();
        let decision = self.sets[selected.set()]
            .replacement
            .decision_for_selected_victim(selected.way(), candidates)?;
        Ok((selected.set(), selected.way(), decision))
    }

    fn select_cross_set_victim(
        &mut self,
        locations: &[CacheIndexingLocation],
    ) -> Result<CacheIndexingLocation, CacheReplacementPolicyError> {
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
            | CacheReplacementPolicyKind::SecondChance
            | CacheReplacementPolicyKind::TreePlru => {
                self.select_cross_set_metadata_victim(locations)
            }
        }
    }

    fn select_cross_set_brrip_victim(
        &mut self,
        locations: &[CacheIndexingLocation],
        rrpv_bits: u8,
    ) -> Result<CacheIndexingLocation, CacheReplacementPolicyError> {
        if locations.is_empty() {
            return Err(CacheReplacementPolicyError::NoCandidates);
        }

        for location in locations {
            if !self.sets[location.set()]
                .replacement
                .entry(location.way())?
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
                    .entry(location.way())?
                    .rrpv(),
            );
        }
        if highest < max {
            let increment = max - highest;
            for location in locations {
                self.sets[location.set()].replacement.age_rrpv_candidate(
                    location.way(),
                    increment,
                    max,
                )?;
            }
        }

        self.select_cross_set_metadata_victim(locations)
    }

    fn select_cross_set_metadata_victim(
        &self,
        locations: &[CacheIndexingLocation],
    ) -> Result<CacheIndexingLocation, CacheReplacementPolicyError> {
        let Some(first) = locations.first().copied() else {
            return Err(CacheReplacementPolicyError::NoCandidates);
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
    ) -> Result<bool, CacheReplacementPolicyError> {
        let current_entry = self.sets[current.set()].replacement.entry(current.way())?;
        let selected_entry = self.sets[selected.set()]
            .replacement
            .entry(selected.way())?;
        let precedes = match self.config.kind() {
            CacheReplacementPolicyKind::Lru | CacheReplacementPolicyKind::Bip { .. } => {
                current_entry.last_touch_tick() < selected_entry.last_touch_tick()
            }
            CacheReplacementPolicyKind::WeightedLru => {
                weighted_lru_precedes(current_entry, selected_entry)
            }
            CacheReplacementPolicyKind::Fifo | CacheReplacementPolicyKind::SecondChance => {
                current_entry.insertion_tick() < selected_entry.insertion_tick()
            }
            CacheReplacementPolicyKind::Mru => {
                if selected_entry.last_touch_tick() == 0 {
                    false
                } else if current_entry.last_touch_tick() == 0 {
                    true
                } else {
                    current_entry.last_touch_tick() > selected_entry.last_touch_tick()
                }
            }
            CacheReplacementPolicyKind::Lfu => {
                current_entry.reference_count() < selected_entry.reference_count()
            }
            CacheReplacementPolicyKind::Brrip { .. } | CacheReplacementPolicyKind::Ship { .. } => {
                if !selected_entry.valid() {
                    false
                } else if !current_entry.valid() {
                    true
                } else {
                    current_entry.rrpv() > selected_entry.rrpv()
                }
            }
            CacheReplacementPolicyKind::TreePlru => {
                if !selected_entry.valid() {
                    false
                } else {
                    !current_entry.valid()
                }
            }
        };
        Ok(precedes)
    }

    fn check_set(&self, set: usize) -> Result<(), CacheReplacementPolicyError> {
        if set >= self.config.sets() {
            return Err(CacheReplacementPolicyError::UnknownSet {
                set,
                sets: self.config.sets(),
            });
        }
        Ok(())
    }

    fn check_way(&self, way: usize) -> Result<(), CacheReplacementPolicyError> {
        if way >= self.config.ways() {
            return Err(CacheReplacementPolicyError::UnknownWay {
                way,
                ways: self.config.ways(),
            });
        }
        Ok(())
    }
}

fn two_directory_sets_mut(
    sets: &mut [ReplacementDirectorySet],
    first: usize,
    second: usize,
) -> (&mut ReplacementDirectorySet, &mut ReplacementDirectorySet) {
    if first < second {
        let (left, right) = sets.split_at_mut(second);
        (&mut left[first], &mut right[0])
    } else {
        let (left, right) = sets.split_at_mut(first);
        (&mut right[0], &mut left[second])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReplacementDirectorySet {
    lines: Vec<Option<Address>>,
    replacement: ReplacementSet,
}

impl ReplacementDirectorySet {
    fn new(config: CacheReplacementPolicyConfig) -> Self {
        Self {
            lines: vec![None; config.ways()],
            replacement: ReplacementSet::new(config),
        }
    }

    fn snapshot(&self) -> ReplacementDirectorySetSnapshot {
        ReplacementDirectorySetSnapshot {
            lines: self.lines.clone(),
            replacement: self.replacement.snapshot(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacementDirectoryInstall {
    line: Address,
    set: usize,
    way: usize,
    evicted_line: Option<Address>,
    decision: Option<ReplacementDecision>,
    update: ReplacementUpdate,
}

impl ReplacementDirectoryInstall {
    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn set(&self) -> usize {
        self.set
    }

    pub const fn way(&self) -> usize {
        self.way
    }

    pub const fn evicted_line(&self) -> Option<Address> {
        self.evicted_line
    }

    pub const fn decision(&self) -> Option<&ReplacementDecision> {
        self.decision.as_ref()
    }

    pub const fn update(&self) -> &ReplacementUpdate {
        &self.update
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacementDirectoryMove {
    line: Address,
    source_set: usize,
    source_way: usize,
    destination_set: usize,
    destination_way: usize,
}

impl ReplacementDirectoryMove {
    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn source_set(&self) -> usize {
        self.source_set
    }

    pub const fn source_way(&self) -> usize {
        self.source_way
    }

    pub const fn destination_set(&self) -> usize {
        self.destination_set
    }

    pub const fn destination_way(&self) -> usize {
        self.destination_way
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheReplacementDirectorySnapshot {
    config: CacheReplacementDirectoryConfig,
    sets: Vec<ReplacementDirectorySetSnapshot>,
}

impl CacheReplacementDirectorySnapshot {
    pub const fn config(&self) -> &CacheReplacementDirectoryConfig {
        &self.config
    }

    pub fn sets(&self) -> &[ReplacementDirectorySetSnapshot] {
        &self.sets
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacementDirectorySetSnapshot {
    lines: Vec<Option<Address>>,
    replacement: ReplacementSetSnapshot,
}

impl ReplacementDirectorySetSnapshot {
    pub fn lines(&self) -> &[Option<Address>] {
        &self.lines
    }

    pub const fn replacement(&self) -> &ReplacementSetSnapshot {
        &self.replacement
    }
}

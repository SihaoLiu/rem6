use std::error::Error;
use std::fmt;

use rem6_memory::Address;

use crate::replacement_directory::CacheReplacementDirectoryConfig;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheReplacementPolicyKind {
    Lru,
    Fifo,
    Mru,
    Lfu,
    Brrip {
        rrpv_bits: u8,
        hit_priority: bool,
        btp_percent: u8,
    },
    Ship {
        rrpv_bits: u8,
        hit_priority: bool,
        shct_entries: usize,
        insertion_threshold_percent: u8,
    },
    Bip {
        btp_percent: u8,
    },
    SecondChance,
    TreePlru,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheReplacementPolicyConfig {
    kind: CacheReplacementPolicyKind,
    ways: usize,
}

impl CacheReplacementPolicyConfig {
    pub fn new(
        kind: CacheReplacementPolicyKind,
        ways: usize,
    ) -> Result<Self, CacheReplacementPolicyError> {
        if ways == 0 {
            return Err(CacheReplacementPolicyError::ZeroWays);
        }
        match kind {
            CacheReplacementPolicyKind::Brrip {
                rrpv_bits,
                btp_percent,
                ..
            } => {
                if !(1..=7).contains(&rrpv_bits) {
                    return Err(CacheReplacementPolicyError::RrpvBitsOutOfRange {
                        bits: rrpv_bits,
                    });
                }
                if btp_percent > 100 {
                    return Err(CacheReplacementPolicyError::BtpOutOfRange {
                        percent: btp_percent,
                    });
                }
            }
            CacheReplacementPolicyKind::Ship {
                rrpv_bits,
                shct_entries,
                insertion_threshold_percent,
                ..
            } => {
                if !(1..=7).contains(&rrpv_bits) {
                    return Err(CacheReplacementPolicyError::RrpvBitsOutOfRange {
                        bits: rrpv_bits,
                    });
                }
                if shct_entries == 0 {
                    return Err(CacheReplacementPolicyError::SignatureHistoryTableEmpty);
                }
                if insertion_threshold_percent > 100 {
                    return Err(CacheReplacementPolicyError::InsertionThresholdOutOfRange {
                        percent: insertion_threshold_percent,
                    });
                }
            }
            CacheReplacementPolicyKind::Bip { btp_percent } => {
                if btp_percent > 100 {
                    return Err(CacheReplacementPolicyError::BtpOutOfRange {
                        percent: btp_percent,
                    });
                }
            }
            CacheReplacementPolicyKind::TreePlru => {
                if !ways.is_power_of_two() {
                    return Err(CacheReplacementPolicyError::TreePlruWaysNotPowerOfTwo { ways });
                }
            }
            CacheReplacementPolicyKind::Lru
            | CacheReplacementPolicyKind::Fifo
            | CacheReplacementPolicyKind::Mru
            | CacheReplacementPolicyKind::Lfu
            | CacheReplacementPolicyKind::SecondChance => {}
        }
        Ok(Self { kind, ways })
    }

    pub const fn kind(&self) -> CacheReplacementPolicyKind {
        self.kind
    }

    pub const fn ways(&self) -> usize {
        self.ways
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheReplacementPolicyError {
    ZeroWays,
    ZeroSets,
    RrpvBitsOutOfRange {
        bits: u8,
    },
    BtpOutOfRange {
        percent: u8,
    },
    InsertionThresholdOutOfRange {
        percent: u8,
    },
    SignatureHistoryTableEmpty,
    TreePlruWaysNotPowerOfTwo {
        ways: usize,
    },
    SignatureRequired,
    UnknownWay {
        way: usize,
        ways: usize,
    },
    UnknownSet {
        set: usize,
        sets: usize,
    },
    UnknownResidentLine {
        line: Address,
    },
    LineSetMismatch {
        line: Address,
        set: usize,
        expected_set: usize,
    },
    OccupiedDestinationWay {
        set: usize,
        way: usize,
    },
    NoCandidates,
    SnapshotConfigMismatch {
        expected: Box<CacheReplacementPolicyConfig>,
        actual: Box<CacheReplacementPolicyConfig>,
    },
    SnapshotDirectoryConfigMismatch {
        expected: Box<CacheReplacementDirectoryConfig>,
        actual: Box<CacheReplacementDirectoryConfig>,
    },
    SnapshotDirectorySetCountMismatch {
        sets: usize,
        expected_sets: usize,
    },
    SnapshotDirectoryWayCountMismatch {
        set: usize,
        ways: usize,
        expected_ways: usize,
    },
    SnapshotDuplicateLine {
        line: Address,
    },
    SnapshotLineSetMismatch {
        line: Address,
        set: usize,
        expected_set: usize,
    },
}

impl fmt::Display for CacheReplacementPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroWays => write!(formatter, "cache replacement policy has no ways"),
            Self::ZeroSets => write!(formatter, "cache replacement directory has no sets"),
            Self::RrpvBitsOutOfRange { bits } => write!(
                formatter,
                "cache replacement policy RRPV width {bits} is outside 1..=7"
            ),
            Self::BtpOutOfRange { percent } => write!(
                formatter,
                "cache replacement policy BTP {percent} is outside 0..=100"
            ),
            Self::InsertionThresholdOutOfRange { percent } => write!(
                formatter,
                "cache replacement policy insertion threshold {percent} is outside 0..=100"
            ),
            Self::SignatureHistoryTableEmpty => {
                write!(formatter, "SHiP replacement policy has no SHCT entries")
            }
            Self::TreePlruWaysNotPowerOfTwo { ways } => write!(
                formatter,
                "TreePLRU replacement policy needs a power-of-two way count, got {ways}"
            ),
            Self::SignatureRequired => write!(
                formatter,
                "SHiP replacement policy requires an access signature"
            ),
            Self::UnknownWay { way, ways } => write!(
                formatter,
                "cache replacement policy way {way} is outside {ways} ways"
            ),
            Self::UnknownSet { set, sets } => write!(
                formatter,
                "cache replacement directory set {set} is outside {sets} sets"
            ),
            Self::UnknownResidentLine { line } => write!(
                formatter,
                "cache replacement directory has no resident line {:#x}",
                line.get()
            ),
            Self::LineSetMismatch {
                line,
                set,
                expected_set,
            } => write!(
                formatter,
                "cache replacement directory line {:#x} cannot move to set {set} instead of {expected_set}",
                line.get()
            ),
            Self::OccupiedDestinationWay { set, way } => write!(
                formatter,
                "cache replacement directory destination set {set} way {way} is occupied"
            ),
            Self::NoCandidates => write!(formatter, "cache replacement policy has no candidates"),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "cache replacement snapshot config {actual:?} does not match policy config {expected:?}"
            ),
            Self::SnapshotDirectoryConfigMismatch { expected, actual } => write!(
                formatter,
                "cache replacement directory snapshot config {actual:?} does not match directory config {expected:?}"
            ),
            Self::SnapshotDirectorySetCountMismatch {
                sets,
                expected_sets,
            } => write!(
                formatter,
                "cache replacement directory snapshot has {sets} sets instead of {expected_sets}"
            ),
            Self::SnapshotDirectoryWayCountMismatch {
                set,
                ways,
                expected_ways,
            } => write!(
                formatter,
                "cache replacement directory snapshot set {set} has {ways} ways instead of {expected_ways}"
            ),
            Self::SnapshotDuplicateLine { line } => write!(
                formatter,
                "cache replacement directory snapshot repeats line {:#x}",
                line.get()
            ),
            Self::SnapshotLineSetMismatch {
                line,
                set,
                expected_set,
            } => write!(
                formatter,
                "cache replacement directory snapshot line {:#x} is in set {set} instead of {expected_set}",
                line.get()
            ),
        }
    }
}

impl Error for CacheReplacementPolicyError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacementSet {
    config: CacheReplacementPolicyConfig,
    entries: Vec<ReplacementEntry>,
    tree_bits: Option<Vec<bool>>,
    ship_signature_counters: Option<Vec<u8>>,
    tick: u64,
    bip_accumulator: u8,
    reset_count: u64,
    touch_count: u64,
    invalidate_count: u64,
    victim_count: u64,
}

impl ReplacementSet {
    pub fn new(config: CacheReplacementPolicyConfig) -> Self {
        let tree_bits = (config.kind() == CacheReplacementPolicyKind::TreePlru)
            .then(|| vec![false; config.ways() - 1]);
        let ship_signature_counters = match config.kind() {
            CacheReplacementPolicyKind::Ship { shct_entries, .. } => Some(vec![0; shct_entries]),
            _ => None,
        };
        let entries = (0..config.ways())
            .map(|way| ReplacementEntry::new(way, config.kind()))
            .collect();
        Self {
            config,
            entries,
            tree_bits,
            ship_signature_counters,
            tick: 0,
            bip_accumulator: 0,
            reset_count: 0,
            touch_count: 0,
            invalidate_count: 0,
            victim_count: 0,
        }
    }

    pub const fn config(&self) -> &CacheReplacementPolicyConfig {
        &self.config
    }

    pub fn entries(&self) -> &[ReplacementEntry] {
        &self.entries
    }

    pub fn tree_bits(&self) -> Option<&[bool]> {
        self.tree_bits.as_deref()
    }

    pub fn ship_signature_counters(&self) -> Option<&[u8]> {
        self.ship_signature_counters.as_deref()
    }

    pub const fn reset_count(&self) -> u64 {
        self.reset_count
    }

    pub const fn touch_count(&self) -> u64 {
        self.touch_count
    }

    pub const fn invalidate_count(&self) -> u64 {
        self.invalidate_count
    }

    pub const fn victim_count(&self) -> u64 {
        self.victim_count
    }

    pub fn entry(&self, way: usize) -> Result<&ReplacementEntry, CacheReplacementPolicyError> {
        self.entries
            .get(way)
            .ok_or(CacheReplacementPolicyError::UnknownWay {
                way,
                ways: self.config.ways(),
            })
    }

    pub fn invalidate(
        &mut self,
        way: usize,
    ) -> Result<ReplacementUpdate, CacheReplacementPolicyError> {
        self.check_way(way)?;
        let before = self.entries[way].clone();
        match self.config.kind() {
            CacheReplacementPolicyKind::Lru
            | CacheReplacementPolicyKind::Mru
            | CacheReplacementPolicyKind::Bip { .. } => {
                self.entries[way].last_touch_tick = 0;
            }
            CacheReplacementPolicyKind::Fifo | CacheReplacementPolicyKind::SecondChance => {
                self.entries[way].insertion_tick = 0;
                self.entries[way].second_chance = false;
            }
            CacheReplacementPolicyKind::Lfu => {
                self.entries[way].reference_count = 0;
            }
            CacheReplacementPolicyKind::Brrip { .. } => {
                self.entries[way].valid = false;
            }
            CacheReplacementPolicyKind::Ship { .. } => {
                self.detrain_ship_entry_if_unused(way);
                self.entries[way].valid = false;
                self.entries[way].ship_re_referenced = false;
            }
            CacheReplacementPolicyKind::TreePlru => {
                self.set_tree_points_to_leaf(way);
                self.entries[way].valid = false;
            }
        }
        self.invalidate_count += 1;
        Ok(ReplacementUpdate {
            way,
            before,
            after: self.entries[way].clone(),
            update_count: self.invalidate_count,
        })
    }

    pub fn touch(&mut self, way: usize) -> Result<ReplacementUpdate, CacheReplacementPolicyError> {
        self.check_way(way)?;
        let before = self.entries[way].clone();
        match self.config.kind() {
            CacheReplacementPolicyKind::Lru
            | CacheReplacementPolicyKind::Mru
            | CacheReplacementPolicyKind::Bip { .. } => {
                self.entries[way].last_touch_tick = self.next_tick();
            }
            CacheReplacementPolicyKind::Fifo => {}
            CacheReplacementPolicyKind::SecondChance => {
                self.entries[way].second_chance = true;
            }
            CacheReplacementPolicyKind::Lfu => {
                self.entries[way].reference_count =
                    self.entries[way].reference_count.saturating_add(1);
            }
            CacheReplacementPolicyKind::Brrip { hit_priority, .. } => {
                if hit_priority {
                    self.entries[way].rrpv = 0;
                } else {
                    self.entries[way].rrpv = self.entries[way].rrpv.saturating_sub(1);
                }
            }
            CacheReplacementPolicyKind::Ship { .. } => {
                return Err(CacheReplacementPolicyError::SignatureRequired);
            }
            CacheReplacementPolicyKind::TreePlru => {
                self.set_tree_points_away_from_leaf(way);
                self.entries[way].valid = true;
            }
        }
        self.touch_count += 1;
        Ok(ReplacementUpdate {
            way,
            before,
            after: self.entries[way].clone(),
            update_count: self.touch_count,
        })
    }

    pub fn touch_with_signature(
        &mut self,
        way: usize,
        signature: u64,
    ) -> Result<ReplacementUpdate, CacheReplacementPolicyError> {
        self.check_way(way)?;
        let CacheReplacementPolicyKind::Ship {
            rrpv_bits,
            hit_priority,
            ..
        } = self.config.kind()
        else {
            return self.touch(way);
        };

        let before = self.entries[way].clone();
        let signature_index = self.ship_signature_index(signature);
        self.increment_ship_counter(signature_index, max_rrpv(rrpv_bits) as u8);
        self.entries[way].ship_re_referenced = true;
        if hit_priority {
            self.entries[way].rrpv = 0;
        } else {
            self.entries[way].rrpv = self.entries[way].rrpv.saturating_sub(1);
        }
        self.touch_count += 1;
        Ok(ReplacementUpdate {
            way,
            before,
            after: self.entries[way].clone(),
            update_count: self.touch_count,
        })
    }

    pub fn reset(&mut self, way: usize) -> Result<ReplacementUpdate, CacheReplacementPolicyError> {
        self.check_way(way)?;
        let before = self.entries[way].clone();
        match self.config.kind() {
            CacheReplacementPolicyKind::Lru | CacheReplacementPolicyKind::Mru => {
                self.entries[way].last_touch_tick = self.next_tick();
                self.entries[way].valid = true;
            }
            CacheReplacementPolicyKind::Fifo => {
                self.entries[way].insertion_tick = self.next_tick();
                self.entries[way].valid = true;
            }
            CacheReplacementPolicyKind::Lfu => {
                self.entries[way].reference_count = 1;
                self.entries[way].valid = true;
            }
            CacheReplacementPolicyKind::Brrip {
                rrpv_bits,
                btp_percent,
                ..
            } => {
                let max = max_rrpv(rrpv_bits);
                self.entries[way].rrpv = if btp_percent == 100 { max - 1 } else { max };
                self.entries[way].valid = true;
            }
            CacheReplacementPolicyKind::Ship { .. } => {
                return Err(CacheReplacementPolicyError::SignatureRequired);
            }
            CacheReplacementPolicyKind::Bip { btp_percent } => {
                self.entries[way].last_touch_tick = if self.bip_insert_as_mru(btp_percent) {
                    self.next_tick()
                } else {
                    self.tick = self.tick.max(1);
                    1
                };
                self.entries[way].valid = true;
            }
            CacheReplacementPolicyKind::SecondChance => {
                self.entries[way].insertion_tick = self.next_tick();
                self.entries[way].second_chance = false;
                self.entries[way].valid = true;
            }
            CacheReplacementPolicyKind::TreePlru => {
                self.set_tree_points_away_from_leaf(way);
                self.entries[way].valid = true;
            }
        }
        self.reset_count += 1;
        Ok(ReplacementUpdate {
            way,
            before,
            after: self.entries[way].clone(),
            update_count: self.reset_count,
        })
    }

    pub fn reset_with_signature(
        &mut self,
        way: usize,
        signature: u64,
    ) -> Result<ReplacementUpdate, CacheReplacementPolicyError> {
        self.check_way(way)?;
        let CacheReplacementPolicyKind::Ship {
            rrpv_bits,
            insertion_threshold_percent,
            ..
        } = self.config.kind()
        else {
            return self.reset(way);
        };

        let before = self.entries[way].clone();
        let signature_index = self.ship_signature_index(signature);
        let max = max_rrpv(rrpv_bits);
        self.entries[way].ship_signature = signature_index as u64;
        self.entries[way].ship_re_referenced = false;
        self.entries[way].rrpv = max;
        if self.ship_counter_reaches_threshold(signature_index, max, insertion_threshold_percent) {
            self.entries[way].rrpv = self.entries[way].rrpv.saturating_sub(1);
        }
        self.entries[way].valid = true;
        self.reset_count += 1;
        Ok(ReplacementUpdate {
            way,
            before,
            after: self.entries[way].clone(),
            update_count: self.reset_count,
        })
    }

    pub(crate) fn relocate_way(
        &mut self,
        source_way: usize,
        destination_way: usize,
    ) -> Result<(), CacheReplacementPolicyError> {
        self.check_way(source_way)?;
        self.check_way(destination_way)?;
        if source_way == destination_way {
            return Ok(());
        }

        let mut moved = self.entries[source_way].clone();
        moved.way = destination_way;
        self.entries[source_way] = ReplacementEntry::new(source_way, self.config.kind());
        self.entries[destination_way] = moved;
        if self.config.kind() == CacheReplacementPolicyKind::TreePlru {
            self.set_tree_points_to_leaf(source_way);
        }
        Ok(())
    }

    pub fn victim<I>(
        &mut self,
        candidates: I,
    ) -> Result<ReplacementDecision, CacheReplacementPolicyError>
    where
        I: IntoIterator<Item = usize>,
    {
        let candidates = candidates.into_iter().collect::<Vec<_>>();
        if candidates.is_empty() {
            return Err(CacheReplacementPolicyError::NoCandidates);
        }
        for way in &candidates {
            self.check_way(*way)?;
        }

        let way = match self.config.kind() {
            CacheReplacementPolicyKind::Lru => {
                self.min_by(&candidates, |entry| entry.last_touch_tick)
            }
            CacheReplacementPolicyKind::Bip { .. } => {
                self.min_by(&candidates, |entry| entry.last_touch_tick)
            }
            CacheReplacementPolicyKind::Fifo => {
                self.min_by(&candidates, |entry| entry.insertion_tick)
            }
            CacheReplacementPolicyKind::SecondChance => self.second_chance_victim(&candidates),
            CacheReplacementPolicyKind::Mru => self.mru_victim(&candidates),
            CacheReplacementPolicyKind::Lfu => {
                self.min_by(&candidates, |entry| entry.reference_count)
            }
            CacheReplacementPolicyKind::Brrip { rrpv_bits, .. } => {
                self.brrip_victim(&candidates, rrpv_bits)
            }
            CacheReplacementPolicyKind::Ship { rrpv_bits, .. } => {
                self.brrip_victim(&candidates, rrpv_bits)
            }
            CacheReplacementPolicyKind::TreePlru => self.tree_plru_victim(&candidates),
        };

        self.victim_count += 1;
        Ok(ReplacementDecision {
            way,
            candidates,
            victim_count: self.victim_count,
        })
    }

    pub fn snapshot(&self) -> ReplacementSetSnapshot {
        ReplacementSetSnapshot {
            config: self.config.clone(),
            entries: self.entries.clone(),
            tree_bits: self.tree_bits.clone(),
            ship_signature_counters: self.ship_signature_counters.clone(),
            tick: self.tick,
            bip_accumulator: self.bip_accumulator,
            reset_count: self.reset_count,
            touch_count: self.touch_count,
            invalidate_count: self.invalidate_count,
            victim_count: self.victim_count,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &ReplacementSetSnapshot,
    ) -> Result<(), CacheReplacementPolicyError> {
        if self.config != snapshot.config {
            return Err(CacheReplacementPolicyError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config.clone()),
            });
        }
        self.entries.clone_from(&snapshot.entries);
        self.tree_bits.clone_from(&snapshot.tree_bits);
        self.ship_signature_counters
            .clone_from(&snapshot.ship_signature_counters);
        self.tick = snapshot.tick;
        self.bip_accumulator = snapshot.bip_accumulator;
        self.reset_count = snapshot.reset_count;
        self.touch_count = snapshot.touch_count;
        self.invalidate_count = snapshot.invalidate_count;
        self.victim_count = snapshot.victim_count;
        Ok(())
    }

    fn brrip_victim(&mut self, candidates: &[usize], rrpv_bits: u8) -> usize {
        if let Some(way) = candidates.iter().find(|way| !self.entries[**way].valid) {
            return *way;
        }
        let max = max_rrpv(rrpv_bits);
        let highest = candidates
            .iter()
            .map(|way| self.entries[*way].rrpv)
            .max()
            .unwrap_or(0);
        if highest < max {
            let diff = max - highest;
            for way in candidates {
                self.entries[*way].rrpv = self.entries[*way].rrpv.saturating_add(diff).min(max);
            }
        }
        self.max_by(candidates, |entry| entry.rrpv)
    }

    fn detrain_ship_entry_if_unused(&mut self, way: usize) {
        if !self.entries[way].valid || self.entries[way].ship_re_referenced {
            return;
        }
        let signature = self.entries[way].ship_signature as usize;
        if let Some(counters) = &mut self.ship_signature_counters {
            if let Some(counter) = counters.get_mut(signature) {
                *counter = counter.saturating_sub(1);
            }
        }
    }

    fn ship_signature_index(&self, signature: u64) -> usize {
        let entries = self
            .ship_signature_counters
            .as_ref()
            .map(Vec::len)
            .unwrap_or(1);
        (signature % entries as u64) as usize
    }

    fn increment_ship_counter(&mut self, signature: usize, max: u8) {
        if let Some(counters) = &mut self.ship_signature_counters {
            if let Some(counter) = counters.get_mut(signature) {
                *counter = counter.saturating_add(1).min(max);
            }
        }
    }

    fn ship_counter_reaches_threshold(
        &self,
        signature: usize,
        max: u64,
        threshold_percent: u8,
    ) -> bool {
        let Some(counters) = &self.ship_signature_counters else {
            return false;
        };
        let Some(counter) = counters.get(signature) else {
            return false;
        };
        u64::from(*counter) * 100 >= u64::from(threshold_percent) * max
    }

    fn second_chance_victim(&mut self, candidates: &[usize]) -> usize {
        if let Some(way) = candidates.iter().find(|way| {
            self.entries[**way].insertion_tick == 0 && !self.entries[**way].second_chance
        }) {
            return *way;
        }

        loop {
            let way = self.min_by(candidates, |entry| entry.insertion_tick);
            if self.entries[way].second_chance {
                let insertion_tick = self.next_tick();
                self.entries[way].insertion_tick = insertion_tick;
                self.entries[way].second_chance = false;
                self.entries[way].valid = true;
            } else {
                return way;
            }
        }
    }

    fn bip_insert_as_mru(&mut self, btp_percent: u8) -> bool {
        if btp_percent == 0 {
            return false;
        }
        if btp_percent == 100 {
            return true;
        }

        let next = self.bip_accumulator + btp_percent;
        self.bip_accumulator = next % 100;
        next >= 100
    }

    fn tree_plru_victim(&self, candidates: &[usize]) -> usize {
        let Some(tree) = &self.tree_bits else {
            return candidates[0];
        };
        let mut node = 0usize;
        while node < tree.len() {
            if tree[node] {
                node = right_child(node);
            } else {
                node = left_child(node);
            }
        }
        let way = node - (self.config.ways() - 1);
        if candidates.contains(&way) {
            way
        } else {
            candidates[0]
        }
    }

    fn mru_victim(&self, candidates: &[usize]) -> usize {
        if let Some(way) = candidates
            .iter()
            .find(|way| self.entries[**way].last_touch_tick == 0)
        {
            *way
        } else {
            self.max_by(candidates, |entry| entry.last_touch_tick)
        }
    }

    fn min_by(&self, candidates: &[usize], key: fn(&ReplacementEntry) -> u64) -> usize {
        *candidates
            .iter()
            .min_by_key(|way| key(&self.entries[**way]))
            .expect("candidate list is not empty")
    }

    fn max_by(&self, candidates: &[usize], key: fn(&ReplacementEntry) -> u64) -> usize {
        let mut selected = candidates[0];
        let mut selected_key = key(&self.entries[selected]);
        for way in &candidates[1..] {
            let current_key = key(&self.entries[*way]);
            if current_key > selected_key {
                selected = *way;
                selected_key = current_key;
            }
        }
        selected
    }

    fn set_tree_points_away_from_leaf(&mut self, way: usize) {
        let Some(tree) = &mut self.tree_bits else {
            return;
        };
        let mut node = leaf_node(self.config.ways(), way);
        while node != 0 {
            let right = is_right_child(node);
            node = parent(node);
            tree[node] = !right;
        }
    }

    fn set_tree_points_to_leaf(&mut self, way: usize) {
        let Some(tree) = &mut self.tree_bits else {
            return;
        };
        let mut node = leaf_node(self.config.ways(), way);
        while node != 0 {
            let right = is_right_child(node);
            node = parent(node);
            tree[node] = right;
        }
    }

    fn next_tick(&mut self) -> u64 {
        self.tick = self.tick.saturating_add(1);
        self.tick
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacementEntry {
    way: usize,
    valid: bool,
    last_touch_tick: u64,
    insertion_tick: u64,
    reference_count: u64,
    rrpv: u64,
    second_chance: bool,
    ship_signature: u64,
    ship_re_referenced: bool,
}

impl ReplacementEntry {
    fn new(way: usize, kind: CacheReplacementPolicyKind) -> Self {
        let rrpv = match kind {
            CacheReplacementPolicyKind::Brrip { rrpv_bits, .. }
            | CacheReplacementPolicyKind::Ship { rrpv_bits, .. } => max_rrpv(rrpv_bits),
            CacheReplacementPolicyKind::Lru
            | CacheReplacementPolicyKind::Fifo
            | CacheReplacementPolicyKind::Mru
            | CacheReplacementPolicyKind::Lfu
            | CacheReplacementPolicyKind::Bip { .. }
            | CacheReplacementPolicyKind::SecondChance
            | CacheReplacementPolicyKind::TreePlru => 0,
        };
        Self {
            way,
            valid: false,
            last_touch_tick: 0,
            insertion_tick: 0,
            reference_count: 0,
            rrpv,
            second_chance: false,
            ship_signature: 0,
            ship_re_referenced: false,
        }
    }

    pub const fn way(&self) -> usize {
        self.way
    }

    pub const fn valid(&self) -> bool {
        self.valid
    }

    pub const fn last_touch_tick(&self) -> u64 {
        self.last_touch_tick
    }

    pub const fn insertion_tick(&self) -> u64 {
        self.insertion_tick
    }

    pub const fn reference_count(&self) -> u64 {
        self.reference_count
    }

    pub const fn rrpv(&self) -> u64 {
        self.rrpv
    }

    pub const fn second_chance(&self) -> bool {
        self.second_chance
    }

    pub const fn ship_signature(&self) -> u64 {
        self.ship_signature
    }

    pub const fn ship_re_referenced(&self) -> bool {
        self.ship_re_referenced
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacementUpdate {
    way: usize,
    before: ReplacementEntry,
    after: ReplacementEntry,
    update_count: u64,
}

impl ReplacementUpdate {
    pub const fn way(&self) -> usize {
        self.way
    }

    pub const fn before(&self) -> &ReplacementEntry {
        &self.before
    }

    pub const fn after(&self) -> &ReplacementEntry {
        &self.after
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacementDecision {
    way: usize,
    candidates: Vec<usize>,
    victim_count: u64,
}

impl ReplacementDecision {
    pub const fn way(&self) -> usize {
        self.way
    }

    pub fn candidates(&self) -> &[usize] {
        &self.candidates
    }

    pub const fn victim_count(&self) -> u64 {
        self.victim_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacementSetSnapshot {
    config: CacheReplacementPolicyConfig,
    entries: Vec<ReplacementEntry>,
    tree_bits: Option<Vec<bool>>,
    ship_signature_counters: Option<Vec<u8>>,
    tick: u64,
    bip_accumulator: u8,
    reset_count: u64,
    touch_count: u64,
    invalidate_count: u64,
    victim_count: u64,
}

fn max_rrpv(bits: u8) -> u64 {
    (1u64 << bits) - 1
}

fn leaf_node(ways: usize, way: usize) -> usize {
    way + ways - 1
}

fn parent(index: usize) -> usize {
    (index - 1) / 2
}

fn left_child(index: usize) -> usize {
    2 * index + 1
}

fn right_child(index: usize) -> usize {
    2 * index + 2
}

fn is_right_child(index: usize) -> bool {
    index.is_multiple_of(2)
}

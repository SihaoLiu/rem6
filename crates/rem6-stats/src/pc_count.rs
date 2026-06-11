use std::collections::{BTreeMap, BTreeSet};

use crate::probes::{ProbeEvent, ProbePayload, ProbePointId};
use crate::StatsError;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PcCountPair {
    pc: u64,
    count: u64,
}

impl PcCountPair {
    pub const fn new(pc: u64, count: u64) -> Self {
        Self { pc, count }
    }

    pub const fn pc(self) -> u64 {
        self.pc
    }

    pub const fn count(self) -> u64 {
        self.count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcCountTrackerUpdate {
    pair: PcCountPair,
    remaining_targets: bool,
}

impl PcCountTrackerUpdate {
    pub const fn new(pair: PcCountPair, remaining_targets: bool) -> Self {
        Self {
            pair,
            remaining_targets,
        }
    }

    pub const fn pair(&self) -> PcCountPair {
        self.pair
    }

    pub const fn has_remaining_targets(&self) -> bool {
        self.remaining_targets
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcCountTrackerSnapshot {
    counters: Vec<(u64, u64)>,
    pending_targets: Vec<PcCountPair>,
    current_pair: PcCountPair,
    armed: bool,
}

impl PcCountTrackerSnapshot {
    pub fn new(
        counters: Vec<(u64, u64)>,
        pending_targets: Vec<PcCountPair>,
        current_pair: PcCountPair,
        armed: bool,
    ) -> Self {
        Self {
            counters,
            pending_targets,
            current_pair,
            armed,
        }
    }

    pub fn counters(&self) -> &[(u64, u64)] {
        &self.counters
    }

    pub fn pending_targets(&self) -> &[PcCountPair] {
        &self.pending_targets
    }

    pub const fn current_pair(&self) -> PcCountPair {
        self.current_pair
    }

    pub const fn is_armed(&self) -> bool {
        self.armed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcCountTracker {
    target_pcs: BTreeSet<u64>,
}

impl PcCountTracker {
    pub fn new(targets: Vec<PcCountPair>) -> Self {
        Self {
            target_pcs: targets.into_iter().map(PcCountPair::pc).collect(),
        }
    }

    pub fn observes_pc(&self, pc: u64) -> bool {
        self.target_pcs.contains(&pc)
    }

    pub fn observe_retired_pc(
        &self,
        pc: u64,
        manager: &mut PcCountTrackerManager,
    ) -> Option<PcCountTrackerUpdate> {
        if self.observes_pc(pc) {
            manager.record_pc(pc)
        } else {
            None
        }
    }

    pub fn observe_retired_pc_probe_event(
        &self,
        event: &ProbeEvent,
        retired_pc_point: ProbePointId,
        manager: &mut PcCountTrackerManager,
    ) -> Option<PcCountTrackerUpdate> {
        if event.point() != retired_pc_point {
            return None;
        }
        let ProbePayload::ProgramCounter { pc } = event.payload() else {
            return None;
        };
        self.observe_retired_pc(*pc, manager)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcCountTrackerManager {
    counters: BTreeMap<u64, u64>,
    pending_targets: BTreeSet<PcCountPair>,
    current_pair: PcCountPair,
    armed: bool,
}

impl PcCountTrackerManager {
    pub fn new(targets: Vec<PcCountPair>) -> Self {
        let mut counters = BTreeMap::new();
        let mut pending_targets = BTreeSet::new();
        for target in targets {
            counters.entry(target.pc()).or_insert(0);
            pending_targets.insert(target);
        }
        let armed = !pending_targets.is_empty();
        Self {
            counters,
            pending_targets,
            current_pair: PcCountPair::new(0, 0),
            armed,
        }
    }

    pub fn from_snapshot(snapshot: &PcCountTrackerSnapshot) -> Result<Self, StatsError> {
        let mut counters = BTreeMap::new();
        for (pc, count) in snapshot.counters() {
            if counters.insert(*pc, *count).is_some() {
                return Err(StatsError::DuplicatePcCountCounter { pc: *pc });
            }
        }

        let mut pending_targets = BTreeSet::new();
        for target in snapshot.pending_targets() {
            let Some(current_count) = counters.get(&target.pc()) else {
                return Err(StatsError::MissingPcCountCounter { pc: target.pc() });
            };
            if target.count() <= *current_count {
                return Err(StatsError::UnreachablePcCountTarget {
                    pair: *target,
                    current_count: *current_count,
                });
            }
            if !pending_targets.insert(*target) {
                return Err(StatsError::DuplicatePcCountTarget { pair: *target });
            }
        }
        if snapshot.is_armed() == pending_targets.is_empty() {
            return Err(StatsError::PcCountSnapshotTargetStateMismatch {
                armed: snapshot.is_armed(),
                pending_targets: pending_targets.len(),
            });
        }

        Ok(Self {
            counters,
            pending_targets,
            current_pair: snapshot.current_pair(),
            armed: snapshot.is_armed(),
        })
    }

    pub fn pc_count(&self, pc: u64) -> Option<u64> {
        self.counters.get(&pc).copied()
    }

    pub const fn current_pair(&self) -> PcCountPair {
        self.current_pair
    }

    pub const fn is_armed(&self) -> bool {
        self.armed
    }

    pub fn record_pc(&mut self, pc: u64) -> Option<PcCountTrackerUpdate> {
        if !self.armed {
            return None;
        }

        let count = self.counters.get_mut(&pc)?;
        *count = count.checked_add(1)?;
        self.current_pair = PcCountPair::new(pc, *count);
        if self.pending_targets.remove(&self.current_pair) {
            self.armed = !self.pending_targets.is_empty();
            Some(PcCountTrackerUpdate::new(self.current_pair, self.armed))
        } else {
            None
        }
    }

    pub fn snapshot(&self) -> PcCountTrackerSnapshot {
        PcCountTrackerSnapshot::new(
            self.counters
                .iter()
                .map(|(pc, count)| (*pc, *count))
                .collect(),
            self.pending_targets.iter().copied().collect(),
            self.current_pair,
            self.armed,
        )
    }
}

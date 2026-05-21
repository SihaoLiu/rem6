use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatId(u64);

impl StatId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatSample {
    id: StatId,
    path: String,
    unit: String,
    value: u64,
}

impl StatSample {
    pub fn new(id: StatId, path: impl Into<String>, unit: impl Into<String>, value: u64) -> Self {
        Self {
            id,
            path: path.into(),
            unit: unit.into(),
            value,
        }
    }

    pub const fn id(&self) -> StatId {
        self.id
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn unit(&self) -> &str {
        &self.unit
    }

    pub const fn value(&self) -> u64 {
        self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatSnapshot {
    tick: Tick,
    epoch: u64,
    reset_tick: Tick,
    samples: Vec<StatSample>,
}

impl StatSnapshot {
    pub const fn new(tick: Tick, epoch: u64, reset_tick: Tick, samples: Vec<StatSample>) -> Self {
        Self {
            tick,
            epoch,
            reset_tick,
            samples,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub const fn reset_tick(&self) -> Tick {
        self.reset_tick
    }

    pub fn samples(&self) -> &[StatSample] {
        &self.samples
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatsResetRecord {
    tick: Tick,
    epoch: u64,
    previous_values: Vec<(StatId, u64)>,
}

impl StatsResetRecord {
    pub const fn new(tick: Tick, epoch: u64, previous_values: Vec<(StatId, u64)>) -> Self {
        Self {
            tick,
            epoch,
            previous_values,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn previous_values(&self) -> &[(StatId, u64)] {
        &self.previous_values
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatsRegistry {
    next_id: u64,
    epoch: u64,
    reset_tick: Tick,
    paths: BTreeSet<String>,
    descriptors: BTreeMap<StatId, StatDescriptor>,
    counters: BTreeMap<StatId, u64>,
}

impl StatsRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            epoch: 0,
            reset_tick: 0,
            paths: BTreeSet::new(),
            descriptors: BTreeMap::new(),
            counters: BTreeMap::new(),
        }
    }

    pub fn register_counter(
        &mut self,
        path: impl Into<String>,
        unit: impl Into<String>,
    ) -> Result<StatId, StatsError> {
        let path = path.into();
        if path.is_empty() {
            return Err(StatsError::EmptyPath);
        }
        if self.paths.contains(&path) {
            return Err(StatsError::DuplicatePath { path });
        }

        let id = StatId::new(self.next_id);
        self.next_id += 1;
        self.paths.insert(path.clone());
        self.descriptors.insert(
            id,
            StatDescriptor {
                path,
                unit: unit.into(),
            },
        );
        self.counters.insert(id, 0);
        Ok(id)
    }

    pub fn increment(&mut self, stat: StatId, value: u64) -> Result<(), StatsError> {
        let counter = self
            .counters
            .get_mut(&stat)
            .ok_or(StatsError::UnknownStat { stat })?;
        *counter = counter
            .checked_add(value)
            .ok_or(StatsError::CounterOverflow { stat })?;
        Ok(())
    }

    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub const fn reset_tick(&self) -> Tick {
        self.reset_tick
    }

    pub fn snapshot(&self, tick: Tick) -> StatSnapshot {
        self.try_snapshot(tick)
            .expect("snapshot tick must be at or after the last reset")
    }

    pub fn try_snapshot(&self, tick: Tick) -> Result<StatSnapshot, StatsError> {
        if tick < self.reset_tick {
            return Err(StatsError::SnapshotBeforeReset {
                tick,
                reset_tick: self.reset_tick,
            });
        }

        let samples = self
            .descriptors
            .iter()
            .map(|(id, descriptor)| {
                StatSample::new(
                    *id,
                    descriptor.path.clone(),
                    descriptor.unit.clone(),
                    self.counters.get(id).copied().unwrap_or_default(),
                )
            })
            .collect();
        Ok(StatSnapshot::new(
            tick,
            self.epoch,
            self.reset_tick,
            samples,
        ))
    }

    pub fn reset(&mut self, tick: Tick) -> StatsResetRecord {
        self.epoch += 1;
        self.reset_tick = tick;
        let mut previous_values = Vec::new();
        for (id, counter) in &mut self.counters {
            previous_values.push((*id, *counter));
            *counter = 0;
        }
        StatsResetRecord::new(tick, self.epoch, previous_values)
    }
}

impl Default for StatsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StatDescriptor {
    path: String,
    unit: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatsError {
    EmptyPath,
    DuplicatePath { path: String },
    UnknownStat { stat: StatId },
    CounterOverflow { stat: StatId },
    SnapshotBeforeReset { tick: Tick, reset_tick: Tick },
}

impl fmt::Display for StatsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPath => write!(formatter, "stat path must not be empty"),
            Self::DuplicatePath { path } => write!(formatter, "stat path already exists: {path}"),
            Self::UnknownStat { stat } => write!(formatter, "unknown stat id {}", stat.get()),
            Self::CounterOverflow { stat } => {
                write!(formatter, "counter {} overflowed", stat.get())
            }
            Self::SnapshotBeforeReset { tick, reset_tick } => write!(
                formatter,
                "cannot snapshot at tick {tick}; last reset was at tick {reset_tick}"
            ),
        }
    }
}

impl Error for StatsError {}

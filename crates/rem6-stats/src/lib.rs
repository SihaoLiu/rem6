use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProbePointId(u64);

impl ProbePointId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProbeListenerId(u64);

impl ProbeListenerId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProbePayload {
    Unit,
    Counter { amount: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeEvent {
    tick: Tick,
    sequence: u64,
    point: ProbePointId,
    listener_count: usize,
    payload: ProbePayload,
}

impl ProbeEvent {
    pub const fn new(
        tick: Tick,
        sequence: u64,
        point: ProbePointId,
        listener_count: usize,
        payload: ProbePayload,
    ) -> Self {
        Self {
            tick,
            sequence,
            point,
            listener_count,
            payload,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn point(&self) -> ProbePointId {
        self.point
    }

    pub const fn listener_count(&self) -> usize {
        self.listener_count
    }

    pub const fn payload(&self) -> &ProbePayload {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeSnapshot {
    points: Vec<(String, String, ProbePointId)>,
    listeners: Vec<(String, ProbePointId, ProbeListenerId)>,
    events: Vec<ProbeEvent>,
}

impl ProbeSnapshot {
    pub const fn new(
        points: Vec<(String, String, ProbePointId)>,
        listeners: Vec<(String, ProbePointId, ProbeListenerId)>,
        events: Vec<ProbeEvent>,
    ) -> Self {
        Self {
            points,
            listeners,
            events,
        }
    }

    pub fn points(&self) -> &[(String, String, ProbePointId)] {
        &self.points
    }

    pub fn listeners(&self) -> &[(String, ProbePointId, ProbeListenerId)] {
        &self.listeners
    }

    pub fn events(&self) -> &[ProbeEvent] {
        &self.events
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeRegistry {
    next_point: u64,
    next_listener: u64,
    next_sequence: u64,
    point_names: BTreeSet<(String, String)>,
    points: BTreeMap<ProbePointId, ProbePointRecord>,
    listeners: BTreeMap<ProbeListenerId, ProbeListenerRecord>,
    point_listeners: BTreeMap<ProbePointId, BTreeSet<ProbeListenerId>>,
    events: Vec<ProbeEvent>,
}

impl ProbeRegistry {
    pub fn new() -> Self {
        Self {
            next_point: 0,
            next_listener: 0,
            next_sequence: 0,
            point_names: BTreeSet::new(),
            points: BTreeMap::new(),
            listeners: BTreeMap::new(),
            point_listeners: BTreeMap::new(),
            events: Vec::new(),
        }
    }

    pub fn register_point(
        &mut self,
        component: impl Into<String>,
        name: impl Into<String>,
    ) -> Result<ProbePointId, StatsError> {
        let component = component.into();
        let name = name.into();
        if component.is_empty() {
            return Err(StatsError::EmptyProbeComponent);
        }
        if name.is_empty() {
            return Err(StatsError::EmptyProbeName);
        }
        if !self.point_names.insert((component.clone(), name.clone())) {
            return Err(StatsError::DuplicateProbePoint { component, name });
        }

        let id = ProbePointId::new(self.next_point);
        self.next_point = self.next_point.saturating_add(1);
        self.points.insert(id, ProbePointRecord { component, name });
        self.point_listeners.insert(id, BTreeSet::new());
        Ok(id)
    }

    pub fn add_listener(
        &mut self,
        point: ProbePointId,
        name: impl Into<String>,
    ) -> Result<ProbeListenerId, StatsError> {
        if !self.points.contains_key(&point) {
            return Err(StatsError::UnknownProbePoint { point });
        }
        let name = name.into();
        if name.is_empty() {
            return Err(StatsError::EmptyProbeListenerName);
        }
        if self
            .listeners
            .values()
            .any(|listener| listener.point == point && listener.name == name)
        {
            return Err(StatsError::DuplicateProbeListener { point, name });
        }

        let id = ProbeListenerId::new(self.next_listener);
        self.next_listener = self.next_listener.saturating_add(1);
        self.listeners
            .insert(id, ProbeListenerRecord { point, name });
        self.point_listeners.entry(point).or_default().insert(id);
        Ok(id)
    }

    pub fn remove_listener(
        &mut self,
        point: ProbePointId,
        listener: ProbeListenerId,
    ) -> Result<(), StatsError> {
        if !self.points.contains_key(&point) {
            return Err(StatsError::UnknownProbePoint { point });
        }
        let record = self
            .listeners
            .remove(&listener)
            .ok_or(StatsError::UnknownProbeListener { listener })?;
        if record.point != point {
            self.listeners.insert(listener, record);
            return Err(StatsError::ProbeListenerPointMismatch { point, listener });
        }
        if let Some(listeners) = self.point_listeners.get_mut(&point) {
            listeners.remove(&listener);
        }
        Ok(())
    }

    pub fn emit(
        &mut self,
        tick: Tick,
        point: ProbePointId,
        payload: ProbePayload,
    ) -> Result<&ProbeEvent, StatsError> {
        if !self.points.contains_key(&point) {
            return Err(StatsError::UnknownProbePoint { point });
        }
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(StatsError::ProbeSequenceOverflow)?;
        let listener_count = self.point_listeners.get(&point).map_or(0, BTreeSet::len);
        self.events.push(ProbeEvent::new(
            tick,
            sequence,
            point,
            listener_count,
            payload,
        ));
        Ok(self.events.last().expect("probe event was just appended"))
    }

    pub fn events(&self) -> &[ProbeEvent] {
        &self.events
    }

    pub fn snapshot(&self) -> ProbeSnapshot {
        let points = self
            .points
            .iter()
            .map(|(id, point)| (point.component.clone(), point.name.clone(), *id))
            .collect();
        let listeners = self
            .listeners
            .iter()
            .map(|(id, listener)| (listener.name.clone(), listener.point, *id))
            .collect();
        ProbeSnapshot::new(points, listeners, self.events.clone())
    }
}

impl Default for ProbeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProbePointRecord {
    component: String,
    name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProbeListenerRecord {
    point: ProbePointId,
    name: String,
}

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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatDumpId(u64);

impl StatDumpId {
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
pub struct StatDeltaSample {
    id: StatId,
    path: String,
    unit: String,
    previous_value: u64,
    current_value: u64,
}

impl StatDeltaSample {
    pub fn new(
        id: StatId,
        path: impl Into<String>,
        unit: impl Into<String>,
        previous_value: u64,
        current_value: u64,
    ) -> Self {
        Self {
            id,
            path: path.into(),
            unit: unit.into(),
            previous_value,
            current_value,
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

    pub const fn previous_value(&self) -> u64 {
        self.previous_value
    }

    pub const fn current_value(&self) -> u64 {
        self.current_value
    }

    pub const fn delta_value(&self) -> u64 {
        self.current_value - self.previous_value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatSnapshotDelta {
    previous_tick: Tick,
    current_tick: Tick,
    epoch: u64,
    reset_tick: Tick,
    samples: Vec<StatDeltaSample>,
}

impl StatSnapshotDelta {
    pub const fn new(
        previous_tick: Tick,
        current_tick: Tick,
        epoch: u64,
        reset_tick: Tick,
        samples: Vec<StatDeltaSample>,
    ) -> Self {
        Self {
            previous_tick,
            current_tick,
            epoch,
            reset_tick,
            samples,
        }
    }

    pub const fn previous_tick(&self) -> Tick {
        self.previous_tick
    }

    pub const fn current_tick(&self) -> Tick {
        self.current_tick
    }

    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub const fn reset_tick(&self) -> Tick {
        self.reset_tick
    }

    pub fn samples(&self) -> &[StatDeltaSample] {
        &self.samples
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

    pub fn delta_since(&self, previous: &Self) -> Result<StatSnapshotDelta, StatsError> {
        if self.tick < previous.tick {
            return Err(StatsError::SnapshotDeltaTimeWentBack {
                previous_tick: previous.tick,
                current_tick: self.tick,
            });
        }
        if self.epoch != previous.epoch || self.reset_tick != previous.reset_tick {
            return Err(StatsError::SnapshotDeltaScopeMismatch {
                previous_epoch: previous.epoch,
                current_epoch: self.epoch,
                previous_reset_tick: previous.reset_tick,
                current_reset_tick: self.reset_tick,
            });
        }

        let current_samples = self
            .samples
            .iter()
            .map(|sample| (sample.id(), sample))
            .collect::<BTreeMap<_, _>>();
        let previous_samples = previous
            .samples
            .iter()
            .map(|sample| (sample.id(), sample))
            .collect::<BTreeMap<_, _>>();
        for current_stat in current_samples.keys() {
            if !previous_samples.contains_key(current_stat) {
                return Err(StatsError::SnapshotDeltaUnexpectedStat {
                    stat: *current_stat,
                });
            }
        }

        let mut deltas = Vec::with_capacity(previous.samples.len());
        for previous_sample in &previous.samples {
            let Some(current_sample) = current_samples.get(&previous_sample.id()) else {
                return Err(StatsError::SnapshotDeltaMissingStat {
                    stat: previous_sample.id(),
                });
            };
            if current_sample.path() != previous_sample.path()
                || current_sample.unit() != previous_sample.unit()
            {
                return Err(StatsError::SnapshotDeltaDescriptorMismatch {
                    stat: previous_sample.id(),
                    previous_path: previous_sample.path().to_string(),
                    current_path: current_sample.path().to_string(),
                    previous_unit: previous_sample.unit().to_string(),
                    current_unit: current_sample.unit().to_string(),
                });
            }
            if current_sample.value() < previous_sample.value() {
                return Err(StatsError::SnapshotDeltaValueWentBack {
                    stat: previous_sample.id(),
                    previous: previous_sample.value(),
                    current: current_sample.value(),
                });
            }
            deltas.push(StatDeltaSample::new(
                previous_sample.id(),
                previous_sample.path(),
                previous_sample.unit(),
                previous_sample.value(),
                current_sample.value(),
            ));
        }

        Ok(StatSnapshotDelta::new(
            previous.tick,
            self.tick,
            self.epoch,
            self.reset_tick,
            deltas,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatDumpRecord {
    id: StatDumpId,
    snapshot: StatSnapshot,
}

impl StatDumpRecord {
    pub const fn new(id: StatDumpId, snapshot: StatSnapshot) -> Self {
        Self { id, snapshot }
    }

    pub const fn id(&self) -> StatDumpId {
        self.id
    }

    pub const fn snapshot(&self) -> &StatSnapshot {
        &self.snapshot
    }

    pub const fn tick(&self) -> Tick {
        self.snapshot.tick()
    }

    pub const fn epoch(&self) -> u64 {
        self.snapshot.epoch()
    }

    pub const fn reset_tick(&self) -> Tick {
        self.snapshot.reset_tick()
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
    next_dump_id: u64,
    epoch: u64,
    reset_tick: Tick,
    paths: BTreeSet<String>,
    descriptors: BTreeMap<StatId, StatDescriptor>,
    counters: BTreeMap<StatId, u64>,
    dump_records: Vec<StatDumpRecord>,
}

impl StatsRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            next_dump_id: 0,
            epoch: 0,
            reset_tick: 0,
            paths: BTreeSet::new(),
            descriptors: BTreeMap::new(),
            counters: BTreeMap::new(),
            dump_records: Vec::new(),
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

    pub fn dump(&mut self, tick: Tick) -> StatDumpRecord {
        self.try_dump(tick)
            .expect("dump tick must be at or after the last reset")
    }

    pub fn try_dump(&mut self, tick: Tick) -> Result<StatDumpRecord, StatsError> {
        let snapshot = self.try_snapshot(tick)?;
        let id = StatDumpId::new(self.next_dump_id);
        self.next_dump_id = self
            .next_dump_id
            .checked_add(1)
            .ok_or(StatsError::DumpSequenceOverflow)?;
        let record = StatDumpRecord::new(id, snapshot);
        self.dump_records.push(record.clone());
        Ok(record)
    }

    pub fn dump_records(&self) -> &[StatDumpRecord] {
        &self.dump_records
    }

    pub fn reset(&mut self, tick: Tick) -> StatsResetRecord {
        self.try_reset(tick)
            .expect("reset tick must be at or after the last reset")
    }

    pub fn try_reset(&mut self, tick: Tick) -> Result<StatsResetRecord, StatsError> {
        if tick < self.reset_tick {
            return Err(StatsError::ResetBeforeLastReset {
                tick,
                reset_tick: self.reset_tick,
            });
        }

        self.epoch += 1;
        self.reset_tick = tick;
        let mut previous_values = Vec::new();
        for (id, counter) in &mut self.counters {
            previous_values.push((*id, *counter));
            *counter = 0;
        }
        Ok(StatsResetRecord::new(tick, self.epoch, previous_values))
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
    DuplicatePath {
        path: String,
    },
    UnknownStat {
        stat: StatId,
    },
    CounterOverflow {
        stat: StatId,
    },
    SnapshotBeforeReset {
        tick: Tick,
        reset_tick: Tick,
    },
    ResetBeforeLastReset {
        tick: Tick,
        reset_tick: Tick,
    },
    SnapshotDeltaTimeWentBack {
        previous_tick: Tick,
        current_tick: Tick,
    },
    SnapshotDeltaScopeMismatch {
        previous_epoch: u64,
        current_epoch: u64,
        previous_reset_tick: Tick,
        current_reset_tick: Tick,
    },
    SnapshotDeltaMissingStat {
        stat: StatId,
    },
    SnapshotDeltaUnexpectedStat {
        stat: StatId,
    },
    SnapshotDeltaDescriptorMismatch {
        stat: StatId,
        previous_path: String,
        current_path: String,
        previous_unit: String,
        current_unit: String,
    },
    SnapshotDeltaValueWentBack {
        stat: StatId,
        previous: u64,
        current: u64,
    },
    EmptyProbeComponent,
    EmptyProbeName,
    DuplicateProbePoint {
        component: String,
        name: String,
    },
    UnknownProbePoint {
        point: ProbePointId,
    },
    EmptyProbeListenerName,
    DuplicateProbeListener {
        point: ProbePointId,
        name: String,
    },
    UnknownProbeListener {
        listener: ProbeListenerId,
    },
    ProbeListenerPointMismatch {
        point: ProbePointId,
        listener: ProbeListenerId,
    },
    ProbeSequenceOverflow,
    DumpSequenceOverflow,
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
            Self::ResetBeforeLastReset { tick, reset_tick } => write!(
                formatter,
                "cannot reset stats at tick {tick}; last reset was at tick {reset_tick}"
            ),
            Self::SnapshotDeltaTimeWentBack {
                previous_tick,
                current_tick,
            } => write!(
                formatter,
                "stat snapshot delta tick {current_tick} is before previous tick {previous_tick}"
            ),
            Self::SnapshotDeltaScopeMismatch {
                previous_epoch,
                current_epoch,
                previous_reset_tick,
                current_reset_tick,
            } => write!(
                formatter,
                "stat snapshot delta scopes differ: previous epoch {previous_epoch} reset {previous_reset_tick}, current epoch {current_epoch} reset {current_reset_tick}"
            ),
            Self::SnapshotDeltaMissingStat { stat } => {
                write!(formatter, "stat snapshot delta is missing stat {}", stat.get())
            }
            Self::SnapshotDeltaUnexpectedStat { stat } => {
                write!(
                    formatter,
                    "stat snapshot delta has unexpected stat {}",
                    stat.get()
                )
            }
            Self::SnapshotDeltaDescriptorMismatch {
                stat,
                previous_path,
                current_path,
                previous_unit,
                current_unit,
            } => write!(
                formatter,
                "stat snapshot delta descriptor for stat {} changed from {previous_path} {previous_unit} to {current_path} {current_unit}",
                stat.get()
            ),
            Self::SnapshotDeltaValueWentBack {
                stat,
                previous,
                current,
            } => write!(
                formatter,
                "stat snapshot delta value for stat {} went from {previous} down to {current}",
                stat.get()
            ),
            Self::EmptyProbeComponent => write!(formatter, "probe component must not be empty"),
            Self::EmptyProbeName => write!(formatter, "probe point name must not be empty"),
            Self::DuplicateProbePoint { component, name } => {
                write!(formatter, "probe point already exists: {component}.{name}")
            }
            Self::UnknownProbePoint { point } => {
                write!(formatter, "unknown probe point id {}", point.get())
            }
            Self::EmptyProbeListenerName => {
                write!(formatter, "probe listener name must not be empty")
            }
            Self::DuplicateProbeListener { point, name } => write!(
                formatter,
                "probe listener {name} already exists for point {}",
                point.get()
            ),
            Self::UnknownProbeListener { listener } => {
                write!(formatter, "unknown probe listener id {}", listener.get())
            }
            Self::ProbeListenerPointMismatch { point, listener } => write!(
                formatter,
                "probe listener {} is not attached to point {}",
                listener.get(),
                point.get()
            ),
            Self::ProbeSequenceOverflow => write!(formatter, "probe event sequence overflowed"),
            Self::DumpSequenceOverflow => write!(formatter, "stat dump sequence overflowed"),
        }
    }
}

impl Error for StatsError {}

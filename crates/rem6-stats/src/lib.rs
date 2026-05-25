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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatGroupId(u64);

impl StatGroupId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatScope {
    spelling: String,
    segments: Vec<String>,
}

impl StatScope {
    pub fn new<I, S>(segments: I) -> Result<Self, StatPathError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::from_segments(segments.into_iter().map(Into::into).collect())
    }

    pub fn from_segments(segments: Vec<String>) -> Result<Self, StatPathError> {
        let spelling = segments.join(".");
        validate_stat_segments(segments.iter().map(String::as_str))?;
        Ok(Self { spelling, segments })
    }

    pub fn as_str(&self) -> &str {
        &self.spelling
    }

    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    pub fn stat_path(&self, name: impl Into<String>) -> Result<StatPath, StatPathError> {
        let mut segments = self.segments.clone();
        segments.push(name.into());
        StatPath::from_segments(segments)
    }
}

impl fmt::Display for StatScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatPath {
    spelling: String,
    segments: Vec<String>,
}

impl StatPath {
    pub fn parse(path: impl Into<String>) -> Result<Self, StatPathError> {
        let spelling = path.into();
        validate_stat_path(&spelling)?;
        let segments = spelling.split('.').map(str::to_string).collect();
        Ok(Self { spelling, segments })
    }

    pub fn new<I, S>(scope: I, name: impl Into<String>) -> Result<Self, StatPathError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut segments = scope.into_iter().map(Into::into).collect::<Vec<_>>();
        segments.push(name.into());
        Self::from_segments(segments)
    }

    pub fn from_segments(segments: Vec<String>) -> Result<Self, StatPathError> {
        let spelling = segments.join(".");
        validate_stat_segments(segments.iter().map(String::as_str))?;
        Ok(Self { spelling, segments })
    }

    pub fn as_str(&self) -> &str {
        &self.spelling
    }

    pub fn scope(&self) -> &[String] {
        let name_index = self.segments.len().saturating_sub(1);
        &self.segments[..name_index]
    }

    pub fn name(&self) -> &str {
        self.segments
            .last()
            .map(String::as_str)
            .expect("stat path must have a name segment")
    }

    pub fn segments(&self) -> &[String] {
        &self.segments
    }
}

impl fmt::Display for StatPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatUnitKind {
    Cycle,
    Tick,
    Second,
    Bit,
    Byte,
    Watt,
    Joule,
    Volt,
    Celsius,
    Count,
    Ratio,
    Unspecified,
    Custom(String),
    Rate {
        numerator: Box<StatUnitKind>,
        denominator: Box<StatUnitKind>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatUnit {
    spelling: String,
    kind: StatUnitKind,
}

impl StatUnit {
    pub fn parse(unit: impl Into<String>) -> Result<Self, StatUnitError> {
        let spelling = unit.into();
        validate_stat_unit_characters(&spelling)?;
        let (kind, consumed) = parse_stat_unit_kind(&spelling, 0)?;
        if consumed != spelling.len() {
            let character = spelling.as_bytes()[consumed] as char;
            return Err(StatUnitError::TrailingInput {
                index: consumed,
                character,
            });
        }
        Ok(Self { spelling, kind })
    }

    pub fn cycle() -> Self {
        Self::builtin("Cycle", StatUnitKind::Cycle)
    }

    pub fn tick() -> Self {
        Self::builtin("Tick", StatUnitKind::Tick)
    }

    pub fn second() -> Self {
        Self::builtin("Second", StatUnitKind::Second)
    }

    pub fn bit() -> Self {
        Self::builtin("Bit", StatUnitKind::Bit)
    }

    pub fn byte() -> Self {
        Self::builtin("Byte", StatUnitKind::Byte)
    }

    pub fn watt() -> Self {
        Self::builtin("Watt", StatUnitKind::Watt)
    }

    pub fn joule() -> Self {
        Self::builtin("Joule", StatUnitKind::Joule)
    }

    pub fn volt() -> Self {
        Self::builtin("Volt", StatUnitKind::Volt)
    }

    pub fn celsius() -> Self {
        Self::builtin("Celsius", StatUnitKind::Celsius)
    }

    pub fn count() -> Self {
        Self::builtin("Count", StatUnitKind::Count)
    }

    pub fn ratio() -> Self {
        Self::builtin("Ratio", StatUnitKind::Ratio)
    }

    pub fn unspecified() -> Self {
        Self::builtin("Unspecified", StatUnitKind::Unspecified)
    }

    pub fn rate(numerator: Self, denominator: Self) -> Self {
        let numerator_spelling = numerator.spelling;
        let numerator_kind = numerator.kind;
        let denominator_spelling = denominator.spelling;
        let denominator_kind = denominator.kind;
        Self {
            spelling: format!("({numerator_spelling}/{denominator_spelling})"),
            kind: StatUnitKind::Rate {
                numerator: Box::new(numerator_kind),
                denominator: Box::new(denominator_kind),
            },
        }
    }

    pub fn as_str(&self) -> &str {
        &self.spelling
    }

    pub const fn kind(&self) -> &StatUnitKind {
        &self.kind
    }

    fn builtin(spelling: &str, kind: StatUnitKind) -> Self {
        Self {
            spelling: spelling.to_string(),
            kind,
        }
    }
}

impl fmt::Display for StatUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatSample {
    id: StatId,
    group: Option<StatGroupId>,
    path: StatPath,
    unit: StatUnit,
    value: u64,
}

impl StatSample {
    pub fn new(id: StatId, path: impl Into<String>, unit: impl Into<String>, value: u64) -> Self {
        Self::try_new(id, path, unit, value).expect("stat sample descriptor must be valid")
    }

    pub fn try_new(
        id: StatId,
        path: impl Into<String>,
        unit: impl Into<String>,
        value: u64,
    ) -> Result<Self, StatsError> {
        let path = path.into();
        let stat_path = StatPath::parse(path.clone())
            .map_err(|reason| StatsError::InvalidPath { path, reason })?;
        let unit = unit.into();
        let stat_unit = StatUnit::parse(unit.clone())
            .map_err(|reason| StatsError::InvalidUnit { unit, reason })?;
        Ok(Self {
            id,
            group: None,
            path: stat_path,
            unit: stat_unit,
            value,
        })
    }

    pub const fn from_parts(id: StatId, path: StatPath, unit: StatUnit, value: u64) -> Self {
        Self::from_registered_parts(id, None, path, unit, value)
    }

    pub const fn from_registered_parts(
        id: StatId,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
        value: u64,
    ) -> Self {
        Self {
            id,
            group,
            path,
            unit,
            value,
        }
    }

    pub const fn id(&self) -> StatId {
        self.id
    }

    pub const fn group(&self) -> Option<StatGroupId> {
        self.group
    }

    pub fn path(&self) -> &str {
        self.path.as_str()
    }

    pub const fn stat_path(&self) -> &StatPath {
        &self.path
    }

    pub fn scope(&self) -> &[String] {
        self.path.scope()
    }

    pub fn name(&self) -> &str {
        self.path.name()
    }

    pub fn unit(&self) -> &str {
        self.unit.as_str()
    }

    pub const fn stat_unit(&self) -> &StatUnit {
        &self.unit
    }

    pub const fn value(&self) -> u64 {
        self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatDeltaSample {
    id: StatId,
    group: Option<StatGroupId>,
    path: StatPath,
    unit: StatUnit,
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
        Self::try_new(id, path, unit, previous_value, current_value)
            .expect("stat delta sample unit must be valid")
    }

    pub fn try_new(
        id: StatId,
        path: impl Into<String>,
        unit: impl Into<String>,
        previous_value: u64,
        current_value: u64,
    ) -> Result<Self, StatsError> {
        let path = path.into();
        let stat_path = StatPath::parse(path.clone())
            .map_err(|reason| StatsError::InvalidPath { path, reason })?;
        let unit = unit.into();
        let stat_unit = StatUnit::parse(unit.clone())
            .map_err(|reason| StatsError::InvalidUnit { unit, reason })?;
        Ok(Self {
            id,
            group: None,
            path: stat_path,
            unit: stat_unit,
            previous_value,
            current_value,
        })
    }

    pub const fn from_parts(
        id: StatId,
        path: StatPath,
        unit: StatUnit,
        previous_value: u64,
        current_value: u64,
    ) -> Self {
        Self::from_registered_parts(id, None, path, unit, previous_value, current_value)
    }

    pub const fn from_registered_parts(
        id: StatId,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
        previous_value: u64,
        current_value: u64,
    ) -> Self {
        Self {
            id,
            group,
            path,
            unit,
            previous_value,
            current_value,
        }
    }

    pub const fn id(&self) -> StatId {
        self.id
    }

    pub const fn group(&self) -> Option<StatGroupId> {
        self.group
    }

    pub fn path(&self) -> &str {
        self.path.as_str()
    }

    pub const fn stat_path(&self) -> &StatPath {
        &self.path
    }

    pub fn scope(&self) -> &[String] {
        self.path.scope()
    }

    pub fn name(&self) -> &str {
        self.path.name()
    }

    pub fn unit(&self) -> &str {
        self.unit.as_str()
    }

    pub const fn stat_unit(&self) -> &StatUnit {
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
            deltas.push(StatDeltaSample::from_registered_parts(
                previous_sample.id(),
                previous_sample.group(),
                previous_sample.stat_path().clone(),
                previous_sample.stat_unit().clone(),
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
pub enum StatPathError {
    EmptySegment { index: usize },
    InvalidSegmentStart { segment: String, character: char },
    InvalidSegmentCharacter { segment: String, character: char },
}

impl fmt::Display for StatPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySegment { index } => {
                write!(formatter, "segment {index} must not be empty")
            }
            Self::InvalidSegmentStart { segment, character } => write!(
                formatter,
                "segment {segment} starts with invalid character {character:?}"
            ),
            Self::InvalidSegmentCharacter { segment, character } => write!(
                formatter,
                "segment {segment} contains invalid character {character:?}"
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatUnitError {
    Empty,
    InvalidCharacter { character: char },
    ExpectedTerm { index: usize },
    ExpectedRateSeparator { index: usize },
    ExpectedRateTerminator { index: usize },
    TrailingInput { index: usize, character: char },
}

impl fmt::Display for StatUnitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(formatter, "unit must not be empty"),
            Self::InvalidCharacter { character } => {
                write!(formatter, "unit contains invalid character {character:?}")
            }
            Self::ExpectedTerm { index } => {
                write!(formatter, "unit needs a term at byte {index}")
            }
            Self::ExpectedRateSeparator { index } => {
                write!(formatter, "unit rate needs '/' at byte {index}")
            }
            Self::ExpectedRateTerminator { index } => {
                write!(formatter, "unit rate needs ')' at byte {index}")
            }
            Self::TrailingInput { index, character } => write!(
                formatter,
                "unit has trailing character {character:?} at byte {index}"
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatsRegistry {
    next_id: u64,
    next_dump_id: u64,
    next_group_id: u64,
    epoch: u64,
    reset_tick: Tick,
    paths: BTreeSet<String>,
    group_paths: BTreeMap<String, StatGroupId>,
    groups: BTreeMap<StatGroupId, StatScope>,
    descriptors: BTreeMap<StatId, StatDescriptor>,
    counters: BTreeMap<StatId, u64>,
    dump_records: Vec<StatDumpRecord>,
}

impl StatsRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            next_dump_id: 0,
            next_group_id: 0,
            epoch: 0,
            reset_tick: 0,
            paths: BTreeSet::new(),
            group_paths: BTreeMap::new(),
            groups: BTreeMap::new(),
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
        let unit = unit.into();
        let unit = match StatUnit::parse(unit.clone()) {
            Ok(unit) => unit,
            Err(reason) => return Err(StatsError::InvalidUnit { unit, reason }),
        };
        self.register_counter_with_unit(path, unit)
    }

    pub fn register_counter_with_unit(
        &mut self,
        path: impl Into<String>,
        unit: StatUnit,
    ) -> Result<StatId, StatsError> {
        let path = path.into();
        if path.is_empty() {
            return Err(StatsError::EmptyPath);
        }
        let stat_path = StatPath::parse(path.clone())
            .map_err(|reason| StatsError::InvalidPath { path, reason })?;
        self.register_counter_path(None, stat_path, unit)
    }

    pub fn register_scoped_counter<I, S>(
        &mut self,
        scope: I,
        name: impl Into<String>,
        unit: impl Into<String>,
    ) -> Result<StatId, StatsError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let unit = unit.into();
        let unit = match StatUnit::parse(unit.clone()) {
            Ok(unit) => unit,
            Err(reason) => return Err(StatsError::InvalidUnit { unit, reason }),
        };
        self.register_scoped_counter_with_unit(scope, name, unit)
    }

    pub fn register_scoped_counter_with_unit<I, S>(
        &mut self,
        scope: I,
        name: impl Into<String>,
        unit: StatUnit,
    ) -> Result<StatId, StatsError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut segments = scope.into_iter().map(Into::into).collect::<Vec<_>>();
        segments.push(name.into());
        let path = segments.join(".");
        let stat_path = StatPath::from_segments(segments)
            .map_err(|reason| StatsError::InvalidPath { path, reason })?;
        self.register_counter_path(None, stat_path, unit)
    }

    pub fn register_group<I, S>(&mut self, scope: I) -> Result<StatGroupId, StatsError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let segments = scope.into_iter().map(Into::into).collect::<Vec<_>>();
        let path = segments.join(".");
        let scope = StatScope::from_segments(segments)
            .map_err(|reason| StatsError::InvalidPath { path, reason })?;
        if self.group_paths.contains_key(scope.as_str()) {
            return Err(StatsError::DuplicateGroup {
                scope: scope.as_str().to_string(),
            });
        }

        let id = StatGroupId::new(self.next_group_id);
        self.next_group_id = self
            .next_group_id
            .checked_add(1)
            .ok_or(StatsError::GroupSequenceOverflow)?;
        self.group_paths.insert(scope.as_str().to_string(), id);
        self.groups.insert(id, scope);
        Ok(id)
    }

    pub fn register_group_counter(
        &mut self,
        group: StatGroupId,
        name: impl Into<String>,
        unit: impl Into<String>,
    ) -> Result<StatId, StatsError> {
        let unit = unit.into();
        let unit = match StatUnit::parse(unit.clone()) {
            Ok(unit) => unit,
            Err(reason) => return Err(StatsError::InvalidUnit { unit, reason }),
        };
        self.register_group_counter_with_unit(group, name, unit)
    }

    pub fn register_group_counter_with_unit(
        &mut self,
        group: StatGroupId,
        name: impl Into<String>,
        unit: StatUnit,
    ) -> Result<StatId, StatsError> {
        let Some(scope) = self.groups.get(&group) else {
            return Err(StatsError::UnknownStatGroup { group });
        };
        let name = name.into();
        let mut segments = scope.segments().to_vec();
        segments.push(name);
        let path = segments.join(".");
        let stat_path = StatPath::from_segments(segments)
            .map_err(|reason| StatsError::InvalidPath { path, reason })?;
        self.register_counter_path(Some(group), stat_path, unit)
    }

    fn register_counter_path(
        &mut self,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
    ) -> Result<StatId, StatsError> {
        if self.paths.contains(path.as_str()) {
            return Err(StatsError::DuplicatePath {
                path: path.as_str().to_string(),
            });
        }

        let id = StatId::new(self.next_id);
        self.next_id += 1;
        self.paths.insert(path.as_str().to_string());
        self.descriptors
            .insert(id, StatDescriptor { group, path, unit });
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
                StatSample::from_registered_parts(
                    *id,
                    descriptor.group,
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

fn validate_stat_path(path: &str) -> Result<(), StatPathError> {
    validate_stat_segments(path.split('.'))
}

fn validate_stat_segments<'a>(
    segments: impl IntoIterator<Item = &'a str>,
) -> Result<(), StatPathError> {
    let mut saw_segment = false;
    for (index, segment) in segments.into_iter().enumerate() {
        saw_segment = true;
        let mut chars = segment.chars();
        let Some(first) = chars.next() else {
            return Err(StatPathError::EmptySegment { index });
        };
        if !first.is_ascii_alphabetic() && first != '_' {
            return Err(StatPathError::InvalidSegmentStart {
                segment: segment.to_string(),
                character: first,
            });
        }
        for character in chars {
            if !character.is_ascii_alphanumeric() && character != '_' {
                return Err(StatPathError::InvalidSegmentCharacter {
                    segment: segment.to_string(),
                    character,
                });
            }
        }
    }
    if !saw_segment {
        return Err(StatPathError::EmptySegment { index: 0 });
    }
    Ok(())
}

fn validate_stat_unit_characters(unit: &str) -> Result<(), StatUnitError> {
    if unit.is_empty() {
        return Err(StatUnitError::Empty);
    }
    for character in unit.chars() {
        if !character.is_ascii_alphanumeric()
            && character != '_'
            && character != '/'
            && character != '('
            && character != ')'
        {
            return Err(StatUnitError::InvalidCharacter { character });
        }
    }
    Ok(())
}

fn parse_stat_unit_kind(unit: &str, index: usize) -> Result<(StatUnitKind, usize), StatUnitError> {
    let Some(character) = unit.as_bytes().get(index).copied().map(char::from) else {
        return Err(StatUnitError::ExpectedTerm { index });
    };
    match character {
        '(' => {
            let (numerator, after_numerator) = parse_stat_unit_kind(unit, index + 1)?;
            if unit.as_bytes().get(after_numerator).copied() != Some(b'/') {
                return Err(StatUnitError::ExpectedRateSeparator {
                    index: after_numerator,
                });
            }
            let (denominator, after_denominator) = parse_stat_unit_kind(unit, after_numerator + 1)?;
            if unit.as_bytes().get(after_denominator).copied() != Some(b')') {
                return Err(StatUnitError::ExpectedRateTerminator {
                    index: after_denominator,
                });
            }
            Ok((
                StatUnitKind::Rate {
                    numerator: Box::new(numerator),
                    denominator: Box::new(denominator),
                },
                after_denominator + 1,
            ))
        }
        ')' | '/' => Err(StatUnitError::ExpectedTerm { index }),
        _ => {
            let mut end = index;
            while let Some(character) = unit.as_bytes().get(end).copied().map(char::from) {
                if !character.is_ascii_alphanumeric() && character != '_' {
                    break;
                }
                end += 1;
            }
            if end == index {
                return Err(StatUnitError::ExpectedTerm { index });
            }
            Ok((stat_unit_symbol_kind(&unit[index..end]), end))
        }
    }
}

fn stat_unit_symbol_kind(symbol: &str) -> StatUnitKind {
    match symbol {
        "Cycle" => StatUnitKind::Cycle,
        "Tick" => StatUnitKind::Tick,
        "Second" => StatUnitKind::Second,
        "Bit" => StatUnitKind::Bit,
        "Byte" => StatUnitKind::Byte,
        "Watt" => StatUnitKind::Watt,
        "Joule" => StatUnitKind::Joule,
        "Volt" => StatUnitKind::Volt,
        "Celsius" => StatUnitKind::Celsius,
        "Count" => StatUnitKind::Count,
        "Ratio" => StatUnitKind::Ratio,
        "Unspecified" => StatUnitKind::Unspecified,
        _ => StatUnitKind::Custom(symbol.to_string()),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StatDescriptor {
    group: Option<StatGroupId>,
    path: StatPath,
    unit: StatUnit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatsError {
    EmptyPath,
    InvalidPath {
        path: String,
        reason: StatPathError,
    },
    InvalidUnit {
        unit: String,
        reason: StatUnitError,
    },
    DuplicatePath {
        path: String,
    },
    DuplicateGroup {
        scope: String,
    },
    UnknownStat {
        stat: StatId,
    },
    UnknownStatGroup {
        group: StatGroupId,
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
    GroupSequenceOverflow,
    DumpSequenceOverflow,
}

impl fmt::Display for StatsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPath => write!(formatter, "stat path must not be empty"),
            Self::InvalidPath { path, reason } => {
                write!(formatter, "stat path {path} is invalid: {reason}")
            }
            Self::InvalidUnit { unit, reason } => {
                write!(formatter, "stat unit {unit} is invalid: {reason}")
            }
            Self::DuplicatePath { path } => write!(formatter, "stat path already exists: {path}"),
            Self::DuplicateGroup { scope } => {
                write!(formatter, "stat group already exists: {scope}")
            }
            Self::UnknownStat { stat } => write!(formatter, "unknown stat id {}", stat.get()),
            Self::UnknownStatGroup { group } => {
                write!(formatter, "unknown stat group id {}", group.get())
            }
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
            Self::GroupSequenceOverflow => write!(formatter, "stat group sequence overflowed"),
            Self::DumpSequenceOverflow => write!(formatter, "stat dump sequence overflowed"),
        }
    }
}

impl Error for StatsError {}

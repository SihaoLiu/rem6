use std::collections::BTreeMap;
use std::fmt;

use rem6_kernel::Tick;

use crate::error::StatsError;
use crate::kind::StatKind;
use crate::reset::{StatResetPolicy, StatsResetRecord};

macro_rules! stat_id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u64);

        impl $name {
            pub const fn new(value: u64) -> Self {
                Self(value)
            }
            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

stat_id_type!(StatId);
stat_id_type!(StatDumpId);
stat_id_type!(StatResetId);
stat_id_type!(StatGroupId);

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatGroupDescriptor {
    id: StatGroupId,
    scope: StatScope,
}

impl StatGroupDescriptor {
    pub const fn new(id: StatGroupId, scope: StatScope) -> Self {
        Self { id, scope }
    }

    pub const fn id(&self) -> StatGroupId {
        self.id
    }

    pub const fn scope(&self) -> &StatScope {
        &self.scope
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
        let spelling = if spelling == "DegreeCelsius" {
            "Celsius".to_string()
        } else {
            spelling
        };
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

    pub fn degree_celsius() -> Self {
        Self::celsius()
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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatDescription {
    spelling: String,
}

impl StatDescription {
    pub fn new(description: impl Into<String>) -> Result<Self, StatDescriptionError> {
        let spelling = description.into();
        validate_stat_description(&spelling)?;
        Ok(Self { spelling })
    }

    pub fn as_str(&self) -> &str {
        &self.spelling
    }
}

impl fmt::Display for StatDescription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatSample {
    id: StatId,
    group: Option<StatGroupId>,
    kind: StatKind,
    path: StatPath,
    unit: StatUnit,
    reset_policy: StatResetPolicy,
    description: Option<StatDescription>,
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
            kind: StatKind::Counter,
            path: stat_path,
            unit: stat_unit,
            reset_policy: StatResetPolicy::Resettable,
            description: None,
            value,
        })
    }

    pub const fn from_parts(id: StatId, path: StatPath, unit: StatUnit, value: u64) -> Self {
        Self::from_registered_parts(id, None, path, unit, value)
    }

    pub const fn from_parts_with_reset_policy(
        id: StatId,
        path: StatPath,
        unit: StatUnit,
        reset_policy: StatResetPolicy,
        value: u64,
    ) -> Self {
        Self::from_registered_parts_with_reset_policy(id, None, path, unit, reset_policy, value)
    }

    pub const fn from_parts_with_description(
        id: StatId,
        path: StatPath,
        unit: StatUnit,
        description: Option<StatDescription>,
        value: u64,
    ) -> Self {
        Self::from_registered_parts_with_description(id, None, path, unit, description, value)
    }

    pub const fn from_registered_parts(
        id: StatId,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
        value: u64,
    ) -> Self {
        Self::from_registered_parts_with_description(id, group, path, unit, None, value)
    }

    pub const fn from_registered_parts_with_reset_policy(
        id: StatId,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
        reset_policy: StatResetPolicy,
        value: u64,
    ) -> Self {
        Self::from_registered_parts_with_reset_policy_and_description(
            id,
            group,
            path,
            unit,
            reset_policy,
            None,
            value,
        )
    }

    pub const fn from_registered_parts_with_description(
        id: StatId,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
        description: Option<StatDescription>,
        value: u64,
    ) -> Self {
        Self::from_registered_parts_with_reset_policy_and_description(
            id,
            group,
            path,
            unit,
            StatResetPolicy::Resettable,
            description,
            value,
        )
    }

    pub const fn from_registered_parts_with_reset_policy_and_description(
        id: StatId,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
        reset_policy: StatResetPolicy,
        description: Option<StatDescription>,
        value: u64,
    ) -> Self {
        Self::from_registered_parts_with_kind_reset_policy_and_description(
            id,
            group,
            StatKind::Counter,
            path,
            unit,
            reset_policy,
            description,
            value,
        )
    }

    pub const fn from_registered_parts_with_kind_reset_policy_and_description(
        id: StatId,
        group: Option<StatGroupId>,
        kind: StatKind,
        path: StatPath,
        unit: StatUnit,
        reset_policy: StatResetPolicy,
        description: Option<StatDescription>,
        value: u64,
    ) -> Self {
        Self {
            id,
            group,
            kind,
            path,
            unit,
            reset_policy,
            description,
            value,
        }
    }

    pub const fn id(&self) -> StatId {
        self.id
    }

    pub const fn group(&self) -> Option<StatGroupId> {
        self.group
    }

    pub const fn kind(&self) -> StatKind {
        self.kind
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

    pub const fn reset_policy(&self) -> StatResetPolicy {
        self.reset_policy
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_ref().map(StatDescription::as_str)
    }

    pub const fn stat_description(&self) -> Option<&StatDescription> {
        self.description.as_ref()
    }

    pub const fn value(&self) -> u64 {
        self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatDeltaSample {
    id: StatId,
    group: Option<StatGroupId>,
    kind: StatKind,
    path: StatPath,
    unit: StatUnit,
    reset_policy: StatResetPolicy,
    description: Option<StatDescription>,
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
            kind: StatKind::Counter,
            path: stat_path,
            unit: stat_unit,
            reset_policy: StatResetPolicy::Resettable,
            description: None,
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

    pub const fn from_parts_with_reset_policy(
        id: StatId,
        path: StatPath,
        unit: StatUnit,
        reset_policy: StatResetPolicy,
        previous_value: u64,
        current_value: u64,
    ) -> Self {
        Self::from_registered_parts_with_reset_policy(
            id,
            None,
            path,
            unit,
            reset_policy,
            previous_value,
            current_value,
        )
    }

    pub const fn from_parts_with_description(
        id: StatId,
        path: StatPath,
        unit: StatUnit,
        description: Option<StatDescription>,
        previous_value: u64,
        current_value: u64,
    ) -> Self {
        Self::from_registered_parts_with_description(
            id,
            None,
            path,
            unit,
            description,
            previous_value,
            current_value,
        )
    }

    pub const fn from_registered_parts(
        id: StatId,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
        previous_value: u64,
        current_value: u64,
    ) -> Self {
        Self::from_registered_parts_with_description(
            id,
            group,
            path,
            unit,
            None,
            previous_value,
            current_value,
        )
    }

    pub const fn from_registered_parts_with_reset_policy(
        id: StatId,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
        reset_policy: StatResetPolicy,
        previous_value: u64,
        current_value: u64,
    ) -> Self {
        Self::from_registered_parts_with_kind_reset_policy_and_description(
            id,
            group,
            StatKind::Counter,
            path,
            unit,
            reset_policy,
            None,
            previous_value,
            current_value,
        )
    }

    pub const fn from_registered_parts_with_description(
        id: StatId,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
        description: Option<StatDescription>,
        previous_value: u64,
        current_value: u64,
    ) -> Self {
        Self::from_registered_parts_with_kind_reset_policy_and_description(
            id,
            group,
            StatKind::Counter,
            path,
            unit,
            StatResetPolicy::Resettable,
            description,
            previous_value,
            current_value,
        )
    }

    pub const fn from_registered_parts_with_kind_reset_policy_and_description(
        id: StatId,
        group: Option<StatGroupId>,
        kind: StatKind,
        path: StatPath,
        unit: StatUnit,
        reset_policy: StatResetPolicy,
        description: Option<StatDescription>,
        previous_value: u64,
        current_value: u64,
    ) -> Self {
        Self {
            id,
            group,
            kind,
            path,
            unit,
            reset_policy,
            description,
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

    pub const fn kind(&self) -> StatKind {
        self.kind
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

    pub const fn reset_policy(&self) -> StatResetPolicy {
        self.reset_policy
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_ref().map(StatDescription::as_str)
    }

    pub const fn stat_description(&self) -> Option<&StatDescription> {
        self.description.as_ref()
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
    groups: Vec<StatGroupDescriptor>,
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
        Self::with_groups(
            previous_tick,
            current_tick,
            epoch,
            reset_tick,
            Vec::new(),
            samples,
        )
    }

    pub const fn with_groups(
        previous_tick: Tick,
        current_tick: Tick,
        epoch: u64,
        reset_tick: Tick,
        groups: Vec<StatGroupDescriptor>,
        samples: Vec<StatDeltaSample>,
    ) -> Self {
        Self {
            previous_tick,
            current_tick,
            epoch,
            reset_tick,
            groups,
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

    pub fn groups(&self) -> &[StatGroupDescriptor] {
        &self.groups
    }

    pub fn group_scope(&self, group: StatGroupId) -> Option<&StatScope> {
        self.groups
            .iter()
            .find(|descriptor| descriptor.id() == group)
            .map(StatGroupDescriptor::scope)
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
    groups: Vec<StatGroupDescriptor>,
    samples: Vec<StatSample>,
}

impl StatSnapshot {
    pub const fn new(tick: Tick, epoch: u64, reset_tick: Tick, samples: Vec<StatSample>) -> Self {
        Self::with_groups(tick, epoch, reset_tick, Vec::new(), samples)
    }

    pub const fn with_groups(
        tick: Tick,
        epoch: u64,
        reset_tick: Tick,
        groups: Vec<StatGroupDescriptor>,
        samples: Vec<StatSample>,
    ) -> Self {
        Self {
            tick,
            epoch,
            reset_tick,
            groups,
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

    pub fn groups(&self) -> &[StatGroupDescriptor] {
        &self.groups
    }

    pub fn group_scope(&self, group: StatGroupId) -> Option<&StatScope> {
        self.groups
            .iter()
            .find(|descriptor| descriptor.id() == group)
            .map(StatGroupDescriptor::scope)
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
        if self.groups != previous.groups {
            return Err(StatsError::SnapshotDeltaGroupCatalogMismatch {
                previous_groups: previous.groups.clone(),
                current_groups: self.groups.clone(),
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
            if current_sample.stat_description() != previous_sample.stat_description() {
                return Err(StatsError::SnapshotDeltaDescriptionMismatch {
                    stat: previous_sample.id(),
                    previous_description: previous_sample.stat_description().cloned(),
                    current_description: current_sample.stat_description().cloned(),
                });
            }
            if current_sample.reset_policy() != previous_sample.reset_policy() {
                return Err(StatsError::SnapshotDeltaResetPolicyMismatch {
                    stat: previous_sample.id(),
                    previous_policy: previous_sample.reset_policy(),
                    current_policy: current_sample.reset_policy(),
                });
            }
            if current_sample.kind() != previous_sample.kind() {
                return Err(StatsError::SnapshotDeltaStatKindMismatch {
                    stat: previous_sample.id(),
                    previous_kind: previous_sample.kind(),
                    current_kind: current_sample.kind(),
                });
            }
            if previous_sample.kind() != StatKind::Counter {
                return Err(StatsError::SnapshotDeltaUnsupportedStatKind {
                    stat: previous_sample.id(),
                    kind: previous_sample.kind(),
                });
            }
            if current_sample.value() < previous_sample.value() {
                return Err(StatsError::SnapshotDeltaValueWentBack {
                    stat: previous_sample.id(),
                    previous: previous_sample.value(),
                    current: current_sample.value(),
                });
            }
            deltas.push(StatDeltaSample {
                id: previous_sample.id(),
                group: previous_sample.group(),
                kind: previous_sample.kind(),
                path: previous_sample.stat_path().clone(),
                unit: previous_sample.stat_unit().clone(),
                reset_policy: previous_sample.reset_policy(),
                description: previous_sample.stat_description().cloned(),
                previous_value: previous_sample.value(),
                current_value: current_sample.value(),
            });
        }

        Ok(StatSnapshotDelta::with_groups(
            previous.tick,
            self.tick,
            self.epoch,
            self.reset_tick,
            previous.groups.clone(),
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
pub enum StatHistoryRecord {
    Dump(StatDumpRecord),
    Reset(StatsResetRecord),
}

impl StatHistoryRecord {
    pub const fn tick(&self) -> Tick {
        match self {
            Self::Dump(record) => record.tick(),
            Self::Reset(record) => record.tick(),
        }
    }

    pub const fn epoch(&self) -> u64 {
        match self {
            Self::Dump(record) => record.epoch(),
            Self::Reset(record) => record.epoch(),
        }
    }

    pub const fn reset_tick(&self) -> Tick {
        match self {
            Self::Dump(record) => record.reset_tick(),
            Self::Reset(record) => record.tick(),
        }
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
pub enum StatDescriptionError {
    Empty,
    InvalidCharacter { character: char },
}

impl fmt::Display for StatDescriptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(formatter, "description must not be empty"),
            Self::InvalidCharacter { character } => {
                write!(
                    formatter,
                    "description contains invalid character {character:?}"
                )
            }
        }
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

fn validate_stat_description(description: &str) -> Result<(), StatDescriptionError> {
    if description.trim().is_empty() {
        return Err(StatDescriptionError::Empty);
    }
    for character in description.chars() {
        if character.is_control() {
            return Err(StatDescriptionError::InvalidCharacter { character });
        }
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
        "Celsius" | "DegreeCelsius" => StatUnitKind::Celsius,
        "Count" => StatUnitKind::Count,
        "Ratio" => StatUnitKind::Ratio,
        "Unspecified" => StatUnitKind::Unspecified,
        _ => StatUnitKind::Custom(symbol.to_string()),
    }
}

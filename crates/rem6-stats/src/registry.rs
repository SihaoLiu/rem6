use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::Tick;

use crate::error::StatsError;
use crate::kind::StatKind;
use crate::reset::{StatResetPolicy, StatResetSample, StatsResetRecord};
use crate::stats::{
    StatDescription, StatDumpId, StatDumpRecord, StatGroupDescriptor, StatGroupId,
    StatHistoryRecord, StatId, StatPath, StatResetId, StatSample, StatScope, StatSnapshot,
    StatUnit,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatsRegistry {
    next_id: u64,
    next_dump_id: u64,
    next_reset_id: u64,
    next_group_id: u64,
    epoch: u64,
    reset_tick: Tick,
    paths: BTreeSet<String>,
    group_paths: BTreeMap<String, StatGroupId>,
    groups: BTreeMap<StatGroupId, StatScope>,
    descriptors: BTreeMap<StatId, StatDescriptor>,
    storage: BTreeMap<StatId, StatStorage>,
    dump_records: Vec<StatDumpRecord>,
    reset_records: Vec<StatsResetRecord>,
    history_records: Vec<StatHistoryRecord>,
}

impl StatsRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            next_dump_id: 0,
            next_reset_id: 0,
            next_group_id: 0,
            epoch: 0,
            reset_tick: 0,
            paths: BTreeSet::new(),
            group_paths: BTreeMap::new(),
            groups: BTreeMap::new(),
            descriptors: BTreeMap::new(),
            storage: BTreeMap::new(),
            dump_records: Vec::new(),
            reset_records: Vec::new(),
            history_records: Vec::new(),
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

    pub fn register_counter_with_description(
        &mut self,
        path: impl Into<String>,
        unit: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<StatId, StatsError> {
        let unit = unit.into();
        let unit = match StatUnit::parse(unit.clone()) {
            Ok(unit) => unit,
            Err(reason) => return Err(StatsError::InvalidUnit { unit, reason }),
        };
        self.register_counter_with_unit_and_description(path, unit, description)
    }

    pub fn register_counter_with_unit(
        &mut self,
        path: impl Into<String>,
        unit: StatUnit,
    ) -> Result<StatId, StatsError> {
        self.register_counter_with_optional_description(
            path,
            unit,
            StatResetPolicy::Resettable,
            None,
        )
    }

    pub fn register_counter_with_reset_policy(
        &mut self,
        path: impl Into<String>,
        unit: impl Into<String>,
        reset_policy: StatResetPolicy,
    ) -> Result<StatId, StatsError> {
        let unit = unit.into();
        let unit = match StatUnit::parse(unit.clone()) {
            Ok(unit) => unit,
            Err(reason) => return Err(StatsError::InvalidUnit { unit, reason }),
        };
        self.register_counter_with_unit_and_reset_policy(path, unit, reset_policy)
    }

    pub fn register_counter_with_unit_and_reset_policy(
        &mut self,
        path: impl Into<String>,
        unit: StatUnit,
        reset_policy: StatResetPolicy,
    ) -> Result<StatId, StatsError> {
        self.register_counter_with_optional_description(path, unit, reset_policy, None)
    }

    pub fn register_counter_with_unit_and_description(
        &mut self,
        path: impl Into<String>,
        unit: StatUnit,
        description: impl Into<String>,
    ) -> Result<StatId, StatsError> {
        let description = parse_stat_description(description)?;
        self.register_counter_with_optional_description(
            path,
            unit,
            StatResetPolicy::Resettable,
            Some(description),
        )
    }

    fn register_counter_with_optional_description(
        &mut self,
        path: impl Into<String>,
        unit: StatUnit,
        reset_policy: StatResetPolicy,
        description: Option<StatDescription>,
    ) -> Result<StatId, StatsError> {
        let path = path.into();
        if path.is_empty() {
            return Err(StatsError::EmptyPath);
        }
        let stat_path = StatPath::parse(path.clone())
            .map_err(|reason| StatsError::InvalidPath { path, reason })?;
        self.register_counter_path(None, stat_path, unit, reset_policy, description)
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
        self.register_counter_path(None, stat_path, unit, StatResetPolicy::Resettable, None)
    }

    pub fn register_average(
        &mut self,
        path: impl Into<String>,
        unit: impl Into<String>,
    ) -> Result<StatId, StatsError> {
        let unit = unit.into();
        let unit = match StatUnit::parse(unit.clone()) {
            Ok(unit) => unit,
            Err(reason) => return Err(StatsError::InvalidUnit { unit, reason }),
        };
        self.register_average_with_unit(path, unit)
    }

    pub fn register_average_with_unit(
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
        self.register_stat_path(
            None,
            stat_path,
            unit,
            StatResetPolicy::Resettable,
            None,
            StatStorage::Average(StatAverageStorage::new(self.reset_tick)),
        )
    }

    pub fn register_group<I, S>(&mut self, scope: I) -> Result<StatGroupId, StatsError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.ensure_schema_open()?;
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

    pub fn register_group_counter_with_description(
        &mut self,
        group: StatGroupId,
        name: impl Into<String>,
        unit: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<StatId, StatsError> {
        let unit = unit.into();
        let unit = match StatUnit::parse(unit.clone()) {
            Ok(unit) => unit,
            Err(reason) => return Err(StatsError::InvalidUnit { unit, reason }),
        };
        self.register_group_counter_with_unit_and_description(group, name, unit, description)
    }

    pub fn register_group_counter_with_unit(
        &mut self,
        group: StatGroupId,
        name: impl Into<String>,
        unit: StatUnit,
    ) -> Result<StatId, StatsError> {
        self.register_group_counter_with_optional_description(
            group,
            name,
            unit,
            StatResetPolicy::Resettable,
            None,
        )
    }

    pub fn register_group_counter_with_reset_policy(
        &mut self,
        group: StatGroupId,
        name: impl Into<String>,
        unit: impl Into<String>,
        reset_policy: StatResetPolicy,
    ) -> Result<StatId, StatsError> {
        let unit = unit.into();
        let unit = match StatUnit::parse(unit.clone()) {
            Ok(unit) => unit,
            Err(reason) => return Err(StatsError::InvalidUnit { unit, reason }),
        };
        self.register_group_counter_with_unit_and_reset_policy(group, name, unit, reset_policy)
    }

    pub fn register_group_counter_with_unit_and_reset_policy(
        &mut self,
        group: StatGroupId,
        name: impl Into<String>,
        unit: StatUnit,
        reset_policy: StatResetPolicy,
    ) -> Result<StatId, StatsError> {
        self.register_group_counter_with_optional_description(group, name, unit, reset_policy, None)
    }

    pub fn register_group_counter_with_unit_and_description(
        &mut self,
        group: StatGroupId,
        name: impl Into<String>,
        unit: StatUnit,
        description: impl Into<String>,
    ) -> Result<StatId, StatsError> {
        let description = parse_stat_description(description)?;
        self.register_group_counter_with_optional_description(
            group,
            name,
            unit,
            StatResetPolicy::Resettable,
            Some(description),
        )
    }

    fn register_group_counter_with_optional_description(
        &mut self,
        group: StatGroupId,
        name: impl Into<String>,
        unit: StatUnit,
        reset_policy: StatResetPolicy,
        description: Option<StatDescription>,
    ) -> Result<StatId, StatsError> {
        self.ensure_schema_open()?;
        let Some(scope) = self.groups.get(&group) else {
            return Err(StatsError::UnknownStatGroup { group });
        };
        let name = name.into();
        let mut segments = scope.segments().to_vec();
        segments.push(name);
        let path = segments.join(".");
        let stat_path = StatPath::from_segments(segments)
            .map_err(|reason| StatsError::InvalidPath { path, reason })?;
        self.register_counter_path(Some(group), stat_path, unit, reset_policy, description)
    }

    fn register_counter_path(
        &mut self,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
        reset_policy: StatResetPolicy,
        description: Option<StatDescription>,
    ) -> Result<StatId, StatsError> {
        self.register_stat_path(
            group,
            path,
            unit,
            reset_policy,
            description,
            StatStorage::Counter(0),
        )
    }

    fn register_stat_path(
        &mut self,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
        reset_policy: StatResetPolicy,
        description: Option<StatDescription>,
        storage: StatStorage,
    ) -> Result<StatId, StatsError> {
        self.ensure_schema_open()?;
        if self.paths.contains(path.as_str()) {
            return Err(StatsError::DuplicatePath {
                path: path.as_str().to_string(),
            });
        }

        let id = StatId::new(self.next_id);
        self.next_id += 1;
        self.paths.insert(path.as_str().to_string());
        self.descriptors.insert(
            id,
            StatDescriptor {
                group,
                kind: storage.kind(),
                path,
                unit,
                reset_policy,
                description,
            },
        );
        self.storage.insert(id, storage);
        Ok(id)
    }

    fn ensure_schema_open(&self) -> Result<(), StatsError> {
        if self.history_records.is_empty() {
            return Ok(());
        }

        Err(StatsError::SchemaLocked {
            history_records: self.history_records.len(),
        })
    }

    fn ensure_history_tick_not_before_last(&self, tick: Tick) -> Result<(), StatsError> {
        let Some(last_record) = self.history_records.last() else {
            return Ok(());
        };
        let last_history_tick = last_record.tick();
        if tick < last_history_tick {
            return Err(StatsError::HistoryTickBeforeLastRecord {
                tick,
                last_history_tick,
            });
        }

        Ok(())
    }

    pub fn increment(&mut self, stat: StatId, value: u64) -> Result<(), StatsError> {
        match self
            .storage
            .get_mut(&stat)
            .ok_or(StatsError::UnknownStat { stat })?
        {
            StatStorage::Counter(counter) => {
                *counter = counter
                    .checked_add(value)
                    .ok_or(StatsError::CounterOverflow { stat })?;
            }
            StatStorage::Average(_) => return Err(StatsError::StatIsNotCounter { stat }),
        }
        Ok(())
    }

    pub fn set_average(&mut self, stat: StatId, tick: Tick, value: u64) -> Result<(), StatsError> {
        match self
            .storage
            .get_mut(&stat)
            .ok_or(StatsError::UnknownStat { stat })?
        {
            StatStorage::Average(average) => average.set(stat, self.reset_tick, tick, value),
            StatStorage::Counter(_) => Err(StatsError::StatIsNotAverage { stat }),
        }
    }

    pub fn average_value(&self, stat: StatId, tick: Tick) -> Result<u64, StatsError> {
        if tick < self.reset_tick {
            return Err(StatsError::SnapshotBeforeReset {
                tick,
                reset_tick: self.reset_tick,
            });
        }
        let storage = self
            .storage
            .get(&stat)
            .ok_or(StatsError::UnknownStat { stat })?;
        match storage {
            StatStorage::Average(average) => average.value_at(stat, self.reset_tick, tick),
            StatStorage::Counter(_) => Err(StatsError::StatIsNotAverage { stat }),
        }
    }

    fn stat_value(&self, stat: StatId, tick: Tick) -> Result<u64, StatsError> {
        let storage = self
            .storage
            .get(&stat)
            .ok_or(StatsError::UnknownStat { stat })?;
        match storage {
            StatStorage::Counter(value) => Ok(*value),
            StatStorage::Average(average) => average.value_at(stat, self.reset_tick, tick),
        }
    }

    fn reset_stat_storage(
        &mut self,
        stat: StatId,
        tick: Tick,
        reset_value: u64,
    ) -> Result<(), StatsError> {
        let storage = self
            .storage
            .get_mut(&stat)
            .ok_or(StatsError::UnknownStat { stat })?;
        match storage {
            StatStorage::Counter(value) => *value = reset_value,
            StatStorage::Average(average) => average.reset(tick, reset_value),
        }
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
            .expect("snapshot tick and average sample order must be valid")
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
                Ok(
                    StatSample::from_registered_parts_with_kind_reset_policy_and_description(
                        *id,
                        descriptor.group,
                        descriptor.kind,
                        descriptor.path.clone(),
                        descriptor.unit.clone(),
                        descriptor.reset_policy,
                        descriptor.description.clone(),
                        self.stat_value(*id, tick)?,
                    ),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let groups = self
            .groups
            .iter()
            .map(|(id, scope)| StatGroupDescriptor::new(*id, scope.clone()))
            .collect();
        Ok(StatSnapshot::with_groups(
            tick,
            self.epoch,
            self.reset_tick,
            groups,
            samples,
        ))
    }

    pub fn dump(&mut self, tick: Tick) -> StatDumpRecord {
        self.try_dump(tick)
            .expect("dump tick must be at or after the last reset")
    }

    pub fn try_dump(&mut self, tick: Tick) -> Result<StatDumpRecord, StatsError> {
        let snapshot = self.try_snapshot(tick)?;
        self.ensure_history_tick_not_before_last(tick)?;
        let id = StatDumpId::new(self.next_dump_id);
        self.next_dump_id = self
            .next_dump_id
            .checked_add(1)
            .ok_or(StatsError::DumpSequenceOverflow)?;
        let record = StatDumpRecord::new(id, snapshot);
        self.dump_records.push(record.clone());
        self.history_records
            .push(StatHistoryRecord::Dump(record.clone()));
        Ok(record)
    }

    pub fn dump_records(&self) -> &[StatDumpRecord] {
        &self.dump_records
    }

    pub fn reset_records(&self) -> &[StatsResetRecord] {
        &self.reset_records
    }

    pub fn history_records(&self) -> &[StatHistoryRecord] {
        &self.history_records
    }

    pub fn reset(&mut self, tick: Tick) -> StatsResetRecord {
        self.try_reset(tick)
            .expect("reset tick, history order, and average sample order must be valid")
    }

    pub fn try_reset(&mut self, tick: Tick) -> Result<StatsResetRecord, StatsError> {
        if tick < self.reset_tick {
            return Err(StatsError::ResetBeforeLastReset {
                tick,
                reset_tick: self.reset_tick,
            });
        }
        self.ensure_history_tick_not_before_last(tick)?;

        let mut reset_samples = Vec::new();
        let mut reset_values = Vec::new();
        let ids = self.descriptors.keys().copied().collect::<Vec<_>>();
        for id in ids {
            let descriptor = self
                .descriptors
                .get(&id)
                .expect("registered stat descriptor exists");
            let previous_value = self.stat_value(id, tick)?;
            let reset_value = self
                .storage
                .get(&id)
                .ok_or(StatsError::UnknownStat { stat: id })?
                .value_after_reset(descriptor.reset_policy, previous_value);
            reset_samples.push(StatResetSample::new(
                id,
                descriptor.reset_policy,
                previous_value,
                reset_value,
            ));
            reset_values.push((id, reset_value));
        }
        let id = StatResetId::new(self.next_reset_id);
        let next_reset_id = self
            .next_reset_id
            .checked_add(1)
            .ok_or(StatsError::ResetSequenceOverflow)?;
        let next_epoch = self
            .epoch
            .checked_add(1)
            .ok_or(StatsError::ResetSequenceOverflow)?;
        for (id, reset_value) in reset_values {
            self.reset_stat_storage(id, tick, reset_value)?;
        }
        self.next_reset_id = next_reset_id;
        self.epoch = next_epoch;
        self.reset_tick = tick;
        let record = StatsResetRecord::with_reset_samples(id, tick, self.epoch, reset_samples);
        self.reset_records.push(record.clone());
        self.history_records
            .push(StatHistoryRecord::Reset(record.clone()));
        Ok(record)
    }
}

impl Default for StatsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StatDescriptor {
    group: Option<StatGroupId>,
    kind: StatKind,
    path: StatPath,
    unit: StatUnit,
    reset_policy: StatResetPolicy,
    description: Option<StatDescription>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StatStorage {
    Counter(u64),
    Average(StatAverageStorage),
}

impl StatStorage {
    const fn kind(&self) -> StatKind {
        match self {
            Self::Counter(_) => StatKind::Counter,
            Self::Average(_) => StatKind::Average,
        }
    }

    const fn value_after_reset(&self, reset_policy: StatResetPolicy, previous_value: u64) -> u64 {
        match self {
            Self::Counter(_) => reset_policy.value_after_reset(previous_value),
            Self::Average(average) => average.current_value(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StatAverageStorage {
    current_value: u64,
    last_tick: Tick,
    weighted_total: u128,
}

impl StatAverageStorage {
    const fn new(reset_tick: Tick) -> Self {
        Self {
            current_value: 0,
            last_tick: reset_tick,
            weighted_total: 0,
        }
    }

    fn set(
        &mut self,
        stat: StatId,
        reset_tick: Tick,
        tick: Tick,
        value: u64,
    ) -> Result<(), StatsError> {
        if tick < reset_tick {
            return Err(StatsError::AverageUpdateBeforeReset {
                stat,
                tick,
                reset_tick,
            });
        }
        self.accrue_until(stat, tick)?;
        self.current_value = value;
        self.last_tick = tick;
        Ok(())
    }

    fn reset(&mut self, tick: Tick, value: u64) {
        self.current_value = value;
        self.last_tick = tick;
        self.weighted_total = 0;
    }

    const fn current_value(&self) -> u64 {
        self.current_value
    }

    fn value_at(&self, stat: StatId, reset_tick: Tick, tick: Tick) -> Result<u64, StatsError> {
        if tick < self.last_tick {
            return Err(StatsError::AverageReadBeforeLastSample {
                stat,
                tick,
                last_tick: self.last_tick,
            });
        }
        let sample_span = u128::from(tick - self.last_tick) + 1;
        let current_total = u128::from(self.current_value) * sample_span;
        let total = self
            .weighted_total
            .checked_add(current_total)
            .ok_or(StatsError::AverageTotalOverflow { stat })?;
        let divisor = u128::from(tick - reset_tick) + 1;
        Ok((total / divisor) as u64)
    }

    fn accrue_until(&mut self, stat: StatId, tick: Tick) -> Result<(), StatsError> {
        if tick < self.last_tick {
            return Err(StatsError::AverageUpdateBeforeLastSample {
                stat,
                tick,
                last_tick: self.last_tick,
            });
        }
        let elapsed = tick - self.last_tick;
        let weighted = u128::from(self.current_value) * u128::from(elapsed);
        self.weighted_total = self
            .weighted_total
            .checked_add(weighted)
            .ok_or(StatsError::AverageTotalOverflow { stat })?;
        Ok(())
    }
}

fn parse_stat_description(description: impl Into<String>) -> Result<StatDescription, StatsError> {
    let description = description.into();
    StatDescription::new(description.clone()).map_err(|reason| StatsError::InvalidDescription {
        description,
        reason,
    })
}

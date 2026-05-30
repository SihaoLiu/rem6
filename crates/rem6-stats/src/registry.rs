use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::Tick;

use crate::error::StatsError;
use crate::stats::{
    StatDescription, StatDumpId, StatDumpRecord, StatGroupDescriptor, StatGroupId,
    StatHistoryRecord, StatId, StatPath, StatResetId, StatSample, StatScope, StatSnapshot,
    StatUnit, StatsResetRecord,
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
    counters: BTreeMap<StatId, u64>,
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
            counters: BTreeMap::new(),
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
        self.register_counter_with_optional_description(path, unit, None)
    }

    pub fn register_counter_with_unit_and_description(
        &mut self,
        path: impl Into<String>,
        unit: StatUnit,
        description: impl Into<String>,
    ) -> Result<StatId, StatsError> {
        let description = parse_stat_description(description)?;
        self.register_counter_with_optional_description(path, unit, Some(description))
    }

    fn register_counter_with_optional_description(
        &mut self,
        path: impl Into<String>,
        unit: StatUnit,
        description: Option<StatDescription>,
    ) -> Result<StatId, StatsError> {
        let path = path.into();
        if path.is_empty() {
            return Err(StatsError::EmptyPath);
        }
        let stat_path = StatPath::parse(path.clone())
            .map_err(|reason| StatsError::InvalidPath { path, reason })?;
        self.register_counter_path(None, stat_path, unit, description)
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
        self.register_counter_path(None, stat_path, unit, None)
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
        self.register_group_counter_with_optional_description(group, name, unit, None)
    }

    pub fn register_group_counter_with_unit_and_description(
        &mut self,
        group: StatGroupId,
        name: impl Into<String>,
        unit: StatUnit,
        description: impl Into<String>,
    ) -> Result<StatId, StatsError> {
        let description = parse_stat_description(description)?;
        self.register_group_counter_with_optional_description(group, name, unit, Some(description))
    }

    fn register_group_counter_with_optional_description(
        &mut self,
        group: StatGroupId,
        name: impl Into<String>,
        unit: StatUnit,
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
        self.register_counter_path(Some(group), stat_path, unit, description)
    }

    fn register_counter_path(
        &mut self,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
        description: Option<StatDescription>,
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
                path,
                unit,
                description,
            },
        );
        self.counters.insert(id, 0);
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
                StatSample::from_registered_parts_with_description(
                    *id,
                    descriptor.group,
                    descriptor.path.clone(),
                    descriptor.unit.clone(),
                    descriptor.description.clone(),
                    self.counters.get(id).copied().unwrap_or_default(),
                )
            })
            .collect();
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
            .expect("reset tick must be at or after the last reset")
    }

    pub fn try_reset(&mut self, tick: Tick) -> Result<StatsResetRecord, StatsError> {
        if tick < self.reset_tick {
            return Err(StatsError::ResetBeforeLastReset {
                tick,
                reset_tick: self.reset_tick,
            });
        }
        self.ensure_history_tick_not_before_last(tick)?;

        let id = StatResetId::new(self.next_reset_id);
        self.next_reset_id = self
            .next_reset_id
            .checked_add(1)
            .ok_or(StatsError::ResetSequenceOverflow)?;

        self.epoch += 1;
        self.reset_tick = tick;
        let mut previous_values = Vec::new();
        for (id, counter) in &mut self.counters {
            previous_values.push((*id, *counter));
            *counter = 0;
        }
        let record = StatsResetRecord::with_id(id, tick, self.epoch, previous_values);
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
    path: StatPath,
    unit: StatUnit,
    description: Option<StatDescription>,
}

fn parse_stat_description(description: impl Into<String>) -> Result<StatDescription, StatsError> {
    let description = description.into();
    StatDescription::new(description.clone()).map_err(|reason| StatsError::InvalidDescription {
        description,
        reason,
    })
}

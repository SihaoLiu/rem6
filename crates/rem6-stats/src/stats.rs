use std::collections::BTreeMap;

use rem6_kernel::Tick;

use crate::error::StatsError;
use crate::kind::StatKind;
use crate::reset::{StatResetPolicy, StatsResetRecord};
use crate::stat_metadata::{StatDescription, StatPath, StatScope, StatUnit};

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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatHistogramBucket {
    bucket: u64,
    count: u64,
}

impl StatHistogramBucket {
    pub const fn new(bucket: u64, count: u64) -> Self {
        Self { bucket, count }
    }

    pub const fn bucket(&self) -> u64 {
        self.bucket
    }

    pub const fn count(&self) -> u64 {
        self.count
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
    histogram_buckets: Vec<StatHistogramBucket>,
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
            histogram_buckets: Vec::new(),
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

    #[allow(clippy::too_many_arguments)]
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
        assert_non_histogram_sample_kind(kind);
        Self {
            id,
            group,
            kind,
            path,
            unit,
            reset_policy,
            description,
            value,
            histogram_buckets: Vec::new(),
        }
    }

    pub fn from_histogram_parts(
        id: StatId,
        path: StatPath,
        unit: StatUnit,
        buckets: Vec<StatHistogramBucket>,
    ) -> Self {
        Self::from_registered_histogram_parts(id, None, path, unit, buckets)
    }

    pub fn from_registered_histogram_parts(
        id: StatId,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
        buckets: Vec<StatHistogramBucket>,
    ) -> Self {
        Self::from_registered_histogram_parts_with_reset_policy_and_description(
            id,
            group,
            path,
            unit,
            StatResetPolicy::Resettable,
            None,
            buckets,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_registered_histogram_parts_with_reset_policy_and_description(
        id: StatId,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
        reset_policy: StatResetPolicy,
        description: Option<StatDescription>,
        buckets: Vec<StatHistogramBucket>,
    ) -> Self {
        let buckets = normalize_histogram_buckets(buckets);
        let value = histogram_bucket_total(&buckets);
        Self {
            id,
            group,
            kind: StatKind::Histogram,
            path,
            unit,
            reset_policy,
            description,
            value,
            histogram_buckets: buckets,
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

    pub fn histogram_buckets(&self) -> &[StatHistogramBucket] {
        &self.histogram_buckets
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
    histogram_delta_buckets: Vec<StatHistogramBucket>,
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
            histogram_delta_buckets: Vec::new(),
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

    #[allow(clippy::too_many_arguments)]
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
        assert_non_histogram_delta_kind(kind);
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
            histogram_delta_buckets: Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_registered_histogram_parts_with_reset_policy_and_description(
        id: StatId,
        group: Option<StatGroupId>,
        path: StatPath,
        unit: StatUnit,
        reset_policy: StatResetPolicy,
        description: Option<StatDescription>,
        previous_value: u64,
        current_value: u64,
        delta_buckets: Vec<StatHistogramBucket>,
    ) -> Self {
        let delta_buckets = normalize_histogram_buckets(delta_buckets);
        Self {
            id,
            group,
            kind: StatKind::Histogram,
            path,
            unit,
            reset_policy,
            description,
            previous_value,
            current_value,
            histogram_delta_buckets: delta_buckets,
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

    pub fn histogram_delta_buckets(&self) -> &[StatHistogramBucket] {
        &self.histogram_delta_buckets
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
            let delta = match previous_sample.kind() {
                StatKind::Counter => {
                    if current_sample.value() < previous_sample.value() {
                        return Err(StatsError::SnapshotDeltaValueWentBack {
                            stat: previous_sample.id(),
                            previous: previous_sample.value(),
                            current: current_sample.value(),
                        });
                    }
                    StatDeltaSample {
                        id: previous_sample.id(),
                        group: previous_sample.group(),
                        kind: previous_sample.kind(),
                        path: previous_sample.stat_path().clone(),
                        unit: previous_sample.stat_unit().clone(),
                        reset_policy: previous_sample.reset_policy(),
                        description: previous_sample.stat_description().cloned(),
                        previous_value: previous_sample.value(),
                        current_value: current_sample.value(),
                        histogram_delta_buckets: Vec::new(),
                    }
                }
                StatKind::Average => {
                    return Err(StatsError::SnapshotDeltaUnsupportedStatKind {
                        stat: previous_sample.id(),
                        kind: previous_sample.kind(),
                    });
                }
                StatKind::Histogram => {
                    let delta_buckets = histogram_bucket_delta(
                        previous_sample.id(),
                        previous_sample,
                        current_sample,
                    )?;
                    if current_sample.value() < previous_sample.value() {
                        return Err(StatsError::SnapshotDeltaValueWentBack {
                            stat: previous_sample.id(),
                            previous: previous_sample.value(),
                            current: current_sample.value(),
                        });
                    }
                    StatDeltaSample::from_registered_histogram_parts_with_reset_policy_and_description(
                        previous_sample.id(),
                        previous_sample.group(),
                        previous_sample.stat_path().clone(),
                        previous_sample.stat_unit().clone(),
                        previous_sample.reset_policy(),
                        previous_sample.stat_description().cloned(),
                        previous_sample.value(),
                        current_sample.value(),
                        delta_buckets,
                    )
                }
            };
            deltas.push(delta);
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

fn histogram_bucket_total(buckets: &[StatHistogramBucket]) -> u64 {
    buckets
        .iter()
        .map(StatHistogramBucket::count)
        .try_fold(0_u64, u64::checked_add)
        .expect("stat histogram bucket counts must not overflow")
}

fn normalize_histogram_buckets(buckets: Vec<StatHistogramBucket>) -> Vec<StatHistogramBucket> {
    let mut normalized = BTreeMap::new();
    for bucket in buckets {
        let count = normalized.entry(bucket.bucket()).or_insert(0_u64);
        *count = count
            .checked_add(bucket.count())
            .expect("stat histogram bucket counts must not overflow");
    }
    normalized
        .into_iter()
        .map(|(bucket, count)| StatHistogramBucket::new(bucket, count))
        .collect()
}

const fn assert_non_histogram_sample_kind(kind: StatKind) {
    match kind {
        StatKind::Histogram => panic!("histogram samples must use histogram constructors"),
        StatKind::Counter | StatKind::Average => {}
    }
}

const fn assert_non_histogram_delta_kind(kind: StatKind) {
    match kind {
        StatKind::Histogram => panic!("histogram deltas must use histogram constructors"),
        StatKind::Counter | StatKind::Average => {}
    }
}

fn histogram_bucket_delta(
    stat: StatId,
    previous: &StatSample,
    current: &StatSample,
) -> Result<Vec<StatHistogramBucket>, StatsError> {
    let current_buckets = current
        .histogram_buckets()
        .iter()
        .map(|bucket| (bucket.bucket(), bucket.count()))
        .collect::<BTreeMap<_, _>>();
    let previous_buckets = previous
        .histogram_buckets()
        .iter()
        .map(|bucket| (bucket.bucket(), bucket.count()))
        .collect::<BTreeMap<_, _>>();
    let mut all_buckets = previous_buckets.keys().copied().collect::<Vec<_>>();
    for bucket in current_buckets.keys() {
        if !previous_buckets.contains_key(bucket) {
            all_buckets.push(*bucket);
        }
    }
    all_buckets.sort_unstable();

    let mut delta_buckets = Vec::new();
    for bucket in all_buckets {
        let previous_count = previous_buckets.get(&bucket).copied().unwrap_or(0);
        let current_count = current_buckets.get(&bucket).copied().unwrap_or(0);
        if current_count < previous_count {
            return Err(StatsError::SnapshotDeltaHistogramBucketWentBack {
                stat,
                bucket,
                previous: previous_count,
                current: current_count,
            });
        }
        let delta = current_count - previous_count;
        if delta != 0 {
            delta_buckets.push(StatHistogramBucket::new(bucket, delta));
        }
    }

    Ok(delta_buckets)
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

use std::fmt;

use rem6_kernel::Tick;

use crate::stats::{StatId, StatResetId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StatResetPolicy {
    Resettable,
    Constant,
    Monotonic,
}

impl StatResetPolicy {
    pub const fn value_after_reset(self, previous_value: u64) -> u64 {
        match self {
            Self::Resettable => 0,
            Self::Constant | Self::Monotonic => previous_value,
        }
    }
}

impl fmt::Display for StatResetPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resettable => formatter.write_str("resettable"),
            Self::Constant => formatter.write_str("constant"),
            Self::Monotonic => formatter.write_str("monotonic"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatResetSample {
    id: StatId,
    reset_policy: StatResetPolicy,
    previous_value: u64,
    reset_value: u64,
}

impl StatResetSample {
    pub const fn new(
        id: StatId,
        reset_policy: StatResetPolicy,
        previous_value: u64,
        reset_value: u64,
    ) -> Self {
        Self {
            id,
            reset_policy,
            previous_value,
            reset_value,
        }
    }

    pub const fn id(&self) -> StatId {
        self.id
    }

    pub const fn reset_policy(&self) -> StatResetPolicy {
        self.reset_policy
    }

    pub const fn previous_value(&self) -> u64 {
        self.previous_value
    }

    pub const fn reset_value(&self) -> u64 {
        self.reset_value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatsResetRecord {
    id: StatResetId,
    tick: Tick,
    epoch: u64,
    previous_values: Vec<(StatId, u64)>,
    reset_samples: Vec<StatResetSample>,
}

impl StatsResetRecord {
    pub fn new(tick: Tick, epoch: u64, previous_values: Vec<(StatId, u64)>) -> Self {
        Self::with_id(StatResetId::new(0), tick, epoch, previous_values)
    }

    pub fn with_id(
        id: StatResetId,
        tick: Tick,
        epoch: u64,
        previous_values: Vec<(StatId, u64)>,
    ) -> Self {
        let reset_samples = previous_values
            .iter()
            .map(|(stat, previous_value)| {
                StatResetSample::new(*stat, StatResetPolicy::Resettable, *previous_value, 0)
            })
            .collect();
        Self {
            id,
            tick,
            epoch,
            previous_values,
            reset_samples,
        }
    }

    pub fn with_reset_samples(
        id: StatResetId,
        tick: Tick,
        epoch: u64,
        reset_samples: Vec<StatResetSample>,
    ) -> Self {
        let previous_values = reset_samples
            .iter()
            .map(|sample| (sample.id(), sample.previous_value()))
            .collect();
        Self {
            id,
            tick,
            epoch,
            previous_values,
            reset_samples,
        }
    }

    pub const fn id(&self) -> StatResetId {
        self.id
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

    pub fn reset_samples(&self) -> &[StatResetSample] {
        &self.reset_samples
    }
}

use std::fmt;

use rem6_kernel::Tick;
use rem6_stats::{StatDumpId, StatHistoryRecord, StatResetId};

use crate::{
    WorkloadError, WorkloadManifest, WorkloadManifestBuilder, WorkloadReplayPlan, WorkloadResult,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkloadStatsHistorySummary {
    reset_count: usize,
    dump_count: usize,
    first_tick: Option<Tick>,
    last_tick: Option<Tick>,
}

impl WorkloadStatsHistorySummary {
    pub fn from_records(records: &[StatHistoryRecord]) -> Self {
        let mut summary = Self::default();
        for record in records {
            match record {
                StatHistoryRecord::Reset(_) => summary.reset_count += 1,
                StatHistoryRecord::Dump(_) => summary.dump_count += 1,
            }
            if summary.first_tick.is_none() {
                summary.first_tick = Some(record.tick());
            }
            summary.last_tick = Some(record.tick());
        }
        summary
    }

    pub const fn reset_count(&self) -> usize {
        self.reset_count
    }

    pub const fn dump_count(&self) -> usize {
        self.dump_count
    }

    pub const fn first_tick(&self) -> Option<Tick> {
        self.first_tick
    }

    pub const fn last_tick(&self) -> Option<Tick> {
        self.last_tick
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkloadStatsHistoryRecordExpectation {
    Reset {
        id: StatResetId,
        tick: Tick,
        epoch: u64,
    },
    Dump {
        id: StatDumpId,
        tick: Tick,
        epoch: u64,
        reset_tick: Tick,
    },
}

impl WorkloadStatsHistoryRecordExpectation {
    pub const fn reset(id: StatResetId, tick: Tick, epoch: u64) -> Self {
        Self::Reset { id, tick, epoch }
    }

    pub const fn dump(id: StatDumpId, tick: Tick, epoch: u64, reset_tick: Tick) -> Self {
        Self::Dump {
            id,
            tick,
            epoch,
            reset_tick,
        }
    }

    pub(crate) fn from_history_record(record: &StatHistoryRecord) -> Self {
        match record {
            StatHistoryRecord::Reset(record) => {
                Self::reset(record.id(), record.tick(), record.epoch())
            }
            StatHistoryRecord::Dump(record) => Self::dump(
                record.id(),
                record.tick(),
                record.epoch(),
                record.reset_tick(),
            ),
        }
    }

    pub const fn kind_code(&self) -> u64 {
        match self {
            Self::Reset { .. } => 0,
            Self::Dump { .. } => 1,
        }
    }

    pub const fn id_value(&self) -> u64 {
        match self {
            Self::Reset { id, .. } => id.get(),
            Self::Dump { id, .. } => id.get(),
        }
    }

    pub const fn tick(&self) -> Tick {
        match self {
            Self::Reset { tick, .. } | Self::Dump { tick, .. } => *tick,
        }
    }

    pub const fn epoch(&self) -> u64 {
        match self {
            Self::Reset { epoch, .. } | Self::Dump { epoch, .. } => *epoch,
        }
    }

    pub const fn reset_tick(&self) -> Option<Tick> {
        match self {
            Self::Reset { .. } => None,
            Self::Dump { reset_tick, .. } => Some(*reset_tick),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadExpectedStatsHistory {
    minimum_reset_count: usize,
    minimum_dump_count: usize,
    first_tick: Option<Tick>,
    last_tick: Option<Tick>,
    exact_records: Vec<WorkloadStatsHistoryRecordExpectation>,
}

impl WorkloadExpectedStatsHistory {
    pub fn new(
        minimum_reset_count: usize,
        minimum_dump_count: usize,
    ) -> Result<Self, WorkloadError> {
        if minimum_reset_count == 0 && minimum_dump_count == 0 {
            return Err(stats_history_error(
                WorkloadStatsHistoryExpectationError::EmptyExpectation,
            ));
        }
        Ok(Self {
            minimum_reset_count,
            minimum_dump_count,
            first_tick: None,
            last_tick: None,
            exact_records: Vec::new(),
        })
    }

    pub fn with_tick_window(
        mut self,
        first_tick: Tick,
        last_tick: Tick,
    ) -> Result<Self, WorkloadError> {
        if first_tick > last_tick {
            return Err(stats_history_error(
                WorkloadStatsHistoryExpectationError::InvalidTickWindow {
                    first_tick,
                    last_tick,
                },
            ));
        }
        self.first_tick = Some(first_tick);
        self.last_tick = Some(last_tick);
        Ok(self)
    }

    pub fn with_exact_records(
        mut self,
        records: impl IntoIterator<Item = WorkloadStatsHistoryRecordExpectation>,
    ) -> Result<Self, WorkloadError> {
        let records = records.into_iter().collect::<Vec<_>>();
        if records.is_empty() {
            return Err(stats_history_error(
                WorkloadStatsHistoryExpectationError::EmptyExactRecordSequence,
            ));
        }
        self.exact_records = records;
        Ok(self)
    }

    pub const fn minimum_reset_count(&self) -> usize {
        self.minimum_reset_count
    }

    pub const fn minimum_dump_count(&self) -> usize {
        self.minimum_dump_count
    }

    pub const fn first_tick(&self) -> Option<Tick> {
        self.first_tick
    }

    pub const fn last_tick(&self) -> Option<Tick> {
        self.last_tick
    }

    pub fn exact_records(&self) -> &[WorkloadStatsHistoryRecordExpectation] {
        &self.exact_records
    }

    pub(crate) fn verify(&self, result: &WorkloadResult) -> Result<(), WorkloadError> {
        self.verify_exact_records(result.stats_history_records())?;
        let actual = result.stats_history_summary();
        if actual.reset_count() < self.minimum_reset_count
            || actual.dump_count() < self.minimum_dump_count
        {
            return Err(stats_history_error(
                WorkloadStatsHistoryExpectationError::BelowMinimum {
                    minimum_reset_count: self.minimum_reset_count,
                    actual_reset_count: actual.reset_count(),
                    minimum_dump_count: self.minimum_dump_count,
                    actual_dump_count: actual.dump_count(),
                },
            ));
        }
        if let (Some(expected_first_tick), Some(expected_last_tick)) =
            (self.first_tick, self.last_tick)
        {
            if actual.first_tick() != Some(expected_first_tick)
                || actual.last_tick() != Some(expected_last_tick)
            {
                return Err(stats_history_error(
                    WorkloadStatsHistoryExpectationError::TickWindowMismatch {
                        expected_first_tick,
                        actual_first_tick: actual.first_tick(),
                        expected_last_tick,
                        actual_last_tick: actual.last_tick(),
                    },
                ));
            }
        }
        Ok(())
    }

    fn verify_exact_records(&self, records: &[StatHistoryRecord]) -> Result<(), WorkloadError> {
        if self.exact_records.is_empty() {
            return Ok(());
        }
        for (index, expected) in self.exact_records.iter().copied().enumerate() {
            let actual = records
                .get(index)
                .map(WorkloadStatsHistoryRecordExpectation::from_history_record);
            if actual != Some(expected) {
                return Err(stats_history_error(
                    WorkloadStatsHistoryExpectationError::ExactRecordMismatch {
                        index,
                        expected,
                        actual,
                    },
                ));
            }
        }
        if let Some(actual) = records.get(self.exact_records.len()) {
            return Err(stats_history_error(
                WorkloadStatsHistoryExpectationError::UnexpectedExactRecord {
                    index: self.exact_records.len(),
                    actual: WorkloadStatsHistoryRecordExpectation::from_history_record(actual),
                },
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkloadStatsHistoryExpectationError {
    EmptyExpectation,
    EmptyExactRecordSequence,
    InvalidTickWindow {
        first_tick: Tick,
        last_tick: Tick,
    },
    DuplicateExpectation,
    BelowMinimum {
        minimum_reset_count: usize,
        actual_reset_count: usize,
        minimum_dump_count: usize,
        actual_dump_count: usize,
    },
    TickWindowMismatch {
        expected_first_tick: Tick,
        actual_first_tick: Option<Tick>,
        expected_last_tick: Tick,
        actual_last_tick: Option<Tick>,
    },
    ExactRecordMismatch {
        index: usize,
        expected: WorkloadStatsHistoryRecordExpectation,
        actual: Option<WorkloadStatsHistoryRecordExpectation>,
    },
    UnexpectedExactRecord {
        index: usize,
        actual: WorkloadStatsHistoryRecordExpectation,
    },
}

impl fmt::Display for WorkloadStatsHistoryExpectationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyExpectation => {
                write!(formatter, "stats history expectation must require at least one record")
            }
            Self::EmptyExactRecordSequence => {
                write!(formatter, "exact stats history sequence must not be empty")
            }
            Self::InvalidTickWindow {
                first_tick,
                last_tick,
            } => write!(
                formatter,
                "expected stats history tick window starts at {first_tick} after ending at {last_tick}"
            ),
            Self::DuplicateExpectation => {
                write!(formatter, "stats history expectation is already defined")
            }
            Self::BelowMinimum {
                minimum_reset_count,
                actual_reset_count,
                minimum_dump_count,
                actual_dump_count,
            } => write!(
                formatter,
                "stats history has {actual_reset_count} resets and {actual_dump_count} dumps, below expected {minimum_reset_count} resets and {minimum_dump_count} dumps"
            ),
            Self::TickWindowMismatch {
                expected_first_tick,
                actual_first_tick,
                expected_last_tick,
                actual_last_tick,
            } => write!(
                formatter,
                "stats history window {actual_first_tick:?}..{actual_last_tick:?} does not match expected {expected_first_tick}..{expected_last_tick}"
            ),
            Self::ExactRecordMismatch {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "stats history record {index} is {actual:?}, expected {expected:?}"
            ),
            Self::UnexpectedExactRecord { index, actual } => write!(
                formatter,
                "stats history has unexpected record {index}: {actual:?}"
            ),
        }
    }
}

pub(crate) fn stats_history_error(error: WorkloadStatsHistoryExpectationError) -> WorkloadError {
    WorkloadError::StatsHistoryExpectation(error)
}

impl WorkloadManifest {
    pub const fn expected_stats_history(&self) -> Option<&WorkloadExpectedStatsHistory> {
        self.expected_stats_history.as_ref()
    }
}

impl WorkloadManifestBuilder {
    pub fn add_expected_stats_history(
        mut self,
        expected: WorkloadExpectedStatsHistory,
    ) -> Result<Self, WorkloadError> {
        if self.expected_stats_history.is_some() {
            return Err(stats_history_error(
                WorkloadStatsHistoryExpectationError::DuplicateExpectation,
            ));
        }
        self.expected_stats_history = Some(expected);
        Ok(self)
    }
}

impl WorkloadReplayPlan {
    pub const fn expected_stats_history(&self) -> Option<&WorkloadExpectedStatsHistory> {
        self.expected_stats_history.as_ref()
    }

    pub(crate) fn verify_expected_stats_history(
        &self,
        result: &WorkloadResult,
    ) -> Result<(), WorkloadError> {
        match &self.expected_stats_history {
            Some(expected) => expected.verify(result),
            None => Ok(()),
        }
    }
}

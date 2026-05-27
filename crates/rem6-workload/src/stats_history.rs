use std::fmt;

use rem6_kernel::Tick;
use rem6_stats::StatHistoryRecord;

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
pub struct WorkloadExpectedStatsHistory {
    minimum_reset_count: usize,
    minimum_dump_count: usize,
    first_tick: Option<Tick>,
    last_tick: Option<Tick>,
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

    pub(crate) fn verify(&self, result: &WorkloadResult) -> Result<(), WorkloadError> {
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkloadStatsHistoryExpectationError {
    EmptyExpectation,
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
}

impl fmt::Display for WorkloadStatsHistoryExpectationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyExpectation => {
                write!(formatter, "stats history expectation must require at least one record")
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
        match self.expected_stats_history {
            Some(expected) => expected.verify(result),
            None => Ok(()),
        }
    }
}

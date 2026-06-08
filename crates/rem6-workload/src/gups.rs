use rem6_kernel::Tick;

use crate::{WorkloadError, WorkloadReplayPlan, WorkloadResult, WorkloadRouteId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadGupsRunSummary {
    route: WorkloadRouteId,
    final_tick: Tick,
    scheduled_count: usize,
    response_count: usize,
    completed_response_count: usize,
    retry_response_count: usize,
    store_conditional_failed_response_count: usize,
    read_response_count: usize,
    write_response_count: usize,
    response_data_byte_count: u64,
    memory_trace_event_count: usize,
}

impl WorkloadGupsRunSummary {
    pub fn new(route: WorkloadRouteId, final_tick: Tick) -> Self {
        Self {
            route,
            final_tick,
            scheduled_count: 0,
            response_count: 0,
            completed_response_count: 0,
            retry_response_count: 0,
            store_conditional_failed_response_count: 0,
            read_response_count: 0,
            write_response_count: 0,
            response_data_byte_count: 0,
            memory_trace_event_count: 0,
        }
    }

    pub fn with_scheduled_count(mut self, scheduled_count: usize) -> Self {
        self.scheduled_count = scheduled_count;
        self
    }

    pub fn with_response_count(mut self, response_count: usize) -> Self {
        self.response_count = response_count;
        self
    }

    pub fn with_completed_response_count(mut self, completed_response_count: usize) -> Self {
        self.completed_response_count = completed_response_count;
        self
    }

    pub fn with_retry_response_count(mut self, retry_response_count: usize) -> Self {
        self.retry_response_count = retry_response_count;
        self
    }

    pub fn with_store_conditional_failed_response_count(
        mut self,
        store_conditional_failed_response_count: usize,
    ) -> Self {
        self.store_conditional_failed_response_count = store_conditional_failed_response_count;
        self
    }

    pub fn with_read_response_count(mut self, read_response_count: usize) -> Self {
        self.read_response_count = read_response_count;
        self
    }

    pub fn with_write_response_count(mut self, write_response_count: usize) -> Self {
        self.write_response_count = write_response_count;
        self
    }

    pub fn with_response_data_byte_count(mut self, response_data_byte_count: u64) -> Self {
        self.response_data_byte_count = response_data_byte_count;
        self
    }

    pub fn with_memory_trace_event_count(mut self, memory_trace_event_count: usize) -> Self {
        self.memory_trace_event_count = memory_trace_event_count;
        self
    }

    pub const fn route(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub const fn final_tick(&self) -> Tick {
        self.final_tick
    }

    pub const fn scheduled_count(&self) -> usize {
        self.scheduled_count
    }

    pub const fn response_count(&self) -> usize {
        self.response_count
    }

    pub const fn completed_response_count(&self) -> usize {
        self.completed_response_count
    }

    pub const fn retry_response_count(&self) -> usize {
        self.retry_response_count
    }

    pub const fn store_conditional_failed_response_count(&self) -> usize {
        self.store_conditional_failed_response_count
    }

    pub const fn read_response_count(&self) -> usize {
        self.read_response_count
    }

    pub const fn write_response_count(&self) -> usize {
        self.write_response_count
    }

    pub const fn response_data_byte_count(&self) -> u64 {
        self.response_data_byte_count
    }

    pub const fn memory_trace_event_count(&self) -> usize {
        self.memory_trace_event_count
    }

    pub(crate) fn sort_key(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub(crate) fn merged(&self, other: &Self) -> Self {
        debug_assert_eq!(self.route(), other.route());
        Self {
            route: self.route.clone(),
            final_tick: self.final_tick.max(other.final_tick),
            scheduled_count: self.scheduled_count + other.scheduled_count,
            response_count: self.response_count + other.response_count,
            completed_response_count: self.completed_response_count
                + other.completed_response_count,
            retry_response_count: self.retry_response_count + other.retry_response_count,
            store_conditional_failed_response_count: self.store_conditional_failed_response_count
                + other.store_conditional_failed_response_count,
            read_response_count: self.read_response_count + other.read_response_count,
            write_response_count: self.write_response_count + other.write_response_count,
            response_data_byte_count: self.response_data_byte_count
                + other.response_data_byte_count,
            memory_trace_event_count: self.memory_trace_event_count
                + other.memory_trace_event_count,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadExpectedGupsRunSummary {
    route: WorkloadRouteId,
    maximum_final_tick: Option<Tick>,
    minimum_scheduled_count: usize,
    minimum_response_count: usize,
    minimum_completed_response_count: usize,
    minimum_retry_response_count: usize,
    minimum_store_conditional_failed_response_count: usize,
    minimum_read_response_count: usize,
    minimum_write_response_count: usize,
    minimum_response_data_byte_count: u64,
    minimum_memory_trace_event_count: usize,
}

impl WorkloadExpectedGupsRunSummary {
    pub fn new(route: WorkloadRouteId) -> Self {
        Self {
            route,
            maximum_final_tick: None,
            minimum_scheduled_count: 0,
            minimum_response_count: 0,
            minimum_completed_response_count: 0,
            minimum_retry_response_count: 0,
            minimum_store_conditional_failed_response_count: 0,
            minimum_read_response_count: 0,
            minimum_write_response_count: 0,
            minimum_response_data_byte_count: 0,
            minimum_memory_trace_event_count: 0,
        }
    }

    pub fn with_maximum_final_tick(mut self, maximum_final_tick: Tick) -> Self {
        self.maximum_final_tick = Some(maximum_final_tick);
        self
    }

    pub fn with_minimum_scheduled_count(mut self, minimum_scheduled_count: usize) -> Self {
        self.minimum_scheduled_count = minimum_scheduled_count;
        self
    }

    pub fn with_minimum_response_count(mut self, minimum_response_count: usize) -> Self {
        self.minimum_response_count = minimum_response_count;
        self
    }

    pub fn with_minimum_completed_response_count(
        mut self,
        minimum_completed_response_count: usize,
    ) -> Self {
        self.minimum_completed_response_count = minimum_completed_response_count;
        self
    }

    pub fn with_minimum_retry_response_count(
        mut self,
        minimum_retry_response_count: usize,
    ) -> Self {
        self.minimum_retry_response_count = minimum_retry_response_count;
        self
    }

    pub fn with_minimum_store_conditional_failed_response_count(
        mut self,
        minimum_store_conditional_failed_response_count: usize,
    ) -> Self {
        self.minimum_store_conditional_failed_response_count =
            minimum_store_conditional_failed_response_count;
        self
    }

    pub fn with_minimum_read_response_count(mut self, minimum_read_response_count: usize) -> Self {
        self.minimum_read_response_count = minimum_read_response_count;
        self
    }

    pub fn with_minimum_write_response_count(
        mut self,
        minimum_write_response_count: usize,
    ) -> Self {
        self.minimum_write_response_count = minimum_write_response_count;
        self
    }

    pub fn with_minimum_response_data_byte_count(
        mut self,
        minimum_response_data_byte_count: u64,
    ) -> Self {
        self.minimum_response_data_byte_count = minimum_response_data_byte_count;
        self
    }

    pub fn with_minimum_memory_trace_event_count(
        mut self,
        minimum_memory_trace_event_count: usize,
    ) -> Self {
        self.minimum_memory_trace_event_count = minimum_memory_trace_event_count;
        self
    }

    pub const fn route(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub const fn maximum_final_tick(&self) -> Option<Tick> {
        self.maximum_final_tick
    }

    pub const fn minimum_scheduled_count(&self) -> usize {
        self.minimum_scheduled_count
    }

    pub const fn minimum_response_count(&self) -> usize {
        self.minimum_response_count
    }

    pub const fn minimum_completed_response_count(&self) -> usize {
        self.minimum_completed_response_count
    }

    pub const fn minimum_retry_response_count(&self) -> usize {
        self.minimum_retry_response_count
    }

    pub const fn minimum_store_conditional_failed_response_count(&self) -> usize {
        self.minimum_store_conditional_failed_response_count
    }

    pub const fn minimum_read_response_count(&self) -> usize {
        self.minimum_read_response_count
    }

    pub const fn minimum_write_response_count(&self) -> usize {
        self.minimum_write_response_count
    }

    pub const fn minimum_response_data_byte_count(&self) -> u64 {
        self.minimum_response_data_byte_count
    }

    pub const fn minimum_memory_trace_event_count(&self) -> usize {
        self.minimum_memory_trace_event_count
    }

    pub(crate) fn sort_key(&self) -> &WorkloadRouteId {
        &self.route
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkloadGupsRunSummaryExpectationError {
    DuplicateExpected {
        route: WorkloadRouteId,
    },
    Missing(WorkloadExpectedGupsRunSummary),
    OutsideBounds {
        expected: WorkloadExpectedGupsRunSummary,
        actual: WorkloadGupsRunSummary,
    },
    AfterResultFinalTick {
        actual: WorkloadGupsRunSummary,
        result_final_tick: Tick,
    },
}

pub(crate) fn verify_expected_gups_run_summaries(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    for expected in plan.expected_gups_run_summaries() {
        let actual = result
            .gups_run_summary(expected.route())
            .cloned()
            .ok_or_else(|| {
                WorkloadError::GupsRunSummaryExpectation(Box::new(
                    WorkloadGupsRunSummaryExpectationError::Missing(expected.clone()),
                ))
            })?;
        if actual.final_tick() > result.final_tick() {
            return Err(WorkloadError::GupsRunSummaryExpectation(Box::new(
                WorkloadGupsRunSummaryExpectationError::AfterResultFinalTick {
                    actual,
                    result_final_tick: result.final_tick(),
                },
            )));
        }
        if !gups_run_summary_within_bounds(expected, &actual) {
            return Err(WorkloadError::GupsRunSummaryExpectation(Box::new(
                WorkloadGupsRunSummaryExpectationError::OutsideBounds {
                    expected: expected.clone(),
                    actual,
                },
            )));
        }
    }

    Ok(())
}

fn gups_run_summary_within_bounds(
    expected: &WorkloadExpectedGupsRunSummary,
    actual: &WorkloadGupsRunSummary,
) -> bool {
    expected
        .maximum_final_tick()
        .is_none_or(|maximum| actual.final_tick() <= maximum)
        && actual.scheduled_count() >= expected.minimum_scheduled_count()
        && actual.response_count() >= expected.minimum_response_count()
        && actual.completed_response_count() >= expected.minimum_completed_response_count()
        && actual.retry_response_count() >= expected.minimum_retry_response_count()
        && actual.store_conditional_failed_response_count()
            >= expected.minimum_store_conditional_failed_response_count()
        && actual.read_response_count() >= expected.minimum_read_response_count()
        && actual.write_response_count() >= expected.minimum_write_response_count()
        && actual.response_data_byte_count() >= expected.minimum_response_data_byte_count()
        && actual.memory_trace_event_count() >= expected.minimum_memory_trace_event_count()
}

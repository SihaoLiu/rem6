use crate::{WorkloadError, WorkloadReplayPlan, WorkloadResult, WorkloadRouteId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadTrafficTraceReplaySummary {
    route: WorkloadRouteId,
    scheduled_count: usize,
    response_delivery_count: usize,
    memory_trace_event_count: usize,
    memory_failure_count: usize,
    control_ack_count: usize,
    control_failure_count: usize,
    sideband_event_count: usize,
}

impl WorkloadTrafficTraceReplaySummary {
    pub fn new(route: WorkloadRouteId, scheduled_count: usize) -> Self {
        Self {
            route,
            scheduled_count,
            response_delivery_count: 0,
            memory_trace_event_count: 0,
            memory_failure_count: 0,
            control_ack_count: 0,
            control_failure_count: 0,
            sideband_event_count: 0,
        }
    }

    pub fn with_response_delivery_count(mut self, response_delivery_count: usize) -> Self {
        self.response_delivery_count = response_delivery_count;
        self
    }

    pub fn with_memory_trace_event_count(mut self, memory_trace_event_count: usize) -> Self {
        self.memory_trace_event_count = memory_trace_event_count;
        self
    }

    pub fn with_memory_failure_count(mut self, memory_failure_count: usize) -> Self {
        self.memory_failure_count = memory_failure_count;
        self
    }

    pub fn with_control_ack_count(mut self, control_ack_count: usize) -> Self {
        self.control_ack_count = control_ack_count;
        self
    }

    pub fn with_control_failure_count(mut self, control_failure_count: usize) -> Self {
        self.control_failure_count = control_failure_count;
        self
    }

    pub fn with_sideband_event_count(mut self, sideband_event_count: usize) -> Self {
        self.sideband_event_count = sideband_event_count;
        self
    }

    pub const fn route(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub const fn scheduled_count(&self) -> usize {
        self.scheduled_count
    }

    pub const fn response_delivery_count(&self) -> usize {
        self.response_delivery_count
    }

    pub const fn memory_trace_event_count(&self) -> usize {
        self.memory_trace_event_count
    }

    pub const fn memory_failure_count(&self) -> usize {
        self.memory_failure_count
    }

    pub const fn control_ack_count(&self) -> usize {
        self.control_ack_count
    }

    pub const fn control_failure_count(&self) -> usize {
        self.control_failure_count
    }

    pub const fn sideband_event_count(&self) -> usize {
        self.sideband_event_count
    }

    pub(crate) fn sort_key(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub(crate) fn merged(&self, other: &Self) -> Self {
        debug_assert_eq!(self.route(), other.route());
        Self {
            route: self.route.clone(),
            scheduled_count: self.scheduled_count + other.scheduled_count,
            response_delivery_count: self.response_delivery_count + other.response_delivery_count,
            memory_trace_event_count: self.memory_trace_event_count
                + other.memory_trace_event_count,
            memory_failure_count: self.memory_failure_count + other.memory_failure_count,
            control_ack_count: self.control_ack_count + other.control_ack_count,
            control_failure_count: self.control_failure_count + other.control_failure_count,
            sideband_event_count: self.sideband_event_count + other.sideband_event_count,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadExpectedTrafficTraceReplaySummary {
    route: WorkloadRouteId,
    minimum_scheduled_count: usize,
    minimum_response_delivery_count: usize,
    minimum_memory_trace_event_count: usize,
    minimum_memory_failure_count: usize,
    minimum_control_ack_count: usize,
    minimum_control_failure_count: usize,
    minimum_sideband_event_count: usize,
}

impl WorkloadExpectedTrafficTraceReplaySummary {
    pub fn new(route: WorkloadRouteId) -> Self {
        Self {
            route,
            minimum_scheduled_count: 0,
            minimum_response_delivery_count: 0,
            minimum_memory_trace_event_count: 0,
            minimum_memory_failure_count: 0,
            minimum_control_ack_count: 0,
            minimum_control_failure_count: 0,
            minimum_sideband_event_count: 0,
        }
    }

    pub fn with_minimum_scheduled_count(mut self, minimum_scheduled_count: usize) -> Self {
        self.minimum_scheduled_count = minimum_scheduled_count;
        self
    }

    pub fn with_minimum_response_delivery_count(
        mut self,
        minimum_response_delivery_count: usize,
    ) -> Self {
        self.minimum_response_delivery_count = minimum_response_delivery_count;
        self
    }

    pub fn with_minimum_memory_trace_event_count(
        mut self,
        minimum_memory_trace_event_count: usize,
    ) -> Self {
        self.minimum_memory_trace_event_count = minimum_memory_trace_event_count;
        self
    }

    pub fn with_minimum_memory_failure_count(
        mut self,
        minimum_memory_failure_count: usize,
    ) -> Self {
        self.minimum_memory_failure_count = minimum_memory_failure_count;
        self
    }

    pub fn with_minimum_control_ack_count(mut self, minimum_control_ack_count: usize) -> Self {
        self.minimum_control_ack_count = minimum_control_ack_count;
        self
    }

    pub fn with_minimum_control_failure_count(
        mut self,
        minimum_control_failure_count: usize,
    ) -> Self {
        self.minimum_control_failure_count = minimum_control_failure_count;
        self
    }

    pub fn with_minimum_sideband_event_count(
        mut self,
        minimum_sideband_event_count: usize,
    ) -> Self {
        self.minimum_sideband_event_count = minimum_sideband_event_count;
        self
    }

    pub const fn route(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub const fn minimum_scheduled_count(&self) -> usize {
        self.minimum_scheduled_count
    }

    pub const fn minimum_response_delivery_count(&self) -> usize {
        self.minimum_response_delivery_count
    }

    pub const fn minimum_memory_trace_event_count(&self) -> usize {
        self.minimum_memory_trace_event_count
    }

    pub const fn minimum_memory_failure_count(&self) -> usize {
        self.minimum_memory_failure_count
    }

    pub const fn minimum_control_ack_count(&self) -> usize {
        self.minimum_control_ack_count
    }

    pub const fn minimum_control_failure_count(&self) -> usize {
        self.minimum_control_failure_count
    }

    pub const fn minimum_sideband_event_count(&self) -> usize {
        self.minimum_sideband_event_count
    }

    pub(crate) fn sort_key(&self) -> &WorkloadRouteId {
        &self.route
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkloadTrafficTraceReplaySummaryExpectationError {
    Missing(WorkloadExpectedTrafficTraceReplaySummary),
    BelowMinimum {
        expected: WorkloadExpectedTrafficTraceReplaySummary,
        actual: WorkloadTrafficTraceReplaySummary,
    },
}

pub(crate) fn verify_expected_traffic_trace_replay_summaries(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    for expected in plan.expected_traffic_trace_replay_summaries() {
        let actual = result
            .traffic_trace_replay_summary(expected.route())
            .cloned()
            .ok_or_else(|| {
                WorkloadError::TrafficTraceReplaySummaryExpectation(Box::new(
                    WorkloadTrafficTraceReplaySummaryExpectationError::Missing(expected.clone()),
                ))
            })?;
        if !traffic_trace_replay_summary_meets_minimum(expected, &actual) {
            return Err(WorkloadError::TrafficTraceReplaySummaryExpectation(
                Box::new(
                    WorkloadTrafficTraceReplaySummaryExpectationError::BelowMinimum {
                        expected: expected.clone(),
                        actual,
                    },
                ),
            ));
        }
    }

    Ok(())
}

fn traffic_trace_replay_summary_meets_minimum(
    expected: &WorkloadExpectedTrafficTraceReplaySummary,
    actual: &WorkloadTrafficTraceReplaySummary,
) -> bool {
    actual.scheduled_count() >= expected.minimum_scheduled_count()
        && actual.response_delivery_count() >= expected.minimum_response_delivery_count()
        && actual.memory_trace_event_count() >= expected.minimum_memory_trace_event_count()
        && actual.memory_failure_count() >= expected.minimum_memory_failure_count()
        && actual.control_ack_count() >= expected.minimum_control_ack_count()
        && actual.control_failure_count() >= expected.minimum_control_failure_count()
        && actual.sideband_event_count() >= expected.minimum_sideband_event_count()
}

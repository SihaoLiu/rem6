use crate::{WorkloadResourceId, WorkloadRouteId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadTrafficTraceReplayRun {
    route: WorkloadRouteId,
    resource: WorkloadResourceId,
    tick_frequency: u64,
    agent: u32,
    line_bytes: u64,
    duration: u64,
    control_partition: u32,
    retry_delay: u64,
}

impl WorkloadTrafficTraceReplayRun {
    pub const fn new(
        route: WorkloadRouteId,
        resource: WorkloadResourceId,
        tick_frequency: u64,
        agent: u32,
        line_bytes: u64,
        duration: u64,
        control_partition: u32,
    ) -> Self {
        Self {
            route,
            resource,
            tick_frequency,
            agent,
            line_bytes,
            duration,
            control_partition,
            retry_delay: 0,
        }
    }

    pub const fn with_retry_delay(mut self, retry_delay: u64) -> Self {
        self.retry_delay = retry_delay;
        self
    }

    pub const fn route(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub const fn resource(&self) -> &WorkloadResourceId {
        &self.resource
    }

    pub const fn tick_frequency(&self) -> u64 {
        self.tick_frequency
    }

    pub const fn agent(&self) -> u32 {
        self.agent
    }

    pub const fn line_bytes(&self) -> u64 {
        self.line_bytes
    }

    pub const fn duration(&self) -> u64 {
        self.duration
    }

    pub const fn control_partition(&self) -> u32 {
        self.control_partition
    }

    pub const fn retry_delay(&self) -> u64 {
        self.retry_delay
    }

    pub fn sort_key(&self) -> (&WorkloadRouteId, &WorkloadResourceId) {
        (&self.route, &self.resource)
    }
}

use crate::{
    WorkloadError, WorkloadExpectedParallelRemoteDelayFloor, WorkloadExpectedParallelRemoteFlow,
    WorkloadExpectedParallelRemoteFlowTiming, WorkloadExpectedParallelRemoteSend, WorkloadManifest,
    WorkloadManifestBuilder, WorkloadReplayPlan,
};

impl WorkloadManifest {
    pub fn expected_parallel_remote_flows(&self) -> &[WorkloadExpectedParallelRemoteFlow] {
        &self.expected_parallel_remote_flows
    }

    pub fn expected_parallel_remote_delay_floors(
        &self,
    ) -> &[WorkloadExpectedParallelRemoteDelayFloor] {
        &self.expected_parallel_remote_delay_floors
    }

    pub fn expected_parallel_remote_sends(&self) -> &[WorkloadExpectedParallelRemoteSend] {
        &self.expected_parallel_remote_sends
    }

    pub fn expected_parallel_remote_flow_timings(
        &self,
    ) -> &[WorkloadExpectedParallelRemoteFlowTiming] {
        &self.expected_parallel_remote_flow_timings
    }
}

impl WorkloadManifestBuilder {
    pub fn add_expected_parallel_remote_flow(
        mut self,
        expected: WorkloadExpectedParallelRemoteFlow,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_remote_flows
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelRemoteFlow {
                scope: expected.scope(),
                source: expected.source().index(),
                target: expected.target().index(),
            });
        }
        self.expected_parallel_remote_flows.push(expected);
        self.expected_parallel_remote_flows
            .sort_by_key(|flow| flow.sort_key());
        Ok(self)
    }

    pub fn add_expected_parallel_remote_delay_floor(
        mut self,
        expected: WorkloadExpectedParallelRemoteDelayFloor,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_remote_delay_floors
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelRemoteDelayFloor {
                scope: expected.scope(),
            });
        }
        self.expected_parallel_remote_delay_floors.push(expected);
        self.expected_parallel_remote_delay_floors
            .sort_by_key(|floor| floor.sort_key());
        Ok(self)
    }

    pub fn add_expected_parallel_remote_send(
        mut self,
        expected: WorkloadExpectedParallelRemoteSend,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_remote_sends
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelRemoteSend {
                scope: expected.scope(),
                source: expected.source().index(),
                target: expected.target().index(),
                source_tick: expected.source_tick(),
                delivery_tick: expected.delivery_tick(),
                order: expected.order(),
            });
        }
        self.expected_parallel_remote_sends.push(expected);
        self.expected_parallel_remote_sends
            .sort_by_key(|send| send.sort_key());
        Ok(self)
    }

    pub fn add_expected_parallel_remote_flow_timing(
        mut self,
        expected: WorkloadExpectedParallelRemoteFlowTiming,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_remote_flow_timings
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelRemoteFlowTiming {
                scope: expected.scope(),
                source: expected.source().index(),
                target: expected.target().index(),
            });
        }
        self.expected_parallel_remote_flow_timings.push(expected);
        self.expected_parallel_remote_flow_timings
            .sort_by_key(|timing| timing.sort_key());
        Ok(self)
    }
}

impl WorkloadReplayPlan {
    pub fn add_expected_parallel_remote_flow(
        mut self,
        expected: WorkloadExpectedParallelRemoteFlow,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_remote_flows
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelRemoteFlow {
                scope: expected.scope(),
                source: expected.source().index(),
                target: expected.target().index(),
            });
        }
        self.expected_parallel_remote_flows.push(expected);
        self.expected_parallel_remote_flows
            .sort_by_key(|flow| flow.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_remote_flows(&self) -> &[WorkloadExpectedParallelRemoteFlow] {
        &self.expected_parallel_remote_flows
    }

    pub fn add_expected_parallel_remote_delay_floor(
        mut self,
        expected: WorkloadExpectedParallelRemoteDelayFloor,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_remote_delay_floors
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelRemoteDelayFloor {
                scope: expected.scope(),
            });
        }
        self.expected_parallel_remote_delay_floors.push(expected);
        self.expected_parallel_remote_delay_floors
            .sort_by_key(|floor| floor.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_remote_delay_floors(
        &self,
    ) -> &[WorkloadExpectedParallelRemoteDelayFloor] {
        &self.expected_parallel_remote_delay_floors
    }

    pub fn add_expected_parallel_remote_send(
        mut self,
        expected: WorkloadExpectedParallelRemoteSend,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_remote_sends
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelRemoteSend {
                scope: expected.scope(),
                source: expected.source().index(),
                target: expected.target().index(),
                source_tick: expected.source_tick(),
                delivery_tick: expected.delivery_tick(),
                order: expected.order(),
            });
        }
        self.expected_parallel_remote_sends.push(expected);
        self.expected_parallel_remote_sends
            .sort_by_key(|send| send.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_remote_sends(&self) -> &[WorkloadExpectedParallelRemoteSend] {
        &self.expected_parallel_remote_sends
    }

    pub fn add_expected_parallel_remote_flow_timing(
        mut self,
        expected: WorkloadExpectedParallelRemoteFlowTiming,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_remote_flow_timings
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelRemoteFlowTiming {
                scope: expected.scope(),
                source: expected.source().index(),
                target: expected.target().index(),
            });
        }
        self.expected_parallel_remote_flow_timings.push(expected);
        self.expected_parallel_remote_flow_timings
            .sort_by_key(|timing| timing.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_remote_flow_timings(
        &self,
    ) -> &[WorkloadExpectedParallelRemoteFlowTiming] {
        &self.expected_parallel_remote_flow_timings
    }
}

use super::WorkloadParallelExecutionSummary;

impl WorkloadParallelExecutionSummary {
    pub const fn with_riscv_core_counts(
        mut self,
        core_count: usize,
        active_core_count: usize,
        fetch_issue_count: usize,
        committed_instruction_count: usize,
        data_access_issue_count: usize,
        scheduled_trap_count: usize,
    ) -> Self {
        self.riscv_core_count = core_count;
        self.active_riscv_core_count = active_core_count;
        self.riscv_fetch_issue_count = fetch_issue_count;
        self.riscv_committed_instruction_count = committed_instruction_count;
        self.riscv_data_access_issue_count = data_access_issue_count;
        self.riscv_scheduled_trap_count = scheduled_trap_count;
        self
    }

    pub const fn with_riscv_data_access_probe_counts(
        mut self,
        sample_count: u64,
        stack_distance_infinite_samples: u64,
        stack_distance_finite_samples: u64,
        stack_depth: usize,
    ) -> Self {
        self.riscv_data_access_probe_sample_count = sample_count;
        self.riscv_data_access_probe_stack_distance_infinite_samples =
            stack_distance_infinite_samples;
        self.riscv_data_access_probe_stack_distance_finite_samples = stack_distance_finite_samples;
        self.riscv_data_access_probe_stack_depth = stack_depth;
        self
    }

    pub const fn riscv_core_count(&self) -> usize {
        self.riscv_core_count
    }

    pub const fn active_riscv_core_count(&self) -> usize {
        self.active_riscv_core_count
    }

    pub const fn riscv_fetch_issue_count(&self) -> usize {
        self.riscv_fetch_issue_count
    }

    pub const fn riscv_committed_instruction_count(&self) -> usize {
        self.riscv_committed_instruction_count
    }

    pub const fn riscv_data_access_issue_count(&self) -> usize {
        self.riscv_data_access_issue_count
    }

    pub const fn riscv_data_access_probe_sample_count(&self) -> u64 {
        self.riscv_data_access_probe_sample_count
    }

    pub const fn riscv_data_access_probe_stack_distance_infinite_samples(&self) -> u64 {
        self.riscv_data_access_probe_stack_distance_infinite_samples
    }

    pub const fn riscv_data_access_probe_stack_distance_finite_samples(&self) -> u64 {
        self.riscv_data_access_probe_stack_distance_finite_samples
    }

    pub const fn riscv_data_access_probe_stack_depth(&self) -> usize {
        self.riscv_data_access_probe_stack_depth
    }

    pub const fn riscv_scheduled_trap_count(&self) -> usize {
        self.riscv_scheduled_trap_count
    }

    pub const fn has_riscv_data_access_probe_activity(&self) -> bool {
        self.riscv_data_access_probe_sample_count != 0
            || self.riscv_data_access_probe_stack_distance_infinite_samples != 0
            || self.riscv_data_access_probe_stack_distance_finite_samples != 0
            || self.riscv_data_access_probe_stack_depth != 0
    }

    pub const fn has_riscv_core_activity(&self) -> bool {
        self.riscv_fetch_issue_count != 0
            || self.riscv_committed_instruction_count != 0
            || self.riscv_data_access_issue_count != 0
            || self.riscv_scheduled_trap_count != 0
    }
}

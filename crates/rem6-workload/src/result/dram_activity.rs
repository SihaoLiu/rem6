use rem6_dram::DramLowPowerState;

use super::WorkloadParallelExecutionSummary;

impl WorkloadParallelExecutionSummary {
    pub const fn with_dram_low_power_activity(
        mut self,
        precharge_powerdown_entry_count: usize,
        precharge_powerdown_cycle_count: u64,
        self_refresh_entry_count: usize,
        self_refresh_cycle_count: u64,
        exit_count: usize,
        exit_latency_cycles: u64,
    ) -> Self {
        self.dram_precharge_powerdown_entry_count = precharge_powerdown_entry_count;
        self.dram_precharge_powerdown_cycle_count = precharge_powerdown_cycle_count;
        self.dram_self_refresh_entry_count = self_refresh_entry_count;
        self.dram_self_refresh_cycle_count = self_refresh_cycle_count;
        self.dram_low_power_exit_count = exit_count;
        self.dram_low_power_exit_latency_cycles = exit_latency_cycles;
        self
    }

    pub const fn dram_low_power_entry_count(&self, state: DramLowPowerState) -> usize {
        match state {
            DramLowPowerState::PrechargePowerdown => self.dram_precharge_powerdown_entry_count,
            DramLowPowerState::SelfRefresh => self.dram_self_refresh_entry_count,
        }
    }

    pub const fn dram_low_power_cycle_count(&self, state: DramLowPowerState) -> u64 {
        match state {
            DramLowPowerState::PrechargePowerdown => self.dram_precharge_powerdown_cycle_count,
            DramLowPowerState::SelfRefresh => self.dram_self_refresh_cycle_count,
        }
    }

    pub const fn dram_low_power_exit_count(&self) -> usize {
        self.dram_low_power_exit_count
    }

    pub const fn dram_low_power_exit_latency_cycles(&self) -> u64 {
        self.dram_low_power_exit_latency_cycles
    }
}

use rem6_dram::DramLowPowerState;

use super::WorkloadParallelExecutionSummary;

impl WorkloadParallelExecutionSummary {
    pub fn with_dram_low_power_activity(
        mut self,
        state_activities: impl IntoIterator<Item = (DramLowPowerState, usize, u64)>,
        exit_count: usize,
        exit_latency_cycles: u64,
    ) -> Self {
        for (state, entry_count, cycle_count) in state_activities {
            match state {
                DramLowPowerState::ActivePowerdown => {
                    self.dram_active_powerdown_entry_count =
                        self.dram_active_powerdown_entry_count.max(entry_count);
                    self.dram_active_powerdown_cycle_count =
                        self.dram_active_powerdown_cycle_count.max(cycle_count);
                }
                DramLowPowerState::PrechargePowerdown => {
                    self.dram_precharge_powerdown_entry_count =
                        self.dram_precharge_powerdown_entry_count.max(entry_count);
                    self.dram_precharge_powerdown_cycle_count =
                        self.dram_precharge_powerdown_cycle_count.max(cycle_count);
                }
                DramLowPowerState::SelfRefresh => {
                    self.dram_self_refresh_entry_count =
                        self.dram_self_refresh_entry_count.max(entry_count);
                    self.dram_self_refresh_cycle_count =
                        self.dram_self_refresh_cycle_count.max(cycle_count);
                }
            }
        }
        self.dram_low_power_exit_count = exit_count;
        self.dram_low_power_exit_latency_cycles = exit_latency_cycles;
        self
    }

    pub const fn dram_low_power_entry_count(&self, state: DramLowPowerState) -> usize {
        match state {
            DramLowPowerState::ActivePowerdown => self.dram_active_powerdown_entry_count,
            DramLowPowerState::PrechargePowerdown => self.dram_precharge_powerdown_entry_count,
            DramLowPowerState::SelfRefresh => self.dram_self_refresh_entry_count,
        }
    }

    pub const fn dram_low_power_cycle_count(&self, state: DramLowPowerState) -> u64 {
        match state {
            DramLowPowerState::ActivePowerdown => self.dram_active_powerdown_cycle_count,
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

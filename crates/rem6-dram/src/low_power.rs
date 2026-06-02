use crate::DramError;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DramLowPowerState {
    PrechargePowerdown,
    SelfRefresh,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DramLowPowerTimingField {
    PrechargePowerdownEntryDelay,
    SelfRefreshEntryDelay,
    ExitLatency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramLowPowerTiming {
    precharge_powerdown_entry_delay: u64,
    self_refresh_entry_delay: u64,
    exit_latency: u64,
}

impl DramLowPowerTiming {
    pub const fn new(
        precharge_powerdown_entry_delay: u64,
        self_refresh_entry_delay: u64,
        exit_latency: u64,
    ) -> Result<Self, DramError> {
        if precharge_powerdown_entry_delay == 0 {
            return Err(DramError::ZeroLowPowerTiming {
                field: DramLowPowerTimingField::PrechargePowerdownEntryDelay,
            });
        }
        if self_refresh_entry_delay == 0 {
            return Err(DramError::ZeroLowPowerTiming {
                field: DramLowPowerTimingField::SelfRefreshEntryDelay,
            });
        }
        if exit_latency == 0 {
            return Err(DramError::ZeroLowPowerTiming {
                field: DramLowPowerTimingField::ExitLatency,
            });
        }
        if self_refresh_entry_delay <= precharge_powerdown_entry_delay {
            return Err(DramError::LowPowerSelfRefreshBeforePowerdown {
                precharge_powerdown_entry_delay,
                self_refresh_entry_delay,
            });
        }

        Ok(Self {
            precharge_powerdown_entry_delay,
            self_refresh_entry_delay,
            exit_latency,
        })
    }

    pub const fn precharge_powerdown_entry_delay(self) -> u64 {
        self.precharge_powerdown_entry_delay
    }

    pub const fn self_refresh_entry_delay(self) -> u64 {
        self.self_refresh_entry_delay
    }

    pub const fn exit_latency(self) -> u64 {
        self.exit_latency
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramLowPowerEvent {
    state: DramLowPowerState,
    parallel_port: u32,
    entry_cycle: u64,
    exit_cycle: u64,
}

impl DramLowPowerEvent {
    const fn new(
        state: DramLowPowerState,
        parallel_port: u32,
        entry_cycle: u64,
        exit_cycle: u64,
    ) -> Self {
        Self {
            state,
            parallel_port,
            entry_cycle,
            exit_cycle,
        }
    }

    pub const fn state(self) -> DramLowPowerState {
        self.state
    }

    pub const fn parallel_port(self) -> u32 {
        self.parallel_port
    }

    pub const fn entry_cycle(self) -> u64 {
        self.entry_cycle
    }

    pub const fn exit_cycle(self) -> u64 {
        self.exit_cycle
    }

    pub const fn cycle_count(self) -> u64 {
        self.exit_cycle - self.entry_cycle
    }
}

pub(crate) fn events_for_idle_window(
    timing: DramLowPowerTiming,
    parallel_port: u32,
    idle_start_cycle: u64,
    arrival_cycle: u64,
) -> Vec<DramLowPowerEvent> {
    if arrival_cycle <= idle_start_cycle {
        return Vec::new();
    }

    let precharge_entry = idle_start_cycle.saturating_add(timing.precharge_powerdown_entry_delay());
    if precharge_entry >= arrival_cycle {
        return Vec::new();
    }

    let self_refresh_entry = idle_start_cycle.saturating_add(timing.self_refresh_entry_delay());
    if self_refresh_entry >= arrival_cycle {
        return vec![DramLowPowerEvent::new(
            DramLowPowerState::PrechargePowerdown,
            parallel_port,
            precharge_entry,
            arrival_cycle,
        )];
    }

    vec![
        DramLowPowerEvent::new(
            DramLowPowerState::PrechargePowerdown,
            parallel_port,
            precharge_entry,
            self_refresh_entry,
        ),
        DramLowPowerEvent::new(
            DramLowPowerState::SelfRefresh,
            parallel_port,
            self_refresh_entry,
            arrival_cycle,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturated_low_power_idle_deadlines_do_not_wrap() {
        let timing = DramLowPowerTiming::new(20, 80, 7).unwrap();

        assert_eq!(
            events_for_idle_window(timing, 0, u64::MAX - 10, u64::MAX),
            Vec::new()
        );
    }
}

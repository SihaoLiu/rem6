use crate::DramError;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DramLowPowerState {
    ActivePowerdown,
    PrechargePowerdown,
    SelfRefresh,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DramLowPowerTimingField {
    PrechargePowerdownEntryDelay,
    SelfRefreshEntryDelay,
    ExitLatency,
    SelfRefreshExitLatency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramLowPowerTiming {
    precharge_powerdown_entry_delay: u64,
    self_refresh_entry_delay: u64,
    powerdown_exit_latency: u64,
    self_refresh_exit_latency: u64,
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
            powerdown_exit_latency: exit_latency,
            self_refresh_exit_latency: exit_latency,
        })
    }

    pub const fn with_self_refresh_exit_latency(
        mut self,
        self_refresh_exit_latency: u64,
    ) -> Result<Self, DramError> {
        if self_refresh_exit_latency == 0 {
            return Err(DramError::ZeroLowPowerTiming {
                field: DramLowPowerTimingField::SelfRefreshExitLatency,
            });
        }

        self.self_refresh_exit_latency = self_refresh_exit_latency;
        Ok(self)
    }

    pub const fn precharge_powerdown_entry_delay(self) -> u64 {
        self.precharge_powerdown_entry_delay
    }

    pub const fn self_refresh_entry_delay(self) -> u64 {
        self.self_refresh_entry_delay
    }

    pub const fn exit_latency(self) -> u64 {
        self.powerdown_exit_latency
    }

    pub const fn self_refresh_exit_latency(self) -> u64 {
        self.self_refresh_exit_latency
    }

    pub const fn exit_latency_for_state(self, state: DramLowPowerState) -> u64 {
        match state {
            DramLowPowerState::ActivePowerdown | DramLowPowerState::PrechargePowerdown => {
                self.powerdown_exit_latency
            }
            DramLowPowerState::SelfRefresh => self.self_refresh_exit_latency,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramLowPowerEvent {
    state: DramLowPowerState,
    parallel_port: u32,
    bank: Option<u32>,
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
            bank: None,
            entry_cycle,
            exit_cycle,
        }
    }

    pub(crate) const fn with_bank(mut self, bank: u32) -> Self {
        self.bank = Some(bank);
        self
    }

    pub const fn state(self) -> DramLowPowerState {
        self.state
    }

    pub const fn parallel_port(self) -> u32 {
        self.parallel_port
    }

    pub const fn bank(self) -> Option<u32> {
        self.bank
    }

    pub const fn applies_to_bank(self, parallel_port: u32, bank: u32) -> bool {
        self.parallel_port == parallel_port
            && match self.bank {
                Some(event_bank) => event_bank == bank,
                None => true,
            }
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DramLowPowerActivity {
    active_powerdown_entry_count: usize,
    active_powerdown_cycle_count: u64,
    precharge_powerdown_entry_count: usize,
    precharge_powerdown_cycle_count: u64,
    self_refresh_entry_count: usize,
    self_refresh_cycle_count: u64,
    exit_count: usize,
    exit_latency_cycles: u64,
}

impl DramLowPowerActivity {
    pub(crate) fn record_event(&mut self, event: DramLowPowerEvent) {
        match event.state() {
            DramLowPowerState::ActivePowerdown => {
                self.active_powerdown_entry_count += 1;
                self.active_powerdown_cycle_count += event.cycle_count();
            }
            DramLowPowerState::PrechargePowerdown => {
                self.precharge_powerdown_entry_count += 1;
                self.precharge_powerdown_cycle_count += event.cycle_count();
            }
            DramLowPowerState::SelfRefresh => {
                self.self_refresh_entry_count += 1;
                self.self_refresh_cycle_count += event.cycle_count();
            }
        }
    }

    pub(crate) fn record_events(&mut self, events: &[DramLowPowerEvent]) {
        for event in events {
            self.record_event(*event);
        }
    }

    pub(crate) fn record_events_for_bank(
        &mut self,
        events: &[DramLowPowerEvent],
        parallel_port: u32,
        bank: u32,
    ) {
        for event in events {
            if event.applies_to_bank(parallel_port, bank) {
                self.record_event(*event);
            }
        }
    }

    pub(crate) fn record_events_until(&mut self, events: &[DramLowPowerEvent], end_cycle: u64) {
        for event in events {
            if event.entry_cycle() >= end_cycle {
                continue;
            }
            let cycle_count = event.exit_cycle().min(end_cycle) - event.entry_cycle();
            match event.state() {
                DramLowPowerState::ActivePowerdown => {
                    self.active_powerdown_entry_count += 1;
                    self.active_powerdown_cycle_count += cycle_count;
                }
                DramLowPowerState::PrechargePowerdown => {
                    self.precharge_powerdown_entry_count += 1;
                    self.precharge_powerdown_cycle_count += cycle_count;
                }
                DramLowPowerState::SelfRefresh => {
                    self.self_refresh_entry_count += 1;
                    self.self_refresh_cycle_count += cycle_count;
                }
            }
        }
    }

    pub(crate) fn record_exit(&mut self, exit_latency_cycles: u64) {
        self.exit_count += 1;
        self.exit_latency_cycles += exit_latency_cycles;
    }

    pub(crate) fn merge(&mut self, later: Self) {
        self.active_powerdown_entry_count += later.active_powerdown_entry_count;
        self.active_powerdown_cycle_count += later.active_powerdown_cycle_count;
        self.precharge_powerdown_entry_count += later.precharge_powerdown_entry_count;
        self.precharge_powerdown_cycle_count += later.precharge_powerdown_cycle_count;
        self.self_refresh_entry_count += later.self_refresh_entry_count;
        self.self_refresh_cycle_count += later.self_refresh_cycle_count;
        self.exit_count += later.exit_count;
        self.exit_latency_cycles += later.exit_latency_cycles;
    }

    pub const fn entry_count(self, state: DramLowPowerState) -> usize {
        match state {
            DramLowPowerState::ActivePowerdown => self.active_powerdown_entry_count,
            DramLowPowerState::PrechargePowerdown => self.precharge_powerdown_entry_count,
            DramLowPowerState::SelfRefresh => self.self_refresh_entry_count,
        }
    }

    pub const fn cycle_count(self, state: DramLowPowerState) -> u64 {
        match state {
            DramLowPowerState::ActivePowerdown => self.active_powerdown_cycle_count,
            DramLowPowerState::PrechargePowerdown => self.precharge_powerdown_cycle_count,
            DramLowPowerState::SelfRefresh => self.self_refresh_cycle_count,
        }
    }

    pub const fn exit_count(self) -> usize {
        self.exit_count
    }

    pub const fn exit_latency_cycles(self) -> u64 {
        self.exit_latency_cycles
    }
}

pub(crate) fn events_for_idle_window(
    timing: DramLowPowerTiming,
    parallel_port: u32,
    idle_start_cycle: u64,
    arrival_cycle: u64,
    has_open_row: bool,
) -> Vec<DramLowPowerEvent> {
    if arrival_cycle <= idle_start_cycle {
        return Vec::new();
    }

    let precharge_entry = idle_start_cycle.saturating_add(timing.precharge_powerdown_entry_delay());
    if precharge_entry >= arrival_cycle {
        return Vec::new();
    }

    if has_open_row {
        return vec![DramLowPowerEvent::new(
            DramLowPowerState::ActivePowerdown,
            parallel_port,
            precharge_entry,
            arrival_cycle,
        )];
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
            events_for_idle_window(timing, 0, u64::MAX - 10, u64::MAX, false),
            Vec::new()
        );
    }
}

use rem6_dram::DramRefreshPolicy;

use super::parse::required_value;
use crate::Rem6CliError;

const DEFAULT_DRAM_ACTIVATE_LATENCY: u64 = 3;
const DEFAULT_DRAM_READ_LATENCY: u64 = 5;
const DEFAULT_DRAM_WRITE_LATENCY: u64 = 7;
const DEFAULT_DRAM_PRECHARGE_LATENCY: u64 = 2;
const DEFAULT_DRAM_BUS_TURNAROUND: u64 = 4;
const DEFAULT_DRAM_BURST_SPACING: u64 = 0;
const DEFAULT_PRECHARGE_POWERDOWN_ENTRY_DELAY: u64 = 20;
const DEFAULT_SELF_REFRESH_ENTRY_DELAY: u64 = 80;
const DEFAULT_LOW_POWER_EXIT_LATENCY: u64 = 7;
const DEFAULT_SELF_REFRESH_EXIT_LATENCY: u64 = 17;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CliDramMemoryProfile {
    Ddr,
    Ddr4_2400_8Gb,
    Ddr5_4800_16Gb,
    Hbm,
    Hbm2_2000_2Gb,
    Lpddr,
    Lpddr4_3200_16Gb,
    Nvm,
}

impl CliDramMemoryProfile {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        match value {
            "ddr" => Ok(Self::Ddr),
            "ddr4-2400-8gb" => Ok(Self::Ddr4_2400_8Gb),
            "ddr5-4800-16gb" => Ok(Self::Ddr5_4800_16Gb),
            "hbm" => Ok(Self::Hbm),
            "hbm2-2000-2gb" => Ok(Self::Hbm2_2000_2Gb),
            "lpddr" => Ok(Self::Lpddr),
            "lpddr4-3200-16gb" => Ok(Self::Lpddr4_3200_16Gb),
            "nvm" => Ok(Self::Nvm),
            _ => Err(Rem6CliError::UnsupportedDramMemoryProfile {
                profile: value.to_string(),
            }),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ddr => "ddr",
            Self::Ddr4_2400_8Gb => "ddr4-2400-8gb",
            Self::Ddr5_4800_16Gb => "ddr5-4800-16gb",
            Self::Hbm => "hbm",
            Self::Hbm2_2000_2Gb => "hbm2-2000-2gb",
            Self::Lpddr => "lpddr",
            Self::Lpddr4_3200_16Gb => "lpddr4-3200-16gb",
            Self::Nvm => "nvm",
        }
    }

    pub const fn supports_low_power_timing(self) -> bool {
        matches!(self, Self::Lpddr | Self::Lpddr4_3200_16Gb | Self::Nvm)
    }

    pub const fn supports_refresh_timing(self) -> bool {
        !matches!(self, Self::Nvm)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CliDramCommandWindow {
    window_cycles: u64,
    max_commands: u32,
}

impl CliDramCommandWindow {
    fn new(window_cycles: u64, max_commands: u32) -> Result<Self, Rem6CliError> {
        if window_cycles == 0 {
            return Err(Rem6CliError::InvalidDramTiming {
                value: window_cycles.to_string(),
            });
        }
        if max_commands == 0 {
            return Err(Rem6CliError::InvalidDramTiming {
                value: max_commands.to_string(),
            });
        }
        Ok(Self {
            window_cycles,
            max_commands,
        })
    }

    pub const fn window_cycles(self) -> u64 {
        self.window_cycles
    }

    pub const fn max_commands(self) -> u32 {
        self.max_commands
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CliDramTiming {
    activate_latency: u64,
    read_latency: u64,
    write_latency: u64,
    precharge_latency: u64,
    bus_turnaround: u64,
    burst_spacing: u64,
    same_bank_group_burst_spacing: Option<u64>,
    command_window: Option<CliDramCommandWindow>,
    refresh_policy: DramRefreshPolicy,
}

impl CliDramTiming {
    fn from_options(
        activate_latency: Option<u64>,
        read_latency: Option<u64>,
        write_latency: Option<u64>,
        precharge_latency: Option<u64>,
        bus_turnaround: Option<u64>,
        burst_spacing: Option<u64>,
        same_bank_group_burst_spacing: Option<u64>,
        command_window_cycles: Option<u64>,
        command_window_max_commands: Option<u32>,
        refresh_policy: Option<DramRefreshPolicy>,
    ) -> Result<Self, Rem6CliError> {
        let command_window = match (command_window_cycles, command_window_max_commands) {
            (None, None) => None,
            (Some(window_cycles), Some(max_commands)) => {
                Some(CliDramCommandWindow::new(window_cycles, max_commands)?)
            }
            _ => return Err(Rem6CliError::IncompleteDramCommandWindowTiming),
        };
        Ok(Self {
            activate_latency: validate_positive_dram_timing_value(
                activate_latency.unwrap_or(DEFAULT_DRAM_ACTIVATE_LATENCY),
            )?,
            read_latency: validate_positive_dram_timing_value(
                read_latency.unwrap_or(DEFAULT_DRAM_READ_LATENCY),
            )?,
            write_latency: validate_positive_dram_timing_value(
                write_latency.unwrap_or(DEFAULT_DRAM_WRITE_LATENCY),
            )?,
            precharge_latency: validate_positive_dram_timing_value(
                precharge_latency.unwrap_or(DEFAULT_DRAM_PRECHARGE_LATENCY),
            )?,
            bus_turnaround: bus_turnaround.unwrap_or(DEFAULT_DRAM_BUS_TURNAROUND),
            burst_spacing: burst_spacing.unwrap_or(DEFAULT_DRAM_BURST_SPACING),
            same_bank_group_burst_spacing: same_bank_group_burst_spacing
                .map(validate_positive_dram_timing_value)
                .transpose()?,
            command_window,
            refresh_policy: refresh_policy.unwrap_or(DramRefreshPolicy::PerBank),
        })
    }

    pub const fn activate_latency(self) -> u64 {
        self.activate_latency
    }

    pub const fn read_latency(self) -> u64 {
        self.read_latency
    }

    pub const fn write_latency(self) -> u64 {
        self.write_latency
    }

    pub const fn precharge_latency(self) -> u64 {
        self.precharge_latency
    }

    pub const fn bus_turnaround(self) -> u64 {
        self.bus_turnaround
    }

    pub const fn burst_spacing(self) -> u64 {
        self.burst_spacing
    }

    pub const fn same_bank_group_burst_spacing(self) -> Option<u64> {
        self.same_bank_group_burst_spacing
    }

    pub const fn command_window(self) -> Option<CliDramCommandWindow> {
        self.command_window
    }

    pub const fn refresh_policy(self) -> DramRefreshPolicy {
        self.refresh_policy
    }
}

impl Default for CliDramTiming {
    fn default() -> Self {
        Self {
            activate_latency: DEFAULT_DRAM_ACTIVATE_LATENCY,
            read_latency: DEFAULT_DRAM_READ_LATENCY,
            write_latency: DEFAULT_DRAM_WRITE_LATENCY,
            precharge_latency: DEFAULT_DRAM_PRECHARGE_LATENCY,
            bus_turnaround: DEFAULT_DRAM_BUS_TURNAROUND,
            burst_spacing: DEFAULT_DRAM_BURST_SPACING,
            same_bank_group_burst_spacing: None,
            command_window: None,
            refresh_policy: DramRefreshPolicy::PerBank,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CliDramTimingOptions {
    activate_latency: Option<u64>,
    read_latency: Option<u64>,
    write_latency: Option<u64>,
    precharge_latency: Option<u64>,
    bus_turnaround: Option<u64>,
    burst_spacing: Option<u64>,
    same_bank_group_burst_spacing: Option<u64>,
    command_window_cycles: Option<u64>,
    command_window_max_commands: Option<u32>,
    refresh_policy: Option<DramRefreshPolicy>,
}

impl CliDramTimingOptions {
    pub(super) fn new(
        activate_latency: Option<u64>,
        read_latency: Option<u64>,
        write_latency: Option<u64>,
        precharge_latency: Option<u64>,
        bus_turnaround: Option<u64>,
        burst_spacing: Option<u64>,
        same_bank_group_burst_spacing: Option<u64>,
        command_window_cycles: Option<u64>,
        command_window_max_commands: Option<u32>,
        refresh_policy: Option<&str>,
    ) -> Result<Self, Rem6CliError> {
        Ok(Self {
            activate_latency,
            read_latency,
            write_latency,
            precharge_latency,
            bus_turnaround,
            burst_spacing,
            same_bank_group_burst_spacing,
            command_window_cycles,
            command_window_max_commands,
            refresh_policy: refresh_policy
                .map(|policy| {
                    parse_dram_refresh_policy(policy).ok_or_else(|| {
                        Rem6CliError::InvalidDramTiming {
                            value: policy.to_string(),
                        }
                    })
                })
                .transpose()?,
        })
    }

    pub(super) const fn was_set(self) -> bool {
        self.activate_latency.is_some()
            || self.read_latency.is_some()
            || self.write_latency.is_some()
            || self.precharge_latency.is_some()
            || self.bus_turnaround.is_some()
            || self.burst_spacing.is_some()
            || self.same_bank_group_burst_spacing.is_some()
            || self.command_window_cycles.is_some()
            || self.command_window_max_commands.is_some()
            || self.refresh_policy.is_some()
    }

    pub(super) const fn refresh_policy_was_set(self) -> bool {
        self.refresh_policy.is_some()
    }

    pub(super) fn set_activate_latency(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.activate_latency = Some(parse_positive_dram_timing_value(value)?);
        Ok(())
    }

    pub(super) fn set_read_latency(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.read_latency = Some(parse_positive_dram_timing_value(value)?);
        Ok(())
    }

    pub(super) fn set_write_latency(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.write_latency = Some(parse_positive_dram_timing_value(value)?);
        Ok(())
    }

    pub(super) fn set_precharge_latency(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.precharge_latency = Some(parse_positive_dram_timing_value(value)?);
        Ok(())
    }

    pub(super) fn set_bus_turnaround(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.bus_turnaround = Some(parse_dram_timing_value(value)?);
        Ok(())
    }

    pub(super) fn set_burst_spacing(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.burst_spacing = Some(parse_dram_timing_value(value)?);
        Ok(())
    }

    pub(super) fn set_same_bank_group_burst_spacing(
        &mut self,
        value: &str,
    ) -> Result<(), Rem6CliError> {
        self.same_bank_group_burst_spacing = Some(parse_positive_dram_timing_value(value)?);
        Ok(())
    }

    pub(super) fn set_command_window_cycles(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.command_window_cycles = Some(parse_positive_dram_timing_value(value)?);
        Ok(())
    }

    pub(super) fn set_command_window_max_commands(
        &mut self,
        value: &str,
    ) -> Result<(), Rem6CliError> {
        self.command_window_max_commands = Some(
            value
                .parse::<u32>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| Rem6CliError::InvalidDramTiming {
                    value: value.to_string(),
                })?,
        );
        Ok(())
    }

    pub(super) fn set_refresh_policy(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.refresh_policy = Some(parse_dram_refresh_policy(value).ok_or_else(|| {
            Rem6CliError::InvalidDramTiming {
                value: value.to_string(),
            }
        })?);
        Ok(())
    }

    pub(super) fn timing(self) -> Result<CliDramTiming, Rem6CliError> {
        CliDramTiming::from_options(
            self.activate_latency,
            self.read_latency,
            self.write_latency,
            self.precharge_latency,
            self.bus_turnaround,
            self.burst_spacing,
            self.same_bank_group_burst_spacing,
            self.command_window_cycles,
            self.command_window_max_commands,
            self.refresh_policy,
        )
    }
}

fn parse_dram_refresh_policy(value: &str) -> Option<DramRefreshPolicy> {
    match value {
        "per-bank" => Some(DramRefreshPolicy::PerBank),
        "all-bank" => Some(DramRefreshPolicy::AllBank),
        _ => None,
    }
}

fn parse_dram_timing_value(value: &str) -> Result<u64, Rem6CliError> {
    value
        .parse::<u64>()
        .map_err(|_| Rem6CliError::InvalidDramTiming {
            value: value.to_string(),
        })
}

fn parse_positive_dram_timing_value(value: &str) -> Result<u64, Rem6CliError> {
    parse_dram_timing_value(value).and_then(validate_positive_dram_timing_value)
}

fn validate_positive_dram_timing_value(value: u64) -> Result<u64, Rem6CliError> {
    (value > 0)
        .then_some(value)
        .ok_or_else(|| Rem6CliError::InvalidDramTiming {
            value: value.to_string(),
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CliDramRefreshTiming {
    interval: u64,
    recovery: u64,
}

impl CliDramRefreshTiming {
    pub(crate) const fn new(interval: u64, recovery: u64) -> Self {
        Self { interval, recovery }
    }

    pub const fn interval(self) -> u64 {
        self.interval
    }

    pub const fn recovery(self) -> u64 {
        self.recovery
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CliDramLowPowerTiming {
    precharge_powerdown_entry_delay: u64,
    self_refresh_entry_delay: u64,
    exit_latency: u64,
    self_refresh_exit_latency: u64,
}

impl CliDramLowPowerTiming {
    pub fn from_options(
        precharge_powerdown_entry_delay: Option<u64>,
        self_refresh_entry_delay: Option<u64>,
        exit_latency: Option<u64>,
        self_refresh_exit_latency: Option<u64>,
    ) -> Result<Self, Rem6CliError> {
        Self::new(
            precharge_powerdown_entry_delay.unwrap_or(DEFAULT_PRECHARGE_POWERDOWN_ENTRY_DELAY),
            self_refresh_entry_delay.unwrap_or(DEFAULT_SELF_REFRESH_ENTRY_DELAY),
            exit_latency.unwrap_or(DEFAULT_LOW_POWER_EXIT_LATENCY),
            self_refresh_exit_latency.unwrap_or(DEFAULT_SELF_REFRESH_EXIT_LATENCY),
        )
    }

    fn new(
        precharge_powerdown_entry_delay: u64,
        self_refresh_entry_delay: u64,
        exit_latency: u64,
        self_refresh_exit_latency: u64,
    ) -> Result<Self, Rem6CliError> {
        for value in [
            precharge_powerdown_entry_delay,
            self_refresh_entry_delay,
            exit_latency,
            self_refresh_exit_latency,
        ] {
            if value == 0 {
                return Err(Rem6CliError::InvalidDramLowPowerTiming {
                    value: "0".to_string(),
                });
            }
        }
        if self_refresh_entry_delay <= precharge_powerdown_entry_delay {
            return Err(Rem6CliError::InvalidDramLowPowerTiming {
                value: format!(
                    "precharge_powerdown_entry_delay={precharge_powerdown_entry_delay} self_refresh_entry_delay={self_refresh_entry_delay}"
                ),
            });
        }

        Ok(Self {
            precharge_powerdown_entry_delay,
            self_refresh_entry_delay,
            exit_latency,
            self_refresh_exit_latency,
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

    pub const fn self_refresh_exit_latency(self) -> u64 {
        self.self_refresh_exit_latency
    }
}

impl Default for CliDramLowPowerTiming {
    fn default() -> Self {
        Self {
            precharge_powerdown_entry_delay: DEFAULT_PRECHARGE_POWERDOWN_ENTRY_DELAY,
            self_refresh_entry_delay: DEFAULT_SELF_REFRESH_ENTRY_DELAY,
            exit_latency: DEFAULT_LOW_POWER_EXIT_LATENCY,
            self_refresh_exit_latency: DEFAULT_SELF_REFRESH_EXIT_LATENCY,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CliDramLowPowerTimingOptions {
    precharge_powerdown_entry_delay: Option<u64>,
    self_refresh_entry_delay: Option<u64>,
    exit_latency: Option<u64>,
    self_refresh_exit_latency: Option<u64>,
}

impl CliDramLowPowerTimingOptions {
    pub(super) const fn new(
        precharge_powerdown_entry_delay: Option<u64>,
        self_refresh_entry_delay: Option<u64>,
        exit_latency: Option<u64>,
        self_refresh_exit_latency: Option<u64>,
    ) -> Self {
        Self {
            precharge_powerdown_entry_delay,
            self_refresh_entry_delay,
            exit_latency,
            self_refresh_exit_latency,
        }
    }

    pub(super) const fn was_set(self) -> bool {
        self.precharge_powerdown_entry_delay.is_some()
            || self.self_refresh_entry_delay.is_some()
            || self.exit_latency.is_some()
            || self.self_refresh_exit_latency.is_some()
    }

    pub(super) fn set_precharge_powerdown_entry_delay(
        &mut self,
        value: &str,
    ) -> Result<(), Rem6CliError> {
        self.precharge_powerdown_entry_delay = Some(parse_low_power_timing_value(value)?);
        Ok(())
    }

    pub(super) fn set_self_refresh_entry_delay(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.self_refresh_entry_delay = Some(parse_low_power_timing_value(value)?);
        Ok(())
    }

    pub(super) fn set_exit_latency(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.exit_latency = Some(parse_low_power_timing_value(value)?);
        Ok(())
    }

    pub(super) fn set_self_refresh_exit_latency(
        &mut self,
        value: &str,
    ) -> Result<(), Rem6CliError> {
        self.self_refresh_exit_latency = Some(parse_low_power_timing_value(value)?);
        Ok(())
    }

    pub(super) fn timing(self) -> Result<CliDramLowPowerTiming, Rem6CliError> {
        CliDramLowPowerTiming::from_options(
            self.precharge_powerdown_entry_delay,
            self.self_refresh_entry_delay,
            self.exit_latency,
            self.self_refresh_exit_latency,
        )
    }
}

fn parse_low_power_timing_value(value: &str) -> Result<u64, Rem6CliError> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| Rem6CliError::InvalidDramLowPowerTiming {
            value: value.to_string(),
        })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CliDramRefreshTimingOptions {
    interval: Option<u64>,
    recovery: Option<u64>,
}

impl CliDramRefreshTimingOptions {
    pub(super) const fn new(interval: Option<u64>, recovery: Option<u64>) -> Self {
        Self { interval, recovery }
    }

    pub(super) const fn was_set(self) -> bool {
        self.interval.is_some() || self.recovery.is_some()
    }

    pub(super) fn set_interval(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.interval = Some(parse_refresh_timing_value(value)?);
        Ok(())
    }

    pub(super) fn set_recovery(&mut self, value: &str) -> Result<(), Rem6CliError> {
        self.recovery = Some(parse_refresh_timing_value(value)?);
        Ok(())
    }

    pub(super) fn timing(self) -> Result<Option<CliDramRefreshTiming>, Rem6CliError> {
        match (self.interval, self.recovery) {
            (None, None) => Ok(None),
            (Some(interval), Some(recovery)) => Ok(Some(CliDramRefreshTiming::new(
                validate_refresh_timing_value(interval)?,
                validate_refresh_timing_value(recovery)?,
            ))),
            _ => Err(Rem6CliError::IncompleteDramRefreshTiming),
        }
    }
}

fn parse_refresh_timing_value(value: &str) -> Result<u64, Rem6CliError> {
    value
        .parse::<u64>()
        .map_err(|_| Rem6CliError::InvalidDramRefreshTiming {
            value: value.to_string(),
        })
        .and_then(validate_refresh_timing_value)
}

fn validate_refresh_timing_value(value: u64) -> Result<u64, Rem6CliError> {
    (value > 0)
        .then_some(value)
        .ok_or_else(|| Rem6CliError::InvalidDramRefreshTiming {
            value: value.to_string(),
        })
}

pub(super) fn apply_dram_option_flag(
    flag: &str,
    args: &mut impl Iterator<Item = String>,
    timing: &mut CliDramTimingOptions,
    low_power: &mut CliDramLowPowerTimingOptions,
    refresh: &mut CliDramRefreshTimingOptions,
) -> Result<bool, Rem6CliError> {
    match flag {
        "--dram-activate-latency" => {
            let value = required_value(flag, args.next())?;
            timing.set_activate_latency(&value)?;
        }
        "--dram-read-latency" => {
            let value = required_value(flag, args.next())?;
            timing.set_read_latency(&value)?;
        }
        "--dram-write-latency" => {
            let value = required_value(flag, args.next())?;
            timing.set_write_latency(&value)?;
        }
        "--dram-precharge-latency" => {
            let value = required_value(flag, args.next())?;
            timing.set_precharge_latency(&value)?;
        }
        "--dram-bus-turnaround" => {
            let value = required_value(flag, args.next())?;
            timing.set_bus_turnaround(&value)?;
        }
        "--dram-burst-spacing" => {
            let value = required_value(flag, args.next())?;
            timing.set_burst_spacing(&value)?;
        }
        "--dram-same-bank-group-burst-spacing" => {
            let value = required_value(flag, args.next())?;
            timing.set_same_bank_group_burst_spacing(&value)?;
        }
        "--dram-command-window-cycles" => {
            let value = required_value(flag, args.next())?;
            timing.set_command_window_cycles(&value)?;
        }
        "--dram-command-window-max-commands" => {
            let value = required_value(flag, args.next())?;
            timing.set_command_window_max_commands(&value)?;
        }
        "--dram-low-power-precharge-powerdown-entry-delay" => {
            let value = required_value(flag, args.next())?;
            low_power.set_precharge_powerdown_entry_delay(&value)?;
        }
        "--dram-low-power-self-refresh-entry-delay" => {
            let value = required_value(flag, args.next())?;
            low_power.set_self_refresh_entry_delay(&value)?;
        }
        "--dram-low-power-exit-latency" => {
            let value = required_value(flag, args.next())?;
            low_power.set_exit_latency(&value)?;
        }
        "--dram-low-power-self-refresh-exit-latency" => {
            let value = required_value(flag, args.next())?;
            low_power.set_self_refresh_exit_latency(&value)?;
        }
        "--dram-refresh-interval" => {
            let value = required_value(flag, args.next())?;
            refresh.set_interval(&value)?;
        }
        "--dram-refresh-recovery" => {
            let value = required_value(flag, args.next())?;
            refresh.set_recovery(&value)?;
        }
        "--dram-refresh-policy" => {
            let value = required_value(flag, args.next())?;
            timing.set_refresh_policy(&value)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

pub(super) fn validate_dram_timing_options(
    dram_memory: bool,
    profile: CliDramMemoryProfile,
    profile_was_set: bool,
    timing_was_set: bool,
    low_power_was_set: bool,
    refresh_was_set: bool,
    timing_options: CliDramTimingOptions,
    low_power_options: CliDramLowPowerTimingOptions,
    refresh_options: CliDramRefreshTimingOptions,
) -> Result<
    (
        CliDramTiming,
        CliDramLowPowerTiming,
        Option<CliDramRefreshTiming>,
    ),
    Rem6CliError,
> {
    let refresh_policy_was_set = timing_options.refresh_policy_was_set();
    if profile_was_set && !dram_memory {
        return Err(Rem6CliError::DramMemoryProfileRequiresDramMemory);
    }
    if timing_was_set && !dram_memory {
        return Err(Rem6CliError::DramTimingRequiresDramMemory);
    }
    if low_power_was_set && !dram_memory {
        return Err(Rem6CliError::DramLowPowerTimingRequiresDramMemory);
    }
    if low_power_was_set && !profile.supports_low_power_timing() {
        return Err(Rem6CliError::DramLowPowerTimingRequiresLowPowerProfile {
            profile: profile.as_str().to_string(),
        });
    }
    if refresh_was_set && !dram_memory {
        return Err(Rem6CliError::DramRefreshTimingRequiresDramMemory);
    }
    if refresh_was_set && !profile.supports_refresh_timing() {
        return Err(Rem6CliError::DramRefreshTimingRequiresRefreshProfile {
            profile: profile.as_str().to_string(),
        });
    }
    if refresh_policy_was_set && !profile.supports_refresh_timing() {
        return Err(Rem6CliError::DramRefreshTimingRequiresRefreshProfile {
            profile: profile.as_str().to_string(),
        });
    }
    Ok((
        timing_options.timing()?,
        low_power_options.timing()?,
        refresh_options.timing()?,
    ))
}

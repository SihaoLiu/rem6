use crate::Rem6CliError;

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

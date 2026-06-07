use std::error::Error;
use std::fmt;

use crate::Tick;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Cycles(u64);

impl Cycles {
    pub const ZERO: Self = Self(0);

    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for Cycles {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClockError {
    ZeroPeriod,
    ZeroClockDivider,
    EmptyPerformancePoints,
    UnsortedPerformancePoints,
    InvalidPerformanceLevel { level: usize, count: usize },
    TickOverflow { period: Tick, cycles: Cycles },
    DerivedClockOverflow { period: Tick, divider: u64 },
}

impl fmt::Display for ClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroPeriod => write!(formatter, "clock period must be greater than zero"),
            Self::ZeroClockDivider => {
                write!(formatter, "derived clock divider must be greater than zero")
            }
            Self::EmptyPerformancePoints => {
                write!(formatter, "source clock domain requires performance points")
            }
            Self::UnsortedPerformancePoints => write!(
                formatter,
                "source clock periods must be sorted from fastest to slowest"
            ),
            Self::InvalidPerformanceLevel { level, count } => write!(
                formatter,
                "source clock performance level {level} is outside {count} configured points"
            ),
            Self::TickOverflow { period, cycles } => write!(
                formatter,
                "clock period {period} overflows tick conversion for {} cycles",
                cycles.get()
            ),
            Self::DerivedClockOverflow { period, divider } => write!(
                formatter,
                "clock period {period} overflows derived clock divider {divider}"
            ),
        }
    }
}

impl Error for ClockError {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ClockDomain {
    period: Tick,
}

impl ClockDomain {
    pub fn new(period: Tick) -> Result<Self, ClockError> {
        if period == 0 {
            return Err(ClockError::ZeroPeriod);
        }

        Ok(Self { period })
    }

    pub fn period(self) -> Tick {
        self.period
    }

    pub fn frequency_hz(self, tick_frequency_hz: u64) -> u64 {
        tick_frequency_hz / self.period
    }

    pub fn cycles_to_ticks(self, cycles: Cycles) -> Result<Tick, ClockError> {
        self.period
            .checked_mul(cycles.get())
            .ok_or(ClockError::TickOverflow {
                period: self.period,
                cycles,
            })
    }

    pub fn ticks_to_cycles_ceil(self, ticks: Tick) -> Cycles {
        if ticks == 0 {
            return Cycles::ZERO;
        }

        Cycles::new(((ticks - 1) / self.period) + 1)
    }

    pub fn clock_edge(self, now: Tick, cycles: Cycles) -> Result<Tick, ClockError> {
        let remainder = now % self.period;
        let aligned = if remainder == 0 {
            now
        } else {
            now.checked_add(self.period - remainder)
                .ok_or(ClockError::TickOverflow {
                    period: self.period,
                    cycles,
                })?
        };
        let offset = self.cycles_to_ticks(cycles)?;

        aligned.checked_add(offset).ok_or(ClockError::TickOverflow {
            period: self.period,
            cycles,
        })
    }

    pub fn derived(self, divider: u64) -> Result<Self, ClockError> {
        if divider == 0 {
            return Err(ClockError::ZeroClockDivider);
        }

        let period = self
            .period
            .checked_mul(divider)
            .ok_or(ClockError::DerivedClockOverflow {
                period: self.period,
                divider,
            })?;
        Self::new(period)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClockDomainId(u32);

impl ClockDomainId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceClockDomain {
    domain_id: Option<ClockDomainId>,
    periods: Vec<Tick>,
    performance_level: usize,
}

impl SourceClockDomain {
    pub fn new(
        domain_id: Option<ClockDomainId>,
        periods: Vec<Tick>,
        performance_level: usize,
    ) -> Result<Self, ClockError> {
        validate_performance_points(&periods)?;
        validate_performance_level(performance_level, periods.len())?;

        Ok(Self {
            domain_id,
            periods,
            performance_level,
        })
    }

    pub const fn domain_id(&self) -> Option<ClockDomainId> {
        self.domain_id
    }

    pub const fn performance_level(&self) -> usize {
        self.performance_level
    }

    pub fn performance_point_count(&self) -> usize {
        self.periods.len()
    }

    pub fn valid_performance_level(&self, level: usize) -> bool {
        level < self.performance_point_count()
    }

    pub fn period(&self) -> Tick {
        self.periods[self.performance_level]
    }

    pub fn period_at_performance_level(&self, level: usize) -> Result<Tick, ClockError> {
        validate_performance_level(level, self.periods.len())?;
        Ok(self.periods[level])
    }

    pub fn performance_points(&self) -> &[Tick] {
        &self.periods
    }

    pub fn clock_domain(&self) -> ClockDomain {
        ClockDomain::new(self.period()).expect("validated source clock period")
    }

    pub fn frequency_hz(&self, tick_frequency_hz: u64) -> Result<u64, ClockError> {
        Ok(self.clock_domain().frequency_hz(tick_frequency_hz))
    }

    pub fn set_performance_level(&mut self, level: usize) -> Result<bool, ClockError> {
        validate_performance_level(level, self.periods.len())?;
        if level == self.performance_level {
            return Ok(false);
        }
        self.performance_level = level;
        Ok(true)
    }

    pub fn derived_clock_domain(&self, divider: u64) -> Result<ClockDomain, ClockError> {
        self.clock_domain().derived(divider)
    }
}

fn validate_performance_points(periods: &[Tick]) -> Result<(), ClockError> {
    if periods.is_empty() {
        return Err(ClockError::EmptyPerformancePoints);
    }
    if periods.contains(&0) {
        return Err(ClockError::ZeroPeriod);
    }
    if !periods.windows(2).all(|window| window[0] <= window[1]) {
        return Err(ClockError::UnsortedPerformancePoints);
    }
    Ok(())
}

fn validate_performance_level(level: usize, count: usize) -> Result<(), ClockError> {
    if level >= count {
        return Err(ClockError::InvalidPerformanceLevel { level, count });
    }
    Ok(())
}

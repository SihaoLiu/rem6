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
    TickOverflow { period: Tick, cycles: Cycles },
}

impl fmt::Display for ClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroPeriod => write!(formatter, "clock period must be greater than zero"),
            Self::TickOverflow { period, cycles } => write!(
                formatter,
                "clock period {period} overflows tick conversion for {} cycles",
                cycles.get()
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
}

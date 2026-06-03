use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DuelingTeam {
    False,
    True,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DuelingRatio {
    numerator: u64,
    denominator: u64,
}

impl DuelingRatio {
    pub const fn new(numerator: u64, denominator: u64) -> Result<Self, DuelingMonitorError> {
        if denominator == 0 {
            return Err(DuelingMonitorError::ZeroThresholdDenominator);
        }
        if numerator == 0 || numerator >= denominator {
            return Err(DuelingMonitorError::ThresholdOutOfRange {
                numerator,
                denominator,
            });
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    pub const fn numerator(&self) -> u64 {
        self.numerator
    }

    pub const fn denominator(&self) -> u64 {
        self.denominator
    }

    fn value_below(&self, value: u64, max: u64) -> bool {
        (value as u128) * (self.denominator as u128) < (self.numerator as u128) * (max as u128)
    }

    fn value_at_least(&self, value: u64, max: u64) -> bool {
        (value as u128) * (self.denominator as u128) >= (self.numerator as u128) * (max as u128)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DuelingMonitorConfig {
    monitor_index: u8,
    constituency_size: usize,
    team_size: usize,
    selector_bits: u8,
    low_threshold: DuelingRatio,
    high_threshold: DuelingRatio,
}

impl DuelingMonitorConfig {
    pub fn new(
        monitor_index: u8,
        constituency_size: usize,
        team_size: usize,
        selector_bits: u8,
        low_threshold: DuelingRatio,
        high_threshold: DuelingRatio,
    ) -> Result<Self, DuelingMonitorError> {
        if monitor_index >= 64 {
            return Err(DuelingMonitorError::MonitorIndexOutOfRange {
                index: monitor_index,
            });
        }
        if team_size == 0 {
            return Err(DuelingMonitorError::ZeroTeamSize);
        }
        if constituency_size < team_size.saturating_mul(2) {
            return Err(DuelingMonitorError::ConstituencyTooSmall {
                constituency_size,
                team_size,
            });
        }
        if !(1..=63).contains(&selector_bits) {
            return Err(DuelingMonitorError::SelectorBitsOutOfRange {
                bits: selector_bits,
            });
        }
        if ratio_above(low_threshold, high_threshold) {
            return Err(DuelingMonitorError::LowThresholdAboveHigh);
        }

        Ok(Self {
            monitor_index,
            constituency_size,
            team_size,
            selector_bits,
            low_threshold,
            high_threshold,
        })
    }

    pub const fn monitor_index(&self) -> u8 {
        self.monitor_index
    }

    pub const fn monitor_id(&self) -> u64 {
        1u64 << self.monitor_index
    }

    pub const fn constituency_size(&self) -> usize {
        self.constituency_size
    }

    pub const fn team_size(&self) -> usize {
        self.team_size
    }

    pub const fn selector_bits(&self) -> u8 {
        self.selector_bits
    }

    pub const fn low_threshold(&self) -> DuelingRatio {
        self.low_threshold
    }

    pub const fn high_threshold(&self) -> DuelingRatio {
        self.high_threshold
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DuelingMonitorError {
    InvalidMonitorId {
        id: u64,
    },
    DuplicateSample {
        id: u64,
    },
    MonitorIndexOutOfRange {
        index: u8,
    },
    ZeroTeamSize,
    ConstituencyTooSmall {
        constituency_size: usize,
        team_size: usize,
    },
    SelectorBitsOutOfRange {
        bits: u8,
    },
    ZeroThresholdDenominator,
    ThresholdOutOfRange {
        numerator: u64,
        denominator: u64,
    },
    LowThresholdAboveHigh,
    SnapshotConfigMismatch {
        expected: Box<DuelingMonitorConfig>,
        actual: Box<DuelingMonitorConfig>,
    },
    SnapshotSelectorOutOfRange {
        selector: u64,
        max: u64,
    },
    SnapshotRegionCounterOutOfRange {
        region_counter: usize,
        constituency_size: usize,
    },
}

impl fmt::Display for DuelingMonitorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMonitorId { id } => {
                write!(formatter, "dueling monitor id {id:#x} is not a single-bit id")
            }
            Self::DuplicateSample { id } => {
                write!(formatter, "dueler is already a sample for id {id:#x}")
            }
            Self::MonitorIndexOutOfRange { index } => {
                write!(formatter, "dueling monitor index {index} is outside 0..64")
            }
            Self::ZeroTeamSize => write!(formatter, "dueling monitor team size is zero"),
            Self::ConstituencyTooSmall {
                constituency_size,
                team_size,
            } => write!(
                formatter,
                "dueling constituency size {constituency_size} cannot hold two teams of {team_size}"
            ),
            Self::SelectorBitsOutOfRange { bits } => write!(
                formatter,
                "dueling selector bit width {bits} is outside 1..=63"
            ),
            Self::ZeroThresholdDenominator => {
                write!(formatter, "dueling threshold denominator is zero")
            }
            Self::ThresholdOutOfRange {
                numerator,
                denominator,
            } => write!(
                formatter,
                "dueling threshold {numerator}/{denominator} is outside the open interval 0..1"
            ),
            Self::LowThresholdAboveHigh => {
                write!(formatter, "dueling low threshold is above high threshold")
            }
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "dueling monitor snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotSelectorOutOfRange { selector, max } => write!(
                formatter,
                "dueling monitor snapshot selector {selector} exceeds max {max}"
            ),
            Self::SnapshotRegionCounterOutOfRange {
                region_counter,
                constituency_size,
            } => write!(
                formatter,
                "dueling monitor snapshot region counter {region_counter} exceeds constituency size {constituency_size}"
            ),
        }
    }
}

impl Error for DuelingMonitorError {}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Dueler {
    sample_mask: u64,
    team_mask: u64,
}

impl Dueler {
    pub const fn new() -> Self {
        Self {
            sample_mask: 0,
            team_mask: 0,
        }
    }

    pub const fn sample_mask(&self) -> u64 {
        self.sample_mask
    }

    pub const fn team_mask(&self) -> u64 {
        self.team_mask
    }

    pub fn set_sample(&mut self, id: u64, team: DuelingTeam) -> Result<(), DuelingMonitorError> {
        if id.count_ones() != 1 {
            return Err(DuelingMonitorError::InvalidMonitorId { id });
        }
        if self.sample_mask & id != 0 {
            return Err(DuelingMonitorError::DuplicateSample { id });
        }

        self.sample_mask |= id;
        match team {
            DuelingTeam::False => {
                self.team_mask &= !id;
            }
            DuelingTeam::True => {
                self.team_mask |= id;
            }
        }
        Ok(())
    }

    pub const fn sample_team(&self, id: u64) -> Option<DuelingTeam> {
        if self.sample_mask & id == 0 {
            None
        } else if self.team_mask & id == 0 {
            Some(DuelingTeam::False)
        } else {
            Some(DuelingTeam::True)
        }
    }

    pub const fn snapshot(&self) -> DuelerSnapshot {
        DuelerSnapshot {
            sample_mask: self.sample_mask,
            team_mask: self.team_mask,
        }
    }

    pub const fn from_snapshot(snapshot: &DuelerSnapshot) -> Self {
        Self {
            sample_mask: snapshot.sample_mask,
            team_mask: snapshot.team_mask,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DuelerSnapshot {
    sample_mask: u64,
    team_mask: u64,
}

impl DuelerSnapshot {
    pub const fn sample_mask(&self) -> u64 {
        self.sample_mask
    }

    pub const fn team_mask(&self) -> u64 {
        self.team_mask
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DuelingMonitorSnapshot {
    config: DuelingMonitorConfig,
    selector: u64,
    region_counter: usize,
    winner: DuelingTeam,
}

impl DuelingMonitorSnapshot {
    pub const fn config(&self) -> &DuelingMonitorConfig {
        &self.config
    }

    pub const fn selector(&self) -> u64 {
        self.selector
    }

    pub const fn region_counter(&self) -> usize {
        self.region_counter
    }

    pub const fn winner(&self) -> DuelingTeam {
        self.winner
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DuelingMonitor {
    config: DuelingMonitorConfig,
    id: u64,
    selector: u64,
    region_counter: usize,
    winner: DuelingTeam,
}

impl DuelingMonitor {
    pub fn new(config: DuelingMonitorConfig) -> Self {
        let max_selector = selector_max(config.selector_bits());
        let selector = max_selector >> 1;
        let winner = if config.low_threshold().value_below(selector, max_selector) {
            DuelingTeam::False
        } else {
            DuelingTeam::True
        };
        let id = config.monitor_id();

        Self {
            config,
            id,
            selector,
            region_counter: 0,
            winner,
        }
    }

    pub const fn config(&self) -> &DuelingMonitorConfig {
        &self.config
    }

    pub const fn id(&self) -> u64 {
        self.id
    }

    pub const fn selector(&self) -> u64 {
        self.selector
    }

    pub const fn region_counter(&self) -> usize {
        self.region_counter
    }

    pub const fn winner(&self) -> DuelingTeam {
        self.winner
    }

    pub const fn sample_team(&self, dueler: &Dueler) -> Option<DuelingTeam> {
        dueler.sample_team(self.id)
    }

    pub fn init_entry(&mut self, dueler: &mut Dueler) -> Result<(), DuelingMonitorError> {
        if self.region_counter < self.config.team_size() {
            dueler.set_sample(self.id, DuelingTeam::False)?;
        } else if self.region_counter >= self.config.constituency_size() - self.config.team_size() {
            dueler.set_sample(self.id, DuelingTeam::True)?;
        }

        self.region_counter += 1;
        if self.region_counter >= self.config.constituency_size() {
            self.region_counter = 0;
        }
        Ok(())
    }

    pub fn sample(&mut self, dueler: &Dueler) {
        let max = selector_max(self.config.selector_bits());
        match self.sample_team(dueler) {
            Some(DuelingTeam::True) => {
                self.selector = self.selector.saturating_add(1).min(max);
                if self
                    .config
                    .high_threshold()
                    .value_at_least(self.selector, max)
                {
                    self.winner = DuelingTeam::True;
                }
            }
            Some(DuelingTeam::False) => {
                self.selector = self.selector.saturating_sub(1);
                if self.config.low_threshold().value_below(self.selector, max) {
                    self.winner = DuelingTeam::False;
                }
            }
            None => {}
        }
    }

    pub fn snapshot(&self) -> DuelingMonitorSnapshot {
        DuelingMonitorSnapshot {
            config: self.config.clone(),
            selector: self.selector,
            region_counter: self.region_counter,
            winner: self.winner,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &DuelingMonitorSnapshot,
    ) -> Result<(), DuelingMonitorError> {
        if snapshot.config() != &self.config {
            return Err(DuelingMonitorError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config().clone()),
            });
        }
        let max = selector_max(self.config.selector_bits());
        if snapshot.selector() > max {
            return Err(DuelingMonitorError::SnapshotSelectorOutOfRange {
                selector: snapshot.selector(),
                max,
            });
        }
        if snapshot.region_counter() >= self.config.constituency_size() {
            return Err(DuelingMonitorError::SnapshotRegionCounterOutOfRange {
                region_counter: snapshot.region_counter(),
                constituency_size: self.config.constituency_size(),
            });
        }

        self.selector = snapshot.selector();
        self.region_counter = snapshot.region_counter();
        self.winner = snapshot.winner();
        Ok(())
    }
}

fn ratio_above(left: DuelingRatio, right: DuelingRatio) -> bool {
    (left.numerator() as u128) * (right.denominator() as u128)
        > (right.numerator() as u128) * (left.denominator() as u128)
}

const fn selector_max(bits: u8) -> u64 {
    (1u64 << bits) - 1
}

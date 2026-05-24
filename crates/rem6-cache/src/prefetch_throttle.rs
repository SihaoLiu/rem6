use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedPrefetchThrottleConfig {
    control_percentage: u8,
}

impl QueuedPrefetchThrottleConfig {
    pub fn new(control_percentage: u8) -> Result<Self, QueuedPrefetchThrottleError> {
        if control_percentage > 100 {
            return Err(QueuedPrefetchThrottleError::ControlPercentageOutOfRange {
                control_percentage,
            });
        }

        Ok(Self { control_percentage })
    }

    pub const fn control_percentage(&self) -> u8 {
        self.control_percentage
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueuedPrefetchThrottleError {
    ControlPercentageOutOfRange {
        control_percentage: u8,
    },
    IssuedCounterOverflow {
        current: u64,
        delta: u64,
    },
    UsefulCounterOverflow {
        current: u64,
        delta: u64,
    },
    UsefulExceedsIssued {
        issued_prefetches: u64,
        useful_prefetches: u64,
    },
    SnapshotConfigMismatch {
        expected: QueuedPrefetchThrottleConfig,
        actual: QueuedPrefetchThrottleConfig,
    },
    SnapshotUsefulExceedsIssued {
        issued_prefetches: u64,
        useful_prefetches: u64,
    },
}

impl fmt::Display for QueuedPrefetchThrottleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ControlPercentageOutOfRange { control_percentage } => write!(
                formatter,
                "queued prefetch throttle control percentage {control_percentage} exceeds 100"
            ),
            Self::IssuedCounterOverflow { current, delta } => write!(
                formatter,
                "queued prefetch issued counter {current} overflows by {delta}"
            ),
            Self::UsefulCounterOverflow { current, delta } => write!(
                formatter,
                "queued prefetch useful counter {current} overflows by {delta}"
            ),
            Self::UsefulExceedsIssued {
                issued_prefetches,
                useful_prefetches,
            } => write!(
                formatter,
                "queued prefetch throttle has {useful_prefetches} useful prefetches for {issued_prefetches} issued prefetches"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "queued prefetch throttle snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotUsefulExceedsIssued {
                issued_prefetches,
                useful_prefetches,
            } => write!(
                formatter,
                "queued prefetch throttle snapshot has {useful_prefetches} useful prefetches for {issued_prefetches} issued prefetches"
            ),
        }
    }
}

impl Error for QueuedPrefetchThrottleError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedPrefetchThrottleSnapshot {
    config: QueuedPrefetchThrottleConfig,
    issued_prefetches: u64,
    useful_prefetches: u64,
}

impl QueuedPrefetchThrottleSnapshot {
    pub const fn config(&self) -> &QueuedPrefetchThrottleConfig {
        &self.config
    }

    pub const fn issued_prefetches(&self) -> u64 {
        self.issued_prefetches
    }

    pub const fn useful_prefetches(&self) -> u64 {
        self.useful_prefetches
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedPrefetchThrottle {
    config: QueuedPrefetchThrottleConfig,
    issued_prefetches: u64,
    useful_prefetches: u64,
}

impl QueuedPrefetchThrottle {
    pub fn new(config: QueuedPrefetchThrottleConfig) -> Self {
        Self {
            config,
            issued_prefetches: 0,
            useful_prefetches: 0,
        }
    }

    pub const fn config(&self) -> &QueuedPrefetchThrottleConfig {
        &self.config
    }

    pub const fn issued_prefetches(&self) -> u64 {
        self.issued_prefetches
    }

    pub const fn useful_prefetches(&self) -> u64 {
        self.useful_prefetches
    }

    pub fn max_permitted(&self, total_candidates: usize) -> usize {
        if total_candidates == 0 || self.issued_prefetches == 0 {
            return total_candidates;
        }

        let throttled = (total_candidates * usize::from(self.config.control_percentage())) / 100;
        let minimum = (total_candidates - throttled).max(1);
        let discretionary = total_candidates - minimum;
        minimum + discretionary * self.useful_prefetches as usize / self.issued_prefetches as usize
    }

    pub fn record_issued(&mut self, delta: u64) -> Result<(), QueuedPrefetchThrottleError> {
        self.issued_prefetches = self.issued_prefetches.checked_add(delta).ok_or(
            QueuedPrefetchThrottleError::IssuedCounterOverflow {
                current: self.issued_prefetches,
                delta,
            },
        )?;
        Ok(())
    }

    pub fn record_useful(&mut self, delta: u64) -> Result<(), QueuedPrefetchThrottleError> {
        let useful_prefetches = self.useful_prefetches.checked_add(delta).ok_or(
            QueuedPrefetchThrottleError::UsefulCounterOverflow {
                current: self.useful_prefetches,
                delta,
            },
        )?;
        if useful_prefetches > self.issued_prefetches {
            return Err(QueuedPrefetchThrottleError::UsefulExceedsIssued {
                issued_prefetches: self.issued_prefetches,
                useful_prefetches,
            });
        }

        self.useful_prefetches = useful_prefetches;
        Ok(())
    }

    pub fn snapshot(&self) -> QueuedPrefetchThrottleSnapshot {
        QueuedPrefetchThrottleSnapshot {
            config: self.config.clone(),
            issued_prefetches: self.issued_prefetches,
            useful_prefetches: self.useful_prefetches,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &QueuedPrefetchThrottleSnapshot,
    ) -> Result<(), QueuedPrefetchThrottleError> {
        if snapshot.config() != &self.config {
            return Err(QueuedPrefetchThrottleError::SnapshotConfigMismatch {
                expected: self.config.clone(),
                actual: snapshot.config().clone(),
            });
        }
        if snapshot.useful_prefetches() > snapshot.issued_prefetches() {
            return Err(QueuedPrefetchThrottleError::SnapshotUsefulExceedsIssued {
                issued_prefetches: snapshot.issued_prefetches(),
                useful_prefetches: snapshot.useful_prefetches(),
            });
        }

        self.issued_prefetches = snapshot.issued_prefetches();
        self.useful_prefetches = snapshot.useful_prefetches();
        Ok(())
    }
}

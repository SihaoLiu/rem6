use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum O3PipelineStage {
    Fetch,
    Decode,
    Rename,
    Iew,
    Commit,
}

impl fmt::Display for O3PipelineStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fetch => write!(formatter, "fetch"),
            Self::Decode => write!(formatter, "decode"),
            Self::Rename => write!(formatter, "rename"),
            Self::Iew => write!(formatter, "IEW"),
            Self::Commit => write!(formatter, "commit"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum O3PipelineError {
    ZeroDownstreamWidth {
        downstream: O3PipelineStage,
    },
    EarlyThresholdOverflow {
        downstream: O3PipelineStage,
        backward_signal_delay_cycles: u64,
        downstream_width: usize,
    },
}

impl fmt::Display for O3PipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDownstreamWidth { downstream } => {
                write!(formatter, "O3 {downstream} downstream width must be positive")
            }
            Self::EarlyThresholdOverflow {
                downstream,
                backward_signal_delay_cycles,
                downstream_width,
            } => write!(
                formatter,
                "O3 {downstream} unblock threshold overflows for delay {backward_signal_delay_cycles} and width {downstream_width}"
            ),
        }
    }
}

impl Error for O3PipelineError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3UnblockDecisionReason {
    SkidBufferAboveEarlyThreshold,
    SignalDelayCovered,
    SkidBufferEmpty,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3UnblockDecision {
    should_signal_unblock: bool,
    reason: O3UnblockDecisionReason,
    skid_entries: usize,
    early_unblock_threshold_entries: usize,
    cycles_to_drain: u64,
}

impl O3UnblockDecision {
    pub const fn should_signal_unblock(self) -> bool {
        self.should_signal_unblock
    }

    pub const fn reason(self) -> O3UnblockDecisionReason {
        self.reason
    }

    pub const fn skid_entries(self) -> usize {
        self.skid_entries
    }

    pub const fn early_unblock_threshold_entries(self) -> usize {
        self.early_unblock_threshold_entries
    }

    pub const fn cycles_to_drain(self) -> u64 {
        self.cycles_to_drain
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct O3UnblockPolicy {
    upstream: O3PipelineStage,
    downstream: O3PipelineStage,
    backward_signal_delay_cycles: u64,
    downstream_width: usize,
    early_unblock_threshold_entries: usize,
}

impl O3UnblockPolicy {
    pub fn new(
        upstream: O3PipelineStage,
        downstream: O3PipelineStage,
        backward_signal_delay_cycles: u64,
        downstream_width: usize,
    ) -> Result<Self, O3PipelineError> {
        if downstream_width == 0 {
            return Err(O3PipelineError::ZeroDownstreamWidth { downstream });
        }
        let threshold = (backward_signal_delay_cycles as u128) * (downstream_width as u128);
        if threshold > (usize::MAX as u128) {
            return Err(O3PipelineError::EarlyThresholdOverflow {
                downstream,
                backward_signal_delay_cycles,
                downstream_width,
            });
        }
        Ok(Self {
            upstream,
            downstream,
            backward_signal_delay_cycles,
            downstream_width,
            early_unblock_threshold_entries: threshold as usize,
        })
    }

    pub const fn upstream(&self) -> O3PipelineStage {
        self.upstream
    }

    pub const fn downstream(&self) -> O3PipelineStage {
        self.downstream
    }

    pub const fn backward_signal_delay_cycles(&self) -> u64 {
        self.backward_signal_delay_cycles
    }

    pub const fn downstream_width(&self) -> usize {
        self.downstream_width
    }

    pub const fn early_unblock_threshold_entries(&self) -> usize {
        self.early_unblock_threshold_entries
    }

    pub const fn empty_only_would_signal(&self, skid_entries: usize) -> bool {
        skid_entries == 0
    }

    pub fn decision(&self, skid_entries: usize) -> O3UnblockDecision {
        let cycles_to_drain = cycles_to_drain(skid_entries, self.downstream_width);
        let (should_signal_unblock, reason) = if skid_entries == 0 {
            (true, O3UnblockDecisionReason::SkidBufferEmpty)
        } else if skid_entries <= self.early_unblock_threshold_entries {
            (true, O3UnblockDecisionReason::SignalDelayCovered)
        } else {
            (
                false,
                O3UnblockDecisionReason::SkidBufferAboveEarlyThreshold,
            )
        };

        O3UnblockDecision {
            should_signal_unblock,
            reason,
            skid_entries,
            early_unblock_threshold_entries: self.early_unblock_threshold_entries,
            cycles_to_drain,
        }
    }
}

fn cycles_to_drain(skid_entries: usize, downstream_width: usize) -> u64 {
    if skid_entries == 0 {
        return 0;
    }
    ((skid_entries - 1) / downstream_width + 1) as u64
}

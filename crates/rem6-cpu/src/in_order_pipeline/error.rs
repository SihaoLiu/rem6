use std::error::Error;
use std::fmt;

use super::InOrderPipelineStage;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InOrderPipelineError {
    ZeroStageWidth {
        stage: InOrderPipelineStage,
    },
    DuplicateStageWidth {
        stage: InOrderPipelineStage,
    },
    MissingStageWidth {
        stage: InOrderPipelineStage,
    },
    DuplicateInFlightInstruction {
        sequence: u64,
    },
    StageAtCapacity {
        stage: InOrderPipelineStage,
        width: usize,
    },
    ExecuteWaitStageMismatch {
        sequence: u64,
        stage: InOrderPipelineStage,
        total_cycles: u64,
        remaining_cycles: u64,
    },
    MissingBranchRedirectInstruction {
        sequence: u64,
    },
    BranchRedirectStageMismatch {
        sequence: u64,
        expected: InOrderPipelineStage,
        actual: InOrderPipelineStage,
    },
    MissingBranchPredictionInstruction {
        sequence: u64,
    },
    BranchPredictionStageMismatch {
        sequence: u64,
        expected: InOrderPipelineStage,
        actual: InOrderPipelineStage,
    },
    MissingBranchPredictionRepairTarget {
        sequence: u64,
    },
    CycleCursorOverflow {
        cycle: u64,
    },
    OverlappingRunSummaryMerge {
        left_first_cycle: u64,
        left_last_cycle: u64,
        right_first_cycle: u64,
        right_last_cycle: u64,
    },
    InvalidCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidCheckpointMagic,
    UnsupportedCheckpointVersion {
        version: u8,
    },
    InvalidCheckpointStageCode {
        code: u8,
    },
    InvalidCheckpointExecuteWait {
        code: u8,
        total_cycles: u64,
        remaining_cycles: u64,
    },
    CheckpointValueTooLarge {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
}

impl fmt::Display for InOrderPipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroStageWidth { stage } => {
                write!(formatter, "in-order {stage} width must be positive")
            }
            Self::DuplicateStageWidth { stage } => {
                write!(
                    formatter,
                    "in-order {stage} width is configured more than once"
                )
            }
            Self::MissingStageWidth { stage } => {
                write!(formatter, "in-order {stage} width is not configured")
            }
            Self::DuplicateInFlightInstruction { sequence } => write!(
                formatter,
                "in-order pipeline has duplicate in-flight instruction sequence {sequence}"
            ),
            Self::StageAtCapacity { stage, width } => write!(
                formatter,
                "in-order {stage} stage is at its configured width {width}"
            ),
            Self::ExecuteWaitStageMismatch {
                sequence,
                stage,
                total_cycles,
                remaining_cycles,
            } => write!(
                formatter,
                "in-order instruction sequence {sequence} has invalid execute-wait progress {remaining_cycles}/{total_cycles} at stage {stage}"
            ),
            Self::MissingBranchRedirectInstruction { sequence } => write!(
                formatter,
                "in-order branch redirect instruction sequence {sequence} is not in flight"
            ),
            Self::BranchRedirectStageMismatch {
                sequence,
                expected,
                actual,
            } => write!(
                formatter,
                "in-order branch redirect instruction sequence {sequence} resolved at {expected}, but in-flight stage is {actual}"
            ),
            Self::MissingBranchPredictionInstruction { sequence } => write!(
                formatter,
                "in-order branch prediction instruction sequence {sequence} is not in flight"
            ),
            Self::BranchPredictionStageMismatch {
                sequence,
                expected,
                actual,
            } => write!(
                formatter,
                "in-order branch prediction instruction sequence {sequence} resolved at {expected}, but in-flight stage is {actual}"
            ),
            Self::MissingBranchPredictionRepairTarget { sequence } => write!(
                formatter,
                "in-order branch prediction instruction sequence {sequence} needs a repair target PC"
            ),
            Self::CycleCursorOverflow { cycle } => {
                write!(
                    formatter,
                    "in-order pipeline cycle cursor {cycle} cannot advance"
                )
            }
            Self::OverlappingRunSummaryMerge {
                left_first_cycle,
                left_last_cycle,
                right_first_cycle,
                right_last_cycle,
            } => write!(
                formatter,
                "in-order run summary windows overlap: left {left_first_cycle}..={left_last_cycle}, right {right_first_cycle}..={right_last_cycle}"
            ),
            Self::InvalidCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "in-order checkpoint payload has {actual} bytes; expected {expected}"
            ),
            Self::InvalidCheckpointMagic => {
                write!(formatter, "in-order checkpoint payload has invalid magic")
            }
            Self::UnsupportedCheckpointVersion { version } => write!(
                formatter,
                "in-order checkpoint payload version {version} is not supported"
            ),
            Self::InvalidCheckpointStageCode { code } => {
                write!(
                    formatter,
                    "in-order checkpoint payload has invalid stage code {code}"
                )
            }
            Self::InvalidCheckpointExecuteWait {
                code,
                total_cycles,
                remaining_cycles,
            } => write!(
                formatter,
                "in-order checkpoint payload has invalid execute-wait code {code} with progress {remaining_cycles}/{total_cycles}"
            ),
            Self::CheckpointValueTooLarge {
                field,
                value,
                maximum,
            } => write!(
                formatter,
                "in-order checkpoint {field} value {value} exceeds maximum {maximum}"
            ),
        }
    }
}

impl Error for InOrderPipelineError {}

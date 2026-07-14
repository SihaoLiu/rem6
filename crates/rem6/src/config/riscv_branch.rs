use rem6_cpu::{RiscvBranchPredictorKind, MAX_RISCV_BRANCH_LOOKAHEAD, MIN_RISCV_BRANCH_LOOKAHEAD};
use rem6_stats::PcCountPair;

use crate::Rem6CliError;

pub(super) fn parse_riscv_branch_predictor(value: &str) -> Option<RiscvBranchPredictorKind> {
    match value {
        "basic" => Some(RiscvBranchPredictorKind::Basic),
        "gshare" => Some(RiscvBranchPredictorKind::GShare),
        "bimode" => Some(RiscvBranchPredictorKind::BiMode),
        "tournament" => Some(RiscvBranchPredictorKind::Tournament),
        "tage-sc-l" => Some(RiscvBranchPredictorKind::TageScL),
        "multiperspective-perceptron" => Some(RiscvBranchPredictorKind::MultiperspectivePerceptron),
        _ => None,
    }
}

pub(super) fn parse_riscv_branch_lookahead(value: &str) -> Result<usize, Rem6CliError> {
    let lookahead = value
        .parse()
        .map_err(|_| Rem6CliError::InvalidRiscvBranchLookahead {
            value: value.to_string(),
        })?;
    validate_riscv_branch_lookahead(lookahead)
}

pub(super) fn validate_riscv_branch_lookahead(value: usize) -> Result<usize, Rem6CliError> {
    if !(MIN_RISCV_BRANCH_LOOKAHEAD..=MAX_RISCV_BRANCH_LOOKAHEAD).contains(&value) {
        return Err(Rem6CliError::InvalidRiscvBranchLookahead {
            value: value.to_string(),
        });
    }
    Ok(value)
}

pub(super) fn parse_riscv_pc_count_target(value: &str) -> Result<PcCountPair, Rem6CliError> {
    let Some((pc, count)) = value.split_once(':') else {
        return Err(Rem6CliError::InvalidRiscvPcCountTarget {
            value: value.to_string(),
        });
    };
    let Some(pc) = super::parse_number(pc) else {
        return Err(Rem6CliError::InvalidRiscvPcCountTarget {
            value: value.to_string(),
        });
    };
    let Some(count) = super::parse_positive_u64(count) else {
        return Err(Rem6CliError::InvalidRiscvPcCountTarget {
            value: value.to_string(),
        });
    };
    Ok(PcCountPair::new(pc, count))
}

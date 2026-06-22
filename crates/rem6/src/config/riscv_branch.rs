use rem6_cpu::RiscvBranchPredictorKind;
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

pub(super) const fn valid_riscv_branch_lookahead(value: usize) -> bool {
    matches!(value, 1 | 2)
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

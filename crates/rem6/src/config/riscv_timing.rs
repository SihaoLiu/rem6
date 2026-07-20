use rem6_cpu::{
    MAX_RISCV_O3_ISSUE_WIDTH, MAX_RISCV_O3_SCALAR_LIVE_WINDOW_DEPTH,
    MAX_RISCV_O3_SCALAR_MEMORY_DEPTH, MAX_RISCV_O3_WRITEBACK_WIDTH, MIN_RISCV_O3_ISSUE_WIDTH,
    MIN_RISCV_O3_SCALAR_LIVE_WINDOW_DEPTH, MIN_RISCV_O3_SCALAR_MEMORY_DEPTH,
    MIN_RISCV_O3_WRITEBACK_WIDTH,
};

use crate::Rem6CliError;

pub(crate) const DEFAULT_RISCV_IN_ORDER_WIDTH: usize = 1;
const MAX_RISCV_IN_ORDER_WIDTH: usize = u32::MAX as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvO3WindowDepths {
    scalar_memory: usize,
    scalar_live: usize,
}

impl RiscvO3WindowDepths {
    pub(crate) const fn scalar_memory(self) -> usize {
        self.scalar_memory
    }

    pub(crate) const fn scalar_live(self) -> usize {
        self.scalar_live
    }
}

pub(crate) fn resolve_riscv_o3_window_depths(
    branch_lookahead: usize,
    scalar_memory_depth: Option<usize>,
    scalar_live_window_depth: Option<usize>,
) -> Result<RiscvO3WindowDepths, Rem6CliError> {
    let scalar_memory = scalar_memory_depth.unwrap_or_else(|| branch_lookahead.saturating_add(1));
    validate_riscv_o3_scalar_memory_depth(scalar_memory)?;
    let scalar_live = scalar_live_window_depth.unwrap_or(scalar_memory);
    validate_riscv_o3_scalar_live_window_depth(scalar_live)?;
    if scalar_live < scalar_memory {
        return Err(Rem6CliError::RiscvO3ScalarLiveWindowDepthBelowMemoryDepth {
            scalar_memory_depth: scalar_memory,
            scalar_live_window_depth: scalar_live,
        });
    }
    Ok(RiscvO3WindowDepths {
        scalar_memory,
        scalar_live,
    })
}

pub(crate) fn parse_riscv_in_order_width(value: &str) -> Result<usize, Rem6CliError> {
    let width = value
        .parse()
        .map_err(|_| Rem6CliError::InvalidRiscvInOrderWidth {
            value: value.to_string(),
        })?;
    validate_riscv_in_order_width(width, value.to_string())
}

pub(crate) fn validate_riscv_in_order_width(
    width: usize,
    value: String,
) -> Result<usize, Rem6CliError> {
    if width == 0 || width > MAX_RISCV_IN_ORDER_WIDTH {
        return Err(Rem6CliError::InvalidRiscvInOrderWidth { value });
    }
    Ok(width)
}

pub(crate) fn parse_riscv_o3_scalar_memory_depth(value: &str) -> Result<usize, Rem6CliError> {
    let depth = value
        .parse()
        .map_err(|_| Rem6CliError::InvalidRiscvO3ScalarMemoryDepth {
            value: value.to_string(),
        })?;
    validate_riscv_o3_scalar_memory_depth(depth)
}

pub(crate) fn parse_riscv_o3_scalar_live_window_depth(value: &str) -> Result<usize, Rem6CliError> {
    let depth = value
        .parse()
        .map_err(|_| Rem6CliError::InvalidRiscvO3ScalarLiveWindowDepth {
            value: value.to_string(),
        })?;
    validate_riscv_o3_scalar_live_window_depth(depth)
}

pub(crate) fn parse_riscv_o3_issue_width(value: &str) -> Result<usize, Rem6CliError> {
    let width = value
        .parse()
        .map_err(|_| Rem6CliError::InvalidRiscvO3IssueWidth {
            value: value.to_string(),
        })?;
    validate_riscv_o3_issue_width(width, value.to_string())
}

pub(crate) fn parse_riscv_o3_writeback_width(value: &str) -> Result<usize, Rem6CliError> {
    let width = value
        .parse()
        .map_err(|_| Rem6CliError::InvalidRiscvO3WritebackWidth {
            value: value.to_string(),
        })?;
    validate_riscv_o3_writeback_width(width, value.to_string())
}

pub(crate) fn validate_optional_riscv_o3_issue_width(
    width: Option<usize>,
) -> Result<Option<usize>, Rem6CliError> {
    width
        .map(|width| validate_riscv_o3_issue_width(width, width.to_string()))
        .transpose()
}

pub(crate) fn validate_optional_riscv_o3_writeback_width(
    width: Option<usize>,
) -> Result<Option<usize>, Rem6CliError> {
    width
        .map(|width| validate_riscv_o3_writeback_width(width, width.to_string()))
        .transpose()
}

pub(crate) fn validate_optional_riscv_o3_widths(
    issue: Option<usize>,
    writeback: Option<usize>,
) -> Result<(Option<usize>, Option<usize>), Rem6CliError> {
    Ok((
        validate_optional_riscv_o3_issue_width(issue)?,
        validate_optional_riscv_o3_writeback_width(writeback)?,
    ))
}

pub(crate) fn apply_riscv_o3_width_flag(
    flag: &str,
    value: &str,
    issue: &mut Option<usize>,
    writeback: &mut Option<usize>,
) -> Result<(), Rem6CliError> {
    match flag {
        "--riscv-o3-issue-width" => *issue = Some(parse_riscv_o3_issue_width(value)?),
        "--riscv-o3-writeback-width" => {
            *writeback = Some(parse_riscv_o3_writeback_width(value)?);
        }
        _ => {
            return Err(Rem6CliError::UnknownFlag {
                flag: flag.to_string(),
            });
        }
    }
    Ok(())
}

fn validate_riscv_o3_scalar_memory_depth(depth: usize) -> Result<usize, Rem6CliError> {
    if !(MIN_RISCV_O3_SCALAR_MEMORY_DEPTH..=MAX_RISCV_O3_SCALAR_MEMORY_DEPTH).contains(&depth) {
        return Err(Rem6CliError::InvalidRiscvO3ScalarMemoryDepth {
            value: depth.to_string(),
        });
    }
    Ok(depth)
}

fn validate_riscv_o3_scalar_live_window_depth(depth: usize) -> Result<usize, Rem6CliError> {
    if !(MIN_RISCV_O3_SCALAR_LIVE_WINDOW_DEPTH..=MAX_RISCV_O3_SCALAR_LIVE_WINDOW_DEPTH)
        .contains(&depth)
    {
        return Err(Rem6CliError::InvalidRiscvO3ScalarLiveWindowDepth {
            value: depth.to_string(),
        });
    }
    Ok(depth)
}

fn validate_riscv_o3_issue_width(width: usize, value: String) -> Result<usize, Rem6CliError> {
    if !(MIN_RISCV_O3_ISSUE_WIDTH..=MAX_RISCV_O3_ISSUE_WIDTH).contains(&width) {
        return Err(Rem6CliError::InvalidRiscvO3IssueWidth { value });
    }
    Ok(width)
}

fn validate_riscv_o3_writeback_width(width: usize, value: String) -> Result<usize, Rem6CliError> {
    if !(MIN_RISCV_O3_WRITEBACK_WIDTH..=MAX_RISCV_O3_WRITEBACK_WIDTH).contains(&width) {
        return Err(Rem6CliError::InvalidRiscvO3WritebackWidth { value });
    }
    Ok(width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn riscv_o3_window_depth_resolution_covers_all_omission_combinations() {
        for (memory, live, expected) in [
            (None, None, (2, 2)),
            (Some(4), None, (4, 4)),
            (None, Some(6), (2, 6)),
            (Some(4), Some(8), (4, 8)),
        ] {
            let depths = resolve_riscv_o3_window_depths(1, memory, live).unwrap();

            assert_eq!((depths.scalar_memory(), depths.scalar_live()), expected);
        }
    }
}

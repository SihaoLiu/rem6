use rem6_system::ExecutionMode;

use crate::Rem6CliError;

pub(crate) const DEFAULT_RISCV_IN_ORDER_WIDTH: usize = 1;
const MAX_RISCV_IN_ORDER_WIDTH: usize = u32::MAX as usize;
const MAX_RISCV_O3_SCALAR_MEMORY_DEPTH: usize = 4;

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

pub(crate) fn validate_optional_riscv_o3_scalar_memory_depth(
    depth: Option<usize>,
) -> Result<Option<usize>, Rem6CliError> {
    depth.map(validate_riscv_o3_scalar_memory_depth).transpose()
}

fn validate_riscv_o3_scalar_memory_depth(depth: usize) -> Result<usize, Rem6CliError> {
    if !(1..=MAX_RISCV_O3_SCALAR_MEMORY_DEPTH).contains(&depth) {
        return Err(Rem6CliError::InvalidRiscvO3ScalarMemoryDepth {
            value: depth.to_string(),
        });
    }
    Ok(depth)
}

pub(crate) fn parse_execution_mode(value: &str) -> Option<ExecutionMode> {
    match value {
        "functional" => Some(ExecutionMode::Functional),
        "timing" => Some(ExecutionMode::Timing),
        "detailed" => Some(ExecutionMode::Detailed),
        _ => None,
    }
}

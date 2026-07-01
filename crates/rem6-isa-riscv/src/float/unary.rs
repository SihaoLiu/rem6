use crate::{FloatRegister, RiscvInstruction};

use super::{
    convert_precision, float_register_write, register_rounding_mode_is_supported,
    sqrt_double_is_exact, sqrt_exception_flags_double, sqrt_exception_flags_single,
    sqrt_single_is_exact,
};

pub(crate) fn float_register_write_unary(
    instruction: RiscvInstruction,
    value: u64,
    frm: u64,
) -> (FloatRegister, u64) {
    match instruction {
        RiscvInstruction::FloatConvertSFromD {
            rd, rounding_mode, ..
        } => (
            rd,
            convert_precision::double_to_single_register_write(
                value,
                rounding_mode
                    .resolve(frm)
                    .expect("unary float rounding mode is valid"),
            ),
        ),
        _ => float_register_write(instruction, value, 0),
    }
}

pub(crate) fn unary_register_rounding_mode_is_supported(
    instruction: RiscvInstruction,
    frm: u64,
    value: u64,
) -> bool {
    register_rounding_mode_is_supported(
        instruction,
        frm,
        unary_result_is_rounding_insensitive(instruction, value),
        unary_rounding_mode_is_implemented(instruction),
    )
}

pub(crate) fn unary_exception_flags(instruction: RiscvInstruction, value: u64, frm: u64) -> u64 {
    match instruction {
        RiscvInstruction::FloatSqrtS { .. } => sqrt_exception_flags_single(value),
        RiscvInstruction::FloatSqrtD { .. } => sqrt_exception_flags_double(value),
        RiscvInstruction::FloatConvertSFromD { rounding_mode, .. } => {
            rounding_mode.resolve(frm).map_or(0, |mode| {
                convert_precision::double_to_single_exception_flags(value, mode)
            })
        }
        _ => 0,
    }
}

fn unary_result_is_rounding_insensitive(instruction: RiscvInstruction, value: u64) -> bool {
    match instruction {
        RiscvInstruction::FloatSqrtS { .. } => sqrt_single_is_exact(value),
        RiscvInstruction::FloatSqrtD { .. } => sqrt_double_is_exact(value),
        RiscvInstruction::FloatConvertSFromD { .. } => {
            f64::from(f64::from_bits(value) as f32) == f64::from_bits(value)
        }
        _ => false,
    }
}

fn unary_rounding_mode_is_implemented(instruction: RiscvInstruction) -> bool {
    matches!(instruction, RiscvInstruction::FloatConvertSFromD { .. })
}

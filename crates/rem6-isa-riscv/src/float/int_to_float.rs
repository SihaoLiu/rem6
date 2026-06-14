use crate::{RiscvFloatRoundingMode, RiscvInstruction};

use super::FLOAT_FLAG_INEXACT;

pub(crate) fn rounding_mode_is_supported(
    instruction: RiscvInstruction,
    frm: u64,
    value: u64,
) -> bool {
    let Some(rounding_mode) = rounding_mode(instruction) else {
        return true;
    };

    match rounding_mode.resolve(frm) {
        Some(RiscvFloatRoundingMode::RoundNearestEven) => true,
        Some(_) => is_exact(instruction, value),
        None => false,
    }
}

pub(crate) fn exception_flags(instruction: RiscvInstruction, value: u64) -> u64 {
    match instruction {
        RiscvInstruction::FloatConvertSFromW { .. }
        | RiscvInstruction::FloatConvertSFromWu { .. }
        | RiscvInstruction::FloatConvertSFromL { .. }
        | RiscvInstruction::FloatConvertSFromLu { .. }
        | RiscvInstruction::FloatConvertDFromW { .. }
        | RiscvInstruction::FloatConvertDFromWu { .. }
        | RiscvInstruction::FloatConvertDFromL { .. }
        | RiscvInstruction::FloatConvertDFromLu { .. } => {
            if is_exact(instruction, value) {
                0
            } else {
                FLOAT_FLAG_INEXACT
            }
        }
        _ => 0,
    }
}

fn rounding_mode(instruction: RiscvInstruction) -> Option<RiscvFloatRoundingMode> {
    let rounding_mode = match instruction {
        RiscvInstruction::FloatConvertSFromW { rounding_mode, .. }
        | RiscvInstruction::FloatConvertSFromWu { rounding_mode, .. }
        | RiscvInstruction::FloatConvertSFromL { rounding_mode, .. }
        | RiscvInstruction::FloatConvertSFromLu { rounding_mode, .. }
        | RiscvInstruction::FloatConvertDFromW { rounding_mode, .. }
        | RiscvInstruction::FloatConvertDFromWu { rounding_mode, .. }
        | RiscvInstruction::FloatConvertDFromL { rounding_mode, .. }
        | RiscvInstruction::FloatConvertDFromLu { rounding_mode, .. } => rounding_mode,
        _ => return None,
    };
    Some(rounding_mode)
}

fn is_exact(instruction: RiscvInstruction, value: u64) -> bool {
    match instruction {
        RiscvInstruction::FloatConvertSFromW { .. } => signed_magnitude_fits_exact_bits(
            i64::from(value as u32 as i32),
            SINGLE_EXACT_INTEGER_BITS,
        ),
        RiscvInstruction::FloatConvertSFromWu { .. } => {
            unsigned_magnitude_fits_exact_bits(u64::from(value as u32), SINGLE_EXACT_INTEGER_BITS)
        }
        RiscvInstruction::FloatConvertSFromL { .. } => {
            signed_magnitude_fits_exact_bits(value as i64, SINGLE_EXACT_INTEGER_BITS)
        }
        RiscvInstruction::FloatConvertSFromLu { .. } => {
            unsigned_magnitude_fits_exact_bits(value, SINGLE_EXACT_INTEGER_BITS)
        }
        RiscvInstruction::FloatConvertDFromW { .. } => signed_magnitude_fits_exact_bits(
            i64::from(value as u32 as i32),
            DOUBLE_EXACT_INTEGER_BITS,
        ),
        RiscvInstruction::FloatConvertDFromWu { .. } => {
            unsigned_magnitude_fits_exact_bits(u64::from(value as u32), DOUBLE_EXACT_INTEGER_BITS)
        }
        RiscvInstruction::FloatConvertDFromL { .. } => {
            signed_magnitude_fits_exact_bits(value as i64, DOUBLE_EXACT_INTEGER_BITS)
        }
        RiscvInstruction::FloatConvertDFromLu { .. } => {
            unsigned_magnitude_fits_exact_bits(value, DOUBLE_EXACT_INTEGER_BITS)
        }
        _ => true,
    }
}

fn signed_magnitude_fits_exact_bits(value: i64, bits: u32) -> bool {
    unsigned_magnitude_fits_exact_bits(value.unsigned_abs(), bits)
}

fn unsigned_magnitude_fits_exact_bits(value: u64, bits: u32) -> bool {
    if value == 0 {
        return true;
    }

    let significant_bits = u64::BITS - value.leading_zeros() - value.trailing_zeros();
    significant_bits <= bits
}

const SINGLE_EXACT_INTEGER_BITS: u32 = 24;
const DOUBLE_EXACT_INTEGER_BITS: u32 = 53;

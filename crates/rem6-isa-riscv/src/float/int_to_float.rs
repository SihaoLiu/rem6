use crate::{FloatRegister, RiscvFloatRoundingMode, RiscvInstruction};

use super::{
    box_canonical_single, box_single, DOUBLE_EXP_MASK, DOUBLE_FRACTION_MASK, DOUBLE_SIGN_BIT,
    FLOAT_FLAG_INEXACT, SINGLE_EXP_MASK, SINGLE_FRACTION_MASK, SINGLE_SIGN_BIT,
};

pub(crate) fn rounding_mode_is_supported(
    instruction: RiscvInstruction,
    frm: u64,
    _value: u64,
) -> bool {
    let Some(rounding_mode) = rounding_mode(instruction) else {
        return true;
    };

    rounding_mode.resolve(frm).is_some()
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

pub(crate) fn register_write(
    instruction: RiscvInstruction,
    value: u64,
    frm: u64,
) -> (FloatRegister, u64) {
    match instruction {
        RiscvInstruction::FloatMoveSFromX { rd, .. } => (rd, box_single(value as u32)),
        RiscvInstruction::FloatConvertSFromW {
            rd, rounding_mode, ..
        } => (rd, convert_signed_word_to_single(value, rounding_mode, frm)),
        RiscvInstruction::FloatConvertSFromWu {
            rd, rounding_mode, ..
        } => (
            rd,
            convert_unsigned_word_to_single(value, rounding_mode, frm),
        ),
        RiscvInstruction::FloatConvertSFromL {
            rd, rounding_mode, ..
        } => (
            rd,
            convert_signed_doubleword_to_single(value, rounding_mode, frm),
        ),
        RiscvInstruction::FloatConvertSFromLu {
            rd, rounding_mode, ..
        } => (
            rd,
            convert_unsigned_doubleword_to_single(value, rounding_mode, frm),
        ),
        RiscvInstruction::FloatMoveDFromX { rd, .. } => (rd, value),
        RiscvInstruction::FloatConvertDFromW {
            rd, rounding_mode, ..
        } => (rd, convert_signed_word_to_double(value, rounding_mode, frm)),
        RiscvInstruction::FloatConvertDFromWu {
            rd, rounding_mode, ..
        } => (
            rd,
            convert_unsigned_word_to_double(value, rounding_mode, frm),
        ),
        RiscvInstruction::FloatConvertDFromL {
            rd, rounding_mode, ..
        } => (
            rd,
            convert_signed_doubleword_to_double(value, rounding_mode, frm),
        ),
        RiscvInstruction::FloatConvertDFromLu {
            rd, rounding_mode, ..
        } => (
            rd,
            convert_unsigned_doubleword_to_double(value, rounding_mode, frm),
        ),
        _ => unreachable!("non-integer-convert instruction dispatched to float register write"),
    }
}

pub(super) fn unsigned_word_to_single_bits(
    value: u32,
    rounding_mode: RiscvFloatRoundingMode,
) -> (u32, u64) {
    let value = u64::from(value);
    let bits = round_unsigned_integer_to_single(value, rounding_mode).to_bits();
    let flags = if unsigned_magnitude_fits_exact_bits(value, SINGLE_EXACT_INTEGER_BITS) {
        0
    } else {
        FLOAT_FLAG_INEXACT
    };
    (bits, flags)
}

pub(super) fn signed_word_to_single_bits(
    value: u32,
    rounding_mode: RiscvFloatRoundingMode,
) -> (u32, u64) {
    let value = i64::from(value as i32);
    let bits = round_signed_integer_to_single(value, rounding_mode).to_bits();
    let flags = if signed_magnitude_fits_exact_bits(value, SINGLE_EXACT_INTEGER_BITS) {
        0
    } else {
        FLOAT_FLAG_INEXACT
    };
    (bits, flags)
}

pub(super) fn unsigned_doubleword_to_double_bits(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> (u64, u64) {
    let bits = round_unsigned_integer_to_double(value, rounding_mode).to_bits();
    let flags = if unsigned_magnitude_fits_exact_bits(value, DOUBLE_EXACT_INTEGER_BITS) {
        0
    } else {
        FLOAT_FLAG_INEXACT
    };
    (bits, flags)
}

pub(super) fn signed_doubleword_to_double_bits(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> (u64, u64) {
    let value = value as i64;
    let bits = round_signed_integer_to_double(value, rounding_mode).to_bits();
    let flags = if signed_magnitude_fits_exact_bits(value, DOUBLE_EXACT_INTEGER_BITS) {
        0
    } else {
        FLOAT_FLAG_INEXACT
    };
    (bits, flags)
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

fn convert_signed_word_to_single(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
    frm: u64,
) -> u64 {
    let value = i64::from(value as u32 as i32);
    box_canonical_single(round_signed_integer_to_single(
        value,
        resolved_rounding_mode(rounding_mode, frm),
    ))
}

fn convert_unsigned_word_to_single(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
    frm: u64,
) -> u64 {
    let value = u64::from(value as u32);
    box_canonical_single(round_unsigned_integer_to_single(
        value,
        resolved_rounding_mode(rounding_mode, frm),
    ))
}

fn convert_signed_doubleword_to_single(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
    frm: u64,
) -> u64 {
    box_canonical_single(round_signed_integer_to_single(
        value as i64,
        resolved_rounding_mode(rounding_mode, frm),
    ))
}

fn convert_unsigned_doubleword_to_single(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
    frm: u64,
) -> u64 {
    box_canonical_single(round_unsigned_integer_to_single(
        value,
        resolved_rounding_mode(rounding_mode, frm),
    ))
}

fn convert_signed_word_to_double(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
    frm: u64,
) -> u64 {
    let value = i64::from(value as u32 as i32);
    round_signed_integer_to_double(value, resolved_rounding_mode(rounding_mode, frm)).to_bits()
}

fn convert_unsigned_word_to_double(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
    frm: u64,
) -> u64 {
    let value = u64::from(value as u32);
    round_unsigned_integer_to_double(value, resolved_rounding_mode(rounding_mode, frm)).to_bits()
}

fn convert_signed_doubleword_to_double(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
    frm: u64,
) -> u64 {
    round_signed_integer_to_double(value as i64, resolved_rounding_mode(rounding_mode, frm))
        .to_bits()
}

fn convert_unsigned_doubleword_to_double(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
    frm: u64,
) -> u64 {
    round_unsigned_integer_to_double(value, resolved_rounding_mode(rounding_mode, frm)).to_bits()
}

fn resolved_rounding_mode(
    rounding_mode: RiscvFloatRoundingMode,
    frm: u64,
) -> RiscvFloatRoundingMode {
    rounding_mode
        .resolve(frm)
        .expect("integer-to-float rounding mode is valid")
}

fn round_signed_integer_to_single(value: i64, rounding_mode: RiscvFloatRoundingMode) -> f32 {
    adjust_integral_single(value as f32, i128::from(value), rounding_mode)
}

fn round_unsigned_integer_to_single(value: u64, rounding_mode: RiscvFloatRoundingMode) -> f32 {
    adjust_integral_single(value as f32, i128::from(value), rounding_mode)
}

fn round_signed_integer_to_double(value: i64, rounding_mode: RiscvFloatRoundingMode) -> f64 {
    adjust_integral_double(value as f64, i128::from(value), rounding_mode)
}

fn round_unsigned_integer_to_double(value: u64, rounding_mode: RiscvFloatRoundingMode) -> f64 {
    adjust_integral_double(value as f64, i128::from(value), rounding_mode)
}

fn adjust_integral_single(
    nearest: f32,
    target: i128,
    rounding_mode: RiscvFloatRoundingMode,
) -> f32 {
    let nearest_integer = integral_single_to_i128(nearest);
    if nearest_integer == target {
        return nearest;
    }

    match rounding_mode {
        RiscvFloatRoundingMode::RoundNearestEven => nearest,
        RiscvFloatRoundingMode::RoundTowardZero => {
            if (target >= 0 && nearest_integer > target) || (target < 0 && nearest_integer < target)
            {
                step_toward_target_single(nearest, target)
            } else {
                nearest
            }
        }
        RiscvFloatRoundingMode::RoundDown => {
            if nearest_integer > target {
                nearest.next_down()
            } else {
                nearest
            }
        }
        RiscvFloatRoundingMode::RoundUp => {
            if nearest_integer < target {
                nearest.next_up()
            } else {
                nearest
            }
        }
        RiscvFloatRoundingMode::RoundNearestMaxMagnitude => {
            round_nearest_max_magnitude_single(nearest, nearest_integer, target)
        }
        RiscvFloatRoundingMode::Dynamic => unreachable!("dynamic rounding mode must be resolved"),
    }
}

fn adjust_integral_double(
    nearest: f64,
    target: i128,
    rounding_mode: RiscvFloatRoundingMode,
) -> f64 {
    let nearest_integer = integral_double_to_i128(nearest);
    if nearest_integer == target {
        return nearest;
    }

    match rounding_mode {
        RiscvFloatRoundingMode::RoundNearestEven => nearest,
        RiscvFloatRoundingMode::RoundTowardZero => {
            if (target >= 0 && nearest_integer > target) || (target < 0 && nearest_integer < target)
            {
                step_toward_target_double(nearest, target)
            } else {
                nearest
            }
        }
        RiscvFloatRoundingMode::RoundDown => {
            if nearest_integer > target {
                nearest.next_down()
            } else {
                nearest
            }
        }
        RiscvFloatRoundingMode::RoundUp => {
            if nearest_integer < target {
                nearest.next_up()
            } else {
                nearest
            }
        }
        RiscvFloatRoundingMode::RoundNearestMaxMagnitude => {
            round_nearest_max_magnitude_double(nearest, nearest_integer, target)
        }
        RiscvFloatRoundingMode::Dynamic => unreachable!("dynamic rounding mode must be resolved"),
    }
}

fn step_toward_target_single(nearest: f32, target: i128) -> f32 {
    if target >= 0 {
        nearest.next_down()
    } else {
        nearest.next_up()
    }
}

fn step_toward_target_double(nearest: f64, target: i128) -> f64 {
    if target >= 0 {
        nearest.next_down()
    } else {
        nearest.next_up()
    }
}

fn round_nearest_max_magnitude_single(nearest: f32, nearest_integer: i128, target: i128) -> f32 {
    let other = if nearest_integer < target {
        nearest.next_up()
    } else {
        nearest.next_down()
    };
    let other_integer = integral_single_to_i128(other);
    if abs_diff_i128(other_integer, target) == abs_diff_i128(nearest_integer, target)
        && other_integer.abs() > nearest_integer.abs()
    {
        other
    } else {
        nearest
    }
}

fn round_nearest_max_magnitude_double(nearest: f64, nearest_integer: i128, target: i128) -> f64 {
    let other = if nearest_integer < target {
        nearest.next_up()
    } else {
        nearest.next_down()
    };
    let other_integer = integral_double_to_i128(other);
    if abs_diff_i128(other_integer, target) == abs_diff_i128(nearest_integer, target)
        && other_integer.abs() > nearest_integer.abs()
    {
        other
    } else {
        nearest
    }
}

fn abs_diff_i128(lhs: i128, rhs: i128) -> u128 {
    lhs.abs_diff(rhs)
}

fn integral_single_to_i128(value: f32) -> i128 {
    let bits = value.to_bits();
    let exponent_bits = (bits & SINGLE_EXP_MASK) >> 23;
    if exponent_bits == 0 {
        return 0;
    }

    let exponent = exponent_bits as i32 - 127;
    let significand = u128::from((bits & SINGLE_FRACTION_MASK) | (1 << 23));
    let magnitude = shift_integral_significand(significand, exponent, 23);
    if bits & SINGLE_SIGN_BIT != 0 {
        -(magnitude as i128)
    } else {
        magnitude as i128
    }
}

fn integral_double_to_i128(value: f64) -> i128 {
    let bits = value.to_bits();
    let exponent_bits = (bits & DOUBLE_EXP_MASK) >> 52;
    if exponent_bits == 0 {
        return 0;
    }

    let exponent = exponent_bits as i32 - 1023;
    let significand = u128::from((bits & DOUBLE_FRACTION_MASK) | (1_u64 << 52));
    let magnitude = shift_integral_significand(significand, exponent, 52);
    if bits & DOUBLE_SIGN_BIT != 0 {
        -(magnitude as i128)
    } else {
        magnitude as i128
    }
}

fn shift_integral_significand(significand: u128, exponent: i32, fraction_bits: i32) -> u128 {
    if exponent >= fraction_bits {
        significand << (exponent - fraction_bits)
    } else {
        significand >> (fraction_bits - exponent)
    }
}

use std::cmp::Ordering;

use crate::{FloatRegister, RiscvFloatRoundingMode, RiscvInstruction};

use super::{
    box_single, convert_precision, float_register_write, is_infinity_double, is_infinity_single,
    is_nan_double, is_nan_single, is_negative_nonzero_non_nan_double,
    is_negative_nonzero_non_nan_single, register_rounding_mode_is_supported,
    single_to_double_exception_flags, sqrt_exception_flags_double, sqrt_exception_flags_single,
    sqrt_single_bits, sqrt_single_is_exact, unbox_single, DEFAULT_NAN_DOUBLE, DOUBLE_EXP_MASK,
    DOUBLE_FRACTION_MASK, FLOAT_FLAG_INEXACT,
};

pub(crate) fn float_register_write_unary(
    instruction: RiscvInstruction,
    value: u64,
    frm: u64,
) -> (FloatRegister, u64) {
    match instruction {
        RiscvInstruction::FloatSqrtS {
            rd, rounding_mode, ..
        } => (
            rd,
            sqrt_single_register_write(
                value,
                rounding_mode
                    .resolve(frm)
                    .expect("unary float rounding mode is valid"),
            ),
        ),
        RiscvInstruction::FloatSqrtD {
            rd, rounding_mode, ..
        } => (
            rd,
            sqrt_double_register_write(
                value,
                rounding_mode
                    .resolve(frm)
                    .expect("unary float rounding mode is valid"),
            ),
        ),
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
        RiscvInstruction::FloatSqrtS { .. } => {
            sqrt_exception_flags_single(value) | sqrt_single_inexact_flag(value)
        }
        RiscvInstruction::FloatSqrtD { .. } => {
            sqrt_exception_flags_double(value) | sqrt_double_inexact_flag(value)
        }
        RiscvInstruction::FloatConvertDFromS { .. } => single_to_double_exception_flags(value),
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
        RiscvInstruction::FloatSqrtD { .. } => !double_sqrt_is_inexact(value),
        RiscvInstruction::FloatConvertSFromD { .. } => {
            f64::from(f64::from_bits(value) as f32) == f64::from_bits(value)
        }
        _ => false,
    }
}

fn unary_rounding_mode_is_implemented(instruction: RiscvInstruction) -> bool {
    matches!(
        instruction,
        RiscvInstruction::FloatSqrtS { .. }
            | RiscvInstruction::FloatSqrtD { .. }
            | RiscvInstruction::FloatConvertSFromD { .. }
    )
}

fn sqrt_single_register_write(value: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let value = unbox_single(value);
    if !single_sqrt_needs_rounding(value) {
        return box_single(sqrt_single_bits(value));
    }

    let nearest = f32::from_bits(value).sqrt();
    let rounded = round_single_sqrt(value, nearest, rounding_mode);
    box_single(rounded.to_bits())
}

fn sqrt_double_register_write(value: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    if !double_sqrt_needs_rounding(value) {
        return native_sqrt_double(value);
    }

    let nearest = f64::from_bits(value).sqrt();
    round_double_sqrt(value, nearest, rounding_mode).to_bits()
}

fn sqrt_single_inexact_flag(value: u64) -> u64 {
    let unboxed = unbox_single(value);
    if single_sqrt_needs_rounding(unboxed) && !sqrt_single_is_exact(value) {
        FLOAT_FLAG_INEXACT
    } else {
        0
    }
}

fn sqrt_double_inexact_flag(value: u64) -> u64 {
    if double_sqrt_is_inexact(value) {
        FLOAT_FLAG_INEXACT
    } else {
        0
    }
}

fn single_sqrt_needs_rounding(value: u32) -> bool {
    !(is_nan_single(value)
        || is_infinity_single(value)
        || is_negative_nonzero_non_nan_single(value))
}

fn double_sqrt_needs_rounding(value: u64) -> bool {
    !(is_nan_double(value)
        || is_infinity_double(value)
        || is_negative_nonzero_non_nan_double(value))
}

fn double_sqrt_is_inexact(value: u64) -> bool {
    if !double_sqrt_needs_rounding(value) {
        return false;
    }
    let nearest = f64::from_bits(value).sqrt().to_bits();
    compare_double_square_to_value(nearest, value) != Ordering::Equal
}

fn native_sqrt_double(value: u64) -> u64 {
    let result = f64::from_bits(value).sqrt().to_bits();
    if is_nan_double(result) {
        DEFAULT_NAN_DOUBLE
    } else {
        result
    }
}

fn round_single_sqrt(value: u32, nearest: f32, rounding_mode: RiscvFloatRoundingMode) -> f32 {
    let ordering = compare_single_square_to_value(nearest, value);
    if ordering == Ordering::Equal {
        return nearest;
    }

    match rounding_mode {
        RiscvFloatRoundingMode::RoundNearestEven => nearest,
        RiscvFloatRoundingMode::RoundTowardZero | RiscvFloatRoundingMode::RoundDown => {
            if ordering == Ordering::Greater {
                nearest.next_down()
            } else {
                nearest
            }
        }
        RiscvFloatRoundingMode::RoundUp => {
            if ordering == Ordering::Less {
                nearest.next_up()
            } else {
                nearest
            }
        }
        RiscvFloatRoundingMode::RoundNearestMaxMagnitude => {
            round_single_sqrt_nearest_max_magnitude(value, nearest, ordering)
        }
        RiscvFloatRoundingMode::Dynamic => unreachable!("dynamic rounding mode must be resolved"),
    }
}

fn round_double_sqrt(value: u64, nearest: f64, rounding_mode: RiscvFloatRoundingMode) -> f64 {
    let ordering = compare_double_square_to_value(nearest.to_bits(), value);
    if ordering == Ordering::Equal {
        return nearest;
    }

    match rounding_mode {
        RiscvFloatRoundingMode::RoundNearestEven => nearest,
        RiscvFloatRoundingMode::RoundTowardZero | RiscvFloatRoundingMode::RoundDown => {
            if ordering == Ordering::Greater {
                nearest.next_down()
            } else {
                nearest
            }
        }
        RiscvFloatRoundingMode::RoundUp => {
            if ordering == Ordering::Less {
                nearest.next_up()
            } else {
                nearest
            }
        }
        RiscvFloatRoundingMode::RoundNearestMaxMagnitude => {
            round_double_sqrt_nearest_max_magnitude(value, nearest, ordering)
        }
        RiscvFloatRoundingMode::Dynamic => unreachable!("dynamic rounding mode must be resolved"),
    }
}

fn round_single_sqrt_nearest_max_magnitude(value: u32, nearest: f32, ordering: Ordering) -> f32 {
    let other = if ordering == Ordering::Less {
        nearest.next_up()
    } else {
        nearest.next_down()
    };
    let lower = nearest.min(other);
    let upper = nearest.max(other);
    let midpoint = (f64::from(lower) + f64::from(upper)) / 2.0;
    if f64::from(f32::from_bits(value)) >= midpoint * midpoint {
        upper
    } else {
        lower
    }
}

fn round_double_sqrt_nearest_max_magnitude(value: u64, nearest: f64, ordering: Ordering) -> f64 {
    let other = if ordering == Ordering::Less {
        nearest.next_up()
    } else {
        nearest.next_down()
    };
    let lower = nearest.min(other);
    let upper = nearest.max(other);
    if compare_double_midpoint_square_to_value(lower.to_bits(), upper.to_bits(), value)
        != Ordering::Greater
    {
        upper
    } else {
        lower
    }
}

fn compare_single_square_to_value(candidate: f32, value: u32) -> Ordering {
    let candidate = f64::from(candidate);
    let square = candidate * candidate;
    square
        .partial_cmp(&f64::from(f32::from_bits(value)))
        .expect("finite single sqrt comparison")
}

fn compare_double_square_to_value(candidate: u64, value: u64) -> Ordering {
    let (_negative, candidate_significand, candidate_shift) =
        double_components(candidate).expect("finite double sqrt candidate");
    let (_negative, value_significand, value_shift) =
        double_components(value).expect("finite double sqrt input");
    compare_scaled_positive(
        candidate_significand * candidate_significand,
        candidate_shift * 2,
        value_significand,
        value_shift,
    )
}

fn compare_double_midpoint_square_to_value(lower: u64, upper: u64, value: u64) -> Ordering {
    let (_negative, lower_significand, lower_shift) =
        double_components(lower).expect("finite lower sqrt candidate");
    let (_negative, upper_significand, upper_shift) =
        double_components(upper).expect("finite upper sqrt candidate");
    let target_shift = lower_shift.min(upper_shift);
    let lower_scaled = lower_significand << (lower_shift - target_shift) as u32;
    let upper_scaled = upper_significand << (upper_shift - target_shift) as u32;
    let midpoint_significand = lower_scaled + upper_scaled;
    let (_negative, value_significand, value_shift) =
        double_components(value).expect("finite double sqrt input");
    compare_scaled_positive(
        midpoint_significand * midpoint_significand,
        target_shift * 2 - 2,
        value_significand,
        value_shift,
    )
}

fn compare_scaled_positive(
    mut lhs_significand: u128,
    mut lhs_shift: i32,
    mut rhs_significand: u128,
    mut rhs_shift: i32,
) -> Ordering {
    if lhs_significand == 0 || rhs_significand == 0 {
        return lhs_significand.cmp(&rhs_significand);
    }
    while lhs_significand & 1 == 0 {
        lhs_significand >>= 1;
        lhs_shift += 1;
    }
    while rhs_significand & 1 == 0 {
        rhs_significand >>= 1;
        rhs_shift += 1;
    }

    let lhs_high_bit = bit_width(lhs_significand) as i32 - 1 + lhs_shift;
    let rhs_high_bit = bit_width(rhs_significand) as i32 - 1 + rhs_shift;
    if lhs_high_bit != rhs_high_bit {
        return lhs_high_bit.cmp(&rhs_high_bit);
    }

    let target_shift = lhs_shift.min(rhs_shift);
    let lhs_scaled = lhs_significand << (lhs_shift - target_shift) as u32;
    let rhs_scaled = rhs_significand << (rhs_shift - target_shift) as u32;
    lhs_scaled.cmp(&rhs_scaled)
}

fn double_components(value: u64) -> Option<(bool, u128, i32)> {
    if value & DOUBLE_EXP_MASK == DOUBLE_EXP_MASK {
        return None;
    }
    let negative = value >> 63 != 0;
    let exponent = (value & DOUBLE_EXP_MASK) >> 52;
    let fraction = value & DOUBLE_FRACTION_MASK;
    if exponent == 0 {
        return Some((negative, u128::from(fraction), -1074));
    }
    Some((
        negative,
        u128::from((1_u64 << 52) | fraction),
        exponent as i32 - 1023 - 52,
    ))
}

fn bit_width(value: u128) -> u32 {
    u128::BITS - value.leading_zeros()
}

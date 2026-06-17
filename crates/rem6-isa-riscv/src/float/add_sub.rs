use crate::RiscvFloatRoundingMode;

use super::{
    box_canonical_single, has_single_sign, is_infinity_single, is_signaling_nan_single,
    is_zero_single, unbox_single, FLOAT_FLAG_INEXACT, FLOAT_FLAG_INVALID, FLOAT_FLAG_OVERFLOW,
    SINGLE_EXP_MASK, SINGLE_FRACTION_MASK, SINGLE_SIGN_BIT,
};

const SINGLE_HIDDEN_BIT: u32 = 1 << 23;
const SINGLE_MAX_MANTISSA: u128 = (1 << 24) - 1;
const SINGLE_MIN_NORMAL_SHIFT: i32 = -149;
const SINGLE_MAX_NORMAL_SHIFT: i32 = 104;

pub(super) fn add_directed_rounding_is_supported(lhs: u64, rhs: u64) -> bool {
    wide_sum_value(lhs, rhs, false).is_some()
}

pub(super) fn sub_directed_rounding_is_supported(lhs: u64, rhs: u64) -> bool {
    wide_sum_value(lhs, rhs, true).is_some()
}

pub(super) fn add_register_write(lhs: u64, rhs: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    register_write(lhs, rhs, rounding_mode, false)
}

pub(super) fn sub_register_write(lhs: u64, rhs: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    register_write(lhs, rhs, rounding_mode, true)
}

pub(super) fn add_exception_flags(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    exception_flags(lhs, rhs, rounding_mode, false)
}

pub(super) fn sub_exception_flags(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    exception_flags(lhs, rhs, rounding_mode, true)
}

pub(super) fn exact_finite_single_bits(
    lhs: u32,
    rhs: u32,
    rounding_mode: RiscvFloatRoundingMode,
    subtract: bool,
) -> Option<u32> {
    let rhs = if subtract { rhs ^ SINGLE_SIGN_BIT } else { rhs };
    if !is_finite(lhs) || !is_finite(rhs) || !finite_sum_is_exact(lhs, rhs) {
        return None;
    }
    let exact = f64::from(f32::from_bits(lhs)) + f64::from(f32::from_bits(rhs));
    Some(round_wide_sum(lhs, rhs, exact, rounding_mode).to_bits())
}

fn register_write(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
    subtract: bool,
) -> u64 {
    let lhs = unbox_single(lhs);
    let rhs = effective_rhs_bits(rhs, subtract);
    match wide_sum_value_from_single_bits(lhs, rhs) {
        Some(exact) => box_canonical_single(round_wide_sum(lhs, rhs, exact, rounding_mode)),
        None => native_register_write(lhs, rhs),
    }
}

fn exception_flags(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
    subtract: bool,
) -> u64 {
    let lhs = unbox_single(lhs);
    let rhs = effective_rhs_bits(rhs, subtract);
    if is_signaling_nan_single(lhs) || is_signaling_nan_single(rhs) || opposite_infinities(lhs, rhs)
    {
        return FLOAT_FLAG_INVALID;
    }
    if !is_finite(lhs) || !is_finite(rhs) {
        return 0;
    }
    if sum_overflows(lhs, rhs, rounding_mode) {
        return FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT;
    }
    if !finite_sum_is_exact(lhs, rhs) {
        FLOAT_FLAG_INEXACT
    } else {
        0
    }
}

fn native_sum_overflows(lhs: u32, rhs: u32) -> bool {
    (f32::from_bits(lhs) + f32::from_bits(rhs)).is_infinite()
}

fn sum_overflows(lhs: u32, rhs: u32, rounding_mode: RiscvFloatRoundingMode) -> bool {
    if native_sum_overflows(lhs, rhs) {
        return true;
    }
    wide_sum_value_from_single_bits(lhs, rhs)
        .map(|exact| round_wide_sum(lhs, rhs, exact, rounding_mode).is_infinite())
        .unwrap_or(false)
}

fn wide_sum_value(lhs: u64, rhs: u64, subtract: bool) -> Option<f64> {
    let lhs = unbox_single(lhs);
    let rhs = effective_rhs_bits(rhs, subtract);
    wide_sum_value_from_single_bits(lhs, rhs)
}

fn wide_sum_value_from_single_bits(lhs: u32, rhs: u32) -> Option<f64> {
    if !is_finite(lhs) || !is_finite(rhs) {
        return None;
    }
    if is_zero_single(lhs) || is_zero_single(rhs) {
        return Some(f64::from(f32::from_bits(lhs)) + f64::from(f32::from_bits(rhs)));
    }
    let lhs_exponent = normal_exponent(lhs)?;
    let rhs_exponent = normal_exponent(rhs)?;
    if lhs_exponent.abs_diff(rhs_exponent) > 29 {
        return None;
    }
    Some(f64::from(f32::from_bits(lhs)) + f64::from(f32::from_bits(rhs)))
}

fn round_wide_sum(lhs: u32, rhs: u32, exact: f64, rounding_mode: RiscvFloatRoundingMode) -> f32 {
    if exact == 0.0
        && rounding_mode == RiscvFloatRoundingMode::RoundDown
        && has_single_sign(lhs) != has_single_sign(rhs)
    {
        return f32::from_bits(SINGLE_SIGN_BIT);
    }

    let nearest = exact as f32;
    if f64::from(nearest) == exact {
        return nearest;
    }

    match rounding_mode {
        RiscvFloatRoundingMode::RoundNearestEven => nearest,
        RiscvFloatRoundingMode::RoundTowardZero => {
            if (exact.is_sign_positive() && f64::from(nearest) > exact)
                || (exact.is_sign_negative() && f64::from(nearest) < exact)
            {
                step_toward_exact(nearest, exact)
            } else {
                nearest
            }
        }
        RiscvFloatRoundingMode::RoundDown => {
            if f64::from(nearest) > exact {
                nearest.next_down()
            } else {
                nearest
            }
        }
        RiscvFloatRoundingMode::RoundUp => {
            if f64::from(nearest) < exact {
                nearest.next_up()
            } else {
                nearest
            }
        }
        RiscvFloatRoundingMode::RoundNearestMaxMagnitude => {
            round_nearest_max_magnitude(exact, nearest)
        }
        RiscvFloatRoundingMode::Dynamic => unreachable!("dynamic rounding mode must be resolved"),
    }
}

fn finite_sum_is_exact(lhs: u32, rhs: u32) -> bool {
    if is_zero_single(lhs) || is_zero_single(rhs) {
        return true;
    }
    let Some(lhs_components) = components(lhs) else {
        return true;
    };
    let Some(rhs_components) = components(rhs) else {
        return true;
    };
    let min_shift = lhs_components.shift.min(rhs_components.shift);
    let lhs_delta = lhs_components.shift - min_shift;
    let rhs_delta = rhs_components.shift - min_shift;
    if lhs_delta > 23 || rhs_delta > 23 {
        return false;
    }
    let lhs_scaled = lhs_components.signed_mantissa(lhs_delta);
    let rhs_scaled = rhs_components.signed_mantissa(rhs_delta);
    scaled_sum_is_representable((lhs_scaled + rhs_scaled).unsigned_abs(), min_shift)
}

fn scaled_sum_is_representable(mut mantissa: u128, mut shift: i32) -> bool {
    if mantissa == 0 {
        return true;
    }
    while mantissa > SINGLE_MAX_MANTISSA {
        if mantissa & 1 != 0 {
            return false;
        }
        mantissa >>= 1;
        shift += 1;
    }
    while mantissa < u128::from(SINGLE_HIDDEN_BIT) && shift > SINGLE_MIN_NORMAL_SHIFT {
        mantissa <<= 1;
        shift -= 1;
    }
    if shift == SINGLE_MIN_NORMAL_SHIFT {
        mantissa <= SINGLE_MAX_MANTISSA
    } else {
        mantissa >= u128::from(SINGLE_HIDDEN_BIT) && shift <= SINGLE_MAX_NORMAL_SHIFT
    }
}

fn native_register_write(lhs: u32, rhs: u32) -> u64 {
    box_canonical_single(f32::from_bits(lhs) + f32::from_bits(rhs))
}

fn step_toward_exact(nearest: f32, exact: f64) -> f32 {
    if exact.is_sign_positive() {
        nearest.next_down()
    } else {
        nearest.next_up()
    }
}

fn round_nearest_max_magnitude(exact: f64, nearest: f32) -> f32 {
    let other = if f64::from(nearest) < exact {
        nearest.next_up()
    } else {
        nearest.next_down()
    };
    let nearest_distance = (f64::from(nearest) - exact).abs();
    let other_distance = (f64::from(other) - exact).abs();
    if nearest_distance == other_distance && f64::from(other).abs() > f64::from(nearest).abs() {
        other
    } else {
        nearest
    }
}

fn opposite_infinities(lhs: u32, rhs: u32) -> bool {
    is_infinity_single(lhs)
        && is_infinity_single(rhs)
        && has_single_sign(lhs) != has_single_sign(rhs)
}

fn is_finite(value: u32) -> bool {
    value & SINGLE_EXP_MASK != SINGLE_EXP_MASK
}

fn effective_rhs_bits(rhs: u64, subtract: bool) -> u32 {
    let rhs = unbox_single(rhs);
    if subtract {
        rhs ^ SINGLE_SIGN_BIT
    } else {
        rhs
    }
}

fn normal_exponent(value: u32) -> Option<u32> {
    let exponent = (value & SINGLE_EXP_MASK) >> 23;
    if exponent == 0 {
        None
    } else {
        Some(exponent)
    }
}

fn components(value: u32) -> Option<SingleComponents> {
    let exponent = (value & SINGLE_EXP_MASK) >> 23;
    let fraction = value & SINGLE_FRACTION_MASK;
    let (mantissa, shift) = if exponent == 0 {
        (fraction, SINGLE_MIN_NORMAL_SHIFT)
    } else {
        (SINGLE_HIDDEN_BIT | fraction, exponent as i32 - 150)
    };
    if mantissa == 0 {
        None
    } else {
        Some(SingleComponents {
            negative: has_single_sign(value),
            mantissa,
            shift,
        })
    }
}

struct SingleComponents {
    negative: bool,
    mantissa: u32,
    shift: i32,
}

impl SingleComponents {
    fn signed_mantissa(&self, shift_delta: i32) -> i128 {
        let value = i128::from(self.mantissa) << shift_delta;
        if self.negative {
            -value
        } else {
            value
        }
    }
}

use crate::RiscvFloatRoundingMode;

use super::{
    box_canonical_single, box_single, double_exact, is_infinity_double, is_infinity_single,
    is_nan_double, is_nan_single, is_signaling_nan_double, is_signaling_nan_single, is_zero_double,
    is_zero_single, unbox_single, FLOAT_FLAG_INEXACT, FLOAT_FLAG_INVALID, FLOAT_FLAG_OVERFLOW,
    FLOAT_FLAG_UNDERFLOW, SINGLE_EXP_MASK,
};

pub(super) fn directed_rounding_is_supported(lhs: u64, rhs: u64) -> bool {
    finite_product(lhs, rhs).is_some()
}

pub(super) fn double_directed_rounding_is_supported(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    double_exact::rounded_mul_bits(lhs, rhs, rounding_mode).is_some()
}

pub(super) fn register_write(lhs: u64, rhs: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let lhs = unbox_single(lhs);
    let rhs = unbox_single(rhs);
    match finite_product_from_single_bits(lhs, rhs) {
        Some(exact) => box_single(round_wide_product(exact, rounding_mode).to_bits()),
        None => native_register_write(lhs, rhs),
    }
}

pub(super) fn register_write_double(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    double_exact::rounded_mul_bits(lhs, rhs, rounding_mode)
        .map(|rounded| rounded.bits())
        .unwrap_or_else(|| native_register_write_double(lhs, rhs))
}

pub(super) fn exception_flags(lhs: u64, rhs: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let lhs = unbox_single(lhs);
    let rhs = unbox_single(rhs);
    if is_signaling_nan_single(lhs) || is_signaling_nan_single(rhs) || infinity_times_zero(lhs, rhs)
    {
        return FLOAT_FLAG_INVALID;
    }
    if !is_finite(lhs) || !is_finite(rhs) {
        return 0;
    }
    if product_overflows(lhs, rhs, rounding_mode) {
        return FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT;
    }
    if product_underflows(lhs, rhs, rounding_mode) {
        return FLOAT_FLAG_UNDERFLOW | FLOAT_FLAG_INEXACT;
    }
    if !finite_product_is_exact(lhs, rhs) {
        FLOAT_FLAG_INEXACT
    } else {
        0
    }
}

pub(super) fn exception_flags_double(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    if is_signaling_nan_double(lhs)
        || is_signaling_nan_double(rhs)
        || infinity_times_zero_double(lhs, rhs)
    {
        return FLOAT_FLAG_INVALID;
    }
    if !is_finite_double(lhs) || !is_finite_double(rhs) {
        return 0;
    }
    if let Some(rounded) = double_exact::rounded_mul_bits(lhs, rhs, rounding_mode) {
        return if rounded.overflow() {
            FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT
        } else if rounded.inexact() {
            FLOAT_FLAG_INEXACT
        } else {
            0
        };
    }
    if native_product_overflows_double(lhs, rhs) {
        FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT
    } else if double_exact::mul_bits(lhs, rhs).is_none() {
        FLOAT_FLAG_INEXACT
    } else {
        0
    }
}

fn finite_product(lhs: u64, rhs: u64) -> Option<f64> {
    finite_product_from_single_bits(unbox_single(lhs), unbox_single(rhs))
}

fn finite_product_from_single_bits(lhs: u32, rhs: u32) -> Option<f64> {
    if is_finite(lhs) && is_finite(rhs) {
        Some(f64::from(f32::from_bits(lhs)) * f64::from(f32::from_bits(rhs)))
    } else {
        None
    }
}

fn product_overflows(lhs: u32, rhs: u32, rounding_mode: RiscvFloatRoundingMode) -> bool {
    if native_product_overflows(lhs, rhs) {
        return true;
    }
    finite_product_from_single_bits(lhs, rhs)
        .map(|exact| round_wide_product(exact, rounding_mode).is_infinite())
        .unwrap_or(false)
}

fn native_product_overflows(lhs: u32, rhs: u32) -> bool {
    (f32::from_bits(lhs) * f32::from_bits(rhs)).is_infinite()
}

fn product_underflows(lhs: u32, rhs: u32, rounding_mode: RiscvFloatRoundingMode) -> bool {
    finite_product_from_single_bits(lhs, rhs)
        .map(|exact| {
            exact != 0.0
                && f64::from(exact as f32) != exact
                && is_tiny_single(round_wide_product(exact, rounding_mode).to_bits())
        })
        .unwrap_or(false)
}

fn finite_product_is_exact(lhs: u32, rhs: u32) -> bool {
    finite_product_from_single_bits(lhs, rhs)
        .map(|exact| f64::from(exact as f32) == exact)
        .unwrap_or(true)
}

fn round_wide_product(exact: f64, rounding_mode: RiscvFloatRoundingMode) -> f32 {
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

fn native_register_write(lhs: u32, rhs: u32) -> u64 {
    box_canonical_single(f32::from_bits(lhs) * f32::from_bits(rhs))
}

fn native_register_write_double(lhs: u64, rhs: u64) -> u64 {
    (f64::from_bits(lhs) * f64::from_bits(rhs)).to_bits()
}

fn infinity_times_zero(lhs: u32, rhs: u32) -> bool {
    (is_infinity_single(lhs) && is_zero_single(rhs))
        || (is_zero_single(lhs) && is_infinity_single(rhs))
}

fn infinity_times_zero_double(lhs: u64, rhs: u64) -> bool {
    (is_infinity_double(lhs) && is_zero_double(rhs))
        || (is_zero_double(lhs) && is_infinity_double(rhs))
}

fn is_finite(value: u32) -> bool {
    !is_nan_single(value) && !is_infinity_single(value)
}

fn is_finite_double(value: u64) -> bool {
    !is_nan_double(value) && !is_infinity_double(value)
}

fn is_tiny_single(value: u32) -> bool {
    value & SINGLE_EXP_MASK == 0
}

fn native_product_overflows_double(lhs: u64, rhs: u64) -> bool {
    (f64::from_bits(lhs) * f64::from_bits(rhs)).is_infinite()
}

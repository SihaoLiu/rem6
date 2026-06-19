use std::cmp::Ordering;

use crate::RiscvFloatRoundingMode;

use super::{
    box_canonical_single, box_single, has_single_sign, is_infinity_single, is_nan_single,
    is_signaling_nan_single, is_zero_single, unbox_single, FLOAT_FLAG_DIVIDE_BY_ZERO,
    FLOAT_FLAG_INEXACT, FLOAT_FLAG_INVALID, FLOAT_FLAG_OVERFLOW, FLOAT_FLAG_UNDERFLOW,
    SINGLE_EXP_MASK, SINGLE_FRACTION_MASK, SINGLE_SIGN_BIT,
};

const SINGLE_HIDDEN_BIT: u32 = 1 << 23;

pub(super) fn directed_rounding_is_supported(lhs: u64, rhs: u64) -> bool {
    let lhs = unbox_single(lhs);
    let rhs = unbox_single(rhs);
    !is_signaling_nan_single(lhs) && !is_signaling_nan_single(rhs)
}

pub(super) fn register_write(lhs: u64, rhs: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let lhs = unbox_single(lhs);
    let rhs = unbox_single(rhs);
    if finite_nonzero_components(lhs).is_some() && finite_nonzero_components(rhs).is_some() {
        box_single(round_finite_nonzero_quotient(lhs, rhs, rounding_mode).to_bits())
    } else {
        native_register_write(lhs, rhs)
    }
}

pub(super) fn exception_flags(lhs: u64, rhs: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let lhs = unbox_single(lhs);
    let rhs = unbox_single(rhs);
    if is_signaling_nan_single(lhs)
        || is_signaling_nan_single(rhs)
        || zero_divided_by_zero(lhs, rhs)
        || infinity_divided_by_infinity(lhs, rhs)
    {
        return FLOAT_FLAG_INVALID;
    }
    if divide_by_zero(lhs, rhs) {
        return FLOAT_FLAG_DIVIDE_BY_ZERO;
    }
    if finite_nonzero_components(lhs).is_none() || finite_nonzero_components(rhs).is_none() {
        return 0;
    }
    if quotient_overflows(lhs, rhs, rounding_mode) {
        return FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT;
    }
    if finite_quotient_is_exact(lhs, rhs) {
        return 0;
    }
    if quotient_underflows(lhs, rhs, rounding_mode) {
        FLOAT_FLAG_UNDERFLOW | FLOAT_FLAG_INEXACT
    } else {
        FLOAT_FLAG_INEXACT
    }
}

fn round_finite_nonzero_quotient(lhs: u32, rhs: u32, rounding_mode: RiscvFloatRoundingMode) -> f32 {
    let nearest = f32::from_bits(lhs) / f32::from_bits(rhs);
    if compare_single_to_exact_quotient(nearest.to_bits(), lhs, rhs) == Some(Ordering::Equal) {
        return nearest;
    }

    match rounding_mode {
        RiscvFloatRoundingMode::RoundNearestEven => nearest,
        RiscvFloatRoundingMode::RoundTowardZero => step_toward_zero_if_needed(nearest, lhs, rhs),
        RiscvFloatRoundingMode::RoundDown => step_down_if_needed(nearest, lhs, rhs),
        RiscvFloatRoundingMode::RoundUp => step_up_if_needed(nearest, lhs, rhs),
        RiscvFloatRoundingMode::RoundNearestMaxMagnitude => {
            round_nearest_max_magnitude(nearest, lhs, rhs)
        }
        RiscvFloatRoundingMode::Dynamic => unreachable!("dynamic rounding mode must be resolved"),
    }
}

fn step_toward_zero_if_needed(nearest: f32, lhs: u32, rhs: u32) -> f32 {
    if nearest.is_infinite() {
        return max_finite_with_sign(quotient_sign(lhs, rhs));
    }
    if nearest == 0.0 {
        return nearest;
    }
    match compare_single_to_exact_quotient(nearest.to_bits(), lhs, rhs) {
        Some(Ordering::Greater) if nearest.is_sign_positive() => nearest.next_down(),
        Some(Ordering::Less) if nearest.is_sign_negative() => nearest.next_up(),
        _ => nearest,
    }
}

fn step_down_if_needed(nearest: f32, lhs: u32, rhs: u32) -> f32 {
    if nearest.is_infinite() {
        return if quotient_sign(lhs, rhs) {
            f32::NEG_INFINITY
        } else {
            f32::MAX
        };
    }
    if nearest == 0.0 && quotient_sign(lhs, rhs) {
        return f32::from_bits(SINGLE_SIGN_BIT | 1);
    }
    match compare_single_to_exact_quotient(nearest.to_bits(), lhs, rhs) {
        Some(Ordering::Greater) => nearest.next_down(),
        _ => nearest,
    }
}

fn step_up_if_needed(nearest: f32, lhs: u32, rhs: u32) -> f32 {
    if nearest.is_infinite() {
        return if quotient_sign(lhs, rhs) {
            -f32::MAX
        } else {
            f32::INFINITY
        };
    }
    if nearest == 0.0 && !quotient_sign(lhs, rhs) {
        return f32::from_bits(1);
    }
    match compare_single_to_exact_quotient(nearest.to_bits(), lhs, rhs) {
        Some(Ordering::Less) => nearest.next_up(),
        _ => nearest,
    }
}

fn round_nearest_max_magnitude(nearest: f32, lhs: u32, rhs: u32) -> f32 {
    if nearest.is_infinite() || nearest == 0.0 {
        let exact = f64::from(f32::from_bits(lhs)) / f64::from(f32::from_bits(rhs));
        return round_nearest_max_magnitude_from_wide(exact, nearest);
    }
    let exact = f64::from(f32::from_bits(lhs)) / f64::from(f32::from_bits(rhs));
    round_nearest_max_magnitude_from_wide(exact, nearest)
}

fn round_nearest_max_magnitude_from_wide(exact: f64, nearest: f32) -> f32 {
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

fn quotient_overflows(lhs: u32, rhs: u32, rounding_mode: RiscvFloatRoundingMode) -> bool {
    let nearest = f32::from_bits(lhs) / f32::from_bits(rhs);
    nearest.is_infinite() || round_finite_nonzero_quotient(lhs, rhs, rounding_mode).is_infinite()
}

fn quotient_underflows(lhs: u32, rhs: u32, rounding_mode: RiscvFloatRoundingMode) -> bool {
    let rounded = round_finite_nonzero_quotient(lhs, rhs, rounding_mode).to_bits();
    rounded & SINGLE_EXP_MASK == 0
}

fn finite_quotient_is_exact(lhs: u32, rhs: u32) -> bool {
    let nearest = f32::from_bits(lhs) / f32::from_bits(rhs);
    compare_single_to_exact_quotient(nearest.to_bits(), lhs, rhs) == Some(Ordering::Equal)
}

fn compare_single_to_exact_quotient(candidate: u32, lhs: u32, rhs: u32) -> Option<Ordering> {
    let candidate_components = finite_nonzero_components(candidate)?;
    let lhs_components = finite_nonzero_components(lhs)?;
    let rhs_components = finite_nonzero_components(rhs)?;
    let magnitude = compare_scaled(
        u128::from(candidate_components.mantissa) * u128::from(rhs_components.mantissa),
        candidate_components.exponent + rhs_components.exponent,
        u128::from(lhs_components.mantissa),
        lhs_components.exponent,
    );
    let ordering = if quotient_sign(lhs, rhs) {
        magnitude.reverse()
    } else {
        magnitude
    };
    Some(ordering)
}

fn compare_scaled(
    lhs_mantissa: u128,
    lhs_exponent: i32,
    rhs_mantissa: u128,
    rhs_exponent: i32,
) -> Ordering {
    let (lhs_mantissa, lhs_exponent) = strip_factor_of_two(lhs_mantissa, lhs_exponent);
    let (rhs_mantissa, rhs_exponent) = strip_factor_of_two(rhs_mantissa, rhs_exponent);
    let lhs_bits = scaled_bit_length(lhs_mantissa, lhs_exponent);
    let rhs_bits = scaled_bit_length(rhs_mantissa, rhs_exponent);
    if lhs_bits != rhs_bits {
        return lhs_bits.cmp(&rhs_bits);
    }
    match lhs_exponent.cmp(&rhs_exponent) {
        Ordering::Greater => {
            let shift = u32::try_from(lhs_exponent - rhs_exponent).unwrap();
            (lhs_mantissa << shift).cmp(&rhs_mantissa)
        }
        Ordering::Less => {
            let shift = u32::try_from(rhs_exponent - lhs_exponent).unwrap();
            lhs_mantissa.cmp(&(rhs_mantissa << shift))
        }
        Ordering::Equal => lhs_mantissa.cmp(&rhs_mantissa),
    }
}

fn strip_factor_of_two(mantissa: u128, exponent: i32) -> (u128, i32) {
    let shift = mantissa.trailing_zeros();
    (mantissa >> shift, exponent + i32::try_from(shift).unwrap())
}

fn scaled_bit_length(mantissa: u128, exponent: i32) -> i32 {
    i32::try_from(u128::BITS - mantissa.leading_zeros()).unwrap() + exponent
}

fn finite_nonzero_components(value: u32) -> Option<SingleComponents> {
    if is_nan_single(value) || is_infinity_single(value) || is_zero_single(value) {
        return None;
    }
    let exponent = (value & SINGLE_EXP_MASK) >> 23;
    let fraction = value & SINGLE_FRACTION_MASK;
    if exponent == 0 {
        Some(SingleComponents {
            mantissa: fraction,
            exponent: -149,
        })
    } else {
        Some(SingleComponents {
            mantissa: SINGLE_HIDDEN_BIT | fraction,
            exponent: i32::try_from(exponent).unwrap() - 150,
        })
    }
}

fn native_register_write(lhs: u32, rhs: u32) -> u64 {
    box_canonical_single(f32::from_bits(lhs) / f32::from_bits(rhs))
}

fn divide_by_zero(lhs: u32, rhs: u32) -> bool {
    is_zero_single(rhs) && !is_zero_single(lhs) && !is_nan_single(lhs) && !is_infinity_single(lhs)
}

fn zero_divided_by_zero(lhs: u32, rhs: u32) -> bool {
    is_zero_single(lhs) && is_zero_single(rhs)
}

fn infinity_divided_by_infinity(lhs: u32, rhs: u32) -> bool {
    is_infinity_single(lhs) && is_infinity_single(rhs)
}

fn quotient_sign(lhs: u32, rhs: u32) -> bool {
    has_single_sign(lhs) ^ has_single_sign(rhs)
}

fn max_finite_with_sign(sign: bool) -> f32 {
    if sign {
        -f32::MAX
    } else {
        f32::MAX
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SingleComponents {
    mantissa: u32,
    exponent: i32,
}

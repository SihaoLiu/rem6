use std::cmp::Ordering;

use crate::RiscvFloatRoundingMode;

use super::{
    box_canonical_single, has_single_sign, is_infinity_double, is_infinity_single,
    is_signaling_nan_double, is_signaling_nan_single, is_zero_single, unbox_single,
    DOUBLE_EXP_MASK, DOUBLE_FRACTION_MASK, DOUBLE_SIGN_BIT, FLOAT_FLAG_INEXACT, FLOAT_FLAG_INVALID,
    FLOAT_FLAG_OVERFLOW, SINGLE_EXP_MASK, SINGLE_FRACTION_MASK, SINGLE_SIGN_BIT,
};

const SINGLE_HIDDEN_BIT: u32 = 1 << 23;
const SINGLE_MAX_MANTISSA: u128 = (1 << 24) - 1;
const SINGLE_MIN_NORMAL_SHIFT: i32 = -149;
const SINGLE_MAX_NORMAL_SHIFT: i32 = 104;
const DOUBLE_MAX_FINITE_BITS: u64 = 0x7fef_ffff_ffff_ffff;

pub(super) fn add_directed_rounding_is_supported(lhs: u64, rhs: u64) -> bool {
    wide_sum_value(lhs, rhs, false).is_some()
}

pub(super) fn sub_directed_rounding_is_supported(lhs: u64, rhs: u64) -> bool {
    wide_sum_value(lhs, rhs, true).is_some()
}

pub(super) fn add_double_directed_rounding_is_supported(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    double_directed_rounding_is_supported(lhs, rhs, rounding_mode, false)
}

pub(super) fn sub_double_directed_rounding_is_supported(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    double_directed_rounding_is_supported(lhs, rhs, rounding_mode, true)
}

pub(super) fn add_register_write(lhs: u64, rhs: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    register_write(lhs, rhs, rounding_mode, false)
}

pub(super) fn sub_register_write(lhs: u64, rhs: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    register_write(lhs, rhs, rounding_mode, true)
}

pub(super) fn add_register_write_double(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    register_write_double(lhs, rhs, rounding_mode, false)
}

pub(super) fn sub_register_write_double(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    register_write_double(lhs, rhs, rounding_mode, true)
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

pub(super) fn add_exception_flags_double(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    exception_flags_double(lhs, rhs, rounding_mode, false)
}

pub(super) fn sub_exception_flags_double(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    exception_flags_double(lhs, rhs, rounding_mode, true)
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

fn register_write_double(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
    subtract: bool,
) -> u64 {
    let Some(sum) = double_sum_value(lhs, rhs, subtract) else {
        return native_register_write_double(lhs, rhs, subtract);
    };
    if sum.exact == 0 {
        return if rounding_mode == RiscvFloatRoundingMode::RoundDown
            && sum.lhs.negative != sum.rhs.negative
        {
            DOUBLE_SIGN_BIT
        } else {
            sum.native_bits
        };
    }
    if is_infinity_double(sum.native_bits) && double_sum_overflows(sum) {
        return double_overflow_bits(sum.exact, rounding_mode);
    }
    let Some(ordering) = compare_double_to_exact_sum(sum.native_bits, sum.exact, sum.shift) else {
        return sum.native_bits;
    };
    if ordering == Ordering::Equal {
        return sum.native_bits;
    }

    let nearest = f64::from_bits(sum.native_bits);
    match rounding_mode {
        RiscvFloatRoundingMode::RoundNearestEven => sum.native_bits,
        RiscvFloatRoundingMode::RoundTowardZero => {
            let adjusted = if sum.exact > 0 && ordering == Ordering::Greater {
                nearest.next_down()
            } else if sum.exact < 0 && ordering == Ordering::Less {
                nearest.next_up()
            } else {
                nearest
            };
            adjusted.to_bits()
        }
        RiscvFloatRoundingMode::RoundDown => {
            if ordering == Ordering::Greater {
                nearest.next_down().to_bits()
            } else {
                sum.native_bits
            }
        }
        RiscvFloatRoundingMode::RoundUp => {
            if ordering == Ordering::Less {
                nearest.next_up().to_bits()
            } else {
                sum.native_bits
            }
        }
        RiscvFloatRoundingMode::RoundNearestMaxMagnitude | RiscvFloatRoundingMode::Dynamic => {
            sum.native_bits
        }
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

fn exception_flags_double(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
    subtract: bool,
) -> u64 {
    let effective_rhs = effective_rhs_double_bits(rhs, subtract);
    if is_signaling_nan_double(lhs)
        || is_signaling_nan_double(effective_rhs)
        || opposite_infinities_double(lhs, effective_rhs)
    {
        return FLOAT_FLAG_INVALID;
    }
    if !is_finite_double(lhs) || !is_finite_double(effective_rhs) {
        return 0;
    }
    if is_zero_double(lhs) || is_zero_double(effective_rhs) {
        return 0;
    }
    if is_infinity_double(register_write_double(lhs, rhs, rounding_mode, subtract)) {
        return FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT;
    }
    let Some(sum) = double_sum_value(lhs, rhs, subtract) else {
        return FLOAT_FLAG_INEXACT;
    };
    if double_sum_overflows(sum) {
        return FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT;
    }
    match compare_double_to_exact_sum(sum.native_bits, sum.exact, sum.shift) {
        Some(Ordering::Equal) => 0,
        _ => FLOAT_FLAG_INEXACT,
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

fn native_register_write_double(lhs: u64, rhs: u64, subtract: bool) -> u64 {
    let rhs = effective_rhs_double_bits(rhs, subtract);
    (f64::from_bits(lhs) + f64::from_bits(rhs)).to_bits()
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

fn double_directed_rounding_is_supported(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
    subtract: bool,
) -> bool {
    matches!(
        rounding_mode,
        RiscvFloatRoundingMode::RoundTowardZero
            | RiscvFloatRoundingMode::RoundDown
            | RiscvFloatRoundingMode::RoundUp
    ) && double_sum_value(lhs, rhs, subtract).is_some()
}

fn double_sum_value(lhs: u64, rhs: u64, subtract: bool) -> Option<DoubleSum> {
    let rhs = effective_rhs_double_bits(rhs, subtract);
    let lhs = double_components(lhs)?;
    let rhs = double_components(rhs)?;
    let shift = lhs.shift.min(rhs.shift);
    let lhs_scaled = lhs.signed_mantissa(shift)?;
    let rhs_scaled = rhs.signed_mantissa(shift)?;
    let exact = lhs_scaled.checked_add(rhs_scaled)?;
    let native_bits = (lhs.value() + rhs.value()).to_bits();
    Some(DoubleSum {
        lhs,
        rhs,
        exact,
        shift,
        native_bits,
    })
}

fn compare_double_to_exact_sum(candidate: u64, exact: i128, exact_shift: i32) -> Option<Ordering> {
    let candidate = double_components(candidate)?;
    let target_shift = candidate.shift.min(exact_shift);
    let candidate_scaled = candidate.signed_mantissa(target_shift)?;
    let exact_scaled = scale_signed_mantissa(exact, exact_shift, target_shift)?;
    Some(candidate_scaled.cmp(&exact_scaled))
}

fn double_sum_overflows(sum: DoubleSum) -> bool {
    if sum.exact > 0 {
        matches!(
            compare_double_to_exact_sum(DOUBLE_MAX_FINITE_BITS, sum.exact, sum.shift),
            Some(Ordering::Less)
        )
    } else if sum.exact < 0 {
        matches!(
            compare_double_to_exact_sum(
                DOUBLE_SIGN_BIT | DOUBLE_MAX_FINITE_BITS,
                sum.exact,
                sum.shift
            ),
            Some(Ordering::Greater)
        )
    } else {
        false
    }
}

fn double_overflow_bits(exact: i128, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    if exact < 0 {
        match rounding_mode {
            RiscvFloatRoundingMode::RoundTowardZero | RiscvFloatRoundingMode::RoundUp => {
                DOUBLE_SIGN_BIT | DOUBLE_MAX_FINITE_BITS
            }
            _ => f64::NEG_INFINITY.to_bits(),
        }
    } else {
        match rounding_mode {
            RiscvFloatRoundingMode::RoundTowardZero | RiscvFloatRoundingMode::RoundDown => {
                DOUBLE_MAX_FINITE_BITS
            }
            _ => f64::INFINITY.to_bits(),
        }
    }
}

fn scale_signed_mantissa(value: i128, shift: i32, target_shift: i32) -> Option<i128> {
    let delta: u32 = shift.checked_sub(target_shift)?.try_into().ok()?;
    let magnitude = if value < 0 {
        value.checked_neg()? as u128
    } else {
        value as u128
    };
    let magnitude = magnitude.checked_shl(delta)?;
    if magnitude > i128::MAX as u128 {
        return None;
    }
    let magnitude = magnitude as i128;
    Some(if value < 0 { -magnitude } else { magnitude })
}

fn double_components(value: u64) -> Option<DoubleComponents> {
    if !is_finite_double(value) {
        return None;
    }
    let negative = value & DOUBLE_SIGN_BIT != 0;
    let exponent = (value & DOUBLE_EXP_MASK) >> 52;
    let fraction = value & DOUBLE_FRACTION_MASK;
    if exponent == 0 {
        return Some(DoubleComponents {
            negative,
            significand: u128::from(fraction),
            shift: -1074,
        });
    }
    Some(DoubleComponents {
        negative,
        significand: u128::from((1_u64 << 52) | fraction),
        shift: exponent as i32 - 1023 - 52,
    })
}

fn effective_rhs_double_bits(rhs: u64, subtract: bool) -> u64 {
    if subtract {
        rhs ^ DOUBLE_SIGN_BIT
    } else {
        rhs
    }
}

fn opposite_infinities_double(lhs: u64, rhs: u64) -> bool {
    is_infinity_double(lhs) && is_infinity_double(rhs) && (lhs ^ rhs) & DOUBLE_SIGN_BIT != 0
}

fn is_finite_double(value: u64) -> bool {
    !is_infinity_double(value) && value & DOUBLE_EXP_MASK != DOUBLE_EXP_MASK
}

fn is_zero_double(value: u64) -> bool {
    value & !DOUBLE_SIGN_BIT == 0
}

#[derive(Clone, Copy)]
struct DoubleSum {
    lhs: DoubleComponents,
    rhs: DoubleComponents,
    exact: i128,
    shift: i32,
    native_bits: u64,
}

#[derive(Clone, Copy)]
struct DoubleComponents {
    negative: bool,
    significand: u128,
    shift: i32,
}

impl DoubleComponents {
    fn signed_mantissa(self, target_shift: i32) -> Option<i128> {
        let delta: u32 = self.shift.checked_sub(target_shift)?.try_into().ok()?;
        let magnitude = self.significand.checked_shl(delta)?;
        if magnitude > i128::MAX as u128 {
            return None;
        }
        let magnitude = magnitude as i128;
        Some(if self.negative { -magnitude } else { magnitude })
    }

    fn value(self) -> f64 {
        let sign = if self.negative { DOUBLE_SIGN_BIT } else { 0 };
        if self.significand == 0 {
            return f64::from_bits(sign);
        }
        f64::from_bits(sign | self.raw_magnitude_bits())
    }

    fn raw_magnitude_bits(self) -> u64 {
        if self.shift == -1074 {
            return self.significand as u64;
        }
        let exponent = self.shift + 1023 + 52;
        ((exponent as u64) << 52) | ((self.significand as u64) & DOUBLE_FRACTION_MASK)
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

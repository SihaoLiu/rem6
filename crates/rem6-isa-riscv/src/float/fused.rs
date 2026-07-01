use crate::RiscvFloatRoundingMode;

use super::{
    box_canonical_single, has_double_sign, has_single_sign, is_infinity_double, is_infinity_single,
    is_nan_double, is_nan_single, is_signaling_nan_double, is_signaling_nan_single, is_zero_double,
    is_zero_single, unbox_single, DOUBLE_EXP_MASK, DOUBLE_FRACTION_MASK, DOUBLE_SIGN_BIT,
    FLOAT_FLAG_INEXACT, FLOAT_FLAG_INVALID, FLOAT_FLAG_OVERFLOW, SINGLE_EXP_MASK,
    SINGLE_FRACTION_MASK, SINGLE_SIGN_BIT,
};

const SINGLE_HIDDEN_BIT: u128 = 1 << 23;
const SINGLE_MIN_NORMAL_SHIFT: i32 = -149;
const DOUBLE_HIDDEN_BIT: u128 = 1 << 52;
const DOUBLE_MIN_NORMAL_SHIFT: i32 = -1074;

pub(super) fn single_directed_rounding_is_supported(lhs: u64, rhs: u64, addend: u64) -> bool {
    exact_single_value(lhs, rhs, addend).is_some()
}

pub(super) fn double_directed_rounding_is_supported(lhs: u64, rhs: u64, addend: u64) -> bool {
    exact_double_value(lhs, rhs, addend).is_some()
        || native_double_overflow_is_negative(lhs, rhs, addend).is_some()
}

pub(super) fn single_register_write(
    lhs: u64,
    rhs: u64,
    addend: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    let lhs = unbox_single(lhs);
    let rhs = unbox_single(rhs);
    let addend = unbox_single(addend);
    match exact_single_value_from_bits(lhs, rhs, addend) {
        Some(exact) => box_single_bits(round_exact_single(lhs, rhs, addend, exact, rounding_mode)),
        None => native_single_register_write(lhs, rhs, addend),
    }
}

pub(super) fn double_register_write(
    lhs: u64,
    rhs: u64,
    addend: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    match exact_double_value(lhs, rhs, addend) {
        Some(exact) => round_exact_double(lhs, rhs, addend, exact, rounding_mode),
        None => match native_double_overflow_is_negative(lhs, rhs, addend) {
            Some(negative) => overflow_bits_double(negative, rounding_mode),
            None => native_double_register_write(lhs, rhs, addend),
        },
    }
}

pub(super) fn single_exception_flags(
    lhs: u64,
    rhs: u64,
    addend: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    let lhs = unbox_single(lhs);
    let rhs = unbox_single(rhs);
    let addend = unbox_single(addend);
    if is_signaling_nan_single(lhs)
        || is_signaling_nan_single(rhs)
        || is_signaling_nan_single(addend)
        || is_infinity_times_zero(lhs, rhs)
        || product_addend_are_opposite_infinities(lhs, rhs, addend)
    {
        return FLOAT_FLAG_INVALID;
    }
    if !is_finite(lhs) || !is_finite(rhs) || !is_finite(addend) {
        return 0;
    }
    if single_overflows(lhs, rhs, addend, rounding_mode) {
        return FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT;
    }
    if !single_is_exact(lhs, rhs, addend) {
        FLOAT_FLAG_INEXACT
    } else {
        0
    }
}

pub(super) fn exact_finite_single_mul_add_bits(
    lhs: u32,
    rhs: u32,
    addend: u32,
    rounding_mode: RiscvFloatRoundingMode,
) -> Option<u32> {
    let exact = exact_single_value_from_bits(lhs, rhs, addend)?;
    exact_is_representable_as_single(exact)
        .then(|| round_exact_single(lhs, rhs, addend, exact, rounding_mode))
}

pub(super) fn double_exception_flags(
    lhs: u64,
    rhs: u64,
    addend: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    if is_signaling_nan_double(lhs)
        || is_signaling_nan_double(rhs)
        || is_signaling_nan_double(addend)
        || is_infinity_times_zero_double(lhs, rhs)
        || product_addend_are_opposite_infinities_double(lhs, rhs, addend)
    {
        return FLOAT_FLAG_INVALID;
    }
    if !is_finite_double(lhs) || !is_finite_double(rhs) || !is_finite_double(addend) {
        return 0;
    }
    if double_overflows(lhs, rhs, addend, rounding_mode) {
        return FLOAT_FLAG_OVERFLOW | FLOAT_FLAG_INEXACT;
    }
    if !double_is_exact(lhs, rhs, addend) {
        FLOAT_FLAG_INEXACT
    } else {
        0
    }
}

fn exact_single_value(lhs: u64, rhs: u64, addend: u64) -> Option<ExactSingleValue> {
    exact_single_value_from_bits(unbox_single(lhs), unbox_single(rhs), unbox_single(addend))
}

fn exact_double_value(lhs: u64, rhs: u64, addend: u64) -> Option<ExactDoubleValue> {
    if !is_finite_double(lhs) || !is_finite_double(rhs) || !is_finite_double(addend) {
        return None;
    }

    let product = product_term_double(lhs, rhs)?;
    let addend = double_term(addend)?;
    let Some(min_shift) = [product.nonzero_shift(), addend.nonzero_shift()]
        .into_iter()
        .flatten()
        .min()
    else {
        return Some(ExactDoubleValue {
            negative: false,
            magnitude: 0,
            shift: 0,
        });
    };

    let product = signed_scaled_term(product, min_shift)?;
    let addend = signed_scaled_term(addend, min_shift)?;
    let sum = product.checked_add(addend)?;
    Some(ExactDoubleValue {
        negative: sum < 0,
        magnitude: sum.unsigned_abs(),
        shift: min_shift,
    })
}

fn exact_single_value_from_bits(lhs: u32, rhs: u32, addend: u32) -> Option<ExactSingleValue> {
    if !is_finite(lhs) || !is_finite(rhs) || !is_finite(addend) {
        return None;
    }

    let product = product_term(lhs, rhs)?;
    let addend = single_term(addend)?;
    let Some(min_shift) = [product.nonzero_shift(), addend.nonzero_shift()]
        .into_iter()
        .flatten()
        .min()
    else {
        return Some(ExactSingleValue {
            negative: false,
            magnitude: 0,
            shift: 0,
        });
    };

    let product = signed_scaled_term(product, min_shift)?;
    let addend = signed_scaled_term(addend, min_shift)?;
    let sum = product.checked_add(addend)?;
    Some(ExactSingleValue {
        negative: sum < 0,
        magnitude: sum.unsigned_abs(),
        shift: min_shift,
    })
}

fn product_term(lhs: u32, rhs: u32) -> Option<ExactTerm> {
    if is_zero_single(lhs) || is_zero_single(rhs) {
        return Some(ExactTerm::zero(product_is_negative(lhs, rhs)));
    }
    let lhs = single_term(lhs)?;
    let rhs = single_term(rhs)?;
    Some(ExactTerm {
        negative: lhs.negative != rhs.negative,
        mantissa: lhs.mantissa.checked_mul(rhs.mantissa)?,
        shift: lhs.shift + rhs.shift,
    })
}

fn product_term_double(lhs: u64, rhs: u64) -> Option<ExactTerm> {
    if is_zero_double(lhs) || is_zero_double(rhs) {
        return Some(ExactTerm::zero(product_is_negative_double(lhs, rhs)));
    }
    let lhs = double_term(lhs)?;
    let rhs = double_term(rhs)?;
    Some(ExactTerm {
        negative: lhs.negative != rhs.negative,
        mantissa: lhs.mantissa.checked_mul(rhs.mantissa)?,
        shift: lhs.shift + rhs.shift,
    })
}

fn single_term(value: u32) -> Option<ExactTerm> {
    if is_zero_single(value) {
        return Some(ExactTerm::zero(has_single_sign(value)));
    }
    let exponent = (value & SINGLE_EXP_MASK) >> 23;
    let fraction = value & SINGLE_FRACTION_MASK;
    let (mantissa, shift) = if exponent == 0 {
        (u128::from(fraction), SINGLE_MIN_NORMAL_SHIFT)
    } else {
        (
            SINGLE_HIDDEN_BIT | u128::from(fraction),
            exponent as i32 - 150,
        )
    };
    Some(ExactTerm {
        negative: has_single_sign(value),
        mantissa,
        shift,
    })
}

fn double_term(value: u64) -> Option<ExactTerm> {
    if is_zero_double(value) {
        return Some(ExactTerm::zero(has_double_sign(value)));
    }
    let exponent = (value & DOUBLE_EXP_MASK) >> 52;
    let fraction = value & DOUBLE_FRACTION_MASK;
    let (mantissa, shift) = if exponent == 0 {
        (u128::from(fraction), DOUBLE_MIN_NORMAL_SHIFT)
    } else {
        (
            DOUBLE_HIDDEN_BIT | u128::from(fraction),
            exponent as i32 - 1075,
        )
    };
    Some(ExactTerm {
        negative: has_double_sign(value),
        mantissa,
        shift,
    })
}

fn signed_scaled_term(term: ExactTerm, min_shift: i32) -> Option<i128> {
    if term.mantissa == 0 {
        return Some(0);
    }
    let shift_delta = (term.shift - min_shift).try_into().ok()?;
    let shifted = term.mantissa.checked_shl(shift_delta)?;
    if shifted > i128::MAX as u128 {
        return None;
    }
    let shifted = shifted as i128;
    if term.negative {
        Some(-shifted)
    } else {
        Some(shifted)
    }
}

fn single_overflows(
    lhs: u32,
    rhs: u32,
    addend: u32,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    match exact_single_value_from_bits(lhs, rhs, addend) {
        Some(exact) => exact_overflows_single(exact, rounding_mode),
        None => f32::from_bits(lhs)
            .mul_add(f32::from_bits(rhs), f32::from_bits(addend))
            .is_infinite(),
    }
}

fn double_overflows(
    lhs: u64,
    rhs: u64,
    addend: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    match exact_double_value(lhs, rhs, addend) {
        Some(exact) => exact_overflows_double(exact, rounding_mode),
        None => native_double_overflow_is_negative(lhs, rhs, addend).is_some(),
    }
}

fn native_double_overflow_is_negative(lhs: u64, rhs: u64, addend: u64) -> Option<bool> {
    if !is_finite_double(lhs) || !is_finite_double(rhs) || !is_finite_double(addend) {
        return None;
    }
    let native_bits = native_double_register_write(lhs, rhs, addend);
    is_infinity_double(native_bits).then(|| has_double_sign(native_bits))
}

fn single_is_exact(lhs: u32, rhs: u32, addend: u32) -> bool {
    exact_single_value_from_bits(lhs, rhs, addend).is_some_and(exact_is_representable_as_single)
}

fn double_is_exact(lhs: u64, rhs: u64, addend: u64) -> bool {
    exact_double_value(lhs, rhs, addend).is_some_and(exact_is_representable_as_double)
}

fn round_exact_single(
    lhs: u32,
    rhs: u32,
    addend: u32,
    exact: ExactSingleValue,
    rounding_mode: RiscvFloatRoundingMode,
) -> u32 {
    if exact.magnitude == 0 {
        return zero_result_bits(lhs, rhs, addend, rounding_mode);
    }

    round_magnitude_to_single(exact.negative, exact.magnitude, exact.shift, rounding_mode)
}

fn round_exact_double(
    lhs: u64,
    rhs: u64,
    addend: u64,
    exact: ExactDoubleValue,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    if exact.magnitude == 0 {
        return zero_result_bits_double(lhs, rhs, addend, rounding_mode);
    }

    round_magnitude_to_double(exact.negative, exact.magnitude, exact.shift, rounding_mode)
}

fn round_magnitude_to_single(
    negative: bool,
    magnitude: u128,
    shift: i32,
    rounding_mode: RiscvFloatRoundingMode,
) -> u32 {
    let bits = bit_length(magnitude);
    let mut exponent = shift + bits as i32 - 1;
    if exponent < -126 {
        return round_subnormal(negative, magnitude, shift, rounding_mode);
    }
    if exponent > 127 {
        return overflow_bits(negative, rounding_mode);
    }

    let mut significand = if bits > 24 {
        let dropped_bits = bits - 24;
        let quotient = magnitude >> dropped_bits;
        let remainder = low_bits(magnitude, dropped_bits);
        let increment =
            should_increment(negative, quotient, remainder, dropped_bits, rounding_mode);
        quotient + u128::from(increment)
    } else {
        magnitude << (24 - bits)
    };

    if significand == 1 << 24 {
        significand >>= 1;
        exponent += 1;
        if exponent > 127 {
            return overflow_bits(negative, rounding_mode);
        }
    }

    let sign = if negative { SINGLE_SIGN_BIT } else { 0 };
    sign | (((exponent + 127) as u32) << 23) | (significand as u32 & SINGLE_FRACTION_MASK)
}

fn round_magnitude_to_double(
    negative: bool,
    magnitude: u128,
    shift: i32,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    let bits = bit_length(magnitude);
    let mut exponent = shift + bits as i32 - 1;
    if exponent < -1022 {
        return round_subnormal_double(negative, magnitude, shift, rounding_mode);
    }
    if exponent > 1023 {
        return overflow_bits_double(negative, rounding_mode);
    }

    let mut significand = if bits > 53 {
        let dropped_bits = bits - 53;
        let quotient = magnitude >> dropped_bits;
        let remainder = low_bits(magnitude, dropped_bits);
        let increment =
            should_increment(negative, quotient, remainder, dropped_bits, rounding_mode);
        quotient + u128::from(increment)
    } else {
        magnitude << (53 - bits)
    };

    if significand == 1 << 53 {
        significand >>= 1;
        exponent += 1;
        if exponent > 1023 {
            return overflow_bits_double(negative, rounding_mode);
        }
    }

    let sign = if negative { DOUBLE_SIGN_BIT } else { 0 };
    sign | (((exponent + 1023) as u64) << 52) | (significand as u64 & DOUBLE_FRACTION_MASK)
}

fn round_subnormal(
    negative: bool,
    magnitude: u128,
    shift: i32,
    rounding_mode: RiscvFloatRoundingMode,
) -> u32 {
    let unit_shift = shift + 149;
    let (mut units, remainder, dropped_bits) = if unit_shift >= 0 {
        let Some(units) = magnitude.checked_shl(unit_shift as u32) else {
            return overflow_bits(negative, rounding_mode);
        };
        (units, 0, 0)
    } else {
        let dropped_bits = (-unit_shift) as u32;
        if dropped_bits >= 128 {
            (0, magnitude, dropped_bits)
        } else {
            (
                magnitude >> dropped_bits,
                low_bits(magnitude, dropped_bits),
                dropped_bits,
            )
        }
    };

    if dropped_bits > 0 && should_increment(negative, units, remainder, dropped_bits, rounding_mode)
    {
        units += 1;
    }

    let sign = if negative { SINGLE_SIGN_BIT } else { 0 };
    if units >= 1 << 23 {
        sign | (1 << 23)
    } else {
        sign | units as u32
    }
}

fn round_subnormal_double(
    negative: bool,
    magnitude: u128,
    shift: i32,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    let unit_shift = shift + 1074;
    let (mut units, remainder, dropped_bits) = if unit_shift >= 0 {
        let Some(units) = magnitude.checked_shl(unit_shift as u32) else {
            return overflow_bits_double(negative, rounding_mode);
        };
        (units, 0, 0)
    } else {
        let dropped_bits = (-unit_shift) as u32;
        if dropped_bits >= 128 {
            (0, magnitude, dropped_bits)
        } else {
            (
                magnitude >> dropped_bits,
                low_bits(magnitude, dropped_bits),
                dropped_bits,
            )
        }
    };

    if dropped_bits > 0 && should_increment(negative, units, remainder, dropped_bits, rounding_mode)
    {
        units += 1;
    }

    let sign = if negative { DOUBLE_SIGN_BIT } else { 0 };
    if units >= 1 << 52 {
        sign | (1 << 52)
    } else {
        sign | units as u64
    }
}

fn should_increment(
    negative: bool,
    quotient: u128,
    remainder: u128,
    dropped_bits: u32,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    if remainder == 0 {
        return false;
    }
    match rounding_mode {
        RiscvFloatRoundingMode::RoundNearestEven => {
            is_above_half(remainder, dropped_bits)
                || (is_exactly_half(remainder, dropped_bits) && quotient & 1 != 0)
        }
        RiscvFloatRoundingMode::RoundTowardZero => false,
        RiscvFloatRoundingMode::RoundDown => negative,
        RiscvFloatRoundingMode::RoundUp => !negative,
        RiscvFloatRoundingMode::RoundNearestMaxMagnitude => {
            is_above_half(remainder, dropped_bits) || is_exactly_half(remainder, dropped_bits)
        }
        RiscvFloatRoundingMode::Dynamic => unreachable!("dynamic rounding mode must be resolved"),
    }
}

fn is_above_half(remainder: u128, dropped_bits: u32) -> bool {
    if dropped_bits > 128 {
        return false;
    }
    remainder > half_bit(dropped_bits)
}

fn is_exactly_half(remainder: u128, dropped_bits: u32) -> bool {
    dropped_bits <= 128 && remainder == half_bit(dropped_bits)
}

fn half_bit(dropped_bits: u32) -> u128 {
    1u128 << (dropped_bits - 1)
}

fn native_single_register_write(lhs: u32, rhs: u32, addend: u32) -> u64 {
    box_canonical_single(f32::from_bits(lhs).mul_add(f32::from_bits(rhs), f32::from_bits(addend)))
}

fn native_double_register_write(lhs: u64, rhs: u64, addend: u64) -> u64 {
    f64::from_bits(lhs)
        .mul_add(f64::from_bits(rhs), f64::from_bits(addend))
        .to_bits()
}

fn box_single_bits(bits: u32) -> u64 {
    box_canonical_single(f32::from_bits(bits))
}

fn zero_result_bits(lhs: u32, rhs: u32, addend: u32, rounding_mode: RiscvFloatRoundingMode) -> u32 {
    let product_negative = product_is_negative(lhs, rhs);
    let addend_negative = has_single_sign(addend);
    if (is_zero_single(addend) && product_negative && addend_negative)
        || (rounding_mode == RiscvFloatRoundingMode::RoundDown
            && (product_negative || addend_negative))
    {
        SINGLE_SIGN_BIT
    } else {
        0
    }
}

fn zero_result_bits_double(
    lhs: u64,
    rhs: u64,
    addend: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    let product_negative = product_is_negative_double(lhs, rhs);
    let addend_negative = has_double_sign(addend);
    if (is_zero_double(addend) && product_negative && addend_negative)
        || (rounding_mode == RiscvFloatRoundingMode::RoundDown
            && (product_negative || addend_negative))
    {
        DOUBLE_SIGN_BIT
    } else {
        0
    }
}

fn overflow_bits(negative: bool, rounding_mode: RiscvFloatRoundingMode) -> u32 {
    let sign = if negative { SINGLE_SIGN_BIT } else { 0 };
    let infinity = sign | SINGLE_EXP_MASK;
    let max_finite = sign | (SINGLE_EXP_MASK - (1 << 23)) | SINGLE_FRACTION_MASK;
    match (negative, rounding_mode) {
        (false, RiscvFloatRoundingMode::RoundTowardZero | RiscvFloatRoundingMode::RoundDown) => {
            max_finite
        }
        (true, RiscvFloatRoundingMode::RoundTowardZero | RiscvFloatRoundingMode::RoundUp) => {
            max_finite
        }
        _ => infinity,
    }
}

fn overflow_bits_double(negative: bool, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let sign = if negative { DOUBLE_SIGN_BIT } else { 0 };
    let infinity = sign | DOUBLE_EXP_MASK;
    let max_finite = sign | (DOUBLE_EXP_MASK - (1 << 52)) | DOUBLE_FRACTION_MASK;
    match (negative, rounding_mode) {
        (false, RiscvFloatRoundingMode::RoundTowardZero | RiscvFloatRoundingMode::RoundDown) => {
            max_finite
        }
        (true, RiscvFloatRoundingMode::RoundTowardZero | RiscvFloatRoundingMode::RoundUp) => {
            max_finite
        }
        _ => infinity,
    }
}

fn exact_overflows_single(exact: ExactSingleValue, rounding_mode: RiscvFloatRoundingMode) -> bool {
    exact_overflows(
        exact.negative,
        exact.magnitude,
        exact.shift,
        24,
        127,
        rounding_mode,
    )
}

fn exact_overflows_double(exact: ExactDoubleValue, rounding_mode: RiscvFloatRoundingMode) -> bool {
    exact_overflows(
        exact.negative,
        exact.magnitude,
        exact.shift,
        53,
        1023,
        rounding_mode,
    )
}

fn exact_overflows(
    negative: bool,
    magnitude: u128,
    shift: i32,
    precision: u32,
    max_exponent: i32,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    if magnitude == 0 {
        return false;
    }
    let bits = bit_length(magnitude);
    let exponent = shift + bits as i32 - 1;
    if exponent > max_exponent {
        return true;
    }
    if exponent < max_exponent || bits <= precision {
        return false;
    }

    let dropped_bits = bits - precision;
    let quotient = magnitude >> dropped_bits;
    let remainder = low_bits(magnitude, dropped_bits);
    should_increment(negative, quotient, remainder, dropped_bits, rounding_mode)
        && quotient + 1 == 1u128 << precision
}

fn exact_is_representable_as_single(exact: ExactSingleValue) -> bool {
    if exact.magnitude == 0 {
        return true;
    }
    let bits = bit_length(exact.magnitude);
    let exponent = exact.shift + bits as i32 - 1;
    if exponent > 127 {
        return false;
    }
    if exponent >= -126 {
        return bits <= 24 || low_bits(exact.magnitude, bits - 24) == 0;
    }

    let unit_shift = exact.shift + 149;
    if unit_shift >= 0 {
        exact
            .magnitude
            .checked_shl(unit_shift as u32)
            .is_some_and(|units| units < 1 << 23)
    } else {
        let dropped_bits = (-unit_shift) as u32;
        dropped_bits < 128 && low_bits(exact.magnitude, dropped_bits) == 0
    }
}

fn exact_is_representable_as_double(exact: ExactDoubleValue) -> bool {
    if exact.magnitude == 0 {
        return true;
    }
    let bits = bit_length(exact.magnitude);
    let exponent = exact.shift + bits as i32 - 1;
    if exponent > 1023 {
        return false;
    }
    if exponent >= -1022 {
        return bits <= 53 || low_bits(exact.magnitude, bits - 53) == 0;
    }

    let unit_shift = exact.shift + 1074;
    if unit_shift >= 0 {
        exact
            .magnitude
            .checked_shl(unit_shift as u32)
            .is_some_and(|units| units < 1 << 52)
    } else {
        let dropped_bits = (-unit_shift) as u32;
        dropped_bits < 128 && low_bits(exact.magnitude, dropped_bits) == 0
    }
}

fn low_bits(value: u128, bits: u32) -> u128 {
    if bits == 0 {
        0
    } else if bits >= 128 {
        value
    } else {
        value & ((1u128 << bits) - 1)
    }
}

fn bit_length(value: u128) -> u32 {
    128 - value.leading_zeros()
}

fn is_infinity_times_zero(lhs: u32, rhs: u32) -> bool {
    (is_infinity_single(lhs) && is_zero_single(rhs))
        || (is_zero_single(lhs) && is_infinity_single(rhs))
}

fn is_infinity_times_zero_double(lhs: u64, rhs: u64) -> bool {
    (is_infinity_double(lhs) && is_zero_double(rhs))
        || (is_zero_double(lhs) && is_infinity_double(rhs))
}

fn product_addend_are_opposite_infinities(lhs: u32, rhs: u32, addend: u32) -> bool {
    product_is_infinity(lhs, rhs)
        && is_infinity_single(addend)
        && product_is_negative(lhs, rhs) != has_single_sign(addend)
}

fn product_addend_are_opposite_infinities_double(lhs: u64, rhs: u64, addend: u64) -> bool {
    product_is_infinity_double(lhs, rhs)
        && is_infinity_double(addend)
        && product_is_negative_double(lhs, rhs) != has_double_sign(addend)
}

fn product_is_infinity(lhs: u32, rhs: u32) -> bool {
    (is_infinity_single(lhs) && finite_nonzero(rhs))
        || (is_infinity_single(rhs) && finite_nonzero(lhs))
}

fn product_is_infinity_double(lhs: u64, rhs: u64) -> bool {
    (is_infinity_double(lhs) && finite_nonzero_double(rhs))
        || (is_infinity_double(rhs) && finite_nonzero_double(lhs))
}

fn finite_nonzero(value: u32) -> bool {
    is_finite(value) && !is_zero_single(value) && !is_nan_single(value)
}

fn finite_nonzero_double(value: u64) -> bool {
    is_finite_double(value) && !is_zero_double(value) && !is_nan_double(value)
}

fn product_is_negative(lhs: u32, rhs: u32) -> bool {
    has_single_sign(lhs) != has_single_sign(rhs)
}

fn product_is_negative_double(lhs: u64, rhs: u64) -> bool {
    has_double_sign(lhs) != has_double_sign(rhs)
}

fn is_finite(value: u32) -> bool {
    value & SINGLE_EXP_MASK != SINGLE_EXP_MASK
}

fn is_finite_double(value: u64) -> bool {
    value & DOUBLE_EXP_MASK != DOUBLE_EXP_MASK
}

#[derive(Clone, Copy)]
struct ExactTerm {
    negative: bool,
    mantissa: u128,
    shift: i32,
}

impl ExactTerm {
    const fn zero(negative: bool) -> Self {
        Self {
            negative,
            mantissa: 0,
            shift: 0,
        }
    }

    fn nonzero_shift(self) -> Option<i32> {
        (self.mantissa != 0).then_some(self.shift)
    }
}

#[derive(Clone, Copy)]
struct ExactSingleValue {
    negative: bool,
    magnitude: u128,
    shift: i32,
}

#[derive(Clone, Copy)]
struct ExactDoubleValue {
    negative: bool,
    magnitude: u128,
    shift: i32,
}

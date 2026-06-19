use crate::RiscvFloatRoundingMode;

use super::{DOUBLE_EXP_MASK, DOUBLE_FRACTION_MASK, DOUBLE_SIGN_BIT};

pub(super) fn add_sub_bits(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
    subtract: bool,
) -> Option<u64> {
    let rhs = if subtract { rhs ^ DOUBLE_SIGN_BIT } else { rhs };
    let result = (f64::from_bits(lhs) + f64::from_bits(rhs)).to_bits();
    let target_shift = common_shift([lhs, rhs, result])?;
    let lhs_exact = scaled_integer(lhs, target_shift)?;
    let rhs_exact = scaled_integer(rhs, target_shift)?;
    let exact = lhs_exact.checked_add(rhs_exact)?;
    if exact == 0 && f64::from_bits(result) == 0.0 {
        return Some(exact_zero_add_bits(lhs, rhs, result, rounding_mode));
    }
    let result_exact = scaled_integer(result, target_shift)?;
    (exact == result_exact).then_some(result)
}

pub(super) fn mul_bits(lhs: u64, rhs: u64) -> Option<u64> {
    let result = (f64::from_bits(lhs) * f64::from_bits(rhs)).to_bits();
    let (lhs_negative, lhs_significand, lhs_shift) = significand_shift(lhs)?;
    let (rhs_negative, rhs_significand, rhs_shift) = significand_shift(rhs)?;
    let result_sign = if lhs_negative ^ rhs_negative {
        DOUBLE_SIGN_BIT
    } else {
        0
    };

    if lhs_significand == 0 || rhs_significand == 0 {
        return (f64::from_bits(result) == 0.0 && result & DOUBLE_SIGN_BIT == result_sign)
            .then_some(result);
    }
    if !f64::from_bits(result).is_finite() {
        return None;
    }

    let exact_significand = lhs_significand.checked_mul(rhs_significand)?;
    let exact_shift = lhs_shift.checked_add(rhs_shift)?;
    let (result_negative, result_significand, result_shift) = significand_shift(result)?;
    if result_significand == 0 || result_negative != (result_sign != 0) {
        return None;
    }

    scaled_equal(
        exact_significand,
        exact_shift,
        result_significand,
        result_shift,
    )?
    .then_some(result)
}

pub(super) fn div_bits(lhs: u64, rhs: u64) -> Option<u64> {
    let result = (f64::from_bits(lhs) / f64::from_bits(rhs)).to_bits();
    let (lhs_negative, lhs_significand, lhs_shift) = significand_shift(lhs)?;
    let (rhs_negative, rhs_significand, rhs_shift) = significand_shift(rhs)?;
    if rhs_significand == 0 {
        return None;
    }
    let result_sign = if lhs_negative ^ rhs_negative {
        DOUBLE_SIGN_BIT
    } else {
        0
    };

    if lhs_significand == 0 {
        return (f64::from_bits(result) == 0.0 && result & DOUBLE_SIGN_BIT == result_sign)
            .then_some(result);
    }
    if !f64::from_bits(result).is_finite() {
        return None;
    }

    let (result_negative, result_significand, result_shift) = significand_shift(result)?;
    if result_significand == 0 || result_negative != (result_sign != 0) {
        return None;
    }
    let recomposed_significand = result_significand.checked_mul(rhs_significand)?;
    let recomposed_shift = result_shift.checked_add(rhs_shift)?;
    scaled_equal(
        lhs_significand,
        lhs_shift,
        recomposed_significand,
        recomposed_shift,
    )?
    .then_some(result)
}

fn scaled_equal(
    lhs_significand: u128,
    lhs_shift: i32,
    rhs_significand: u128,
    rhs_shift: i32,
) -> Option<bool> {
    let target_shift = lhs_shift.min(rhs_shift);
    let lhs_scaled = scaled_significand(lhs_significand, lhs_shift, target_shift)?;
    let rhs_scaled = scaled_significand(rhs_significand, rhs_shift, target_shift)?;
    Some(lhs_scaled == rhs_scaled)
}

fn scaled_significand(significand: u128, shift: i32, target_shift: i32) -> Option<u128> {
    let delta: u32 = shift.checked_sub(target_shift)?.try_into().ok()?;
    significand.checked_shl(delta)
}

fn common_shift(values: [u64; 3]) -> Option<i32> {
    let mut common = None;
    for value in values {
        let (_negative, significand, shift) = significand_shift(value)?;
        if significand == 0 {
            continue;
        }
        common = Some(common.map_or(shift, |current: i32| current.min(shift)));
    }
    Some(common.unwrap_or(0))
}

fn scaled_integer(value: u64, target_shift: i32) -> Option<i128> {
    let (negative, significand, shift) = significand_shift(value)?;
    if significand == 0 {
        return Some(0);
    }
    let shift_delta = shift.checked_sub(target_shift)?;
    let magnitude = significand.checked_shl(shift_delta.try_into().ok()?)?;
    if magnitude > i128::MAX as u128 {
        return None;
    }
    let magnitude = magnitude as i128;
    Some(if negative { -magnitude } else { magnitude })
}

fn exact_zero_add_bits(
    lhs: u64,
    rhs: u64,
    result: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    let opposite_signs = (lhs ^ rhs) & DOUBLE_SIGN_BIT != 0;
    if opposite_signs && rounding_mode == RiscvFloatRoundingMode::RoundDown {
        DOUBLE_SIGN_BIT
    } else {
        result
    }
}

fn significand_shift(value: u64) -> Option<(bool, u128, i32)> {
    if !f64::from_bits(value).is_finite() {
        return None;
    }
    let negative = value & DOUBLE_SIGN_BIT != 0;
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

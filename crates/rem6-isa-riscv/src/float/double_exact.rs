use super::{finite_double_significand_shift, DOUBLE_SIGN_BIT};

pub(super) fn mul_bits(lhs: u64, rhs: u64) -> Option<u64> {
    let result = (f64::from_bits(lhs) * f64::from_bits(rhs)).to_bits();
    let (lhs_negative, lhs_significand, lhs_shift) = finite_double_significand_shift(lhs)?;
    let (rhs_negative, rhs_significand, rhs_shift) = finite_double_significand_shift(rhs)?;
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
    let (result_negative, result_significand, result_shift) =
        finite_double_significand_shift(result)?;
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
    let (lhs_negative, lhs_significand, lhs_shift) = finite_double_significand_shift(lhs)?;
    let (rhs_negative, rhs_significand, rhs_shift) = finite_double_significand_shift(rhs)?;
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

    let (result_negative, result_significand, result_shift) =
        finite_double_significand_shift(result)?;
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

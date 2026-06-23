use crate::RiscvFloatRoundingMode;

use super::{DOUBLE_EXP_MASK, DOUBLE_FRACTION_MASK, DOUBLE_SIGN_BIT};

pub(super) struct RoundedDouble {
    bits: u64,
    inexact: bool,
    overflow: bool,
}

impl RoundedDouble {
    const fn new(bits: u64, inexact: bool, overflow: bool) -> Self {
        Self {
            bits,
            inexact,
            overflow,
        }
    }

    pub(super) const fn bits(&self) -> u64 {
        self.bits
    }

    pub(super) const fn inexact(&self) -> bool {
        self.inexact
    }

    pub(super) const fn overflow(&self) -> bool {
        self.overflow
    }
}

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

pub(super) fn rounded_mul_bits(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> Option<RoundedDouble> {
    let (lhs_negative, lhs_significand, lhs_shift) = significand_shift(lhs)?;
    let (rhs_negative, rhs_significand, rhs_shift) = significand_shift(rhs)?;
    let result_sign = if lhs_negative ^ rhs_negative {
        DOUBLE_SIGN_BIT
    } else {
        0
    };
    if lhs_significand == 0 || rhs_significand == 0 {
        return Some(RoundedDouble::new(result_sign, false, false));
    }

    let exact_significand = lhs_significand.checked_mul(rhs_significand)?;
    let exact_shift = lhs_shift.checked_add(rhs_shift)?;
    round_significand_to_double(result_sign, exact_significand, exact_shift, rounding_mode)
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

fn round_significand_to_double(
    sign: u64,
    significand: u128,
    shift: i32,
    rounding_mode: RiscvFloatRoundingMode,
) -> Option<RoundedDouble> {
    let bit_width = bit_width(significand);
    if bit_width < DOUBLE_SIGNIFICAND_BITS {
        return None;
    }

    let discarded_bits = bit_width - DOUBLE_SIGNIFICAND_BITS;
    let mut retained = significand >> discarded_bits;
    let remainder = if discarded_bits == 0 {
        0
    } else {
        significand & ((1_u128 << discarded_bits) - 1)
    };
    let mut exponent = shift.checked_add((bit_width - 1).try_into().ok()?)?;

    if should_increment(
        sign != 0,
        retained,
        remainder,
        discarded_bits,
        rounding_mode,
    ) {
        retained = retained.checked_add(1)?;
        if retained == (1_u128 << DOUBLE_SIGNIFICAND_BITS) {
            retained >>= 1;
            exponent = exponent.checked_add(1)?;
        }
    }

    if exponent > DOUBLE_MAX_NORMAL_EXPONENT {
        return Some(RoundedDouble::new(
            overflow_bits(sign, rounding_mode),
            true,
            true,
        ));
    }
    if retained < (1_u128 << DOUBLE_FRACTION_BITS) || exponent < DOUBLE_MIN_NORMAL_EXPONENT {
        return None;
    }

    let exponent_bits = u64::try_from(exponent + DOUBLE_EXPONENT_BIAS).ok()?;
    let fraction = u64::try_from(retained & u128::from(DOUBLE_FRACTION_MASK)).ok()?;
    Some(RoundedDouble::new(
        sign | (exponent_bits << DOUBLE_FRACTION_BITS) | fraction,
        remainder != 0,
        false,
    ))
}

fn overflow_bits(sign: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let max_finite = sign | DOUBLE_MAX_FINITE_MAGNITUDE;
    let infinity = sign | DOUBLE_EXP_MASK;
    match rounding_mode {
        RiscvFloatRoundingMode::RoundTowardZero => max_finite,
        RiscvFloatRoundingMode::RoundDown if sign == 0 => max_finite,
        RiscvFloatRoundingMode::RoundUp if sign != 0 => max_finite,
        RiscvFloatRoundingMode::RoundNearestEven
        | RiscvFloatRoundingMode::RoundDown
        | RiscvFloatRoundingMode::RoundUp
        | RiscvFloatRoundingMode::RoundNearestMaxMagnitude => infinity,
        RiscvFloatRoundingMode::Dynamic => unreachable!("dynamic rounding mode must be resolved"),
    }
}

fn should_increment(
    negative: bool,
    retained: u128,
    remainder: u128,
    discarded_bits: u32,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    if remainder == 0 {
        return false;
    }

    match rounding_mode {
        RiscvFloatRoundingMode::RoundNearestEven => {
            let half = 1_u128 << (discarded_bits - 1);
            remainder > half || (remainder == half && retained & 1 == 1)
        }
        RiscvFloatRoundingMode::RoundTowardZero => false,
        RiscvFloatRoundingMode::RoundDown => negative,
        RiscvFloatRoundingMode::RoundUp => !negative,
        RiscvFloatRoundingMode::RoundNearestMaxMagnitude => {
            let half = 1_u128 << (discarded_bits - 1);
            remainder >= half
        }
        RiscvFloatRoundingMode::Dynamic => unreachable!("dynamic rounding mode must be resolved"),
    }
}

fn bit_width(value: u128) -> u32 {
    u128::BITS - value.leading_zeros()
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

const DOUBLE_FRACTION_BITS: u32 = 52;
const DOUBLE_SIGNIFICAND_BITS: u32 = 53;
const DOUBLE_EXPONENT_BIAS: i32 = 1023;
const DOUBLE_MIN_NORMAL_EXPONENT: i32 = -1022;
const DOUBLE_MAX_NORMAL_EXPONENT: i32 = 1023;
const DOUBLE_MAX_FINITE_MAGNITUDE: u64 = DOUBLE_EXP_MASK - 1;

use crate::RiscvFloatRoundingMode;

use super::{
    round_double, round_single, unbox_single, FLOAT_FLAG_INVALID, I32_MAX_PLUS_ONE_AS_SINGLE,
    I64_MAX_PLUS_ONE_AS_SINGLE, U32_MAX_PLUS_ONE_AS_SINGLE, U64_MAX_PLUS_ONE_AS_SINGLE,
};

pub(super) fn single_to_signed_word(value: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let value = f32::from_bits(unbox_single(value));
    if value.is_nan() {
        return FLOAT_FLAG_INVALID;
    }

    let rounded = round_single(value, rounding_mode);
    if !(-I32_MAX_PLUS_ONE_AS_SINGLE..I32_MAX_PLUS_ONE_AS_SINGLE).contains(&rounded) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

pub(super) fn single_to_unsigned_word(value: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let value = f32::from_bits(unbox_single(value));
    if value.is_nan() {
        return FLOAT_FLAG_INVALID;
    }

    let rounded = round_single(value, rounding_mode);
    if !(0.0..U32_MAX_PLUS_ONE_AS_SINGLE).contains(&rounded) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

pub(super) fn single_to_signed_doubleword(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    let value = f32::from_bits(unbox_single(value));
    if value.is_nan() {
        return FLOAT_FLAG_INVALID;
    }

    let rounded = round_single(value, rounding_mode);
    if !(-I64_MAX_PLUS_ONE_AS_SINGLE..I64_MAX_PLUS_ONE_AS_SINGLE).contains(&rounded) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

pub(super) fn single_to_unsigned_doubleword(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    let value = f32::from_bits(unbox_single(value));
    if value.is_nan() {
        return FLOAT_FLAG_INVALID;
    }

    let rounded = round_single(value, rounding_mode);
    if !(0.0..U64_MAX_PLUS_ONE_AS_SINGLE).contains(&rounded) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

pub(super) fn double_to_signed_word(value: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let value = f64::from_bits(value);
    if value.is_nan() {
        return FLOAT_FLAG_INVALID;
    }

    let rounded = round_double(value, rounding_mode);
    if !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&rounded) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

pub(super) fn double_to_unsigned_word(value: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let value = f64::from_bits(value);
    if value.is_nan() {
        return FLOAT_FLAG_INVALID;
    }

    let rounded = round_double(value, rounding_mode);
    if !(0.0..=f64::from(u32::MAX)).contains(&rounded) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

pub(super) fn double_to_signed_doubleword(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    let value = f64::from_bits(value);
    if value.is_nan() {
        return FLOAT_FLAG_INVALID;
    }

    let rounded = round_double(value, rounding_mode);
    if !((i64::MIN as f64)..(i64::MAX as f64)).contains(&rounded) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

pub(super) fn double_to_unsigned_doubleword(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> u64 {
    let value = f64::from_bits(value);
    if value.is_nan() {
        return FLOAT_FLAG_INVALID;
    }

    let rounded = round_double(value, rounding_mode);
    if !(0.0..(u64::MAX as f64)).contains(&rounded) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

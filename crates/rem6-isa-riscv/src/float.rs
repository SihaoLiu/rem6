use crate::{
    FloatRegister, FloatRegisterWrite, Register, RiscvFloatRoundingMode, RiscvHartState,
    RiscvInstruction,
};

mod add_sub;
mod constants;
mod convert_flags;
mod decode;
mod div;
mod double_exact;
mod fused;
mod int_to_float;
mod mul;
mod ternary;

pub(crate) use constants::{
    DEFAULT_NAN_DOUBLE, DEFAULT_NAN_SINGLE, DEFAULT_NAN_SINGLE_BITS, DOUBLE_EXP_MASK,
    DOUBLE_FRACTION_MASK, DOUBLE_QUIET_NAN_BIT, DOUBLE_SIGN_BIT, FLOAT_FLAG_DIVIDE_BY_ZERO,
    FLOAT_FLAG_INEXACT, FLOAT_FLAG_INVALID, FLOAT_FLAG_OVERFLOW, FLOAT_FLAG_UNDERFLOW,
    I32_MAX_PLUS_ONE_AS_SINGLE, I64_MAX_PLUS_ONE_AS_SINGLE, SINGLE_BOX_MASK, SINGLE_EXP_MASK,
    SINGLE_FRACTION_MASK, SINGLE_QUIET_NAN_BIT, SINGLE_SIGN_BIT, U32_MAX_PLUS_ONE_AS_SINGLE,
    U64_MAX_PLUS_ONE_AS_SINGLE,
};
pub(crate) use decode::{
    decode_float_load, decode_float_multiply_add, decode_float_op, decode_float_store,
};
pub(crate) use int_to_float::{
    exception_flags as integer_to_float_exception_flags,
    register_write as float_register_write_from_integer,
    rounding_mode_is_supported as integer_to_float_rounding_mode_is_supported,
};
pub(crate) use ternary::{
    float_register_write_ternary, ternary_exception_flags,
    ternary_register_rounding_mode_is_supported,
};

fn add_double(lhs: u64, rhs: u64) -> u64 {
    (f64::from_bits(lhs) + f64::from_bits(rhs)).to_bits()
}

pub(crate) fn float_register_write(
    instruction: RiscvInstruction,
    lhs: u64,
    rhs: u64,
) -> (FloatRegister, u64) {
    match instruction {
        RiscvInstruction::FloatAddS { rd, .. } => (rd, add_single(lhs, rhs)),
        RiscvInstruction::FloatAddD { rd, .. } => (rd, add_double(lhs, rhs)),
        RiscvInstruction::FloatSubS { rd, .. } => (rd, sub_single(lhs, rhs)),
        RiscvInstruction::FloatSubD { rd, .. } => (rd, sub_double(lhs, rhs)),
        RiscvInstruction::FloatMulS { rd, .. } => (rd, mul_single(lhs, rhs)),
        RiscvInstruction::FloatMulD { rd, .. } => (rd, mul_double(lhs, rhs)),
        RiscvInstruction::FloatDivS { rd, .. } => (rd, div_single(lhs, rhs)),
        RiscvInstruction::FloatDivD { rd, .. } => (rd, div_double(lhs, rhs)),
        RiscvInstruction::FloatSqrtS { rd, .. } => (rd, sqrt_single(lhs)),
        RiscvInstruction::FloatSqrtD { rd, .. } => (rd, sqrt_double(lhs)),
        RiscvInstruction::FloatSignInjectS { rd, .. } => (rd, sign_inject_single(lhs, rhs)),
        RiscvInstruction::FloatSignInjectD { rd, .. } => (rd, sign_inject_double(lhs, rhs)),
        RiscvInstruction::FloatSignInjectNegS { rd, .. } => (rd, sign_inject_neg_single(lhs, rhs)),
        RiscvInstruction::FloatSignInjectNegD { rd, .. } => (rd, sign_inject_neg_double(lhs, rhs)),
        RiscvInstruction::FloatSignInjectXorS { rd, .. } => (rd, sign_inject_xor_single(lhs, rhs)),
        RiscvInstruction::FloatSignInjectXorD { rd, .. } => (rd, sign_inject_xor_double(lhs, rhs)),
        RiscvInstruction::FloatMinS { rd, .. } => (rd, min_single(lhs, rhs)),
        RiscvInstruction::FloatMinD { rd, .. } => (rd, min_double(lhs, rhs)),
        RiscvInstruction::FloatMaxS { rd, .. } => (rd, max_single(lhs, rhs)),
        RiscvInstruction::FloatMaxD { rd, .. } => (rd, max_double(lhs, rhs)),
        RiscvInstruction::FloatConvertSFromD { rd, .. } => (rd, convert_double_to_single(lhs)),
        RiscvInstruction::FloatConvertDFromS { rd, .. } => (rd, convert_single_to_double(lhs)),
        _ => unreachable!("non-float-register instruction dispatched to float register write"),
    }
}

pub(crate) fn float_register_write_binary(
    instruction: RiscvInstruction,
    lhs: u64,
    rhs: u64,
    frm: u64,
) -> (FloatRegister, u64) {
    match instruction {
        RiscvInstruction::FloatAddS {
            rd, rounding_mode, ..
        } => (
            rd,
            add_sub::add_register_write(
                lhs,
                rhs,
                rounding_mode
                    .resolve(frm)
                    .expect("binary float rounding mode is valid"),
            ),
        ),
        RiscvInstruction::FloatSubS {
            rd, rounding_mode, ..
        } => (
            rd,
            add_sub::sub_register_write(
                lhs,
                rhs,
                rounding_mode
                    .resolve(frm)
                    .expect("binary float rounding mode is valid"),
            ),
        ),
        RiscvInstruction::FloatMulS {
            rd, rounding_mode, ..
        } => (
            rd,
            mul::register_write(
                lhs,
                rhs,
                rounding_mode
                    .resolve(frm)
                    .expect("binary float rounding mode is valid"),
            ),
        ),
        RiscvInstruction::FloatMulD {
            rd, rounding_mode, ..
        } => (
            rd,
            mul::register_write_double(
                lhs,
                rhs,
                rounding_mode
                    .resolve(frm)
                    .expect("binary float rounding mode is valid"),
            ),
        ),
        RiscvInstruction::FloatDivS {
            rd, rounding_mode, ..
        } => (
            rd,
            div::register_write(
                lhs,
                rhs,
                rounding_mode
                    .resolve(frm)
                    .expect("binary float rounding mode is valid"),
            ),
        ),
        RiscvInstruction::FloatDivD {
            rd, rounding_mode, ..
        } => (
            rd,
            div::register_write_double(
                lhs,
                rhs,
                rounding_mode
                    .resolve(frm)
                    .expect("binary float rounding mode is valid"),
            ),
        ),
        _ => float_register_write(instruction, lhs, rhs),
    }
}

pub(crate) fn exact_finite_single_add_sub_bits(
    lhs: u32,
    rhs: u32,
    rounding_mode: RiscvFloatRoundingMode,
    subtract: bool,
) -> Option<u32> {
    add_sub::exact_finite_single_bits(lhs, rhs, rounding_mode, subtract)
}

pub(crate) fn exact_finite_double_add_sub_bits(
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
    subtract: bool,
) -> Option<u64> {
    double_exact::add_sub_bits(lhs, rhs, rounding_mode, subtract)
}

pub(crate) fn exact_finite_double_mul_bits(lhs: u64, rhs: u64) -> Option<u64> {
    double_exact::mul_bits(lhs, rhs)
}

pub(crate) fn exact_finite_double_div_bits(lhs: u64, rhs: u64) -> Option<u64> {
    double_exact::div_bits(lhs, rhs)
}

pub(crate) fn exact_finite_single_mul_add_bits(
    lhs: u32,
    rhs: u32,
    addend: u32,
    rounding_mode: RiscvFloatRoundingMode,
) -> Option<u32> {
    fused::exact_finite_single_mul_add_bits(lhs, rhs, addend, rounding_mode)
}

pub(crate) fn unsigned_word_to_single_bits(
    value: u32,
    rounding_mode: RiscvFloatRoundingMode,
) -> (u32, u64) {
    int_to_float::unsigned_word_to_single_bits(value, rounding_mode)
}

pub(crate) fn signed_word_to_single_bits(
    value: u32,
    rounding_mode: RiscvFloatRoundingMode,
) -> (u32, u64) {
    int_to_float::signed_word_to_single_bits(value, rounding_mode)
}

pub(crate) fn unsigned_doubleword_to_double_bits(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> (u64, u64) {
    int_to_float::unsigned_doubleword_to_double_bits(value, rounding_mode)
}

pub(crate) fn signed_doubleword_to_double_bits(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> (u64, u64) {
    int_to_float::signed_doubleword_to_double_bits(value, rounding_mode)
}

pub(crate) fn single_to_unsigned_word_bits(
    value: u32,
    rounding_mode: RiscvFloatRoundingMode,
) -> (u32, u64) {
    let boxed = box_single(value);
    (
        convert_single_to_unsigned_word(boxed, rounding_mode) as u32,
        convert_flags::single_to_unsigned_word(boxed, rounding_mode),
    )
}

pub(crate) fn single_to_signed_word_bits(
    value: u32,
    rounding_mode: RiscvFloatRoundingMode,
) -> (u32, u64) {
    let boxed = box_single(value);
    (
        convert_single_to_signed_word(boxed, rounding_mode) as u32,
        convert_flags::single_to_signed_word(boxed, rounding_mode),
    )
}

pub(crate) fn double_to_unsigned_doubleword_bits(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> (u64, u64) {
    (
        convert_double_to_unsigned_doubleword(value, rounding_mode),
        convert_flags::double_to_unsigned_doubleword(value, rounding_mode),
    )
}

pub(crate) fn double_to_signed_doubleword_bits(
    value: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> (u64, u64) {
    (
        convert_double_to_signed_doubleword(value, rounding_mode),
        convert_flags::double_to_signed_doubleword(value, rounding_mode),
    )
}

pub(crate) fn single_register_bits(value: u64) -> u32 {
    unbox_single(value)
}

pub(crate) fn sqrt_single_bits(value: u32) -> u32 {
    let result = f32::from_bits(value).sqrt().to_bits();
    if is_nan_single(result) {
        DEFAULT_NAN_SINGLE_BITS
    } else {
        result
    }
}

pub(crate) fn sqrt_exception_flags_single_bits(value: u32) -> u64 {
    if is_signaling_nan_single(value) || is_negative_nonzero_non_nan_single(value) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

pub(crate) fn sqrt_single_rounding_insensitive_bits(value: u32) -> bool {
    sqrt_single_is_exact_bits(value)
}

pub(crate) fn equal_single_bits(lhs: u32, rhs: u32) -> bool {
    f32::from_bits(lhs) == f32::from_bits(rhs)
}

pub(crate) fn less_than_single_bits(lhs: u32, rhs: u32) -> bool {
    f32::from_bits(lhs) < f32::from_bits(rhs)
}

pub(crate) fn less_or_equal_single_bits(lhs: u32, rhs: u32) -> bool {
    f32::from_bits(lhs) <= f32::from_bits(rhs)
}

pub(crate) fn quiet_compare_exception_flags_single_bits(lhs: u32, rhs: u32) -> u64 {
    if is_signaling_nan_single(lhs) || is_signaling_nan_single(rhs) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

pub(crate) fn signaling_compare_exception_flags_single_bits(lhs: u32, rhs: u32) -> u64 {
    if is_nan_single(lhs) || is_nan_single(rhs) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

pub(crate) fn min_single_bits(lhs: u32, rhs: u32) -> u32 {
    if is_nan_single(lhs) && is_nan_single(rhs) {
        return DEFAULT_NAN_SINGLE_BITS;
    }
    if is_nan_single(lhs) {
        return rhs;
    }
    if is_nan_single(rhs) {
        return lhs;
    }

    let lhs_value = f32::from_bits(lhs);
    let rhs_value = f32::from_bits(rhs);
    if lhs_value < rhs_value || (lhs_value == rhs_value && has_single_sign(lhs)) {
        lhs
    } else {
        rhs
    }
}

pub(crate) fn max_single_bits(lhs: u32, rhs: u32) -> u32 {
    if is_nan_single(lhs) && is_nan_single(rhs) {
        return DEFAULT_NAN_SINGLE_BITS;
    }
    if is_nan_single(lhs) {
        return rhs;
    }
    if is_nan_single(rhs) {
        return lhs;
    }

    let lhs_value = f32::from_bits(lhs);
    let rhs_value = f32::from_bits(rhs);
    if rhs_value < lhs_value || (lhs_value == rhs_value && has_single_sign(rhs)) {
        lhs
    } else {
        rhs
    }
}

pub(crate) fn minmax_exception_flags_single_bits(lhs: u32, rhs: u32) -> u64 {
    if is_signaling_nan_single(lhs) || is_signaling_nan_single(rhs) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

pub(crate) fn min_double_bits(lhs: u64, rhs: u64) -> u64 {
    min_double(lhs, rhs)
}

pub(crate) fn max_double_bits(lhs: u64, rhs: u64) -> u64 {
    max_double(lhs, rhs)
}

pub(crate) fn minmax_exception_flags_double_bits(lhs: u64, rhs: u64) -> u64 {
    minmax_exception_flags_double(lhs, rhs)
}

pub(crate) fn binary_register_rounding_mode_is_supported(
    instruction: RiscvInstruction,
    frm: u64,
    lhs: u64,
    rhs: u64,
) -> bool {
    let Some(rounding_mode) = register_rounding_mode(instruction) else {
        return true;
    };

    match rounding_mode.resolve(frm) {
        Some(RiscvFloatRoundingMode::RoundNearestEven) => true,
        Some(
            mode @ (RiscvFloatRoundingMode::RoundTowardZero
            | RiscvFloatRoundingMode::RoundDown
            | RiscvFloatRoundingMode::RoundUp
            | RiscvFloatRoundingMode::RoundNearestMaxMagnitude),
        ) if binary_rounding_mode_is_implemented(instruction, lhs, rhs, mode) => true,
        Some(_) => binary_result_is_rounding_insensitive(instruction, lhs, rhs),
        None => false,
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
        false,
    )
}

fn register_rounding_mode_is_supported(
    instruction: RiscvInstruction,
    frm: u64,
    rounding_insensitive: bool,
    implemented_rounding: bool,
) -> bool {
    let Some(rounding_mode) = register_rounding_mode(instruction) else {
        return true;
    };

    match rounding_mode.resolve(frm) {
        Some(RiscvFloatRoundingMode::RoundNearestEven) => true,
        Some(
            RiscvFloatRoundingMode::RoundTowardZero
            | RiscvFloatRoundingMode::RoundDown
            | RiscvFloatRoundingMode::RoundUp
            | RiscvFloatRoundingMode::RoundNearestMaxMagnitude,
        ) if implemented_rounding => true,
        Some(_) => rounding_insensitive,
        None => false,
    }
}

fn register_rounding_mode(instruction: RiscvInstruction) -> Option<RiscvFloatRoundingMode> {
    let rounding_mode = match instruction {
        RiscvInstruction::FloatAddS { rounding_mode, .. }
        | RiscvInstruction::FloatAddD { rounding_mode, .. }
        | RiscvInstruction::FloatSubS { rounding_mode, .. }
        | RiscvInstruction::FloatSubD { rounding_mode, .. }
        | RiscvInstruction::FloatMulS { rounding_mode, .. }
        | RiscvInstruction::FloatMulD { rounding_mode, .. }
        | RiscvInstruction::FloatDivS { rounding_mode, .. }
        | RiscvInstruction::FloatDivD { rounding_mode, .. }
        | RiscvInstruction::FloatSqrtS { rounding_mode, .. }
        | RiscvInstruction::FloatSqrtD { rounding_mode, .. }
        | RiscvInstruction::FloatMultiplyAddS { rounding_mode, .. }
        | RiscvInstruction::FloatMultiplyAddD { rounding_mode, .. }
        | RiscvInstruction::FloatMultiplySubtractS { rounding_mode, .. }
        | RiscvInstruction::FloatMultiplySubtractD { rounding_mode, .. }
        | RiscvInstruction::FloatNegativeMultiplySubtractS { rounding_mode, .. }
        | RiscvInstruction::FloatNegativeMultiplySubtractD { rounding_mode, .. }
        | RiscvInstruction::FloatNegativeMultiplyAddS { rounding_mode, .. }
        | RiscvInstruction::FloatNegativeMultiplyAddD { rounding_mode, .. } => rounding_mode,
        _ => return None,
    };
    Some(rounding_mode)
}

fn binary_rounding_mode_is_implemented(
    instruction: RiscvInstruction,
    lhs: u64,
    rhs: u64,
    rounding_mode: RiscvFloatRoundingMode,
) -> bool {
    match instruction {
        RiscvInstruction::FloatAddS { .. } => add_sub::add_directed_rounding_is_supported(lhs, rhs),
        RiscvInstruction::FloatSubS { .. } => add_sub::sub_directed_rounding_is_supported(lhs, rhs),
        RiscvInstruction::FloatMulS { .. } => mul::directed_rounding_is_supported(lhs, rhs),
        RiscvInstruction::FloatMulD { .. } => {
            mul::double_directed_rounding_is_supported(lhs, rhs, rounding_mode)
        }
        RiscvInstruction::FloatDivS { .. } => div::directed_rounding_is_supported(lhs, rhs),
        RiscvInstruction::FloatDivD { .. } => {
            div::double_directed_rounding_is_supported(lhs, rhs, rounding_mode)
        }
        _ => false,
    }
}

fn binary_result_is_rounding_insensitive(
    instruction: RiscvInstruction,
    lhs: u64,
    rhs: u64,
) -> bool {
    match instruction {
        RiscvInstruction::FloatAddS { .. } => add_sub_single_is_identity(lhs, rhs),
        RiscvInstruction::FloatAddD { .. } => add_sub_double_is_identity(lhs, rhs),
        RiscvInstruction::FloatSubS { .. } => add_sub_single_is_identity(lhs, rhs),
        RiscvInstruction::FloatSubD { .. } => add_sub_double_is_identity(lhs, rhs),
        RiscvInstruction::FloatMulS { .. } => multiply_single_is_identity(lhs, rhs),
        RiscvInstruction::FloatMulD { .. } => multiply_double_is_identity(lhs, rhs),
        RiscvInstruction::FloatDivS { .. } => divide_single_is_identity(lhs, rhs),
        RiscvInstruction::FloatDivD { .. } => divide_double_is_identity(lhs, rhs),
        _ => false,
    }
}

fn unary_result_is_rounding_insensitive(instruction: RiscvInstruction, value: u64) -> bool {
    match instruction {
        RiscvInstruction::FloatSqrtS { .. } => sqrt_single_is_exact(value),
        RiscvInstruction::FloatSqrtD { .. } => sqrt_double_is_exact(value),
        _ => false,
    }
}

fn add_sub_single_is_identity(lhs: u64, rhs: u64) -> bool {
    let lhs = f32::from_bits(unbox_single(lhs));
    let rhs = f32::from_bits(unbox_single(rhs));
    lhs.is_finite() && rhs.is_finite() && ((lhs != 0.0 && rhs == 0.0) || (lhs == 0.0 && rhs != 0.0))
}

fn add_sub_double_is_identity(lhs: u64, rhs: u64) -> bool {
    let lhs = f64::from_bits(lhs);
    let rhs = f64::from_bits(rhs);
    lhs.is_finite() && rhs.is_finite() && ((lhs != 0.0 && rhs == 0.0) || (lhs == 0.0 && rhs != 0.0))
}

fn multiply_single_is_identity(lhs: u64, rhs: u64) -> bool {
    let lhs = f32::from_bits(unbox_single(lhs));
    let rhs = f32::from_bits(unbox_single(rhs));
    lhs.is_finite() && rhs.is_finite() && (lhs.abs() == 1.0 || rhs.abs() == 1.0)
}

fn multiply_double_is_identity(lhs: u64, rhs: u64) -> bool {
    let lhs = f64::from_bits(lhs);
    let rhs = f64::from_bits(rhs);
    lhs.is_finite() && rhs.is_finite() && (lhs.abs() == 1.0 || rhs.abs() == 1.0)
}

fn divide_single_is_identity(lhs: u64, rhs: u64) -> bool {
    let lhs = f32::from_bits(unbox_single(lhs));
    let rhs = f32::from_bits(unbox_single(rhs));
    lhs.is_finite() && rhs.abs() == 1.0
}

fn divide_double_is_identity(lhs: u64, rhs: u64) -> bool {
    let lhs = f64::from_bits(lhs);
    let rhs = f64::from_bits(rhs);
    lhs.is_finite() && rhs.abs() == 1.0
}

fn sqrt_single_is_exact(value: u64) -> bool {
    sqrt_single_is_exact_bits(unbox_single(value))
}

fn sqrt_single_is_exact_bits(value: u32) -> bool {
    let value = f32::from_bits(value);
    if value == 0.0 {
        return true;
    }
    if !value.is_finite() || value < 0.0 {
        return false;
    }

    let root = value.sqrt().to_bits();
    [root.saturating_sub(1), root, root.saturating_add(1)]
        .into_iter()
        .filter(|candidate| candidate & 0x8000_0000 == 0)
        .any(|candidate| {
            let root = f32::from_bits(candidate);
            root.is_finite() && f64::from(root) * f64::from(root) == f64::from(value)
        })
}

fn sqrt_double_is_exact(value: u64) -> bool {
    let value = f64::from_bits(value);
    value.is_finite()
        && value >= 0.0
        && value.fract() == 0.0
        && value <= 9_007_199_254_740_992.0
        && is_square_u64(value as u64)
}

fn is_square_u64(value: u64) -> bool {
    let root = (value as f64).sqrt() as u64;
    [root.saturating_sub(1), root, root.saturating_add(1)]
        .into_iter()
        .any(|candidate| candidate.checked_mul(candidate) == Some(value))
}

pub(crate) fn integer_register_write(
    instruction: RiscvInstruction,
    lhs: u64,
    rhs: u64,
) -> Option<(Register, u64)> {
    match instruction {
        RiscvInstruction::FloatLessOrEqualS { rd, .. } => {
            Some((rd, u64::from(less_or_equal_single(lhs, rhs))))
        }
        RiscvInstruction::FloatLessOrEqualD { rd, .. } => {
            Some((rd, u64::from(less_or_equal_double(lhs, rhs))))
        }
        RiscvInstruction::FloatLessThanS { rd, .. } => {
            Some((rd, u64::from(less_than_single(lhs, rhs))))
        }
        RiscvInstruction::FloatLessThanD { rd, .. } => {
            Some((rd, u64::from(less_than_double(lhs, rhs))))
        }
        RiscvInstruction::FloatEqualS { rd, .. } => Some((rd, u64::from(equal_single(lhs, rhs)))),
        RiscvInstruction::FloatEqualD { rd, .. } => Some((rd, u64::from(equal_double(lhs, rhs)))),
        RiscvInstruction::FloatClassS { rd, .. } => Some((rd, class_single(lhs))),
        RiscvInstruction::FloatClassD { rd, .. } => Some((rd, class_double(lhs))),
        RiscvInstruction::FloatMoveXFromS { rd, .. } => {
            Some((rd, unbox_raw_single(lhs) as i32 as i64 as u64))
        }
        RiscvInstruction::FloatConvertWFromS {
            rd, rounding_mode, ..
        } => Some((
            rd,
            convert_single_to_signed_word(lhs, rounding_mode.resolve(rhs)?),
        )),
        RiscvInstruction::FloatConvertWuFromS {
            rd, rounding_mode, ..
        } => Some((
            rd,
            convert_single_to_unsigned_word(lhs, rounding_mode.resolve(rhs)?),
        )),
        RiscvInstruction::FloatConvertLFromS {
            rd, rounding_mode, ..
        } => Some((
            rd,
            convert_single_to_signed_doubleword(lhs, rounding_mode.resolve(rhs)?),
        )),
        RiscvInstruction::FloatConvertLuFromS {
            rd, rounding_mode, ..
        } => Some((
            rd,
            convert_single_to_unsigned_doubleword(lhs, rounding_mode.resolve(rhs)?),
        )),
        RiscvInstruction::FloatMoveXFromD { rd, .. } => Some((rd, lhs)),
        RiscvInstruction::FloatConvertWFromD {
            rd, rounding_mode, ..
        } => Some((
            rd,
            convert_double_to_signed_word(lhs, rounding_mode.resolve(rhs)?),
        )),
        RiscvInstruction::FloatConvertWuFromD {
            rd, rounding_mode, ..
        } => Some((
            rd,
            convert_double_to_unsigned_word(lhs, rounding_mode.resolve(rhs)?),
        )),
        RiscvInstruction::FloatConvertLFromD {
            rd, rounding_mode, ..
        } => Some((
            rd,
            convert_double_to_signed_doubleword(lhs, rounding_mode.resolve(rhs)?),
        )),
        RiscvInstruction::FloatConvertLuFromD {
            rd, rounding_mode, ..
        } => Some((
            rd,
            convert_double_to_unsigned_doubleword(lhs, rounding_mode.resolve(rhs)?),
        )),
        _ => unreachable!("non-float-compare instruction dispatched to integer register write"),
    }
}

pub(crate) fn integer_register_write_rne(
    instruction: RiscvInstruction,
    lhs: u64,
    rhs: u64,
) -> (Register, u64) {
    integer_register_write(instruction, lhs, rhs).expect("RNE float integer write is valid")
}

pub(crate) fn integer_exception_flags(instruction: RiscvInstruction, lhs: u64, rhs: u64) -> u64 {
    match instruction {
        RiscvInstruction::FloatLessOrEqualS { .. } | RiscvInstruction::FloatLessThanS { .. } => {
            signaling_compare_exception_flags_single(lhs, rhs)
        }
        RiscvInstruction::FloatLessOrEqualD { .. } | RiscvInstruction::FloatLessThanD { .. } => {
            signaling_compare_exception_flags_double(lhs, rhs)
        }
        RiscvInstruction::FloatEqualS { .. } => quiet_compare_exception_flags_single(lhs, rhs),
        RiscvInstruction::FloatEqualD { .. } => quiet_compare_exception_flags_double(lhs, rhs),
        RiscvInstruction::FloatConvertWFromS { rounding_mode, .. } => rounding_mode
            .resolve(rhs)
            .map_or(0, |mode| convert_flags::single_to_signed_word(lhs, mode)),
        RiscvInstruction::FloatConvertWuFromS { rounding_mode, .. } => rounding_mode
            .resolve(rhs)
            .map_or(0, |mode| convert_flags::single_to_unsigned_word(lhs, mode)),
        RiscvInstruction::FloatConvertLFromS { rounding_mode, .. } => {
            rounding_mode.resolve(rhs).map_or(0, |mode| {
                convert_flags::single_to_signed_doubleword(lhs, mode)
            })
        }
        RiscvInstruction::FloatConvertLuFromS { rounding_mode, .. } => {
            rounding_mode.resolve(rhs).map_or(0, |mode| {
                convert_flags::single_to_unsigned_doubleword(lhs, mode)
            })
        }
        RiscvInstruction::FloatConvertWFromD { rounding_mode, .. } => rounding_mode
            .resolve(rhs)
            .map_or(0, |mode| convert_flags::double_to_signed_word(lhs, mode)),
        RiscvInstruction::FloatConvertWuFromD { rounding_mode, .. } => rounding_mode
            .resolve(rhs)
            .map_or(0, |mode| convert_flags::double_to_unsigned_word(lhs, mode)),
        RiscvInstruction::FloatConvertLFromD { rounding_mode, .. } => {
            rounding_mode.resolve(rhs).map_or(0, |mode| {
                convert_flags::double_to_signed_doubleword(lhs, mode)
            })
        }
        RiscvInstruction::FloatConvertLuFromD { rounding_mode, .. } => {
            rounding_mode.resolve(rhs).map_or(0, |mode| {
                convert_flags::double_to_unsigned_doubleword(lhs, mode)
            })
        }
        _ => 0,
    }
}

fn round_single(value: f32, rounding_mode: RiscvFloatRoundingMode) -> f32 {
    match rounding_mode {
        RiscvFloatRoundingMode::RoundNearestEven => value.round_ties_even(),
        RiscvFloatRoundingMode::RoundTowardZero => value.trunc(),
        RiscvFloatRoundingMode::RoundDown => value.floor(),
        RiscvFloatRoundingMode::RoundUp => value.ceil(),
        RiscvFloatRoundingMode::RoundNearestMaxMagnitude => value.round(),
        RiscvFloatRoundingMode::Dynamic => unreachable!("dynamic rounding mode must be resolved"),
    }
}

fn round_double(value: f64, rounding_mode: RiscvFloatRoundingMode) -> f64 {
    match rounding_mode {
        RiscvFloatRoundingMode::RoundNearestEven => value.round_ties_even(),
        RiscvFloatRoundingMode::RoundTowardZero => value.trunc(),
        RiscvFloatRoundingMode::RoundDown => value.floor(),
        RiscvFloatRoundingMode::RoundUp => value.ceil(),
        RiscvFloatRoundingMode::RoundNearestMaxMagnitude => value.round(),
        RiscvFloatRoundingMode::Dynamic => unreachable!("dynamic rounding mode must be resolved"),
    }
}

pub(crate) fn write_float_register(
    hart: &mut RiscvHartState,
    writes: &mut Vec<FloatRegisterWrite>,
    register: FloatRegister,
    value: u64,
) {
    hart.write_float(register, value);
    writes.push(FloatRegisterWrite::new(register, value));
}

pub(crate) fn binary_exception_flags(
    instruction: RiscvInstruction,
    lhs: u64,
    rhs: u64,
    frm: u64,
) -> u64 {
    match instruction {
        RiscvInstruction::FloatAddS { rounding_mode, .. } => add_sub::add_exception_flags(
            lhs,
            rhs,
            rounding_mode
                .resolve(frm)
                .expect("binary float rounding mode is valid"),
        ),
        RiscvInstruction::FloatSubS { rounding_mode, .. } => add_sub::sub_exception_flags(
            lhs,
            rhs,
            rounding_mode
                .resolve(frm)
                .expect("binary float rounding mode is valid"),
        ),
        RiscvInstruction::FloatDivS { rounding_mode, .. } => div::exception_flags(
            lhs,
            rhs,
            rounding_mode
                .resolve(frm)
                .expect("binary float rounding mode is valid"),
        ),
        RiscvInstruction::FloatDivD { rounding_mode, .. } => div::exception_flags_double(
            lhs,
            rhs,
            rounding_mode
                .resolve(frm)
                .expect("binary float rounding mode is valid"),
        ),
        RiscvInstruction::FloatMulD { rounding_mode, .. } => mul::exception_flags_double(
            lhs,
            rhs,
            rounding_mode
                .resolve(frm)
                .expect("binary float rounding mode is valid"),
        ),
        RiscvInstruction::FloatMulS { rounding_mode, .. } => mul::exception_flags(
            lhs,
            rhs,
            rounding_mode
                .resolve(frm)
                .expect("binary float rounding mode is valid"),
        ),
        RiscvInstruction::FloatMinS { .. } | RiscvInstruction::FloatMaxS { .. } => {
            minmax_exception_flags_single(lhs, rhs)
        }
        RiscvInstruction::FloatMinD { .. } | RiscvInstruction::FloatMaxD { .. } => {
            minmax_exception_flags_double(lhs, rhs)
        }
        _ => 0,
    }
}

pub(crate) fn unary_exception_flags(instruction: RiscvInstruction, value: u64) -> u64 {
    match instruction {
        RiscvInstruction::FloatSqrtS { .. } => sqrt_exception_flags_single(value),
        RiscvInstruction::FloatSqrtD { .. } => sqrt_exception_flags_double(value),
        _ => 0,
    }
}

fn add_single(lhs: u64, rhs: u64) -> u64 {
    box_canonical_single(f32::from_bits(unbox_single(lhs)) + f32::from_bits(unbox_single(rhs)))
}

fn sub_double(lhs: u64, rhs: u64) -> u64 {
    (f64::from_bits(lhs) - f64::from_bits(rhs)).to_bits()
}

fn sub_single(lhs: u64, rhs: u64) -> u64 {
    box_canonical_single(f32::from_bits(unbox_single(lhs)) - f32::from_bits(unbox_single(rhs)))
}

fn mul_double(lhs: u64, rhs: u64) -> u64 {
    (f64::from_bits(lhs) * f64::from_bits(rhs)).to_bits()
}

fn mul_single(lhs: u64, rhs: u64) -> u64 {
    box_canonical_single(f32::from_bits(unbox_single(lhs)) * f32::from_bits(unbox_single(rhs)))
}

fn div_double(lhs: u64, rhs: u64) -> u64 {
    (f64::from_bits(lhs) / f64::from_bits(rhs)).to_bits()
}

fn div_single(lhs: u64, rhs: u64) -> u64 {
    box_canonical_single(f32::from_bits(unbox_single(lhs)) / f32::from_bits(unbox_single(rhs)))
}

fn minmax_exception_flags_single(lhs: u64, rhs: u64) -> u64 {
    minmax_exception_flags_single_bits(unbox_single(lhs), unbox_single(rhs))
}

fn minmax_exception_flags_double(lhs: u64, rhs: u64) -> u64 {
    if is_signaling_nan_double(lhs) || is_signaling_nan_double(rhs) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

fn sqrt_exception_flags_single(value: u64) -> u64 {
    sqrt_exception_flags_single_bits(unbox_single(value))
}

fn sqrt_exception_flags_double(value: u64) -> u64 {
    if is_signaling_nan_double(value) || is_negative_nonzero_non_nan_double(value) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

fn signaling_compare_exception_flags_single(lhs: u64, rhs: u64) -> u64 {
    signaling_compare_exception_flags_single_bits(unbox_single(lhs), unbox_single(rhs))
}

fn signaling_compare_exception_flags_double(lhs: u64, rhs: u64) -> u64 {
    if is_nan_double(lhs) || is_nan_double(rhs) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

fn quiet_compare_exception_flags_single(lhs: u64, rhs: u64) -> u64 {
    quiet_compare_exception_flags_single_bits(unbox_single(lhs), unbox_single(rhs))
}

fn quiet_compare_exception_flags_double(lhs: u64, rhs: u64) -> u64 {
    if is_signaling_nan_double(lhs) || is_signaling_nan_double(rhs) {
        FLOAT_FLAG_INVALID
    } else {
        0
    }
}

fn sqrt_single(value: u64) -> u64 {
    box_single(sqrt_single_bits(unbox_single(value)))
}

fn sqrt_double(value: u64) -> u64 {
    let result = f64::from_bits(value).sqrt().to_bits();
    if is_nan_double(result) {
        DEFAULT_NAN_DOUBLE
    } else {
        result
    }
}

fn sign_inject_single(lhs: u64, rhs: u64) -> u64 {
    box_single((unbox_single(lhs) & !SINGLE_SIGN_BIT) | (unbox_raw_single(rhs) & SINGLE_SIGN_BIT))
}

fn sign_inject_double(lhs: u64, rhs: u64) -> u64 {
    (lhs & !DOUBLE_SIGN_BIT) | (rhs & DOUBLE_SIGN_BIT)
}

fn sign_negate_single(value: u64) -> u64 {
    box_single(unbox_single(value) ^ SINGLE_SIGN_BIT)
}

fn sign_negate_double(value: u64) -> u64 {
    value ^ DOUBLE_SIGN_BIT
}

fn convert_double_to_single(value: u64) -> u64 {
    box_canonical_single(f64::from_bits(value) as f32)
}

fn convert_single_to_double(value: u64) -> u64 {
    let result = (f32::from_bits(unbox_single(value)) as f64).to_bits();
    if is_nan_double(result) {
        DEFAULT_NAN_DOUBLE
    } else {
        result
    }
}

fn convert_single_to_signed_word(value: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let value = f32::from_bits(unbox_single(value));
    if value.is_nan() {
        return i32::MAX as u64;
    }

    let rounded = round_single(value, rounding_mode);
    if rounded >= I32_MAX_PLUS_ONE_AS_SINGLE {
        i32::MAX as u64
    } else if rounded < -I32_MAX_PLUS_ONE_AS_SINGLE {
        i32::MIN as i64 as u64
    } else {
        rounded as i32 as i64 as u64
    }
}

fn convert_single_to_unsigned_word(value: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let value = f32::from_bits(unbox_single(value));
    if value.is_nan() {
        return sign_extend_unsigned_word(u32::MAX);
    }

    let rounded = round_single(value, rounding_mode);
    if rounded < 0.0 {
        0
    } else if rounded >= U32_MAX_PLUS_ONE_AS_SINGLE {
        sign_extend_unsigned_word(u32::MAX)
    } else {
        sign_extend_unsigned_word(rounded as u32)
    }
}

fn convert_single_to_signed_doubleword(value: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let value = f32::from_bits(unbox_single(value));
    if value.is_nan() {
        return i64::MAX as u64;
    }

    let rounded = round_single(value, rounding_mode);
    if rounded >= I64_MAX_PLUS_ONE_AS_SINGLE {
        i64::MAX as u64
    } else if rounded < -I64_MAX_PLUS_ONE_AS_SINGLE {
        i64::MIN as u64
    } else {
        rounded as i64 as u64
    }
}

fn convert_single_to_unsigned_doubleword(value: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let value = f32::from_bits(unbox_single(value));
    if value.is_nan() {
        return u64::MAX;
    }

    let rounded = round_single(value, rounding_mode);
    if rounded < 0.0 {
        0
    } else if rounded >= U64_MAX_PLUS_ONE_AS_SINGLE {
        u64::MAX
    } else {
        rounded as u64
    }
}

fn convert_double_to_signed_word(value: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let value = f64::from_bits(value);
    if value.is_nan() {
        return i32::MAX as u64;
    }

    let rounded = round_double(value, rounding_mode);
    if rounded > f64::from(i32::MAX) {
        i32::MAX as u64
    } else if rounded < f64::from(i32::MIN) {
        i32::MIN as i64 as u64
    } else {
        rounded as i32 as i64 as u64
    }
}

fn convert_double_to_unsigned_word(value: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let value = f64::from_bits(value);
    if value.is_nan() {
        return sign_extend_unsigned_word(u32::MAX);
    }

    let rounded = round_double(value, rounding_mode);
    if rounded < 0.0 {
        0
    } else if rounded > f64::from(u32::MAX) {
        sign_extend_unsigned_word(u32::MAX)
    } else {
        sign_extend_unsigned_word(rounded as u32)
    }
}

fn convert_double_to_signed_doubleword(value: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let value = f64::from_bits(value);
    if value.is_nan() {
        return i64::MAX as u64;
    }

    let rounded = round_double(value, rounding_mode);
    if rounded >= i64::MAX as f64 {
        i64::MAX as u64
    } else if rounded < i64::MIN as f64 {
        i64::MIN as u64
    } else {
        rounded as i64 as u64
    }
}

fn convert_double_to_unsigned_doubleword(value: u64, rounding_mode: RiscvFloatRoundingMode) -> u64 {
    let value = f64::from_bits(value);
    if value.is_nan() {
        return u64::MAX;
    }

    let rounded = round_double(value, rounding_mode);
    if rounded < 0.0 {
        0
    } else if rounded >= u64::MAX as f64 {
        u64::MAX
    } else {
        rounded as u64
    }
}

fn sign_extend_unsigned_word(value: u32) -> u64 {
    value as i32 as i64 as u64
}

fn sign_inject_neg_single(lhs: u64, rhs: u64) -> u64 {
    box_single(
        (unbox_single(lhs) & !SINGLE_SIGN_BIT) | ((!unbox_raw_single(rhs)) & SINGLE_SIGN_BIT),
    )
}

fn sign_inject_neg_double(lhs: u64, rhs: u64) -> u64 {
    (lhs & !DOUBLE_SIGN_BIT) | ((!rhs) & DOUBLE_SIGN_BIT)
}

fn sign_inject_xor_single(lhs: u64, rhs: u64) -> u64 {
    box_single(
        (unbox_single(lhs) & !SINGLE_SIGN_BIT)
            | ((unbox_single(lhs) ^ unbox_raw_single(rhs)) & SINGLE_SIGN_BIT),
    )
}

fn sign_inject_xor_double(lhs: u64, rhs: u64) -> u64 {
    (lhs & !DOUBLE_SIGN_BIT) | ((lhs ^ rhs) & DOUBLE_SIGN_BIT)
}

fn min_single(lhs: u64, rhs: u64) -> u64 {
    box_single(min_single_bits(unbox_single(lhs), unbox_single(rhs)))
}

fn min_double(lhs: u64, rhs: u64) -> u64 {
    if is_nan_double(lhs) && is_nan_double(rhs) {
        return DEFAULT_NAN_DOUBLE;
    }
    if is_nan_double(lhs) {
        return rhs;
    }
    if is_nan_double(rhs) {
        return lhs;
    }

    let lhs_value = f64::from_bits(lhs);
    let rhs_value = f64::from_bits(rhs);
    if lhs_value < rhs_value || (lhs_value == rhs_value && has_double_sign(lhs)) {
        lhs
    } else {
        rhs
    }
}

fn max_single(lhs: u64, rhs: u64) -> u64 {
    box_single(max_single_bits(unbox_single(lhs), unbox_single(rhs)))
}

fn max_double(lhs: u64, rhs: u64) -> u64 {
    if is_nan_double(lhs) && is_nan_double(rhs) {
        return DEFAULT_NAN_DOUBLE;
    }
    if is_nan_double(lhs) {
        return rhs;
    }
    if is_nan_double(rhs) {
        return lhs;
    }

    let lhs_value = f64::from_bits(lhs);
    let rhs_value = f64::from_bits(rhs);
    if rhs_value < lhs_value || (lhs_value == rhs_value && has_double_sign(rhs)) {
        lhs
    } else {
        rhs
    }
}

fn less_or_equal_single(lhs: u64, rhs: u64) -> bool {
    less_or_equal_single_bits(unbox_single(lhs), unbox_single(rhs))
}

fn less_or_equal_double(lhs: u64, rhs: u64) -> bool {
    f64::from_bits(lhs) <= f64::from_bits(rhs)
}

fn less_than_single(lhs: u64, rhs: u64) -> bool {
    less_than_single_bits(unbox_single(lhs), unbox_single(rhs))
}

fn less_than_double(lhs: u64, rhs: u64) -> bool {
    f64::from_bits(lhs) < f64::from_bits(rhs)
}

fn equal_single(lhs: u64, rhs: u64) -> bool {
    equal_single_bits(unbox_single(lhs), unbox_single(rhs))
}

fn equal_double(lhs: u64, rhs: u64) -> bool {
    f64::from_bits(lhs) == f64::from_bits(rhs)
}

pub(crate) fn class_single_bits(value: u32) -> u32 {
    let exponent = value & SINGLE_EXP_MASK;
    let fraction = value & SINGLE_FRACTION_MASK;
    let sign = has_single_sign(value);

    if exponent == SINGLE_EXP_MASK {
        if fraction == 0 {
            return if sign { 1 << 0 } else { 1 << 7 };
        }
        return if value & SINGLE_QUIET_NAN_BIT == 0 {
            1 << 8
        } else {
            1 << 9
        };
    }

    if exponent == 0 {
        if fraction == 0 {
            return if sign { 1 << 3 } else { 1 << 4 };
        }
        return if sign { 1 << 2 } else { 1 << 5 };
    }

    if sign {
        1 << 1
    } else {
        1 << 6
    }
}

fn class_single(value: u64) -> u64 {
    u64::from(class_single_bits(unbox_single(value)))
}

fn class_double(value: u64) -> u64 {
    let exponent = value & DOUBLE_EXP_MASK;
    let fraction = value & DOUBLE_FRACTION_MASK;
    let sign = has_double_sign(value);

    if exponent == DOUBLE_EXP_MASK {
        if fraction == 0 {
            return if sign { 1 << 0 } else { 1 << 7 };
        }
        return if value & DOUBLE_QUIET_NAN_BIT == 0 {
            1 << 8
        } else {
            1 << 9
        };
    }

    if exponent == 0 {
        if fraction == 0 {
            return if sign { 1 << 3 } else { 1 << 4 };
        }
        return if sign { 1 << 2 } else { 1 << 5 };
    }

    if sign {
        1 << 1
    } else {
        1 << 6
    }
}

fn box_canonical_single(value: f32) -> u64 {
    let bits = value.to_bits();
    if is_nan_single(bits) {
        DEFAULT_NAN_SINGLE
    } else {
        box_single(bits)
    }
}

fn box_single(value: u32) -> u64 {
    SINGLE_BOX_MASK | u64::from(value)
}

fn unbox_single(value: u64) -> u32 {
    if value & SINGLE_BOX_MASK == SINGLE_BOX_MASK {
        value as u32
    } else {
        DEFAULT_NAN_SINGLE_BITS
    }
}

fn unbox_raw_single(value: u64) -> u32 {
    value as u32
}

fn is_nan_single(value: u32) -> bool {
    value & SINGLE_EXP_MASK == SINGLE_EXP_MASK && value & SINGLE_FRACTION_MASK != 0
}

fn is_nan_double(value: u64) -> bool {
    value & DOUBLE_EXP_MASK == DOUBLE_EXP_MASK && value & DOUBLE_FRACTION_MASK != 0
}

fn is_signaling_nan_single(value: u32) -> bool {
    is_nan_single(value) && value & SINGLE_QUIET_NAN_BIT == 0
}

fn is_signaling_nan_double(value: u64) -> bool {
    is_nan_double(value) && value & DOUBLE_QUIET_NAN_BIT == 0
}

fn has_single_sign(value: u32) -> bool {
    value & SINGLE_SIGN_BIT != 0
}

fn has_double_sign(value: u64) -> bool {
    value & DOUBLE_SIGN_BIT != 0
}

fn is_zero_single(value: u32) -> bool {
    value & !SINGLE_SIGN_BIT == 0
}

fn is_zero_double(value: u64) -> bool {
    value & !DOUBLE_SIGN_BIT == 0
}

fn is_infinity_single(value: u32) -> bool {
    value & !SINGLE_SIGN_BIT == SINGLE_EXP_MASK
}

fn is_infinity_double(value: u64) -> bool {
    value & !DOUBLE_SIGN_BIT == DOUBLE_EXP_MASK
}

fn is_negative_nonzero_non_nan_single(value: u32) -> bool {
    has_single_sign(value) && value & !SINGLE_SIGN_BIT != 0 && !is_nan_single(value)
}

fn is_negative_nonzero_non_nan_double(value: u64) -> bool {
    has_double_sign(value) && value & !DOUBLE_SIGN_BIT != 0 && !is_nan_double(value)
}

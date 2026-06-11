use crate::encoding::{funct3, funct7, i_imm, rd, rs1, rs2, s_imm};
use crate::{
    FloatRegister, FloatRegisterWrite, Immediate, MemoryWidth, Register, RiscvError,
    RiscvHartState, RiscvInstruction,
};

pub(crate) fn decode_float_load(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let width = match funct3(raw) {
        0x2 => MemoryWidth::Word,
        0x3 => MemoryWidth::Doubleword,
        _ => return Err(RiscvError::UnknownEncoding { raw }),
    };

    Ok(RiscvInstruction::FloatLoad {
        rd: float_rd(raw),
        rs1: rs1(raw),
        offset: Immediate::new(i_imm(raw)),
        width,
    })
}

pub(crate) fn decode_float_store(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let width = match funct3(raw) {
        0x2 => MemoryWidth::Word,
        0x3 => MemoryWidth::Doubleword,
        _ => return Err(RiscvError::UnknownEncoding { raw }),
    };

    Ok(RiscvInstruction::FloatStore {
        rs1: rs1(raw),
        rs2: float_rs2(raw),
        offset: Immediate::new(s_imm(raw)),
        width,
    })
}

pub(crate) fn decode_float_op(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    match (funct7(raw), funct3(raw)) {
        (0x00, 0x0) => Ok(RiscvInstruction::FloatAddS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x01, 0x0) => Ok(RiscvInstruction::FloatAddD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x04, 0x0) => Ok(RiscvInstruction::FloatSubS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x05, 0x0) => Ok(RiscvInstruction::FloatSubD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x08, 0x0) => Ok(RiscvInstruction::FloatMulS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x09, 0x0) => Ok(RiscvInstruction::FloatMulD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x0c, 0x0) => Ok(RiscvInstruction::FloatDivS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x0d, 0x0) => Ok(RiscvInstruction::FloatDivD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x2c, 0x0) if rs2(raw).is_zero() => Ok(RiscvInstruction::FloatSqrtS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
        }),
        (0x2d, 0x0) if rs2(raw).is_zero() => Ok(RiscvInstruction::FloatSqrtD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
        }),
        (0x10, 0x0) => Ok(RiscvInstruction::FloatSignInjectS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x10, 0x1) => Ok(RiscvInstruction::FloatSignInjectNegS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x10, 0x2) => Ok(RiscvInstruction::FloatSignInjectXorS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x11, 0x0) => Ok(RiscvInstruction::FloatSignInjectD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x11, 0x1) => Ok(RiscvInstruction::FloatSignInjectNegD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x11, 0x2) => Ok(RiscvInstruction::FloatSignInjectXorD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x14, 0x0) => Ok(RiscvInstruction::FloatMinS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x14, 0x1) => Ok(RiscvInstruction::FloatMaxS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x15, 0x0) => Ok(RiscvInstruction::FloatMinD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x15, 0x1) => Ok(RiscvInstruction::FloatMaxD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x50, 0x0) => Ok(RiscvInstruction::FloatLessOrEqualS {
            rd: rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x50, 0x1) => Ok(RiscvInstruction::FloatLessThanS {
            rd: rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x50, 0x2) => Ok(RiscvInstruction::FloatEqualS {
            rd: rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x51, 0x0) => Ok(RiscvInstruction::FloatLessOrEqualD {
            rd: rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x51, 0x1) => Ok(RiscvInstruction::FloatLessThanD {
            rd: rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x51, 0x2) => Ok(RiscvInstruction::FloatEqualD {
            rd: rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x70, 0x0) if rs2(raw).is_zero() => Ok(RiscvInstruction::FloatMoveXFromS {
            rd: rd(raw),
            rs1: float_rs1(raw),
        }),
        (0x70, 0x1) if rs2(raw).is_zero() => Ok(RiscvInstruction::FloatClassS {
            rd: rd(raw),
            rs1: float_rs1(raw),
        }),
        (0x71, 0x0) if rs2(raw).is_zero() => Ok(RiscvInstruction::FloatMoveXFromD {
            rd: rd(raw),
            rs1: float_rs1(raw),
        }),
        (0x71, 0x1) if rs2(raw).is_zero() => Ok(RiscvInstruction::FloatClassD {
            rd: rd(raw),
            rs1: float_rs1(raw),
        }),
        (0x78, 0x0) if rs2(raw).is_zero() => Ok(RiscvInstruction::FloatMoveSFromX {
            rd: float_rd(raw),
            rs1: rs1(raw),
        }),
        (0x79, 0x0) if rs2(raw).is_zero() => Ok(RiscvInstruction::FloatMoveDFromX {
            rd: float_rd(raw),
            rs1: rs1(raw),
        }),
        (0x69, 0x0) if rs2(raw).index() == 0 => Ok(RiscvInstruction::FloatConvertDFromW {
            rd: float_rd(raw),
            rs1: rs1(raw),
        }),
        (0x69, 0x0) if rs2(raw).index() == 1 => Ok(RiscvInstruction::FloatConvertDFromWu {
            rd: float_rd(raw),
            rs1: rs1(raw),
        }),
        (0x69, 0x0) if rs2(raw).index() == 2 => Ok(RiscvInstruction::FloatConvertDFromL {
            rd: float_rd(raw),
            rs1: rs1(raw),
        }),
        (0x69, 0x0) if rs2(raw).index() == 3 => Ok(RiscvInstruction::FloatConvertDFromLu {
            rd: float_rd(raw),
            rs1: rs1(raw),
        }),
        (0x61, 0x0) if rs2(raw).index() == 0 => Ok(RiscvInstruction::FloatConvertWFromD {
            rd: rd(raw),
            rs1: float_rs1(raw),
        }),
        (0x61, 0x0) if rs2(raw).index() == 1 => Ok(RiscvInstruction::FloatConvertWuFromD {
            rd: rd(raw),
            rs1: float_rs1(raw),
        }),
        (0x61, 0x0) if rs2(raw).index() == 2 => Ok(RiscvInstruction::FloatConvertLFromD {
            rd: rd(raw),
            rs1: float_rs1(raw),
        }),
        (0x61, 0x0) if rs2(raw).index() == 3 => Ok(RiscvInstruction::FloatConvertLuFromD {
            rd: rd(raw),
            rs1: float_rs1(raw),
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

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
        _ => unreachable!("non-float-register instruction dispatched to float register write"),
    }
}

pub(crate) fn float_register_write_from_integer(
    instruction: RiscvInstruction,
    value: u64,
) -> (FloatRegister, u64) {
    match instruction {
        RiscvInstruction::FloatMoveSFromX { rd, .. } => (rd, box_single(value as u32)),
        RiscvInstruction::FloatMoveDFromX { rd, .. } => (rd, value),
        RiscvInstruction::FloatConvertDFromW { rd, .. } => {
            (rd, convert_signed_word_to_double(value))
        }
        RiscvInstruction::FloatConvertDFromWu { rd, .. } => {
            (rd, convert_unsigned_word_to_double(value))
        }
        RiscvInstruction::FloatConvertDFromL { rd, .. } => {
            (rd, convert_signed_doubleword_to_double(value))
        }
        RiscvInstruction::FloatConvertDFromLu { rd, .. } => {
            (rd, convert_unsigned_doubleword_to_double(value))
        }
        _ => unreachable!("non-integer-convert instruction dispatched to float register write"),
    }
}

pub(crate) fn integer_register_write(
    instruction: RiscvInstruction,
    lhs: u64,
    rhs: u64,
) -> (Register, u64) {
    match instruction {
        RiscvInstruction::FloatLessOrEqualS { rd, .. } => {
            (rd, u64::from(less_or_equal_single(lhs, rhs)))
        }
        RiscvInstruction::FloatLessOrEqualD { rd, .. } => {
            (rd, u64::from(less_or_equal_double(lhs, rhs)))
        }
        RiscvInstruction::FloatLessThanS { rd, .. } => (rd, u64::from(less_than_single(lhs, rhs))),
        RiscvInstruction::FloatLessThanD { rd, .. } => (rd, u64::from(less_than_double(lhs, rhs))),
        RiscvInstruction::FloatEqualS { rd, .. } => (rd, u64::from(equal_single(lhs, rhs))),
        RiscvInstruction::FloatEqualD { rd, .. } => (rd, u64::from(equal_double(lhs, rhs))),
        RiscvInstruction::FloatClassS { rd, .. } => (rd, class_single(lhs)),
        RiscvInstruction::FloatClassD { rd, .. } => (rd, class_double(lhs)),
        RiscvInstruction::FloatMoveXFromS { rd, .. } => {
            (rd, unbox_raw_single(lhs) as i32 as i64 as u64)
        }
        RiscvInstruction::FloatMoveXFromD { rd, .. } => (rd, lhs),
        RiscvInstruction::FloatConvertWFromD { rd, .. } => (rd, convert_double_to_signed_word(lhs)),
        RiscvInstruction::FloatConvertWuFromD { rd, .. } => {
            (rd, convert_double_to_unsigned_word(lhs))
        }
        RiscvInstruction::FloatConvertLFromD { rd, .. } => {
            (rd, convert_double_to_signed_doubleword(lhs))
        }
        RiscvInstruction::FloatConvertLuFromD { rd, .. } => {
            (rd, convert_double_to_unsigned_doubleword(lhs))
        }
        _ => unreachable!("non-float-compare instruction dispatched to integer register write"),
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

fn sqrt_single(value: u64) -> u64 {
    box_canonical_single(f32::from_bits(unbox_single(value)).sqrt())
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

fn convert_signed_word_to_double(value: u64) -> u64 {
    ((value as u32) as i32 as f64).to_bits()
}

fn convert_unsigned_word_to_double(value: u64) -> u64 {
    (value as u32 as f64).to_bits()
}

fn convert_signed_doubleword_to_double(value: u64) -> u64 {
    (value as i64 as f64).to_bits()
}

fn convert_unsigned_doubleword_to_double(value: u64) -> u64 {
    (value as f64).to_bits()
}

fn convert_double_to_signed_word(value: u64) -> u64 {
    let value = f64::from_bits(value);
    if value.is_nan() {
        return i32::MAX as u64;
    }

    let rounded = value.round_ties_even();
    if rounded > f64::from(i32::MAX) {
        i32::MAX as u64
    } else if rounded < f64::from(i32::MIN) {
        i32::MIN as i64 as u64
    } else {
        rounded as i32 as i64 as u64
    }
}

fn convert_double_to_unsigned_word(value: u64) -> u64 {
    let value = f64::from_bits(value);
    if value.is_nan() {
        return sign_extend_unsigned_word(u32::MAX);
    }

    let rounded = value.round_ties_even();
    if rounded < 0.0 {
        0
    } else if rounded > f64::from(u32::MAX) {
        sign_extend_unsigned_word(u32::MAX)
    } else {
        sign_extend_unsigned_word(rounded as u32)
    }
}

fn convert_double_to_signed_doubleword(value: u64) -> u64 {
    let value = f64::from_bits(value);
    if value.is_nan() {
        return i64::MAX as u64;
    }

    let rounded = value.round_ties_even();
    if rounded >= i64::MAX as f64 {
        i64::MAX as u64
    } else if rounded < i64::MIN as f64 {
        i64::MIN as u64
    } else {
        rounded as i64 as u64
    }
}

fn convert_double_to_unsigned_doubleword(value: u64) -> u64 {
    let value = f64::from_bits(value);
    if value.is_nan() {
        return u64::MAX;
    }

    let rounded = value.round_ties_even();
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
    let lhs = unbox_single(lhs);
    let rhs = unbox_single(rhs);
    if is_nan_single(lhs) && is_nan_single(rhs) {
        return DEFAULT_NAN_SINGLE;
    }
    if is_nan_single(lhs) {
        return box_single(rhs);
    }
    if is_nan_single(rhs) {
        return box_single(lhs);
    }

    let lhs_value = f32::from_bits(lhs);
    let rhs_value = f32::from_bits(rhs);
    if lhs_value < rhs_value || (lhs_value == rhs_value && has_single_sign(lhs)) {
        box_single(lhs)
    } else {
        box_single(rhs)
    }
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
    let lhs = unbox_single(lhs);
    let rhs = unbox_single(rhs);
    if is_nan_single(lhs) && is_nan_single(rhs) {
        return DEFAULT_NAN_SINGLE;
    }
    if is_nan_single(lhs) {
        return box_single(rhs);
    }
    if is_nan_single(rhs) {
        return box_single(lhs);
    }

    let lhs_value = f32::from_bits(lhs);
    let rhs_value = f32::from_bits(rhs);
    if rhs_value < lhs_value || (lhs_value == rhs_value && has_single_sign(rhs)) {
        box_single(lhs)
    } else {
        box_single(rhs)
    }
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
    f32::from_bits(unbox_single(lhs)) <= f32::from_bits(unbox_single(rhs))
}

fn less_or_equal_double(lhs: u64, rhs: u64) -> bool {
    f64::from_bits(lhs) <= f64::from_bits(rhs)
}

fn less_than_single(lhs: u64, rhs: u64) -> bool {
    f32::from_bits(unbox_single(lhs)) < f32::from_bits(unbox_single(rhs))
}

fn less_than_double(lhs: u64, rhs: u64) -> bool {
    f64::from_bits(lhs) < f64::from_bits(rhs)
}

fn equal_single(lhs: u64, rhs: u64) -> bool {
    f32::from_bits(unbox_single(lhs)) == f32::from_bits(unbox_single(rhs))
}

fn equal_double(lhs: u64, rhs: u64) -> bool {
    f64::from_bits(lhs) == f64::from_bits(rhs)
}

fn class_single(value: u64) -> u64 {
    let value = unbox_single(value);
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

fn has_single_sign(value: u32) -> bool {
    value & SINGLE_SIGN_BIT != 0
}

fn has_double_sign(value: u64) -> bool {
    value & DOUBLE_SIGN_BIT != 0
}

const SINGLE_BOX_MASK: u64 = 0xffff_ffff_0000_0000;
const SINGLE_SIGN_BIT: u32 = 1 << 31;
const SINGLE_EXP_MASK: u32 = 0x7f80_0000;
const SINGLE_FRACTION_MASK: u32 = 0x007f_ffff;
const SINGLE_QUIET_NAN_BIT: u32 = 1 << 22;
const DEFAULT_NAN_SINGLE_BITS: u32 = 0x7fc0_0000;
const DEFAULT_NAN_SINGLE: u64 = SINGLE_BOX_MASK | DEFAULT_NAN_SINGLE_BITS as u64;
const DOUBLE_SIGN_BIT: u64 = 1 << 63;
const DOUBLE_EXP_MASK: u64 = 0x7ff0_0000_0000_0000;
const DOUBLE_FRACTION_MASK: u64 = 0x000f_ffff_ffff_ffff;
const DOUBLE_QUIET_NAN_BIT: u64 = 1 << 51;
const DEFAULT_NAN_DOUBLE: u64 = 0x7ff8_0000_0000_0000;

fn float_rd(raw: u32) -> FloatRegister {
    FloatRegister::from_field(rd(raw).index().into())
}

fn float_rs1(raw: u32) -> FloatRegister {
    FloatRegister::from_field(rs1(raw).index().into())
}

fn float_rs2(raw: u32) -> FloatRegister {
    FloatRegister::from_field(rs2(raw).index().into())
}

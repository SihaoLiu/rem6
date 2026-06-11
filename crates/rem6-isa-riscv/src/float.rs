use crate::encoding::{funct3, funct7, i_imm, rd, rs1, rs2, s_imm};
use crate::{FloatRegister, Immediate, MemoryWidth, Register, RiscvError, RiscvInstruction};

pub(crate) fn decode_float_load(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let width = match funct3(raw) {
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
        (0x01, 0x0) => Ok(RiscvInstruction::FloatAddD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x05, 0x0) => Ok(RiscvInstruction::FloatSubD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x09, 0x0) => Ok(RiscvInstruction::FloatMulD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x0d, 0x0) => Ok(RiscvInstruction::FloatDivD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
        }),
        (0x2d, 0x0) if rs2(raw).is_zero() => Ok(RiscvInstruction::FloatSqrtD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
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
        (0x71, 0x0) if rs2(raw).is_zero() => Ok(RiscvInstruction::FloatMoveXFromD {
            rd: rd(raw),
            rs1: float_rs1(raw),
        }),
        (0x71, 0x1) if rs2(raw).is_zero() => Ok(RiscvInstruction::FloatClassD {
            rd: rd(raw),
            rs1: float_rs1(raw),
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
        RiscvInstruction::FloatAddD { rd, .. } => (rd, add_double(lhs, rhs)),
        RiscvInstruction::FloatSubD { rd, .. } => (rd, sub_double(lhs, rhs)),
        RiscvInstruction::FloatMulD { rd, .. } => (rd, mul_double(lhs, rhs)),
        RiscvInstruction::FloatDivD { rd, .. } => (rd, div_double(lhs, rhs)),
        RiscvInstruction::FloatSqrtD { rd, .. } => (rd, sqrt_double(lhs)),
        RiscvInstruction::FloatSignInjectD { rd, .. } => (rd, sign_inject_double(lhs, rhs)),
        RiscvInstruction::FloatSignInjectNegD { rd, .. } => (rd, sign_inject_neg_double(lhs, rhs)),
        RiscvInstruction::FloatSignInjectXorD { rd, .. } => (rd, sign_inject_xor_double(lhs, rhs)),
        RiscvInstruction::FloatMinD { rd, .. } => (rd, min_double(lhs, rhs)),
        RiscvInstruction::FloatMaxD { rd, .. } => (rd, max_double(lhs, rhs)),
        _ => unreachable!("non-float-register instruction dispatched to float register write"),
    }
}

pub(crate) fn float_register_write_from_integer(
    instruction: RiscvInstruction,
    value: u64,
) -> (FloatRegister, u64) {
    match instruction {
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
        RiscvInstruction::FloatLessOrEqualD { rd, .. } => {
            (rd, u64::from(less_or_equal_double(lhs, rhs)))
        }
        RiscvInstruction::FloatLessThanD { rd, .. } => (rd, u64::from(less_than_double(lhs, rhs))),
        RiscvInstruction::FloatEqualD { rd, .. } => (rd, u64::from(equal_double(lhs, rhs))),
        RiscvInstruction::FloatClassD { rd, .. } => (rd, class_double(lhs)),
        RiscvInstruction::FloatMoveXFromD { rd, .. } => (rd, lhs),
        _ => unreachable!("non-float-compare instruction dispatched to integer register write"),
    }
}

fn sub_double(lhs: u64, rhs: u64) -> u64 {
    (f64::from_bits(lhs) - f64::from_bits(rhs)).to_bits()
}

fn mul_double(lhs: u64, rhs: u64) -> u64 {
    (f64::from_bits(lhs) * f64::from_bits(rhs)).to_bits()
}

fn div_double(lhs: u64, rhs: u64) -> u64 {
    (f64::from_bits(lhs) / f64::from_bits(rhs)).to_bits()
}

fn sqrt_double(value: u64) -> u64 {
    let result = f64::from_bits(value).sqrt().to_bits();
    if is_nan_double(result) {
        DEFAULT_NAN_DOUBLE
    } else {
        result
    }
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

fn sign_inject_neg_double(lhs: u64, rhs: u64) -> u64 {
    (lhs & !DOUBLE_SIGN_BIT) | ((!rhs) & DOUBLE_SIGN_BIT)
}

fn sign_inject_xor_double(lhs: u64, rhs: u64) -> u64 {
    (lhs & !DOUBLE_SIGN_BIT) | ((lhs ^ rhs) & DOUBLE_SIGN_BIT)
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

fn less_or_equal_double(lhs: u64, rhs: u64) -> bool {
    f64::from_bits(lhs) <= f64::from_bits(rhs)
}

fn less_than_double(lhs: u64, rhs: u64) -> bool {
    f64::from_bits(lhs) < f64::from_bits(rhs)
}

fn equal_double(lhs: u64, rhs: u64) -> bool {
    f64::from_bits(lhs) == f64::from_bits(rhs)
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

fn is_nan_double(value: u64) -> bool {
    value & DOUBLE_EXP_MASK == DOUBLE_EXP_MASK && value & DOUBLE_FRACTION_MASK != 0
}

fn has_double_sign(value: u64) -> bool {
    value & DOUBLE_SIGN_BIT != 0
}

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

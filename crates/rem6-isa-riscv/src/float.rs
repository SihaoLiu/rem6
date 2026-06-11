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
        RiscvInstruction::FloatSignInjectD { rd, .. } => (rd, sign_inject_double(lhs, rhs)),
        RiscvInstruction::FloatSignInjectNegD { rd, .. } => (rd, sign_inject_neg_double(lhs, rhs)),
        RiscvInstruction::FloatSignInjectXorD { rd, .. } => (rd, sign_inject_xor_double(lhs, rhs)),
        _ => unreachable!("non-float-register instruction dispatched to float register write"),
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

fn sign_inject_double(lhs: u64, rhs: u64) -> u64 {
    (lhs & !DOUBLE_SIGN_BIT) | (rhs & DOUBLE_SIGN_BIT)
}

fn sign_inject_neg_double(lhs: u64, rhs: u64) -> u64 {
    (lhs & !DOUBLE_SIGN_BIT) | ((!rhs) & DOUBLE_SIGN_BIT)
}

fn sign_inject_xor_double(lhs: u64, rhs: u64) -> u64 {
    (lhs & !DOUBLE_SIGN_BIT) | ((lhs ^ rhs) & DOUBLE_SIGN_BIT)
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

const DOUBLE_SIGN_BIT: u64 = 1 << 63;

fn float_rd(raw: u32) -> FloatRegister {
    FloatRegister::from_field(rd(raw).index().into())
}

fn float_rs1(raw: u32) -> FloatRegister {
    FloatRegister::from_field(rs1(raw).index().into())
}

fn float_rs2(raw: u32) -> FloatRegister {
    FloatRegister::from_field(rs2(raw).index().into())
}

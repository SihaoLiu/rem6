use crate::encoding::{funct2, funct3, funct7, i_imm, rd, rs1, rs2, rs3, s_imm};
use crate::{
    FloatRegister, Immediate, MemoryWidth, RiscvError, RiscvFloatRoundingMode, RiscvInstruction,
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
        (0x00, _) => Ok(RiscvInstruction::FloatAddS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x01, _) => Ok(RiscvInstruction::FloatAddD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x04, _) => Ok(RiscvInstruction::FloatSubS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x05, _) => Ok(RiscvInstruction::FloatSubD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x08, _) => Ok(RiscvInstruction::FloatMulS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x09, _) => Ok(RiscvInstruction::FloatMulD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x0c, _) => Ok(RiscvInstruction::FloatDivS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x0d, _) => Ok(RiscvInstruction::FloatDivD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x2c, _) if rs2(raw).is_zero() => Ok(RiscvInstruction::FloatSqrtS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x2d, _) if rs2(raw).is_zero() => Ok(RiscvInstruction::FloatSqrtD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
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
        (0x68, _) if rs2(raw).index() == 0 => Ok(RiscvInstruction::FloatConvertSFromW {
            rd: float_rd(raw),
            rs1: rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        (0x68, _) if rs2(raw).index() == 1 => Ok(RiscvInstruction::FloatConvertSFromWu {
            rd: float_rd(raw),
            rs1: rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        (0x68, _) if rs2(raw).index() == 2 => Ok(RiscvInstruction::FloatConvertSFromL {
            rd: float_rd(raw),
            rs1: rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        (0x68, _) if rs2(raw).index() == 3 => Ok(RiscvInstruction::FloatConvertSFromLu {
            rd: float_rd(raw),
            rs1: rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        (0x60, _) if rs2(raw).index() == 0 => Ok(RiscvInstruction::FloatConvertWFromS {
            rd: rd(raw),
            rs1: float_rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        (0x60, _) if rs2(raw).index() == 1 => Ok(RiscvInstruction::FloatConvertWuFromS {
            rd: rd(raw),
            rs1: float_rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        (0x60, _) if rs2(raw).index() == 2 => Ok(RiscvInstruction::FloatConvertLFromS {
            rd: rd(raw),
            rs1: float_rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        (0x60, _) if rs2(raw).index() == 3 => Ok(RiscvInstruction::FloatConvertLuFromS {
            rd: rd(raw),
            rs1: float_rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        (0x20, 0x0) if rs2(raw).index() == 1 => Ok(RiscvInstruction::FloatConvertSFromD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
        }),
        (0x79, 0x0) if rs2(raw).is_zero() => Ok(RiscvInstruction::FloatMoveDFromX {
            rd: float_rd(raw),
            rs1: rs1(raw),
        }),
        (0x69, _) if rs2(raw).index() == 0 => Ok(RiscvInstruction::FloatConvertDFromW {
            rd: float_rd(raw),
            rs1: rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        (0x69, _) if rs2(raw).index() == 1 => Ok(RiscvInstruction::FloatConvertDFromWu {
            rd: float_rd(raw),
            rs1: rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        (0x69, _) if rs2(raw).index() == 2 => Ok(RiscvInstruction::FloatConvertDFromL {
            rd: float_rd(raw),
            rs1: rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        (0x69, _) if rs2(raw).index() == 3 => Ok(RiscvInstruction::FloatConvertDFromLu {
            rd: float_rd(raw),
            rs1: rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        (0x21, 0x0) if rs2(raw).is_zero() => Ok(RiscvInstruction::FloatConvertDFromS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
        }),
        (0x61, _) if rs2(raw).index() == 0 => Ok(RiscvInstruction::FloatConvertWFromD {
            rd: rd(raw),
            rs1: float_rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        (0x61, _) if rs2(raw).index() == 1 => Ok(RiscvInstruction::FloatConvertWuFromD {
            rd: rd(raw),
            rs1: float_rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        (0x61, _) if rs2(raw).index() == 2 => Ok(RiscvInstruction::FloatConvertLFromD {
            rd: rd(raw),
            rs1: float_rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        (0x61, _) if rs2(raw).index() == 3 => Ok(RiscvInstruction::FloatConvertLuFromD {
            rd: rd(raw),
            rs1: float_rs1(raw),
            rounding_mode: float_rounding_mode(raw)?,
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

pub(crate) fn decode_float_multiply_add(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    match (raw & 0x7f, funct2(raw), funct3(raw)) {
        (0x43, 0x0, _) => Ok(RiscvInstruction::FloatMultiplyAddS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rs3: float_rs3(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x43, 0x1, _) => Ok(RiscvInstruction::FloatMultiplyAddD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rs3: float_rs3(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x47, 0x0, _) => Ok(RiscvInstruction::FloatMultiplySubtractS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rs3: float_rs3(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x47, 0x1, _) => Ok(RiscvInstruction::FloatMultiplySubtractD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rs3: float_rs3(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x4b, 0x0, _) => Ok(RiscvInstruction::FloatNegativeMultiplySubtractS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rs3: float_rs3(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x4b, 0x1, _) => Ok(RiscvInstruction::FloatNegativeMultiplySubtractD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rs3: float_rs3(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x4f, 0x0, _) => Ok(RiscvInstruction::FloatNegativeMultiplyAddS {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rs3: float_rs3(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        (0x4f, 0x1, _) => Ok(RiscvInstruction::FloatNegativeMultiplyAddD {
            rd: float_rd(raw),
            rs1: float_rs1(raw),
            rs2: float_rs2(raw),
            rs3: float_rs3(raw),
            rounding_mode: supported_arithmetic_rounding_mode(raw)?,
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

fn supported_arithmetic_rounding_mode(raw: u32) -> Result<RiscvFloatRoundingMode, RiscvError> {
    float_rounding_mode(raw)
}

fn float_rd(raw: u32) -> FloatRegister {
    FloatRegister::from_field(rd(raw).index().into())
}

fn float_rounding_mode(raw: u32) -> Result<RiscvFloatRoundingMode, RiscvError> {
    RiscvFloatRoundingMode::from_rm_bits(funct3(raw) as u8)
        .ok_or(RiscvError::UnknownEncoding { raw })
}

fn float_rs1(raw: u32) -> FloatRegister {
    FloatRegister::from_field(rs1(raw).index().into())
}

fn float_rs2(raw: u32) -> FloatRegister {
    FloatRegister::from_field(rs2(raw).index().into())
}

fn float_rs3(raw: u32) -> FloatRegister {
    FloatRegister::from_field(rs3(raw).index().into())
}

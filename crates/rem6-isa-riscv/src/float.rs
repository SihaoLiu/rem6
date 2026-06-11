use crate::encoding::{funct3, funct7, i_imm, rd, rs1, rs2, s_imm};
use crate::{FloatRegister, Immediate, MemoryWidth, RiscvError, RiscvInstruction};

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
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

pub(crate) fn add_double(lhs: u64, rhs: u64) -> u64 {
    (f64::from_bits(lhs) + f64::from_bits(rhs)).to_bits()
}

fn float_rd(raw: u32) -> FloatRegister {
    FloatRegister::from_field(rd(raw).index().into())
}

fn float_rs1(raw: u32) -> FloatRegister {
    FloatRegister::from_field(rs1(raw).index().into())
}

fn float_rs2(raw: u32) -> FloatRegister {
    FloatRegister::from_field(rs2(raw).index().into())
}

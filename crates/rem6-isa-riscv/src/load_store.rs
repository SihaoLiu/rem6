use crate::encoding::{funct3, i_imm, rd, rs1, rs2, s_imm};
use crate::{
    Immediate, MemoryWidth, RiscvError, RiscvInstruction, RiscvVectorMaskMode,
    RiscvVectorMemoryInstruction, VectorRegister,
};

pub(crate) fn decode_integer_load(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let (width, signed) = match funct3(raw) {
        0x0 => (MemoryWidth::Byte, true),
        0x1 => (MemoryWidth::Halfword, true),
        0x2 => (MemoryWidth::Word, true),
        0x3 => (MemoryWidth::Doubleword, true),
        0x4 => (MemoryWidth::Byte, false),
        0x5 => (MemoryWidth::Halfword, false),
        0x6 => (MemoryWidth::Word, false),
        _ => return Err(RiscvError::UnknownEncoding { raw }),
    };

    Ok(RiscvInstruction::Load {
        rd: rd(raw),
        rs1: rs1(raw),
        offset: Immediate::new(i_imm(raw)),
        width,
        signed,
    })
}

pub(crate) fn decode_integer_store(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let width = match funct3(raw) {
        0x0 => MemoryWidth::Byte,
        0x1 => MemoryWidth::Halfword,
        0x2 => MemoryWidth::Word,
        0x3 => MemoryWidth::Doubleword,
        _ => return Err(RiscvError::UnknownEncoding { raw }),
    };

    Ok(RiscvInstruction::Store {
        rs1: rs1(raw),
        rs2: rs2(raw),
        offset: Immediate::new(s_imm(raw)),
        width,
    })
}

pub(crate) fn opcode_uses_vector_memory(raw: u32) -> bool {
    vector_memory_width(raw).is_some()
}

pub(crate) fn decode_vector_load(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let width = vector_memory_width(raw).ok_or(RiscvError::UnknownEncoding { raw })?;

    if is_unit_stride_vector_memory(raw) {
        return Ok(RiscvInstruction::VectorMemory(
            RiscvVectorMemoryInstruction::LoadUnitStride {
                vd: vector_register(raw, 7),
                rs1: rs1(raw),
                width,
                mask: vector_memory_mask_mode(raw),
            },
        ));
    }
    if is_strided_vector_memory(raw) {
        return Ok(RiscvInstruction::VectorMemory(
            RiscvVectorMemoryInstruction::LoadStrided {
                vd: vector_register(raw, 7),
                rs1: rs1(raw),
                rs2: rs2(raw),
                width,
                mask: vector_memory_mask_mode(raw),
            },
        ));
    }

    Err(RiscvError::UnknownEncoding { raw })
}

pub(crate) fn decode_vector_store(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let width = vector_memory_width(raw).ok_or(RiscvError::UnknownEncoding { raw })?;

    if is_unit_stride_vector_memory(raw) {
        return Ok(RiscvInstruction::VectorMemory(
            RiscvVectorMemoryInstruction::StoreUnitStride {
                vs3: vector_register(raw, 7),
                rs1: rs1(raw),
                width,
                mask: vector_memory_mask_mode(raw),
            },
        ));
    }
    if is_strided_vector_memory(raw) {
        return Ok(RiscvInstruction::VectorMemory(
            RiscvVectorMemoryInstruction::StoreStrided {
                vs3: vector_register(raw, 7),
                rs1: rs1(raw),
                rs2: rs2(raw),
                width,
                mask: vector_memory_mask_mode(raw),
            },
        ));
    }

    Err(RiscvError::UnknownEncoding { raw })
}

fn is_unit_stride_vector_memory(raw: u32) -> bool {
    let mop = (raw >> 26) & 0x3;
    let lumop_or_sumop = (raw >> 20) & 0x1f;
    let mew_and_nf = (raw >> 28) & 0xf;
    mop == 0 && lumop_or_sumop == 0 && mew_and_nf == 0
}

fn is_strided_vector_memory(raw: u32) -> bool {
    let mop = (raw >> 26) & 0x3;
    let mew_and_nf = (raw >> 28) & 0xf;
    mop == 0b10 && mew_and_nf == 0
}

fn vector_memory_width(raw: u32) -> Option<MemoryWidth> {
    match funct3(raw) {
        0b000 => Some(MemoryWidth::Byte),
        0b101 => Some(MemoryWidth::Halfword),
        0b110 => Some(MemoryWidth::Word),
        0b111 => Some(MemoryWidth::Doubleword),
        _ => None,
    }
}

fn vector_register(raw: u32, shift: u32) -> VectorRegister {
    VectorRegister::from_field((raw >> shift) & 0x1f)
}

fn vector_memory_mask_mode(raw: u32) -> RiscvVectorMaskMode {
    RiscvVectorMaskMode::from_vm_bit((raw & (1 << 25)) != 0)
}

use crate::{Immediate, MemoryWidth, Register, RiscvError, RiscvInstruction};

pub(crate) fn decode_compressed(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let half = raw as u16;
    match (bits(half, 1, 0), bits(half, 15, 13)) {
        (0b00, 0b000) => decode_addi4spn(raw, half),
        (0b00, 0b010) => Ok(RiscvInstruction::Load {
            rd: compressed_register(bits(half, 4, 2)),
            rs1: compressed_register(bits(half, 9, 7)),
            offset: Immediate::new(i64::from(compressed_lw_offset(half))),
            width: MemoryWidth::Word,
            signed: true,
        }),
        (0b00, 0b011) => Ok(RiscvInstruction::Load {
            rd: compressed_register(bits(half, 4, 2)),
            rs1: compressed_register(bits(half, 9, 7)),
            offset: Immediate::new(i64::from(compressed_ld_offset(half))),
            width: MemoryWidth::Doubleword,
            signed: true,
        }),
        (0b00, 0b110) => Ok(RiscvInstruction::Store {
            rs1: compressed_register(bits(half, 9, 7)),
            rs2: compressed_register(bits(half, 4, 2)),
            offset: Immediate::new(i64::from(compressed_lw_offset(half))),
            width: MemoryWidth::Word,
        }),
        (0b00, 0b111) => Ok(RiscvInstruction::Store {
            rs1: compressed_register(bits(half, 9, 7)),
            rs2: compressed_register(bits(half, 4, 2)),
            offset: Immediate::new(i64::from(compressed_ld_offset(half))),
            width: MemoryWidth::Doubleword,
        }),
        (0b01, 0b000) => Ok(RiscvInstruction::Addi {
            rd: register(bits(half, 11, 7)),
            rs1: register(bits(half, 11, 7)),
            imm: Immediate::new(compressed_ci_immediate(half)),
        }),
        (0b01, 0b001) => decode_addiw(raw, half),
        (0b01, 0b010) => Ok(RiscvInstruction::Addi {
            rd: register(bits(half, 11, 7)),
            rs1: register(0),
            imm: Immediate::new(compressed_ci_immediate(half)),
        }),
        (0b01, 0b011) => decode_lui_or_addi16sp(raw, half),
        (0b01, 0b100) => decode_register_arithmetic(raw, half),
        (0b01, 0b101) => Ok(RiscvInstruction::Jal {
            rd: register(0),
            offset: Immediate::new(compressed_jump_offset(half)),
        }),
        (0b01, 0b110) => Ok(RiscvInstruction::Beq {
            rs1: compressed_register(bits(half, 9, 7)),
            rs2: register(0),
            offset: Immediate::new(compressed_branch_offset(half)),
        }),
        (0b01, 0b111) => Ok(RiscvInstruction::Bne {
            rs1: compressed_register(bits(half, 9, 7)),
            rs2: register(0),
            offset: Immediate::new(compressed_branch_offset(half)),
        }),
        (0b10, 0b000) => decode_slli(raw, half),
        (0b10, 0b010) => decode_lwsp(raw, half),
        (0b10, 0b011) => decode_ldsp(raw, half),
        (0b10, 0b100) => decode_jump_move_break_or_add(raw, half),
        (0b10, 0b110) => Ok(RiscvInstruction::Store {
            rs1: register(2),
            rs2: register(bits(half, 6, 2)),
            offset: Immediate::new(i64::from(compressed_swsp_offset(half))),
            width: MemoryWidth::Word,
        }),
        (0b10, 0b111) => Ok(RiscvInstruction::Store {
            rs1: register(2),
            rs2: register(bits(half, 6, 2)),
            offset: Immediate::new(i64::from(compressed_sdsp_offset(half))),
            width: MemoryWidth::Doubleword,
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

fn decode_addi4spn(raw: u32, half: u16) -> Result<RiscvInstruction, RiscvError> {
    let offset = (bits(half, 12, 11) << 4)
        | (bits(half, 10, 7) << 6)
        | (bit(half, 6) << 2)
        | (bit(half, 5) << 3);
    if offset == 0 {
        return Err(RiscvError::UnknownEncoding { raw });
    }
    Ok(RiscvInstruction::Addi {
        rd: compressed_register(bits(half, 4, 2)),
        rs1: register(2),
        imm: Immediate::new(i64::from(offset)),
    })
}

fn decode_addiw(raw: u32, half: u16) -> Result<RiscvInstruction, RiscvError> {
    let rd = register(bits(half, 11, 7));
    if rd.is_zero() {
        return Err(RiscvError::UnknownEncoding { raw });
    }
    Ok(RiscvInstruction::Addiw {
        rd,
        rs1: rd,
        imm: Immediate::new(compressed_ci_immediate(half)),
    })
}

fn decode_lui_or_addi16sp(raw: u32, half: u16) -> Result<RiscvInstruction, RiscvError> {
    let rd = register(bits(half, 11, 7));
    if rd.is_zero() {
        return Err(RiscvError::UnknownEncoding { raw });
    }
    if rd == register(2) {
        let immediate = compressed_addi16sp_immediate(half);
        if immediate == 0 {
            return Err(RiscvError::UnknownEncoding { raw });
        }
        return Ok(RiscvInstruction::Addi {
            rd,
            rs1: rd,
            imm: Immediate::new(immediate),
        });
    }

    let immediate = compressed_ci_immediate(half);
    if immediate == 0 {
        return Err(RiscvError::UnknownEncoding { raw });
    }
    Ok(RiscvInstruction::Lui {
        rd,
        imm: Immediate::new(immediate << 12),
    })
}

fn decode_register_arithmetic(raw: u32, half: u16) -> Result<RiscvInstruction, RiscvError> {
    let rd = compressed_register(bits(half, 9, 7));
    match bits(half, 11, 10) {
        0b00 => Ok(RiscvInstruction::Srli {
            rd,
            rs1: rd,
            shamt: compressed_shift_amount(half),
        }),
        0b01 => Ok(RiscvInstruction::Srai {
            rd,
            rs1: rd,
            shamt: compressed_shift_amount(half),
        }),
        0b10 => Ok(RiscvInstruction::Andi {
            rd,
            rs1: rd,
            imm: Immediate::new(compressed_ci_immediate(half)),
        }),
        0b11 => decode_register_register_arithmetic(raw, half, rd),
        _ => unreachable!("two-bit field is exhausted"),
    }
}

fn decode_register_register_arithmetic(
    raw: u32,
    half: u16,
    rd: Register,
) -> Result<RiscvInstruction, RiscvError> {
    let rs2 = compressed_register(bits(half, 4, 2));
    match (bit(half, 12), bits(half, 6, 5)) {
        (0, 0b00) => Ok(RiscvInstruction::Sub { rd, rs1: rd, rs2 }),
        (0, 0b01) => Ok(RiscvInstruction::Xor { rd, rs1: rd, rs2 }),
        (0, 0b10) => Ok(RiscvInstruction::Or { rd, rs1: rd, rs2 }),
        (0, 0b11) => Ok(RiscvInstruction::And { rd, rs1: rd, rs2 }),
        (1, 0b00) => Ok(RiscvInstruction::Subw { rd, rs1: rd, rs2 }),
        (1, 0b01) => Ok(RiscvInstruction::Addw { rd, rs1: rd, rs2 }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

fn decode_slli(raw: u32, half: u16) -> Result<RiscvInstruction, RiscvError> {
    let rd = register(bits(half, 11, 7));
    if rd.is_zero() {
        return Err(RiscvError::UnknownEncoding { raw });
    }
    Ok(RiscvInstruction::Slli {
        rd,
        rs1: rd,
        shamt: compressed_shift_amount(half),
    })
}

fn decode_lwsp(raw: u32, half: u16) -> Result<RiscvInstruction, RiscvError> {
    let rd = register(bits(half, 11, 7));
    if rd.is_zero() {
        return Err(RiscvError::UnknownEncoding { raw });
    }
    Ok(RiscvInstruction::Load {
        rd,
        rs1: register(2),
        offset: Immediate::new(i64::from(compressed_lwsp_offset(half))),
        width: MemoryWidth::Word,
        signed: true,
    })
}

fn decode_ldsp(raw: u32, half: u16) -> Result<RiscvInstruction, RiscvError> {
    let rd = register(bits(half, 11, 7));
    if rd.is_zero() {
        return Err(RiscvError::UnknownEncoding { raw });
    }
    Ok(RiscvInstruction::Load {
        rd,
        rs1: register(2),
        offset: Immediate::new(i64::from(compressed_ldsp_offset(half))),
        width: MemoryWidth::Doubleword,
        signed: true,
    })
}

fn decode_jump_move_break_or_add(raw: u32, half: u16) -> Result<RiscvInstruction, RiscvError> {
    let rd = register(bits(half, 11, 7));
    let rs2 = register(bits(half, 6, 2));
    match (bit(half, 12), rd.is_zero(), rs2.is_zero()) {
        (0, false, true) => Ok(RiscvInstruction::Jalr {
            rd: register(0),
            rs1: rd,
            offset: Immediate::new(0),
        }),
        (0, _, false) => Ok(RiscvInstruction::Add {
            rd,
            rs1: register(0),
            rs2,
        }),
        (1, true, true) => Ok(RiscvInstruction::Ebreak),
        (1, false, true) => Ok(RiscvInstruction::Jalr {
            rd: register(1),
            rs1: rd,
            offset: Immediate::new(0),
        }),
        (1, false, false) => Ok(RiscvInstruction::Add { rd, rs1: rd, rs2 }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

fn compressed_lw_offset(half: u16) -> u32 {
    (bits(half, 12, 10) << 3) | (bit(half, 6) << 2) | (bit(half, 5) << 6)
}

fn compressed_ld_offset(half: u16) -> u32 {
    (bits(half, 12, 10) << 3) | (bits(half, 6, 5) << 6)
}

fn compressed_lwsp_offset(half: u16) -> u32 {
    (bit(half, 12) << 5) | (bits(half, 6, 4) << 2) | (bits(half, 3, 2) << 6)
}

fn compressed_ldsp_offset(half: u16) -> u32 {
    (bit(half, 12) << 5) | (bits(half, 6, 5) << 3) | (bits(half, 4, 2) << 6)
}

fn compressed_swsp_offset(half: u16) -> u32 {
    (bits(half, 12, 9) << 2) | (bits(half, 8, 7) << 6)
}

fn compressed_sdsp_offset(half: u16) -> u32 {
    (bits(half, 12, 10) << 3) | (bits(half, 9, 7) << 6)
}

fn compressed_ci_immediate(half: u16) -> i64 {
    sign_extend((bit(half, 12) << 5) | bits(half, 6, 2), 6)
}

fn compressed_addi16sp_immediate(half: u16) -> i64 {
    sign_extend(
        (bit(half, 12) << 9)
            | (bit(half, 6) << 4)
            | (bit(half, 5) << 6)
            | (bits(half, 4, 3) << 7)
            | (bit(half, 2) << 5),
        10,
    )
}

fn compressed_branch_offset(half: u16) -> i64 {
    sign_extend(
        (bit(half, 12) << 8)
            | (bits(half, 11, 10) << 3)
            | (bits(half, 6, 5) << 6)
            | (bits(half, 4, 3) << 1)
            | (bit(half, 2) << 5),
        9,
    )
}

fn compressed_jump_offset(half: u16) -> i64 {
    sign_extend(
        (bit(half, 12) << 11)
            | (bit(half, 11) << 4)
            | (bits(half, 10, 9) << 8)
            | (bit(half, 8) << 10)
            | (bit(half, 7) << 6)
            | (bit(half, 6) << 7)
            | (bits(half, 5, 3) << 1)
            | (bit(half, 2) << 5),
        12,
    )
}

fn compressed_shift_amount(half: u16) -> u8 {
    ((bit(half, 12) << 5) | bits(half, 6, 2)) as u8
}

fn compressed_register(field: u32) -> Register {
    register(field + 8)
}

fn register(index: u32) -> Register {
    Register::from_field(index)
}

fn bit(half: u16, index: u32) -> u32 {
    bits(half, index, index)
}

fn bits(half: u16, high: u32, low: u32) -> u32 {
    debug_assert!(high < 16);
    debug_assert!(low <= high);
    (u32::from(half) >> low) & ((1 << (high - low + 1)) - 1)
}

fn sign_extend(value: u32, bits: u32) -> i64 {
    let shift = 64 - bits;
    ((u64::from(value) << shift) as i64) >> shift
}

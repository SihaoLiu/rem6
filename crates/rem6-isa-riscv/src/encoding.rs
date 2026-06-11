use crate::Register;

pub(crate) fn rd(raw: u32) -> Register {
    Register::from_field((raw >> 7) & 0x1f)
}

pub(crate) fn rs1(raw: u32) -> Register {
    Register::from_field((raw >> 15) & 0x1f)
}

pub(crate) fn rs2(raw: u32) -> Register {
    Register::from_field((raw >> 20) & 0x1f)
}

pub(crate) fn rs3(raw: u32) -> Register {
    Register::from_field((raw >> 27) & 0x1f)
}

pub(crate) fn funct3(raw: u32) -> u32 {
    (raw >> 12) & 0x7
}

pub(crate) fn funct2(raw: u32) -> u32 {
    (raw >> 25) & 0x3
}

pub(crate) fn funct7(raw: u32) -> u32 {
    (raw >> 25) & 0x7f
}

pub(crate) fn shift_funct6(raw: u32) -> u32 {
    (raw >> 26) & 0x3f
}

pub(crate) fn shamt64(raw: u32) -> u8 {
    ((raw >> 20) & 0x3f) as u8
}

pub(crate) fn shamt32(raw: u32) -> u8 {
    ((raw >> 20) & 0x1f) as u8
}

pub(crate) fn funct5(raw: u32) -> u32 {
    (raw >> 27) & 0x1f
}

pub(crate) fn csr(raw: u32) -> u16 {
    ((raw >> 20) & 0x0fff) as u16
}

pub(crate) fn aq(raw: u32) -> bool {
    ((raw >> 26) & 0x1) != 0
}

pub(crate) fn rl(raw: u32) -> bool {
    ((raw >> 25) & 0x1) != 0
}

pub(crate) fn i_imm(raw: u32) -> i64 {
    sign_extend((raw >> 20) as u64, 12)
}

pub(crate) fn s_imm(raw: u32) -> i64 {
    let imm = ((raw >> 25) << 5) | ((raw >> 7) & 0x1f);
    sign_extend(imm as u64, 12)
}

pub(crate) fn b_imm(raw: u32) -> i64 {
    let imm = (((raw >> 31) & 0x1) << 12)
        | (((raw >> 7) & 0x1) << 11)
        | (((raw >> 25) & 0x3f) << 5)
        | (((raw >> 8) & 0xf) << 1);
    sign_extend(imm as u64, 13)
}

pub(crate) fn u_imm(raw: u32) -> i64 {
    (raw & 0xffff_f000) as i32 as i64
}

pub(crate) fn j_imm(raw: u32) -> i64 {
    let imm = (((raw >> 31) & 0x1) << 20)
        | (((raw >> 12) & 0xff) << 12)
        | (((raw >> 20) & 0x1) << 11)
        | (((raw >> 21) & 0x3ff) << 1);
    sign_extend(imm as u64, 21)
}

fn sign_extend(value: u64, bits: u32) -> i64 {
    let shift = 64 - bits;
    ((value << shift) as i64) >> shift
}

use crate::encoding::{
    b_imm, csr, funct3, funct7, i_imm, rd, rs1, rs2, shamt32, shamt64, shift_funct6,
};
use crate::{
    Immediate, RiscvCounterCsr, RiscvError, RiscvFenceSet, RiscvFloatCsr, RiscvInstruction,
    RiscvInterruptCsr, RiscvMachineTrapCsr, RiscvStatusCsr, RiscvSupervisorTrapCsr,
    RiscvTranslationCsr, VectorRegister,
};

pub(crate) fn decode_system(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    match raw {
        0x0000_0073 => Ok(RiscvInstruction::Ecall),
        0x0010_0073 => Ok(RiscvInstruction::Ebreak),
        0x1050_0073 => Ok(RiscvInstruction::WaitForInterrupt),
        0x1020_0073 => Ok(RiscvInstruction::SupervisorReturn),
        0x3020_0073 => Ok(RiscvInstruction::MachineReturn),
        raw if is_sfence_vma(raw) => Ok(RiscvInstruction::SfenceVma {
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        _ => decode_csr(raw),
    }
}

const fn is_sfence_vma(raw: u32) -> bool {
    raw & 0xfe00_7fff == 0x1200_0073
}

pub(crate) fn decode_fence(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    match funct3(raw) {
        0x0 => Ok(RiscvInstruction::Fence {
            predecessor: RiscvFenceSet::from_bits((raw >> 24) & 0x0f),
            successor: RiscvFenceSet::from_bits((raw >> 20) & 0x0f),
            mode: ((raw >> 28) & 0x0f) as u8,
        }),
        0x1 => Ok(RiscvInstruction::FenceI),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

pub(crate) fn decode_op_imm(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    match funct3(raw) {
        0x0 => Ok(RiscvInstruction::Addi {
            rd: rd(raw),
            rs1: rs1(raw),
            imm: Immediate::new(i_imm(raw)),
        }),
        0x1 if shift_funct6(raw) == 0x00 => Ok(RiscvInstruction::Slli {
            rd: rd(raw),
            rs1: rs1(raw),
            shamt: shamt64(raw),
        }),
        0x2 => Ok(RiscvInstruction::Slti {
            rd: rd(raw),
            rs1: rs1(raw),
            imm: Immediate::new(i_imm(raw)),
        }),
        0x3 => Ok(RiscvInstruction::Sltiu {
            rd: rd(raw),
            rs1: rs1(raw),
            imm: Immediate::new(i_imm(raw)),
        }),
        0x4 => Ok(RiscvInstruction::Xori {
            rd: rd(raw),
            rs1: rs1(raw),
            imm: Immediate::new(i_imm(raw)),
        }),
        0x5 if shift_funct6(raw) == 0x00 => Ok(RiscvInstruction::Srli {
            rd: rd(raw),
            rs1: rs1(raw),
            shamt: shamt64(raw),
        }),
        0x5 if shift_funct6(raw) == 0x10 => Ok(RiscvInstruction::Srai {
            rd: rd(raw),
            rs1: rs1(raw),
            shamt: shamt64(raw),
        }),
        0x6 => Ok(RiscvInstruction::Ori {
            rd: rd(raw),
            rs1: rs1(raw),
            imm: Immediate::new(i_imm(raw)),
        }),
        0x7 => Ok(RiscvInstruction::Andi {
            rd: rd(raw),
            rs1: rs1(raw),
            imm: Immediate::new(i_imm(raw)),
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

pub(crate) fn decode_op_imm_32(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    match (funct7(raw), funct3(raw)) {
        (_, 0x0) => Ok(RiscvInstruction::Addiw {
            rd: rd(raw),
            rs1: rs1(raw),
            imm: Immediate::new(i_imm(raw)),
        }),
        (0x00, 0x1) => Ok(RiscvInstruction::Slliw {
            rd: rd(raw),
            rs1: rs1(raw),
            shamt: shamt32(raw),
        }),
        (0x00, 0x5) => Ok(RiscvInstruction::Srliw {
            rd: rd(raw),
            rs1: rs1(raw),
            shamt: shamt32(raw),
        }),
        (0x20, 0x5) => Ok(RiscvInstruction::Sraiw {
            rd: rd(raw),
            rs1: rs1(raw),
            shamt: shamt32(raw),
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

pub(crate) fn decode_op(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    match (funct7(raw), funct3(raw)) {
        (0x00, 0x0) => Ok(RiscvInstruction::Add {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x20, 0x0) => Ok(RiscvInstruction::Sub {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x00, 0x1) => Ok(RiscvInstruction::Sll {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x00, 0x2) => Ok(RiscvInstruction::Slt {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x00, 0x3) => Ok(RiscvInstruction::Sltu {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x00, 0x4) => Ok(RiscvInstruction::Xor {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x00, 0x5) => Ok(RiscvInstruction::Srl {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x20, 0x5) => Ok(RiscvInstruction::Sra {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x00, 0x6) => Ok(RiscvInstruction::Or {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x00, 0x7) => Ok(RiscvInstruction::And {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x01, 0x0) => Ok(RiscvInstruction::Mul {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x01, 0x1) => Ok(RiscvInstruction::Mulh {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x01, 0x2) => Ok(RiscvInstruction::Mulhsu {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x01, 0x3) => Ok(RiscvInstruction::Mulhu {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x01, 0x4) => Ok(RiscvInstruction::Div {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x01, 0x5) => Ok(RiscvInstruction::Divu {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x01, 0x6) => Ok(RiscvInstruction::Rem {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x01, 0x7) => Ok(RiscvInstruction::Remu {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

pub(crate) fn decode_op_32(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    match (funct7(raw), funct3(raw)) {
        (0x00, 0x0) => Ok(RiscvInstruction::Addw {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x20, 0x0) => Ok(RiscvInstruction::Subw {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x00, 0x1) => Ok(RiscvInstruction::Sllw {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x00, 0x5) => Ok(RiscvInstruction::Srlw {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x20, 0x5) => Ok(RiscvInstruction::Sraw {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x01, 0x0) => Ok(RiscvInstruction::Mulw {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x01, 0x4) => Ok(RiscvInstruction::Divw {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x01, 0x5) => Ok(RiscvInstruction::Divuw {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x01, 0x6) => Ok(RiscvInstruction::Remw {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        (0x01, 0x7) => Ok(RiscvInstruction::Remuw {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

pub(crate) fn decode_vector(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    match funct3(raw) {
        0x0 if vector_funct6(raw) == 0 && vector_unmasked(raw) => {
            Ok(RiscvInstruction::VectorAddVv {
                vd: vector_register(raw, 7),
                vs1: vector_register(raw, 15),
                vs2: vector_register(raw, 20),
            })
        }
        0x3 if vector_funct6(raw) == 0 && vector_unmasked(raw) => {
            Ok(RiscvInstruction::VectorAddVi {
                vd: vector_register(raw, 7),
                vs2: vector_register(raw, 20),
                imm: vector_signed_imm5(raw),
            })
        }
        0x4 if vector_funct6(raw) == 0 && vector_unmasked(raw) => {
            Ok(RiscvInstruction::VectorAddVx {
                vd: vector_register(raw, 7),
                vs2: vector_register(raw, 20),
                rs1: rs1(raw),
            })
        }
        0x7 if (raw & 0x8000_0000) == 0 => Ok(RiscvInstruction::VectorSetVli {
            rd: rd(raw),
            rs1: rs1(raw),
            vtype: u64::from((raw >> 20) & 0x7ff),
        }),
        0x7 if (raw & 0xfe00_0000) == 0x8000_0000 => Ok(RiscvInstruction::VectorSetVl {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
        }),
        0x7 if (raw & 0xc000_0000) == 0xc000_0000 => Ok(RiscvInstruction::VectorSetIvli {
            rd: rd(raw),
            avl: ((raw >> 15) & 0x1f) as u8,
            vtype: u64::from((raw >> 20) & 0x3ff),
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

fn vector_funct6(raw: u32) -> u32 {
    (raw >> 26) & 0x3f
}

fn vector_unmasked(raw: u32) -> bool {
    (raw & (1 << 25)) != 0
}

fn vector_register(raw: u32, shift: u32) -> VectorRegister {
    VectorRegister::from_field((raw >> shift) & 0x1f)
}

fn vector_signed_imm5(raw: u32) -> i8 {
    let value = ((raw >> 15) & 0x1f) as i8;
    (value << 3) >> 3
}

pub(crate) fn decode_branch(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    match funct3(raw) {
        0x0 => Ok(RiscvInstruction::Beq {
            rs1: rs1(raw),
            rs2: rs2(raw),
            offset: Immediate::new(b_imm(raw)),
        }),
        0x1 => Ok(RiscvInstruction::Bne {
            rs1: rs1(raw),
            rs2: rs2(raw),
            offset: Immediate::new(b_imm(raw)),
        }),
        0x4 => Ok(RiscvInstruction::Blt {
            rs1: rs1(raw),
            rs2: rs2(raw),
            offset: Immediate::new(b_imm(raw)),
        }),
        0x5 => Ok(RiscvInstruction::Bge {
            rs1: rs1(raw),
            rs2: rs2(raw),
            offset: Immediate::new(b_imm(raw)),
        }),
        0x6 => Ok(RiscvInstruction::Bltu {
            rs1: rs1(raw),
            rs2: rs2(raw),
            offset: Immediate::new(b_imm(raw)),
        }),
        0x7 => Ok(RiscvInstruction::Bgeu {
            rs1: rs1(raw),
            rs2: rs2(raw),
            offset: Immediate::new(b_imm(raw)),
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

pub(crate) fn decode_jalr(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    match funct3(raw) {
        0x0 => Ok(RiscvInstruction::Jalr {
            rd: rd(raw),
            rs1: rs1(raw),
            offset: Immediate::new(i_imm(raw)),
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

pub(crate) fn decode_csr(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let csr_address = csr(raw);
    if is_csr_no_write_read(raw) {
        return match csr_address {
            0xf14 => Ok(RiscvInstruction::ReadMachineHartId { rd: rd(raw) }),
            csr_address => RiscvCounterCsr::from_user_address(csr_address)
                .ok()
                .map(|csr| RiscvInstruction::ReadCounterCsr { rd: rd(raw), csr })
                .or_else(|| {
                    machine_counter_csr(csr_address)
                        .map(|csr| RiscvInstruction::ReadMachineCounterCsr { rd: rd(raw), csr })
                })
                .or_else(|| {
                    RiscvFloatCsr::from_address(csr_address)
                        .map(|csr| RiscvInstruction::ReadFloatCsr { rd: rd(raw), csr })
                })
                .or_else(|| {
                    RiscvStatusCsr::from_address(csr_address)
                        .map(|csr| RiscvInstruction::ReadStatusCsr { rd: rd(raw), csr })
                })
                .or_else(|| {
                    RiscvInterruptCsr::from_address(csr_address)
                        .map(|csr| RiscvInstruction::ReadInterruptCsr { rd: rd(raw), csr })
                })
                .or_else(|| {
                    RiscvMachineTrapCsr::from_address(csr_address)
                        .map(|csr| RiscvInstruction::ReadMachineTrapCsr { rd: rd(raw), csr })
                })
                .or_else(|| {
                    RiscvSupervisorTrapCsr::from_address(csr_address)
                        .map(|csr| RiscvInstruction::ReadSupervisorTrapCsr { rd: rd(raw), csr })
                })
                .or_else(|| {
                    RiscvTranslationCsr::from_address(csr_address)
                        .map(|csr| RiscvInstruction::ReadTranslationCsr { rd: rd(raw), csr })
                })
                .ok_or(RiscvError::UnknownEncoding { raw }),
        };
    }

    if let Some(csr) = RiscvFloatCsr::from_address(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(RiscvInstruction::WriteFloatCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x2 => Ok(RiscvInstruction::SetFloatCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x3 => Ok(RiscvInstruction::ClearFloatCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x5 => Ok(RiscvInstruction::WriteFloatCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x6 => Ok(RiscvInstruction::SetFloatCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x7 => Ok(RiscvInstruction::ClearFloatCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        };
    }

    if let Some(csr) = RiscvInterruptCsr::from_address(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(RiscvInstruction::WriteInterruptCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x2 => Ok(RiscvInstruction::SetInterruptCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x3 => Ok(RiscvInstruction::ClearInterruptCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x5 => Ok(RiscvInstruction::WriteInterruptCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x6 => Ok(RiscvInstruction::SetInterruptCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x7 => Ok(RiscvInstruction::ClearInterruptCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        };
    }

    if let Some(csr) = machine_counter_csr(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(RiscvInstruction::WriteCounterCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x2 => Ok(RiscvInstruction::SetCounterCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x3 => Ok(RiscvInstruction::ClearCounterCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x5 => Ok(RiscvInstruction::WriteCounterCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x6 => Ok(RiscvInstruction::SetCounterCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x7 => Ok(RiscvInstruction::ClearCounterCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        };
    }

    if let Some(csr) = RiscvStatusCsr::from_address(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(RiscvInstruction::WriteStatusCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x2 => Ok(RiscvInstruction::SetStatusCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x3 => Ok(RiscvInstruction::ClearStatusCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x5 => Ok(RiscvInstruction::WriteStatusCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x6 => Ok(RiscvInstruction::SetStatusCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x7 => Ok(RiscvInstruction::ClearStatusCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        };
    }

    if let Some(csr) = RiscvMachineTrapCsr::from_address(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(RiscvInstruction::WriteMachineTrapCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x2 => Ok(RiscvInstruction::SetMachineTrapCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x3 => Ok(RiscvInstruction::ClearMachineTrapCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x5 => Ok(RiscvInstruction::WriteMachineTrapCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x6 => Ok(RiscvInstruction::SetMachineTrapCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x7 => Ok(RiscvInstruction::ClearMachineTrapCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        };
    }

    if let Some(csr) = RiscvSupervisorTrapCsr::from_address(csr_address) {
        return match funct3(raw) {
            0x1 => Ok(RiscvInstruction::WriteSupervisorTrapCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x2 => Ok(RiscvInstruction::SetSupervisorTrapCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x3 => Ok(RiscvInstruction::ClearSupervisorTrapCsr {
                rd: rd(raw),
                csr,
                rs1: rs1(raw),
            }),
            0x5 => Ok(RiscvInstruction::WriteSupervisorTrapCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x6 => Ok(RiscvInstruction::SetSupervisorTrapCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            0x7 => Ok(RiscvInstruction::ClearSupervisorTrapCsrImmediate {
                rd: rd(raw),
                csr,
                zimm: rs1(raw).index(),
            }),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        };
    }

    let Some(csr) = RiscvTranslationCsr::from_address(csr_address) else {
        return Err(RiscvError::UnknownEncoding { raw });
    };
    match funct3(raw) {
        0x1 => Ok(RiscvInstruction::WriteTranslationCsr {
            rd: rd(raw),
            csr,
            rs1: rs1(raw),
        }),
        0x2 => Ok(RiscvInstruction::SetTranslationCsr {
            rd: rd(raw),
            csr,
            rs1: rs1(raw),
        }),
        0x3 => Ok(RiscvInstruction::ClearTranslationCsr {
            rd: rd(raw),
            csr,
            rs1: rs1(raw),
        }),
        0x5 => Ok(RiscvInstruction::WriteTranslationCsrImmediate {
            rd: rd(raw),
            csr,
            zimm: rs1(raw).index(),
        }),
        0x6 => Ok(RiscvInstruction::SetTranslationCsrImmediate {
            rd: rd(raw),
            csr,
            zimm: rs1(raw).index(),
        }),
        0x7 => Ok(RiscvInstruction::ClearTranslationCsrImmediate {
            rd: rd(raw),
            csr,
            zimm: rs1(raw).index(),
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

fn is_csr_no_write_read(raw: u32) -> bool {
    matches!((funct3(raw), rs1(raw).index()), (0x2 | 0x3 | 0x6 | 0x7, 0))
}

fn machine_counter_csr(address: u16) -> Option<RiscvCounterCsr> {
    RiscvCounterCsr::from_machine_address(address).ok()
}

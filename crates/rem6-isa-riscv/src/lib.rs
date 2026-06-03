mod control_flow;
mod csr;
mod decode;
mod encoding;
mod error;
mod gdb_target;
mod hart;
mod instruction;
mod integer;
mod pma;
mod pmp;
mod record;
mod sv39;
mod types;
mod vector;

use encoding::{
    aq, b_imm, funct3, funct5, funct7, i_imm, j_imm, rd, rl, rs1, rs2, s_imm, shamt32, shamt64,
    shift_funct6, u_imm,
};
use integer::{
    div_signed, div_signed_word, div_unsigned, div_unsigned_word, mulh_signed,
    mulh_signed_unsigned, mulh_unsigned, rem_signed, rem_signed_word, rem_unsigned,
    rem_unsigned_word, sign_extend_word,
};

pub use control_flow::{
    RiscvBranchPredictionTarget, RiscvControlFlowSnapshot, RiscvControlFlowUpdate,
    RiscvVectorConfig, RiscvVectorConfigUpdate,
};
pub use csr::{
    RiscvCounterBank, RiscvCounterCsr, RiscvCounterCsrWord, RiscvCounterSnapshot,
    RiscvMachineTrapCsr, RiscvStatusCsr, RiscvStatusWord, RiscvSupervisorTrapCsr,
    RiscvTranslationCsr,
};
pub use error::{RiscvCsrError, RiscvError};
pub use gdb_target::{RiscvGdbTargetDescription, RiscvGdbTargetDocument, RiscvGdbXlen};
pub use instruction::RiscvInstruction;
pub use pma::{RiscvPmaAccessKind, RiscvPmaError, RiscvPmaRange, RiscvPmaTable};
pub use pmp::{
    RiscvPmpAccessKind, RiscvPmpAddressMode, RiscvPmpConfig, RiscvPmpEntry, RiscvPmpError,
    RiscvPmpRange, RiscvPmpSnapshot, RiscvPmpSnapshotEntry, RiscvPmpTable, RiscvPrivilegeMode,
};
pub use record::{RegisterWrite, RiscvExecutionRecord, RiscvSystemEvent, RiscvTrap, RiscvTrapKind};
pub use sv39::{
    walk_sv39_page_table, walk_sv39_page_table_with_context, RiscvSv39AccessContext,
    RiscvSv39AccessKind, RiscvSv39PageFault, RiscvSv39PageTableLevel, RiscvSv39Pte,
    RiscvSv39VirtualAddress, RiscvSv39WalkAdvance, RiscvSv39WalkResult, RiscvSv39WalkState,
};
pub use types::{
    AtomicMemoryOp, Immediate, MemoryAccessKind, MemoryResponseError, MemoryResponseWriteback,
    MemoryWidth, Register, RiscvFenceSet, RiscvMemoryOrdering,
};
pub use vector::{
    RiscvInstructionFlags, RiscvVectorCompressPlan, RiscvVectorCompressResult, RiscvVectorElements,
    RiscvVectorError, RiscvVectorFixedPointState, RiscvVectorFixedRoundingMode, RiscvVectorMicroOp,
    RiscvVectorMicroOpExpansion, RiscvVectorNarrowClipPlan, RiscvVectorNarrowClipResult,
    RiscvVectorTailPolicy,
};

impl RiscvInstruction {
    pub fn decode(raw: u32) -> Result<Self, RiscvError> {
        if raw & 0x3 != 0x3 {
            return Err(RiscvError::CompressedNotSupported { raw });
        }

        let opcode = raw & 0x7f;
        match opcode {
            0x03 => decode_load(raw),
            0x0f => decode_fence(raw),
            0x13 => decode_op_imm(raw),
            0x17 => Ok(Self::Auipc {
                rd: rd(raw),
                imm: Immediate::new(u_imm(raw)),
            }),
            0x1b => decode_op_imm_32(raw),
            0x23 => decode_store(raw),
            0x2f => decode_atomic(raw),
            0x33 => decode_op(raw),
            0x3b => decode_op_32(raw),
            0x37 => Ok(Self::Lui {
                rd: rd(raw),
                imm: Immediate::new(u_imm(raw)),
            }),
            0x63 => decode_branch(raw),
            0x67 => decode_jalr(raw),
            0x6f => Ok(Self::Jal {
                rd: rd(raw),
                offset: Immediate::new(j_imm(raw)),
            }),
            0x73 => decode_system(raw),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        }
    }
}

fn decode_fence(raw: u32) -> Result<RiscvInstruction, RiscvError> {
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

fn decode_system(raw: u32) -> Result<RiscvInstruction, RiscvError> {
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
        _ => decode::decode_csr(raw),
    }
}

const fn is_sfence_vma(raw: u32) -> bool {
    raw & 0xfe00_7fff == 0x1200_0073
}

fn decode_op_imm(raw: u32) -> Result<RiscvInstruction, RiscvError> {
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

fn decode_op_imm_32(raw: u32) -> Result<RiscvInstruction, RiscvError> {
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

fn decode_op(raw: u32) -> Result<RiscvInstruction, RiscvError> {
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

fn decode_op_32(raw: u32) -> Result<RiscvInstruction, RiscvError> {
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

fn decode_branch(raw: u32) -> Result<RiscvInstruction, RiscvError> {
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

fn decode_jalr(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    match funct3(raw) {
        0x0 => Ok(RiscvInstruction::Jalr {
            rd: rd(raw),
            rs1: rs1(raw),
            offset: Immediate::new(i_imm(raw)),
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

fn decode_load(raw: u32) -> Result<RiscvInstruction, RiscvError> {
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

fn decode_store(raw: u32) -> Result<RiscvInstruction, RiscvError> {
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

fn decode_atomic(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let width = match funct3(raw) {
        0x2 => MemoryWidth::Word,
        0x3 => MemoryWidth::Doubleword,
        _ => return Err(RiscvError::UnknownEncoding { raw }),
    };

    match (funct5(raw), rs2(raw).index()) {
        (0x02, 0) => Ok(RiscvInstruction::LoadReserved {
            rd: rd(raw),
            rs1: rs1(raw),
            width,
            acquire: aq(raw),
            release: rl(raw),
        }),
        (0x03, _) => Ok(RiscvInstruction::StoreConditional {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
            width,
            acquire: aq(raw),
            release: rl(raw),
        }),
        (funct5, _) => atomic_memory_op(funct5)
            .map(|op| RiscvInstruction::AtomicMemory {
                rd: rd(raw),
                rs1: rs1(raw),
                rs2: rs2(raw),
                width,
                op,
                acquire: aq(raw),
                release: rl(raw),
            })
            .ok_or(RiscvError::UnknownEncoding { raw }),
    }
}

fn atomic_memory_op(funct5: u32) -> Option<AtomicMemoryOp> {
    match funct5 {
        0x00 => Some(AtomicMemoryOp::Add),
        0x01 => Some(AtomicMemoryOp::Swap),
        0x04 => Some(AtomicMemoryOp::Xor),
        0x08 => Some(AtomicMemoryOp::Or),
        0x0c => Some(AtomicMemoryOp::And),
        0x10 => Some(AtomicMemoryOp::MinSigned),
        0x14 => Some(AtomicMemoryOp::MaxSigned),
        0x18 => Some(AtomicMemoryOp::MinUnsigned),
        0x1c => Some(AtomicMemoryOp::MaxUnsigned),
        _ => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvHartState {
    pc: u64,
    hart_id: u64,
    counters: RiscvCounterBank,
    supervisor_trap_vector: u64,
    supervisor_exception_pc: u64,
    supervisor_trap_cause: u64,
    supervisor_trap_value: u64,
    machine_exception_delegation: u64,
    machine_interrupt_delegation: u64,
    machine_trap_vector: u64,
    machine_exception_pc: u64,
    machine_trap_cause: u64,
    machine_trap_value: u64,
    translation_satp: u64,
    privilege_mode: RiscvPrivilegeMode,
    status: RiscvStatusWord,
    vector_config: RiscvVectorConfig,
    registers: [u64; 32],
}

impl RiscvHartState {
    pub const fn new(pc: u64) -> Self {
        Self::with_hart_id(pc, 0)
    }

    pub const fn with_hart_id(pc: u64, hart_id: u64) -> Self {
        Self {
            pc,
            hart_id,
            counters: RiscvCounterBank::new(),
            supervisor_trap_vector: 0,
            supervisor_exception_pc: 0,
            supervisor_trap_cause: 0,
            supervisor_trap_value: 0,
            machine_exception_delegation: 0,
            machine_interrupt_delegation: 0,
            machine_trap_vector: 0,
            machine_exception_pc: 0,
            machine_trap_cause: 0,
            machine_trap_value: 0,
            translation_satp: 0,
            privilege_mode: RiscvPrivilegeMode::Machine,
            status: RiscvStatusWord::new(0),
            vector_config: RiscvVectorConfig::invalid(),
            registers: [0; 32],
        }
    }

    pub fn execute(
        &mut self,
        instruction: RiscvInstruction,
    ) -> Result<RiscvExecutionRecord, RiscvError> {
        let pc = self.pc;
        let mut next_pc = pc
            .checked_add(4)
            .ok_or(RiscvError::PcOverflow { pc, offset: 4 })?;
        let mut register_writes = Vec::new();
        let mut memory_access = None;
        let mut system_event = None;

        match instruction {
            RiscvInstruction::Lui { rd, imm } => {
                write_register(self, &mut register_writes, rd, imm.value() as u64);
            }
            RiscvInstruction::Auipc { rd, imm } => {
                let value = add_signed(pc, imm.value())?;
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Addi { rd, rs1, imm } => {
                let value = wrapping_add_signed(self.read(rs1), imm.value());
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Slti { rd, rs1, imm } => {
                let value = u64::from((self.read(rs1) as i64) < imm.value());
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Sltiu { rd, rs1, imm } => {
                let value = u64::from(self.read(rs1) < (imm.value() as u64));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Xori { rd, rs1, imm } => {
                let value = self.read(rs1) ^ (imm.value() as u64);
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Ori { rd, rs1, imm } => {
                let value = self.read(rs1) | (imm.value() as u64);
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Andi { rd, rs1, imm } => {
                let value = self.read(rs1) & (imm.value() as u64);
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Slli { rd, rs1, shamt } => {
                let value = self.read(rs1).wrapping_shl(u32::from(shamt));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Srli { rd, rs1, shamt } => {
                let value = self.read(rs1).wrapping_shr(u32::from(shamt));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Srai { rd, rs1, shamt } => {
                let value = (self.read(rs1) as i64).wrapping_shr(u32::from(shamt)) as u64;
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Addiw { rd, rs1, imm } => {
                let value = (self.read(rs1) as u32).wrapping_add(imm.value() as u32);
                write_register(self, &mut register_writes, rd, sign_extend_word(value));
            }
            RiscvInstruction::Slliw { rd, rs1, shamt } => {
                let value = (self.read(rs1) as u32).wrapping_shl(u32::from(shamt));
                write_register(self, &mut register_writes, rd, sign_extend_word(value));
            }
            RiscvInstruction::Srliw { rd, rs1, shamt } => {
                let value = (self.read(rs1) as u32).wrapping_shr(u32::from(shamt));
                write_register(self, &mut register_writes, rd, sign_extend_word(value));
            }
            RiscvInstruction::Sraiw { rd, rs1, shamt } => {
                let value = (self.read(rs1) as u32 as i32).wrapping_shr(u32::from(shamt));
                write_register(
                    self,
                    &mut register_writes,
                    rd,
                    sign_extend_word(value as u32),
                );
            }
            RiscvInstruction::Add { rd, rs1, rs2 } => {
                let value = self.read(rs1).wrapping_add(self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Sub { rd, rs1, rs2 } => {
                let value = self.read(rs1).wrapping_sub(self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Sll { rd, rs1, rs2 } => {
                let value = self.read(rs1).wrapping_shl((self.read(rs2) & 0x3f) as u32);
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Slt { rd, rs1, rs2 } => {
                let value = u64::from((self.read(rs1) as i64) < (self.read(rs2) as i64));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Sltu { rd, rs1, rs2 } => {
                let value = u64::from(self.read(rs1) < self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Xor { rd, rs1, rs2 } => {
                let value = self.read(rs1) ^ self.read(rs2);
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Srl { rd, rs1, rs2 } => {
                let value = self.read(rs1).wrapping_shr((self.read(rs2) & 0x3f) as u32);
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Sra { rd, rs1, rs2 } => {
                let value = (self.read(rs1) as i64).wrapping_shr((self.read(rs2) & 0x3f) as u32);
                write_register(self, &mut register_writes, rd, value as u64);
            }
            RiscvInstruction::Or { rd, rs1, rs2 } => {
                let value = self.read(rs1) | self.read(rs2);
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::And { rd, rs1, rs2 } => {
                let value = self.read(rs1) & self.read(rs2);
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Mul { rd, rs1, rs2 } => {
                let value = self.read(rs1).wrapping_mul(self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Mulh { rd, rs1, rs2 } => {
                let value = mulh_signed(self.read(rs1), self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Mulhsu { rd, rs1, rs2 } => {
                let value = mulh_signed_unsigned(self.read(rs1), self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Mulhu { rd, rs1, rs2 } => {
                let value = mulh_unsigned(self.read(rs1), self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Div { rd, rs1, rs2 } => {
                let value = div_signed(self.read(rs1), self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Divu { rd, rs1, rs2 } => {
                let value = div_unsigned(self.read(rs1), self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Rem { rd, rs1, rs2 } => {
                let value = rem_signed(self.read(rs1), self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Remu { rd, rs1, rs2 } => {
                let value = rem_unsigned(self.read(rs1), self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Mulw { rd, rs1, rs2 } => {
                let value = (self.read(rs1) as u32).wrapping_mul(self.read(rs2) as u32);
                write_register(self, &mut register_writes, rd, sign_extend_word(value));
            }
            RiscvInstruction::Divw { rd, rs1, rs2 } => {
                let value = div_signed_word(self.read(rs1), self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Divuw { rd, rs1, rs2 } => {
                let value = div_unsigned_word(self.read(rs1), self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Remw { rd, rs1, rs2 } => {
                let value = rem_signed_word(self.read(rs1), self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Remuw { rd, rs1, rs2 } => {
                let value = rem_unsigned_word(self.read(rs1), self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Addw { rd, rs1, rs2 } => {
                let value = (self.read(rs1) as u32).wrapping_add(self.read(rs2) as u32);
                write_register(self, &mut register_writes, rd, sign_extend_word(value));
            }
            RiscvInstruction::Subw { rd, rs1, rs2 } => {
                let value = (self.read(rs1) as u32).wrapping_sub(self.read(rs2) as u32);
                write_register(self, &mut register_writes, rd, sign_extend_word(value));
            }
            RiscvInstruction::Sllw { rd, rs1, rs2 } => {
                let value = (self.read(rs1) as u32).wrapping_shl((self.read(rs2) & 0x1f) as u32);
                write_register(self, &mut register_writes, rd, sign_extend_word(value));
            }
            RiscvInstruction::Srlw { rd, rs1, rs2 } => {
                let value = (self.read(rs1) as u32).wrapping_shr((self.read(rs2) & 0x1f) as u32);
                write_register(self, &mut register_writes, rd, sign_extend_word(value));
            }
            RiscvInstruction::Sraw { rd, rs1, rs2 } => {
                let value =
                    (self.read(rs1) as u32 as i32).wrapping_shr((self.read(rs2) & 0x1f) as u32);
                write_register(
                    self,
                    &mut register_writes,
                    rd,
                    sign_extend_word(value as u32),
                );
            }
            RiscvInstruction::Beq { rs1, rs2, offset } => {
                if self.read(rs1) == self.read(rs2) {
                    next_pc = add_signed(pc, offset.value())?;
                }
            }
            RiscvInstruction::Bne { rs1, rs2, offset } => {
                if self.read(rs1) != self.read(rs2) {
                    next_pc = add_signed(pc, offset.value())?;
                }
            }
            RiscvInstruction::Blt { rs1, rs2, offset } => {
                if (self.read(rs1) as i64) < (self.read(rs2) as i64) {
                    next_pc = add_signed(pc, offset.value())?;
                }
            }
            RiscvInstruction::Bge { rs1, rs2, offset } => {
                if (self.read(rs1) as i64) >= (self.read(rs2) as i64) {
                    next_pc = add_signed(pc, offset.value())?;
                }
            }
            RiscvInstruction::Bltu { rs1, rs2, offset } => {
                if self.read(rs1) < self.read(rs2) {
                    next_pc = add_signed(pc, offset.value())?;
                }
            }
            RiscvInstruction::Bgeu { rs1, rs2, offset } => {
                if self.read(rs1) >= self.read(rs2) {
                    next_pc = add_signed(pc, offset.value())?;
                }
            }
            RiscvInstruction::Jal { rd, offset } => {
                write_register(self, &mut register_writes, rd, next_pc);
                next_pc = add_signed(pc, offset.value())?;
            }
            RiscvInstruction::Jalr { rd, rs1, offset } => {
                write_register(self, &mut register_writes, rd, next_pc);
                next_pc = add_signed(self.read(rs1), offset.value())? & !1;
            }
            RiscvInstruction::Load {
                rd,
                rs1,
                offset,
                width,
                signed,
            } => {
                let address = add_signed(self.read(rs1), offset.value())?;
                memory_access = Some(MemoryAccessKind::Load {
                    rd,
                    address,
                    width,
                    signed,
                });
            }
            RiscvInstruction::Store {
                rs1,
                rs2,
                offset,
                width,
            } => {
                let address = add_signed(self.read(rs1), offset.value())?;
                memory_access = Some(MemoryAccessKind::Store {
                    address,
                    width,
                    value: self.read(rs2),
                });
            }
            RiscvInstruction::LoadReserved {
                rd,
                rs1,
                width,
                acquire,
                release,
            } => {
                memory_access = Some(MemoryAccessKind::LoadReserved {
                    rd,
                    address: self.read(rs1),
                    width,
                    acquire,
                    release,
                });
            }
            RiscvInstruction::StoreConditional {
                rd,
                rs1,
                rs2,
                width,
                acquire,
                release,
            } => {
                memory_access = Some(MemoryAccessKind::StoreConditional {
                    rd,
                    address: self.read(rs1),
                    width,
                    value: self.read(rs2),
                    acquire,
                    release,
                });
            }
            RiscvInstruction::AtomicMemory {
                rd,
                rs1,
                rs2,
                width,
                op,
                acquire,
                release,
            } => {
                memory_access = Some(MemoryAccessKind::AtomicMemory {
                    rd,
                    address: self.read(rs1),
                    width,
                    op,
                    value: self.read(rs2),
                    acquire,
                    release,
                });
            }
            RiscvInstruction::Fence { .. } | RiscvInstruction::FenceI => {}
            RiscvInstruction::WaitForInterrupt => {
                system_event = Some(RiscvSystemEvent::WaitForInterrupt { pc });
            }
            RiscvInstruction::SupervisorReturn => {
                let privilege = self.status.spp();
                next_pc = self.supervisor_exception_pc;
                self.privilege_mode = privilege;
                self.status = self
                    .status
                    .with_sie(self.status.spie())
                    .with_spie(true)
                    .with_spp(RiscvPrivilegeMode::User)
                    .with_mprv(false);
            }
            RiscvInstruction::MachineReturn => {
                let privilege = self.status.mpp();
                next_pc = self.machine_exception_pc;
                self.privilege_mode = privilege;
                self.status = self
                    .status
                    .with_mie(self.status.mpie())
                    .with_mpie(true)
                    .with_mpp(RiscvPrivilegeMode::User)
                    .with_mprv(privilege == RiscvPrivilegeMode::Machine && self.status.mprv());
            }
            RiscvInstruction::SfenceVma { rs1, rs2 } => {
                system_event = Some(RiscvSystemEvent::SfenceVma {
                    pc,
                    virtual_address: (!rs1.is_zero()).then(|| self.read(rs1)),
                    address_space: (!rs2.is_zero()).then(|| self.read(rs2)),
                });
            }
            RiscvInstruction::ReadMachineHartId { rd } => {
                write_register(self, &mut register_writes, rd, self.hart_id);
            }
            RiscvInstruction::ReadCounterCsr { rd, csr } => {
                let value = self.counters.read_machine(csr);
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::WriteCounterCsr { rd, csr, rs1 } => {
                let value = self.read(rs1);
                write_counter_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::SetCounterCsr { rd, csr, rs1 } => {
                let value = self.counters.read_machine(csr) | self.read(rs1);
                write_counter_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ClearCounterCsr { rd, csr, rs1 } => {
                let value = self.counters.read_machine(csr) & !self.read(rs1);
                write_counter_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::WriteCounterCsrImmediate { rd, csr, zimm } => {
                write_counter_csr(self, &mut register_writes, rd, csr, u64::from(zimm));
            }
            RiscvInstruction::SetCounterCsrImmediate { rd, csr, zimm } => {
                let value = self.counters.read_machine(csr) | u64::from(zimm);
                write_counter_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ClearCounterCsrImmediate { rd, csr, zimm } => {
                let value = self.counters.read_machine(csr) & !u64::from(zimm);
                write_counter_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ReadStatusCsr { rd, csr } => {
                write_register(self, &mut register_writes, rd, read_status_csr(self, csr));
            }
            RiscvInstruction::WriteStatusCsr { rd, csr, rs1 } => {
                write_status_csr(self, &mut register_writes, rd, csr, self.read(rs1));
            }
            RiscvInstruction::SetStatusCsr { rd, csr, rs1 } => {
                let value = read_status_csr(self, csr) | self.read(rs1);
                write_status_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ClearStatusCsr { rd, csr, rs1 } => {
                let value = read_status_csr(self, csr) & !self.read(rs1);
                write_status_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::WriteStatusCsrImmediate { rd, csr, zimm } => {
                write_status_csr(self, &mut register_writes, rd, csr, u64::from(zimm));
            }
            RiscvInstruction::SetStatusCsrImmediate { rd, csr, zimm } => {
                let value = read_status_csr(self, csr) | u64::from(zimm);
                write_status_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ClearStatusCsrImmediate { rd, csr, zimm } => {
                let value = read_status_csr(self, csr) & !u64::from(zimm);
                write_status_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ReadMachineTrapCsr { rd, csr } => {
                write_register(
                    self,
                    &mut register_writes,
                    rd,
                    read_machine_trap_csr(self, csr),
                );
            }
            RiscvInstruction::WriteMachineTrapCsr { rd, csr, rs1 } => {
                write_machine_trap_csr(self, &mut register_writes, rd, csr, self.read(rs1));
            }
            RiscvInstruction::SetMachineTrapCsr { rd, csr, rs1 } => {
                let value = read_machine_trap_csr(self, csr) | self.read(rs1);
                write_machine_trap_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ClearMachineTrapCsr { rd, csr, rs1 } => {
                let value = read_machine_trap_csr(self, csr) & !self.read(rs1);
                write_machine_trap_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::WriteMachineTrapCsrImmediate { rd, csr, zimm } => {
                write_machine_trap_csr(self, &mut register_writes, rd, csr, u64::from(zimm));
            }
            RiscvInstruction::SetMachineTrapCsrImmediate { rd, csr, zimm } => {
                let value = read_machine_trap_csr(self, csr) | u64::from(zimm);
                write_machine_trap_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ClearMachineTrapCsrImmediate { rd, csr, zimm } => {
                let value = read_machine_trap_csr(self, csr) & !u64::from(zimm);
                write_machine_trap_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ReadSupervisorTrapCsr { rd, csr } => {
                write_register(
                    self,
                    &mut register_writes,
                    rd,
                    read_supervisor_trap_csr(self, csr),
                );
            }
            RiscvInstruction::WriteSupervisorTrapCsr { rd, csr, rs1 } => {
                write_supervisor_trap_csr(self, &mut register_writes, rd, csr, self.read(rs1));
            }
            RiscvInstruction::SetSupervisorTrapCsr { rd, csr, rs1 } => {
                let value = read_supervisor_trap_csr(self, csr) | self.read(rs1);
                write_supervisor_trap_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ClearSupervisorTrapCsr { rd, csr, rs1 } => {
                let value = read_supervisor_trap_csr(self, csr) & !self.read(rs1);
                write_supervisor_trap_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::WriteSupervisorTrapCsrImmediate { rd, csr, zimm } => {
                write_supervisor_trap_csr(self, &mut register_writes, rd, csr, u64::from(zimm));
            }
            RiscvInstruction::SetSupervisorTrapCsrImmediate { rd, csr, zimm } => {
                let value = read_supervisor_trap_csr(self, csr) | u64::from(zimm);
                write_supervisor_trap_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ClearSupervisorTrapCsrImmediate { rd, csr, zimm } => {
                let value = read_supervisor_trap_csr(self, csr) & !u64::from(zimm);
                write_supervisor_trap_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ReadTranslationCsr { rd, csr } => {
                write_register(
                    self,
                    &mut register_writes,
                    rd,
                    read_translation_csr(self, csr),
                );
            }
            RiscvInstruction::WriteTranslationCsr { rd, csr, rs1 } => {
                write_translation_csr(self, &mut register_writes, rd, csr, self.read(rs1));
            }
            RiscvInstruction::SetTranslationCsr { rd, csr, rs1 } => {
                let value = read_translation_csr(self, csr) | self.read(rs1);
                write_translation_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ClearTranslationCsr { rd, csr, rs1 } => {
                let value = read_translation_csr(self, csr) & !self.read(rs1);
                write_translation_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::WriteTranslationCsrImmediate { rd, csr, zimm } => {
                write_translation_csr(self, &mut register_writes, rd, csr, u64::from(zimm));
            }
            RiscvInstruction::SetTranslationCsrImmediate { rd, csr, zimm } => {
                let value = read_translation_csr(self, csr) | u64::from(zimm);
                write_translation_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ClearTranslationCsrImmediate { rd, csr, zimm } => {
                let value = read_translation_csr(self, csr) & !u64::from(zimm);
                write_translation_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::Ecall => {
                return Ok(enter_synchronous_trap(
                    self,
                    instruction,
                    pc,
                    RiscvTrapKind::EnvironmentCall,
                ));
            }
            RiscvInstruction::Ebreak => {
                return Ok(enter_synchronous_trap(
                    self,
                    instruction,
                    pc,
                    RiscvTrapKind::Breakpoint,
                ));
            }
        }

        self.pc = next_pc;
        self.counters.add_cycles(1);
        self.counters.retire_instructions(1);
        match system_event {
            Some(system_event) => {
                debug_assert!(register_writes.is_empty());
                debug_assert!(memory_access.is_none());
                Ok(RiscvExecutionRecord::with_system_event(
                    instruction,
                    pc,
                    next_pc,
                    system_event,
                ))
            }
            None => Ok(RiscvExecutionRecord::new(
                instruction,
                pc,
                next_pc,
                register_writes,
                memory_access,
            )),
        }
    }
}

fn enter_synchronous_trap(
    hart: &mut RiscvHartState,
    instruction: RiscvInstruction,
    pc: u64,
    kind: RiscvTrapKind,
) -> RiscvExecutionRecord {
    let previous_privilege = hart.privilege_mode();
    let cause = machine_trap_cause(kind, previous_privilege);
    if exception_delegated_to_supervisor(hart, previous_privilege, cause) {
        enter_supervisor_trap(hart, instruction, pc, kind, cause, previous_privilege)
    } else {
        enter_machine_trap(hart, instruction, pc, kind)
    }
}

fn exception_delegated_to_supervisor(
    hart: &RiscvHartState,
    previous_privilege: RiscvPrivilegeMode,
    cause: u64,
) -> bool {
    if matches!(previous_privilege, RiscvPrivilegeMode::Machine) || cause >= u64::BITS as u64 {
        return false;
    }
    (hart.machine_exception_delegation() & (1_u64 << (cause as u32))) != 0
}

fn enter_supervisor_trap(
    hart: &mut RiscvHartState,
    instruction: RiscvInstruction,
    pc: u64,
    kind: RiscvTrapKind,
    cause: u64,
    previous_privilege: RiscvPrivilegeMode,
) -> RiscvExecutionRecord {
    let handler_pc = hart.supervisor_trap_vector() & !0b11;
    hart.set_supervisor_exception_pc(pc);
    hart.set_supervisor_trap_cause(cause);
    hart.set_supervisor_trap_value(0);
    hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let status = hart.status();
    hart.set_status(
        status
            .with_spp(previous_privilege)
            .with_spie(status.sie())
            .with_sie(false),
    );
    hart.set_pc(handler_pc);
    RiscvExecutionRecord::with_trap(instruction, pc, handler_pc, RiscvTrap::new(kind, pc))
}

fn enter_machine_trap(
    hart: &mut RiscvHartState,
    instruction: RiscvInstruction,
    pc: u64,
    kind: RiscvTrapKind,
) -> RiscvExecutionRecord {
    let previous_privilege = hart.privilege_mode();
    let cause = machine_trap_cause(kind, previous_privilege);
    let handler_pc = hart.machine_trap_vector() & !0b11;
    hart.set_machine_exception_pc(pc);
    hart.set_machine_trap_cause(cause);
    hart.set_machine_trap_value(0);
    hart.set_privilege_mode(RiscvPrivilegeMode::Machine);
    let status = hart.status();
    hart.set_status(
        status
            .with_mpp(previous_privilege)
            .with_mpie(status.mie())
            .with_mie(false),
    );
    hart.set_pc(handler_pc);
    RiscvExecutionRecord::with_trap(instruction, pc, handler_pc, RiscvTrap::new(kind, pc))
}

const fn machine_trap_cause(kind: RiscvTrapKind, privilege: RiscvPrivilegeMode) -> u64 {
    match kind {
        RiscvTrapKind::EnvironmentCall => match privilege {
            RiscvPrivilegeMode::User => 8,
            RiscvPrivilegeMode::Supervisor => 9,
            RiscvPrivilegeMode::Machine => 11,
        },
        RiscvTrapKind::Breakpoint => 3,
    }
}

fn write_register(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    register: Register,
    value: u64,
) {
    if register.is_zero() {
        return;
    }

    hart.write(register, value);
    writes.push(RegisterWrite::new(register, value));
}

fn write_counter_csr(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    register: Register,
    csr: RiscvCounterCsr,
    value: u64,
) {
    let old_value = hart.counters.read_machine(csr);
    write_register(hart, writes, register, old_value);
    hart.counters.set_machine(csr, value);
}

fn read_status_csr(hart: &RiscvHartState, csr: RiscvStatusCsr) -> u64 {
    csr.read(hart.status())
}

fn write_status_csr(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    register: Register,
    csr: RiscvStatusCsr,
    value: u64,
) {
    let old_value = read_status_csr(hart, csr);
    write_register(hart, writes, register, old_value);
    hart.set_status(csr.write(hart.status(), value));
}

fn read_machine_trap_csr(hart: &RiscvHartState, csr: RiscvMachineTrapCsr) -> u64 {
    match csr {
        RiscvMachineTrapCsr::Medeleg => hart.machine_exception_delegation(),
        RiscvMachineTrapCsr::Mideleg => hart.machine_interrupt_delegation(),
        RiscvMachineTrapCsr::Mtvec => hart.machine_trap_vector(),
        RiscvMachineTrapCsr::Mepc => hart.machine_exception_pc(),
        RiscvMachineTrapCsr::Mcause => hart.machine_trap_cause(),
        RiscvMachineTrapCsr::Mtval => hart.machine_trap_value(),
    }
}

fn write_machine_trap_csr(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    register: Register,
    csr: RiscvMachineTrapCsr,
    value: u64,
) {
    let old_value = read_machine_trap_csr(hart, csr);
    write_register(hart, writes, register, old_value);
    match csr {
        RiscvMachineTrapCsr::Medeleg => hart.set_machine_exception_delegation(value),
        RiscvMachineTrapCsr::Mideleg => hart.set_machine_interrupt_delegation(value),
        RiscvMachineTrapCsr::Mtvec => hart.set_machine_trap_vector(value),
        RiscvMachineTrapCsr::Mepc => hart.set_machine_exception_pc(value),
        RiscvMachineTrapCsr::Mcause => hart.set_machine_trap_cause(value),
        RiscvMachineTrapCsr::Mtval => hart.set_machine_trap_value(value),
    }
}

fn read_supervisor_trap_csr(hart: &RiscvHartState, csr: RiscvSupervisorTrapCsr) -> u64 {
    match csr {
        RiscvSupervisorTrapCsr::Stvec => hart.supervisor_trap_vector(),
        RiscvSupervisorTrapCsr::Sepc => hart.supervisor_exception_pc(),
        RiscvSupervisorTrapCsr::Scause => hart.supervisor_trap_cause(),
        RiscvSupervisorTrapCsr::Stval => hart.supervisor_trap_value(),
    }
}

fn write_supervisor_trap_csr(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    register: Register,
    csr: RiscvSupervisorTrapCsr,
    value: u64,
) {
    let old_value = read_supervisor_trap_csr(hart, csr);
    write_register(hart, writes, register, old_value);
    match csr {
        RiscvSupervisorTrapCsr::Stvec => hart.set_supervisor_trap_vector(value),
        RiscvSupervisorTrapCsr::Sepc => hart.set_supervisor_exception_pc(value),
        RiscvSupervisorTrapCsr::Scause => hart.set_supervisor_trap_cause(value),
        RiscvSupervisorTrapCsr::Stval => hart.set_supervisor_trap_value(value),
    }
}

fn read_translation_csr(hart: &RiscvHartState, csr: RiscvTranslationCsr) -> u64 {
    match csr {
        RiscvTranslationCsr::Satp => hart.translation_satp(),
    }
}

fn write_translation_csr(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    register: Register,
    csr: RiscvTranslationCsr,
    value: u64,
) {
    let old_value = read_translation_csr(hart, csr);
    write_register(hart, writes, register, old_value);
    match csr {
        RiscvTranslationCsr::Satp => hart.set_translation_satp(value),
    }
}

fn add_signed(value: u64, offset: i64) -> Result<u64, RiscvError> {
    if offset >= 0 {
        value
            .checked_add(offset as u64)
            .ok_or(RiscvError::AddressOverflow { value, offset })
    } else {
        value
            .checked_sub(offset.unsigned_abs())
            .ok_or(RiscvError::AddressOverflow { value, offset })
    }
}

fn wrapping_add_signed(value: u64, offset: i64) -> u64 {
    if offset >= 0 {
        value.wrapping_add(offset as u64)
    } else {
        value.wrapping_sub(offset.unsigned_abs())
    }
}

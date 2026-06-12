mod atomic;
mod compressed;
mod control_flow;
mod csr;
mod decode;
mod encoding;
mod error;
mod float;
mod float_execute;
mod gdb_target;
mod hart;
mod instruction;
mod integer;
mod load_store;
mod pma;
mod pmp;
mod pseudo;
mod record;
mod sv39;
mod trap;
mod types;
mod vector;

use encoding::{j_imm, rd, u_imm};
use instruction::csr_privilege_allowed;
use integer::{
    div_signed, div_signed_word, div_unsigned, div_unsigned_word, mulh_signed,
    mulh_signed_unsigned, mulh_unsigned, rem_signed, rem_signed_word, rem_unsigned,
    rem_unsigned_word, sign_extend_word,
};
use trap::{
    enter_pending_interrupt, enter_synchronous_trap, machine_return_allowed,
    supervisor_return_allowed,
};

pub use control_flow::{
    RiscvBranchPredictionTarget, RiscvControlFlowSnapshot, RiscvControlFlowUpdate,
    RiscvVectorConfig, RiscvVectorConfigUpdate,
};
pub use csr::{
    RiscvCounterBank, RiscvCounterCsr, RiscvCounterCsrWord, RiscvCounterSnapshot, RiscvFloatCsr,
    RiscvFloatStatus, RiscvInterruptCsr, RiscvMachineTrapCsr, RiscvStatusCsr, RiscvStatusWord,
    RiscvSupervisorTrapCsr, RiscvTranslationCsr,
};
pub use error::{RiscvCsrError, RiscvError};
pub use gdb_target::{RiscvGdbTargetDescription, RiscvGdbTargetDocument, RiscvGdbXlen};
pub use hart::RiscvHartState;
pub use instruction::RiscvInstruction;
pub use pma::{RiscvPmaAccessKind, RiscvPmaError, RiscvPmaRange, RiscvPmaTable};
pub use pmp::{
    RiscvPmpAccessKind, RiscvPmpAddressMode, RiscvPmpConfig, RiscvPmpEntry, RiscvPmpError,
    RiscvPmpRange, RiscvPmpSnapshot, RiscvPmpSnapshotEntry, RiscvPmpTable, RiscvPrivilegeMode,
};
pub use pseudo::RiscvPseudoOp;
pub use record::{
    FloatRegisterWrite, RegisterWrite, RiscvExecutionRecord, RiscvSystemEvent, RiscvTrap,
    RiscvTrapKind,
};
pub use sv39::{
    walk_sv39_page_table, walk_sv39_page_table_with_context, RiscvSv39AccessContext,
    RiscvSv39AccessKind, RiscvSv39PageFault, RiscvSv39PageTableLevel, RiscvSv39Pte,
    RiscvSv39VirtualAddress, RiscvSv39WalkAdvance, RiscvSv39WalkResult, RiscvSv39WalkState,
};
pub use types::{
    AtomicMemoryOp, FloatRegister, Immediate, MemoryAccessKind, MemoryResponseError,
    MemoryResponseWriteback, MemoryResponseWritebackTarget, MemoryWidth, Register, RiscvFenceSet,
    RiscvMemoryOrdering,
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
        Self::decode_with_length(raw).map(RiscvDecodedInstruction::instruction)
    }

    pub fn decode_with_length(raw: u32) -> Result<RiscvDecodedInstruction, RiscvError> {
        if raw & 0x3 != 0x3 {
            return Ok(RiscvDecodedInstruction::new(
                compressed::decode_compressed(raw)?,
                2,
            ));
        }

        let opcode = raw & 0x7f;
        let instruction = match opcode {
            0x03 => load_store::decode_integer_load(raw),
            0x07 => float::decode_float_load(raw),
            0x0f => decode::decode_fence(raw),
            0x13 => decode::decode_op_imm(raw),
            0x17 => Ok(Self::Auipc {
                rd: rd(raw),
                imm: Immediate::new(u_imm(raw)),
            }),
            0x1b => decode::decode_op_imm_32(raw),
            0x23 => load_store::decode_integer_store(raw),
            0x27 => float::decode_float_store(raw),
            0x2f => atomic::decode_atomic(raw),
            0x33 => decode::decode_op(raw),
            0x3b => decode::decode_op_32(raw),
            0x37 => Ok(Self::Lui {
                rd: rd(raw),
                imm: Immediate::new(u_imm(raw)),
            }),
            0x63 => decode::decode_branch(raw),
            0x67 => decode::decode_jalr(raw),
            0x6f => Ok(Self::Jal {
                rd: rd(raw),
                offset: Immediate::new(j_imm(raw)),
            }),
            0x73 => decode::decode_system(raw),
            0x43 | 0x47 | 0x4b | 0x4f => float::decode_float_multiply_add(raw),
            0x53 => float::decode_float_op(raw),
            0x7b => pseudo::decode_gem5_pseudo_op(raw),
            _ => Err(RiscvError::UnknownEncoding { raw }),
        }?;
        Ok(RiscvDecodedInstruction::new(instruction, 4))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvDecodedInstruction {
    instruction: RiscvInstruction,
    bytes: u8,
}

impl RiscvDecodedInstruction {
    pub(crate) const fn new(instruction: RiscvInstruction, bytes: u8) -> Self {
        Self { instruction, bytes }
    }

    pub const fn instruction(self) -> RiscvInstruction {
        self.instruction
    }

    pub const fn bytes(self) -> u8 {
        self.bytes
    }
}

impl RiscvHartState {
    pub fn execute(
        &mut self,
        instruction: RiscvInstruction,
    ) -> Result<RiscvExecutionRecord, RiscvError> {
        self.execute_with_instruction_bytes(instruction, 4)
    }

    pub fn execute_decoded(
        &mut self,
        decoded: RiscvDecodedInstruction,
    ) -> Result<RiscvExecutionRecord, RiscvError> {
        self.execute_with_instruction_bytes(decoded.instruction(), decoded.bytes())
    }

    fn execute_with_instruction_bytes(
        &mut self,
        instruction: RiscvInstruction,
        instruction_bytes: u8,
    ) -> Result<RiscvExecutionRecord, RiscvError> {
        let pc = self.pc;
        if let Some(record) = enter_pending_interrupt(self, instruction, instruction_bytes, pc) {
            return Ok(record);
        }

        let instruction_bytes_u8 = instruction_bytes;
        let instruction_bytes = u64::from(instruction_bytes_u8);
        let mut next_pc = pc
            .checked_add(instruction_bytes)
            .ok_or(RiscvError::PcOverflow {
                pc,
                offset: instruction_bytes,
            })?;
        let mut register_writes = Vec::new();
        let mut float_register_writes = Vec::new();
        let mut memory_access = None;
        let mut system_event = None;

        if let Some(required_privilege) = instruction.required_csr_privilege() {
            if !csr_privilege_allowed(self.privilege_mode(), required_privilege) {
                return Ok(enter_synchronous_trap(
                    self,
                    instruction,
                    instruction_bytes_u8,
                    pc,
                    RiscvTrapKind::IllegalInstruction,
                ));
            }
        }

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
            RiscvInstruction::FloatLoad {
                rd,
                rs1,
                offset,
                width,
            } => {
                let address = add_signed(self.read(rs1), offset.value())?;
                memory_access = Some(MemoryAccessKind::FloatLoad { rd, address, width });
            }
            RiscvInstruction::FloatStore {
                rs1,
                rs2,
                offset,
                width,
            } => {
                let address = add_signed(self.read(rs1), offset.value())?;
                memory_access = Some(MemoryAccessKind::FloatStore {
                    address,
                    width,
                    value: self.read_float(rs2),
                });
            }
            instruction @ (RiscvInstruction::FloatAddS { .. }
            | RiscvInstruction::FloatAddD { .. }
            | RiscvInstruction::FloatSubS { .. }
            | RiscvInstruction::FloatSubD { .. }
            | RiscvInstruction::FloatMulS { .. }
            | RiscvInstruction::FloatMulD { .. }
            | RiscvInstruction::FloatDivS { .. }
            | RiscvInstruction::FloatDivD { .. }
            | RiscvInstruction::FloatMultiplyAddS { .. }
            | RiscvInstruction::FloatMultiplyAddD { .. }
            | RiscvInstruction::FloatMultiplySubtractS { .. }
            | RiscvInstruction::FloatMultiplySubtractD { .. }
            | RiscvInstruction::FloatNegativeMultiplySubtractS { .. }
            | RiscvInstruction::FloatNegativeMultiplySubtractD { .. }
            | RiscvInstruction::FloatNegativeMultiplyAddS { .. }
            | RiscvInstruction::FloatNegativeMultiplyAddD { .. }
            | RiscvInstruction::FloatSqrtS { .. }
            | RiscvInstruction::FloatSqrtD { .. }
            | RiscvInstruction::FloatSignInjectS { .. }
            | RiscvInstruction::FloatSignInjectD { .. }
            | RiscvInstruction::FloatSignInjectNegS { .. }
            | RiscvInstruction::FloatSignInjectNegD { .. }
            | RiscvInstruction::FloatSignInjectXorS { .. }
            | RiscvInstruction::FloatSignInjectXorD { .. }
            | RiscvInstruction::FloatMinS { .. }
            | RiscvInstruction::FloatMinD { .. }
            | RiscvInstruction::FloatMaxS { .. }
            | RiscvInstruction::FloatMaxD { .. }
            | RiscvInstruction::FloatConvertSFromD { .. }
            | RiscvInstruction::FloatConvertDFromS { .. }
            | RiscvInstruction::FloatMoveSFromX { .. }
            | RiscvInstruction::FloatMoveDFromX { .. }
            | RiscvInstruction::FloatConvertSFromW { .. }
            | RiscvInstruction::FloatConvertSFromWu { .. }
            | RiscvInstruction::FloatConvertSFromL { .. }
            | RiscvInstruction::FloatConvertSFromLu { .. }
            | RiscvInstruction::FloatConvertDFromW { .. }
            | RiscvInstruction::FloatConvertDFromWu { .. }
            | RiscvInstruction::FloatConvertDFromL { .. }
            | RiscvInstruction::FloatConvertDFromLu { .. }) => {
                float_execute::execute_float_register_instruction(
                    self,
                    &mut float_register_writes,
                    instruction,
                );
            }
            instruction @ (RiscvInstruction::FloatLessOrEqualS { .. }
            | RiscvInstruction::FloatLessOrEqualD { .. }
            | RiscvInstruction::FloatLessThanS { .. }
            | RiscvInstruction::FloatLessThanD { .. }
            | RiscvInstruction::FloatEqualS { .. }
            | RiscvInstruction::FloatEqualD { .. }
            | RiscvInstruction::FloatClassS { .. }
            | RiscvInstruction::FloatClassD { .. }
            | RiscvInstruction::FloatMoveXFromS { .. }
            | RiscvInstruction::FloatMoveXFromD { .. }
            | RiscvInstruction::FloatConvertWFromS { .. }
            | RiscvInstruction::FloatConvertWuFromS { .. }
            | RiscvInstruction::FloatConvertLFromS { .. }
            | RiscvInstruction::FloatConvertLuFromS { .. }
            | RiscvInstruction::FloatConvertWFromD { .. }
            | RiscvInstruction::FloatConvertWuFromD { .. }
            | RiscvInstruction::FloatConvertLFromD { .. }
            | RiscvInstruction::FloatConvertLuFromD { .. }) => {
                float_execute::execute_float_integer_instruction(
                    self,
                    &mut register_writes,
                    instruction,
                );
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
                if !supervisor_return_allowed(self.privilege_mode()) {
                    return Ok(enter_synchronous_trap(
                        self,
                        instruction,
                        instruction_bytes_u8,
                        pc,
                        RiscvTrapKind::IllegalInstruction,
                    ));
                }
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
                if !machine_return_allowed(self.privilege_mode()) {
                    return Ok(enter_synchronous_trap(
                        self,
                        instruction,
                        instruction_bytes_u8,
                        pc,
                        RiscvTrapKind::IllegalInstruction,
                    ));
                }
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
            RiscvInstruction::Gem5PseudoOp { op } => {
                system_event = Some(pseudo::gem5_pseudo_system_event(op, pc, self));
                write_register(self, &mut register_writes, Register::from_field(10), 0);
            }
            RiscvInstruction::ReadMachineHartId { rd } => {
                write_register(self, &mut register_writes, rd, self.hart_id);
            }
            RiscvInstruction::ReadCounterCsr { rd, csr } => {
                let value = self.counters.read_machine(csr);
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::ReadMachineCounterCsr { rd, csr } => {
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
            RiscvInstruction::ReadFloatCsr { rd, csr } => {
                write_register(self, &mut register_writes, rd, read_float_csr(self, csr));
            }
            RiscvInstruction::WriteFloatCsr { rd, csr, rs1 } => {
                write_float_csr(self, &mut register_writes, rd, csr, self.read(rs1));
            }
            RiscvInstruction::SetFloatCsr { rd, csr, rs1 } => {
                let value = read_float_csr(self, csr) | self.read(rs1);
                write_float_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ClearFloatCsr { rd, csr, rs1 } => {
                let value = read_float_csr(self, csr) & !self.read(rs1);
                write_float_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::WriteFloatCsrImmediate { rd, csr, zimm } => {
                write_float_csr(self, &mut register_writes, rd, csr, u64::from(zimm));
            }
            RiscvInstruction::SetFloatCsrImmediate { rd, csr, zimm } => {
                let value = read_float_csr(self, csr) | u64::from(zimm);
                write_float_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ClearFloatCsrImmediate { rd, csr, zimm } => {
                let value = read_float_csr(self, csr) & !u64::from(zimm);
                write_float_csr(self, &mut register_writes, rd, csr, value);
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
            RiscvInstruction::ReadInterruptCsr { rd, csr } => {
                write_register(self, &mut register_writes, rd, self.read_interrupt_csr(csr));
            }
            RiscvInstruction::WriteInterruptCsr { rd, csr, rs1 } => {
                write_interrupt_csr(self, &mut register_writes, rd, csr, self.read(rs1));
            }
            RiscvInstruction::SetInterruptCsr { rd, csr, rs1 } => {
                let value = self.read_interrupt_csr(csr) | self.read(rs1);
                write_interrupt_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ClearInterruptCsr { rd, csr, rs1 } => {
                let value = self.read_interrupt_csr(csr) & !self.read(rs1);
                write_interrupt_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::WriteInterruptCsrImmediate { rd, csr, zimm } => {
                write_interrupt_csr(self, &mut register_writes, rd, csr, u64::from(zimm));
            }
            RiscvInstruction::SetInterruptCsrImmediate { rd, csr, zimm } => {
                let value = self.read_interrupt_csr(csr) | u64::from(zimm);
                write_interrupt_csr(self, &mut register_writes, rd, csr, value);
            }
            RiscvInstruction::ClearInterruptCsrImmediate { rd, csr, zimm } => {
                let value = self.read_interrupt_csr(csr) & !u64::from(zimm);
                write_interrupt_csr(self, &mut register_writes, rd, csr, value);
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
                    instruction_bytes_u8,
                    pc,
                    RiscvTrapKind::EnvironmentCall,
                ));
            }
            RiscvInstruction::Ebreak => {
                return Ok(enter_synchronous_trap(
                    self,
                    instruction,
                    instruction_bytes_u8,
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
                debug_assert!(memory_access.is_none());
                debug_assert!(float_register_writes.is_empty());
                Ok(RiscvExecutionRecord::with_system_event_and_register_writes_with_instruction_bytes(
                    instruction,
                    instruction_bytes_u8,
                    pc,
                    next_pc,
                    system_event,
                    register_writes,
                ))
            }
            None => Ok(
                RiscvExecutionRecord::new_with_instruction_bytes_and_float_register_writes(
                    instruction,
                    instruction_bytes_u8,
                    pc,
                    next_pc,
                    register_writes,
                    float_register_writes,
                    memory_access,
                ),
            ),
        }
    }
}

pub(crate) fn write_register(
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
fn read_float_csr(hart: &RiscvHartState, csr: RiscvFloatCsr) -> u64 {
    csr.read(hart.float_status())
}
fn write_float_csr(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    register: Register,
    csr: RiscvFloatCsr,
    value: u64,
) {
    let old_value = read_float_csr(hart, csr);
    write_register(hart, writes, register, old_value);
    hart.set_float_status(csr.write(hart.float_status(), value));
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
fn write_interrupt_csr(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    register: Register,
    csr: RiscvInterruptCsr,
    value: u64,
) {
    let old_value = hart.read_interrupt_csr(csr);
    write_register(hart, writes, register, old_value);
    hart.write_interrupt_csr(csr, value);
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

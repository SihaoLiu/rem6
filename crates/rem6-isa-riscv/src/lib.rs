mod control_flow;
mod encoding;
mod error;
mod gdb_target;
mod pma;
mod pmp;
mod types;
mod vector;

use encoding::{
    aq, b_imm, csr, funct3, funct5, funct7, i_imm, j_imm, rd, rl, rs1, rs2, s_imm, shamt64,
    shift_funct6, u_imm,
};

pub use control_flow::{
    RiscvBranchPredictionTarget, RiscvControlFlowSnapshot, RiscvControlFlowUpdate,
    RiscvVectorConfig, RiscvVectorConfigUpdate,
};
pub use error::{RiscvCsrError, RiscvError};
pub use gdb_target::{RiscvGdbTargetDescription, RiscvGdbTargetDocument, RiscvGdbXlen};
pub use pma::{RiscvPmaAccessKind, RiscvPmaError, RiscvPmaRange, RiscvPmaTable};
pub use pmp::{
    RiscvPmpAccessKind, RiscvPmpAddressMode, RiscvPmpConfig, RiscvPmpEntry, RiscvPmpError,
    RiscvPmpRange, RiscvPmpSnapshot, RiscvPmpSnapshotEntry, RiscvPmpTable, RiscvPrivilegeMode,
};
pub use types::{
    AtomicMemoryOp, Immediate, MemoryAccessKind, MemoryWidth, Register, RiscvFenceSet,
    RiscvMemoryOrdering,
};
pub use vector::{
    RiscvInstructionFlags, RiscvVectorCompressPlan, RiscvVectorCompressResult, RiscvVectorElements,
    RiscvVectorError, RiscvVectorFixedPointState, RiscvVectorFixedRoundingMode, RiscvVectorMicroOp,
    RiscvVectorMicroOpExpansion, RiscvVectorNarrowClipPlan, RiscvVectorNarrowClipResult,
    RiscvVectorTailPolicy,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvTrapKind {
    EnvironmentCall,
    Breakpoint,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvCounterCsr {
    Cycle,
    Instret,
}

impl RiscvCounterCsr {
    pub const fn user_address(self) -> u16 {
        match self {
            Self::Cycle => 0xc00,
            Self::Instret => 0xc02,
        }
    }

    pub const fn machine_address(self) -> u16 {
        match self {
            Self::Cycle => 0xb00,
            Self::Instret => 0xb02,
        }
    }

    pub const fn from_user_address(address: u16) -> Result<Self, RiscvCsrError> {
        match address {
            0xc00 => Ok(Self::Cycle),
            0xc02 => Ok(Self::Instret),
            _ => Err(RiscvCsrError::UnknownCounterCsr { address }),
        }
    }

    pub const fn from_machine_address(address: u16) -> Result<Self, RiscvCsrError> {
        match address {
            0xb00 => Ok(Self::Cycle),
            0xb02 => Ok(Self::Instret),
            _ => Err(RiscvCsrError::UnknownCounterCsr { address }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvCounterCsrWord {
    CycleLow,
    CycleHigh,
    InstretLow,
    InstretHigh,
}

impl RiscvCounterCsrWord {
    pub const fn counter(self) -> RiscvCounterCsr {
        match self {
            Self::CycleLow | Self::CycleHigh => RiscvCounterCsr::Cycle,
            Self::InstretLow | Self::InstretHigh => RiscvCounterCsr::Instret,
        }
    }

    pub const fn user_address(self) -> u16 {
        match self {
            Self::CycleLow => 0xc00,
            Self::InstretLow => 0xc02,
            Self::CycleHigh => 0xc80,
            Self::InstretHigh => 0xc82,
        }
    }

    pub const fn machine_address(self) -> u16 {
        match self {
            Self::CycleLow => 0xb00,
            Self::InstretLow => 0xb02,
            Self::CycleHigh => 0xb80,
            Self::InstretHigh => 0xb82,
        }
    }

    pub const fn from_user_address(address: u16) -> Result<Self, RiscvCsrError> {
        match address {
            0xc00 => Ok(Self::CycleLow),
            0xc02 => Ok(Self::InstretLow),
            0xc80 => Ok(Self::CycleHigh),
            0xc82 => Ok(Self::InstretHigh),
            _ => Err(RiscvCsrError::UnknownCounterCsr { address }),
        }
    }

    pub const fn from_machine_address(address: u16) -> Result<Self, RiscvCsrError> {
        match address {
            0xb00 => Ok(Self::CycleLow),
            0xb02 => Ok(Self::InstretLow),
            0xb80 => Ok(Self::CycleHigh),
            0xb82 => Ok(Self::InstretHigh),
            _ => Err(RiscvCsrError::UnknownCounterCsr { address }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCounterSnapshot {
    cycle: u64,
    instret: u64,
}

impl RiscvCounterSnapshot {
    pub const fn new(cycle: u64, instret: u64) -> Self {
        Self { cycle, instret }
    }

    pub const fn cycle(&self) -> u64 {
        self.cycle
    }

    pub const fn instret(&self) -> u64 {
        self.instret
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCounterBank {
    cycle: u64,
    instret: u64,
}

impl RiscvCounterBank {
    pub const fn new() -> Self {
        Self {
            cycle: 0,
            instret: 0,
        }
    }

    pub const fn read_user(&self, csr: RiscvCounterCsr) -> u64 {
        self.read(csr)
    }

    pub const fn read_machine(&self, csr: RiscvCounterCsr) -> u64 {
        self.read(csr)
    }

    pub const fn read_user_word(&self, csr: RiscvCounterCsrWord) -> u32 {
        self.read_word(csr)
    }

    pub const fn read_machine_word(&self, csr: RiscvCounterCsrWord) -> u32 {
        self.read_word(csr)
    }

    pub fn write_user(&mut self, csr: RiscvCounterCsr, _value: u64) -> Result<(), RiscvCsrError> {
        Err(RiscvCsrError::ReadOnlyCounterAlias { csr })
    }

    pub fn write_user_word(
        &mut self,
        csr: RiscvCounterCsrWord,
        _value: u32,
    ) -> Result<(), RiscvCsrError> {
        Err(RiscvCsrError::ReadOnlyCounterWordAlias { csr })
    }

    pub fn write_machine(&mut self, csr: RiscvCounterCsr, value: u64) -> Result<(), RiscvCsrError> {
        match csr {
            RiscvCounterCsr::Cycle => self.cycle = value,
            RiscvCounterCsr::Instret => self.instret = value,
        }
        Ok(())
    }

    pub fn write_machine_word(
        &mut self,
        csr: RiscvCounterCsrWord,
        value: u32,
    ) -> Result<(), RiscvCsrError> {
        match csr {
            RiscvCounterCsrWord::CycleLow => self.cycle = replace_low_word(self.cycle, value),
            RiscvCounterCsrWord::CycleHigh => self.cycle = replace_high_word(self.cycle, value),
            RiscvCounterCsrWord::InstretLow => self.instret = replace_low_word(self.instret, value),
            RiscvCounterCsrWord::InstretHigh => {
                self.instret = replace_high_word(self.instret, value);
            }
        }
        Ok(())
    }

    pub fn add_cycles(&mut self, cycles: u64) {
        self.cycle = self.cycle.wrapping_add(cycles);
    }

    pub fn retire_instructions(&mut self, instructions: u64) {
        self.instret = self.instret.wrapping_add(instructions);
    }

    pub const fn snapshot(&self) -> RiscvCounterSnapshot {
        RiscvCounterSnapshot::new(self.cycle, self.instret)
    }

    pub fn restore(&mut self, snapshot: &RiscvCounterSnapshot) {
        self.cycle = snapshot.cycle;
        self.instret = snapshot.instret;
    }

    const fn read(&self, csr: RiscvCounterCsr) -> u64 {
        match csr {
            RiscvCounterCsr::Cycle => self.cycle,
            RiscvCounterCsr::Instret => self.instret,
        }
    }

    const fn read_word(&self, csr: RiscvCounterCsrWord) -> u32 {
        let counter = self.read(csr.counter());
        match csr {
            RiscvCounterCsrWord::CycleLow | RiscvCounterCsrWord::InstretLow => counter as u32,
            RiscvCounterCsrWord::CycleHigh | RiscvCounterCsrWord::InstretHigh => {
                (counter >> 32) as u32
            }
        }
    }
}

impl Default for RiscvCounterBank {
    fn default() -> Self {
        Self::new()
    }
}

const fn replace_low_word(counter: u64, value: u32) -> u64 {
    (counter & 0xffff_ffff_0000_0000) | value as u64
}

const fn replace_high_word(counter: u64, value: u32) -> u64 {
    (counter & 0x0000_0000_ffff_ffff) | ((value as u64) << 32)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvTrap {
    kind: RiscvTrapKind,
    pc: u64,
}

impl RiscvTrap {
    pub const fn new(kind: RiscvTrapKind, pc: u64) -> Self {
        Self { kind, pc }
    }

    pub const fn kind(self) -> RiscvTrapKind {
        self.kind
    }

    pub const fn pc(self) -> u64 {
        self.pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvInstruction {
    Lui {
        rd: Register,
        imm: Immediate,
    },
    Auipc {
        rd: Register,
        imm: Immediate,
    },
    Addi {
        rd: Register,
        rs1: Register,
        imm: Immediate,
    },
    Slti {
        rd: Register,
        rs1: Register,
        imm: Immediate,
    },
    Sltiu {
        rd: Register,
        rs1: Register,
        imm: Immediate,
    },
    Xori {
        rd: Register,
        rs1: Register,
        imm: Immediate,
    },
    Ori {
        rd: Register,
        rs1: Register,
        imm: Immediate,
    },
    Andi {
        rd: Register,
        rs1: Register,
        imm: Immediate,
    },
    Slli {
        rd: Register,
        rs1: Register,
        shamt: u8,
    },
    Srli {
        rd: Register,
        rs1: Register,
        shamt: u8,
    },
    Srai {
        rd: Register,
        rs1: Register,
        shamt: u8,
    },
    Add {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Sub {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Sll {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Slt {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Sltu {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Xor {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Srl {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Sra {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Or {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    And {
        rd: Register,
        rs1: Register,
        rs2: Register,
    },
    Beq {
        rs1: Register,
        rs2: Register,
        offset: Immediate,
    },
    Bne {
        rs1: Register,
        rs2: Register,
        offset: Immediate,
    },
    Blt {
        rs1: Register,
        rs2: Register,
        offset: Immediate,
    },
    Bge {
        rs1: Register,
        rs2: Register,
        offset: Immediate,
    },
    Bltu {
        rs1: Register,
        rs2: Register,
        offset: Immediate,
    },
    Bgeu {
        rs1: Register,
        rs2: Register,
        offset: Immediate,
    },
    Jal {
        rd: Register,
        offset: Immediate,
    },
    Jalr {
        rd: Register,
        rs1: Register,
        offset: Immediate,
    },
    Load {
        rd: Register,
        rs1: Register,
        offset: Immediate,
        width: MemoryWidth,
        signed: bool,
    },
    Store {
        rs1: Register,
        rs2: Register,
        offset: Immediate,
        width: MemoryWidth,
    },
    LoadReserved {
        rd: Register,
        rs1: Register,
        width: MemoryWidth,
        acquire: bool,
        release: bool,
    },
    StoreConditional {
        rd: Register,
        rs1: Register,
        rs2: Register,
        width: MemoryWidth,
        acquire: bool,
        release: bool,
    },
    AtomicMemory {
        rd: Register,
        rs1: Register,
        rs2: Register,
        width: MemoryWidth,
        op: AtomicMemoryOp,
        acquire: bool,
        release: bool,
    },
    Fence {
        predecessor: RiscvFenceSet,
        successor: RiscvFenceSet,
        mode: u8,
    },
    FenceI,
    ReadMachineHartId {
        rd: Register,
    },
    ReadCounterCsr {
        rd: Register,
        csr: RiscvCounterCsr,
    },
    Ecall,
    Ebreak,
}

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
            0x23 => decode_store(raw),
            0x2f => decode_atomic(raw),
            0x33 => decode_op(raw),
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
        _ => decode_csr(raw),
    }
}

fn decode_csr(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let csr = csr(raw);
    match (funct3(raw), csr, rs1(raw).index()) {
        (0x2, 0xf14, 0) => Ok(RiscvInstruction::ReadMachineHartId { rd: rd(raw) }),
        (0x2, csr, 0) => counter_csr(csr)
            .map(|csr| RiscvInstruction::ReadCounterCsr { rd: rd(raw), csr })
            .ok_or(RiscvError::UnknownEncoding { raw }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

fn counter_csr(address: u16) -> Option<RiscvCounterCsr> {
    RiscvCounterCsr::from_user_address(address)
        .or_else(|_| RiscvCounterCsr::from_machine_address(address))
        .ok()
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
pub struct RegisterWrite {
    register: Register,
    value: u64,
}

impl RegisterWrite {
    pub const fn new(register: Register, value: u64) -> Self {
        Self { register, value }
    }

    pub const fn register(&self) -> Register {
        self.register
    }

    pub const fn value(&self) -> u64 {
        self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvExecutionRecord {
    instruction: RiscvInstruction,
    pc: u64,
    next_pc: u64,
    register_writes: Vec<RegisterWrite>,
    memory_access: Option<MemoryAccessKind>,
    trap: Option<RiscvTrap>,
}

impl RiscvExecutionRecord {
    pub fn new(
        instruction: RiscvInstruction,
        pc: u64,
        next_pc: u64,
        register_writes: Vec<RegisterWrite>,
        memory_access: Option<MemoryAccessKind>,
    ) -> Self {
        Self {
            instruction,
            pc,
            next_pc,
            register_writes,
            memory_access,
            trap: None,
        }
    }

    pub fn with_trap(
        instruction: RiscvInstruction,
        pc: u64,
        next_pc: u64,
        trap: RiscvTrap,
    ) -> Self {
        Self {
            instruction,
            pc,
            next_pc,
            register_writes: Vec::new(),
            memory_access: None,
            trap: Some(trap),
        }
    }

    pub const fn instruction(&self) -> RiscvInstruction {
        self.instruction
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn next_pc(&self) -> u64 {
        self.next_pc
    }

    pub fn register_writes(&self) -> &[RegisterWrite] {
        &self.register_writes
    }

    pub fn memory_access(&self) -> Option<&MemoryAccessKind> {
        self.memory_access.as_ref()
    }

    pub fn trap(&self) -> Option<&RiscvTrap> {
        self.trap.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvHartState {
    pc: u64,
    hart_id: u64,
    counters: RiscvCounterBank,
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
            vector_config: RiscvVectorConfig::invalid(),
            registers: [0; 32],
        }
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn hart_id(&self) -> u64 {
        self.hart_id
    }

    pub const fn counter_snapshot(&self) -> RiscvCounterSnapshot {
        self.counters.snapshot()
    }

    pub fn set_pc(&mut self, pc: u64) {
        self.pc = pc;
    }

    pub const fn vector_config(&self) -> RiscvVectorConfig {
        self.vector_config
    }

    pub fn set_vector_config(&mut self, vector_config: RiscvVectorConfig) {
        self.vector_config = vector_config;
    }

    pub const fn control_flow_snapshot(&self) -> RiscvControlFlowSnapshot {
        RiscvControlFlowSnapshot::new(self.pc, self.vector_config)
    }

    pub fn apply_control_flow_update(&mut self, update: RiscvControlFlowUpdate) {
        match update {
            RiscvControlFlowUpdate::BranchPrediction(target) => {
                self.pc = target.pc();
            }
            RiscvControlFlowUpdate::VectorConfig(update) => {
                self.pc = update.pc();
                self.vector_config = update.vector_config();
            }
        }
    }

    pub fn read(&self, register: Register) -> u64 {
        if register.is_zero() {
            0
        } else {
            self.registers[register.index() as usize]
        }
    }

    pub fn write(&mut self, register: Register, value: u64) {
        if !register.is_zero() {
            self.registers[register.index() as usize] = value;
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
            RiscvInstruction::ReadMachineHartId { rd } => {
                write_register(self, &mut register_writes, rd, self.hart_id);
            }
            RiscvInstruction::ReadCounterCsr { rd, csr } => {
                let value = self.counters.read_machine(csr);
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Ecall => {
                next_pc = pc;
                self.pc = next_pc;
                return Ok(RiscvExecutionRecord::with_trap(
                    instruction,
                    pc,
                    next_pc,
                    RiscvTrap::new(RiscvTrapKind::EnvironmentCall, pc),
                ));
            }
            RiscvInstruction::Ebreak => {
                next_pc = pc;
                self.pc = next_pc;
                return Ok(RiscvExecutionRecord::with_trap(
                    instruction,
                    pc,
                    next_pc,
                    RiscvTrap::new(RiscvTrapKind::Breakpoint, pc),
                ));
            }
        }

        self.pc = next_pc;
        self.counters.add_cycles(1);
        self.counters.retire_instructions(1);
        Ok(RiscvExecutionRecord::new(
            instruction,
            pc,
            next_pc,
            register_writes,
            memory_access,
        ))
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

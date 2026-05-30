use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Register(u8);

impl Register {
    pub fn new(index: u8) -> Result<Self, RiscvError> {
        if index < 32 {
            Ok(Self(index))
        } else {
            Err(RiscvError::InvalidRegister { index })
        }
    }

    const fn from_field(index: u32) -> Self {
        Self(index as u8)
    }

    pub const fn index(self) -> u8 {
        self.0
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Immediate(i64);

impl Immediate {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> i64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryWidth {
    Byte,
    Halfword,
    Word,
    Doubleword,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicMemoryOp {
    Swap,
    Add,
    Xor,
    Or,
    And,
    MinSigned,
    MaxSigned,
    MinUnsigned,
    MaxUnsigned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvFenceSet {
    input: bool,
    output: bool,
    read: bool,
    write: bool,
}

impl RiscvFenceSet {
    pub const fn new(input: bool, output: bool, read: bool, write: bool) -> Self {
        Self {
            input,
            output,
            read,
            write,
        }
    }

    pub const fn memory() -> Self {
        Self {
            input: false,
            output: false,
            read: true,
            write: true,
        }
    }

    const fn from_bits(bits: u32) -> Self {
        Self {
            input: bits & 0b1000 != 0,
            output: bits & 0b0100 != 0,
            read: bits & 0b0010 != 0,
            write: bits & 0b0001 != 0,
        }
    }

    pub const fn input(self) -> bool {
        self.input
    }

    pub const fn output(self) -> bool {
        self.output
    }

    pub const fn read(self) -> bool {
        self.read
    }

    pub const fn write(self) -> bool {
        self.write
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvMemoryOrdering {
    before: Option<RiscvFenceSet>,
    after: Option<RiscvFenceSet>,
}

impl RiscvMemoryOrdering {
    pub const fn new(before: Option<RiscvFenceSet>, after: Option<RiscvFenceSet>) -> Self {
        Self { before, after }
    }

    pub const fn none() -> Self {
        Self {
            before: None,
            after: None,
        }
    }

    pub const fn before(self) -> Option<RiscvFenceSet> {
        self.before
    }

    pub const fn after(self) -> Option<RiscvFenceSet> {
        self.after
    }

    pub const fn is_ordered(self) -> bool {
        self.before.is_some() || self.after.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryAccessKind {
    Load {
        rd: Register,
        address: u64,
        width: MemoryWidth,
        signed: bool,
    },
    LoadReserved {
        rd: Register,
        address: u64,
        width: MemoryWidth,
        acquire: bool,
        release: bool,
    },
    StoreConditional {
        rd: Register,
        address: u64,
        width: MemoryWidth,
        value: u64,
        acquire: bool,
        release: bool,
    },
    AtomicMemory {
        rd: Register,
        address: u64,
        width: MemoryWidth,
        op: AtomicMemoryOp,
        value: u64,
        acquire: bool,
        release: bool,
    },
    Store {
        address: u64,
        width: MemoryWidth,
        value: u64,
    },
}

impl MemoryAccessKind {
    pub fn memory_ordering(&self) -> RiscvMemoryOrdering {
        match self {
            Self::LoadReserved {
                acquire, release, ..
            }
            | Self::StoreConditional {
                acquire, release, ..
            }
            | Self::AtomicMemory {
                acquire, release, ..
            } => aq_rl_ordering(*acquire, *release),
            Self::Load { .. } | Self::Store { .. } => RiscvMemoryOrdering::none(),
        }
    }
}

fn aq_rl_ordering(acquire: bool, release: bool) -> RiscvMemoryOrdering {
    RiscvMemoryOrdering::new(
        release.then_some(RiscvFenceSet::memory()),
        acquire.then_some(RiscvFenceSet::memory()),
    )
}

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
        _ => Err(RiscvError::UnknownEncoding { raw }),
    }
}

fn decode_op_imm(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    match funct3(raw) {
        0x0 => Ok(RiscvInstruction::Addi {
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
    registers: [u64; 32],
}

impl RiscvHartState {
    pub const fn new(pc: u64) -> Self {
        Self {
            pc,
            registers: [0; 32],
        }
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub fn set_pc(&mut self, pc: u64) {
        self.pc = pc;
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
            RiscvInstruction::Add { rd, rs1, rs2 } => {
                let value = self.read(rs1).wrapping_add(self.read(rs2));
                write_register(self, &mut register_writes, rd, value);
            }
            RiscvInstruction::Sub { rd, rs1, rs2 } => {
                let value = self.read(rs1).wrapping_sub(self.read(rs2));
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

fn rd(raw: u32) -> Register {
    Register::from_field((raw >> 7) & 0x1f)
}

fn rs1(raw: u32) -> Register {
    Register::from_field((raw >> 15) & 0x1f)
}

fn rs2(raw: u32) -> Register {
    Register::from_field((raw >> 20) & 0x1f)
}

fn funct3(raw: u32) -> u32 {
    (raw >> 12) & 0x7
}

fn funct7(raw: u32) -> u32 {
    (raw >> 25) & 0x7f
}

fn funct5(raw: u32) -> u32 {
    (raw >> 27) & 0x1f
}

fn aq(raw: u32) -> bool {
    ((raw >> 26) & 0x1) != 0
}

fn rl(raw: u32) -> bool {
    ((raw >> 25) & 0x1) != 0
}

fn i_imm(raw: u32) -> i64 {
    sign_extend((raw >> 20) as u64, 12)
}

fn s_imm(raw: u32) -> i64 {
    let imm = ((raw >> 25) << 5) | ((raw >> 7) & 0x1f);
    sign_extend(imm as u64, 12)
}

fn b_imm(raw: u32) -> i64 {
    let imm = (((raw >> 31) & 0x1) << 12)
        | (((raw >> 7) & 0x1) << 11)
        | (((raw >> 25) & 0x3f) << 5)
        | (((raw >> 8) & 0xf) << 1);
    sign_extend(imm as u64, 13)
}

fn u_imm(raw: u32) -> i64 {
    (raw & 0xffff_f000) as i32 as i64
}

fn j_imm(raw: u32) -> i64 {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvError {
    InvalidRegister { index: u8 },
    CompressedNotSupported { raw: u32 },
    UnknownEncoding { raw: u32 },
    PcOverflow { pc: u64, offset: u64 },
    AddressOverflow { value: u64, offset: i64 },
}

impl fmt::Display for RiscvError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRegister { index } => {
                write!(formatter, "register index {index} is outside x0..x31")
            }
            Self::CompressedNotSupported { raw } => {
                write!(
                    formatter,
                    "compressed instruction {raw:#010x} is not supported"
                )
            }
            Self::UnknownEncoding { raw } => {
                write!(formatter, "instruction {raw:#010x} is not supported")
            }
            Self::PcOverflow { pc, offset } => {
                write!(formatter, "pc {pc:#x} overflows by {offset} bytes")
            }
            Self::AddressOverflow { value, offset } => {
                write!(
                    formatter,
                    "address {value:#x} overflows with offset {offset}"
                )
            }
        }
    }
}

impl Error for RiscvError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvCsrError {
    UnknownCounterCsr { address: u16 },
    ReadOnlyCounterAlias { csr: RiscvCounterCsr },
    ReadOnlyCounterWordAlias { csr: RiscvCounterCsrWord },
}

impl fmt::Display for RiscvCsrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCounterCsr { address } => {
                write!(
                    formatter,
                    "RISC-V counter CSR {address:#05x} is not supported"
                )
            }
            Self::ReadOnlyCounterAlias { csr } => write!(
                formatter,
                "RISC-V user counter CSR {:#05x} is read-only",
                csr.user_address()
            ),
            Self::ReadOnlyCounterWordAlias { csr } => write!(
                formatter,
                "RISC-V user counter CSR {:#05x} is read-only",
                csr.user_address()
            ),
        }
    }
}

impl Error for RiscvCsrError {}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvTrapKind {
    EnvironmentCall,
    Breakpoint,
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
    match (funct5(raw), funct3(raw), rs2(raw).index()) {
        (0x02, 0x3, 0) => Ok(RiscvInstruction::LoadReserved {
            rd: rd(raw),
            rs1: rs1(raw),
            width: MemoryWidth::Doubleword,
            acquire: aq(raw),
            release: rl(raw),
        }),
        (0x03, 0x3, _) => Ok(RiscvInstruction::StoreConditional {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
            width: MemoryWidth::Doubleword,
            acquire: aq(raw),
            release: rl(raw),
        }),
        (0x01, 0x3, _) => Ok(RiscvInstruction::AtomicMemory {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
            width: MemoryWidth::Doubleword,
            op: AtomicMemoryOp::Swap,
            acquire: aq(raw),
            release: rl(raw),
        }),
        (0x00, 0x3, _) => Ok(RiscvInstruction::AtomicMemory {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
            width: MemoryWidth::Doubleword,
            op: AtomicMemoryOp::Add,
            acquire: aq(raw),
            release: rl(raw),
        }),
        (0x04, 0x3, _) => Ok(RiscvInstruction::AtomicMemory {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
            width: MemoryWidth::Doubleword,
            op: AtomicMemoryOp::Xor,
            acquire: aq(raw),
            release: rl(raw),
        }),
        (0x08, 0x3, _) => Ok(RiscvInstruction::AtomicMemory {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
            width: MemoryWidth::Doubleword,
            op: AtomicMemoryOp::Or,
            acquire: aq(raw),
            release: rl(raw),
        }),
        (0x0c, 0x3, _) => Ok(RiscvInstruction::AtomicMemory {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
            width: MemoryWidth::Doubleword,
            op: AtomicMemoryOp::And,
            acquire: aq(raw),
            release: rl(raw),
        }),
        (0x10, 0x3, _) => Ok(RiscvInstruction::AtomicMemory {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
            width: MemoryWidth::Doubleword,
            op: AtomicMemoryOp::MinSigned,
            acquire: aq(raw),
            release: rl(raw),
        }),
        (0x14, 0x3, _) => Ok(RiscvInstruction::AtomicMemory {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
            width: MemoryWidth::Doubleword,
            op: AtomicMemoryOp::MaxSigned,
            acquire: aq(raw),
            release: rl(raw),
        }),
        (0x18, 0x3, _) => Ok(RiscvInstruction::AtomicMemory {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
            width: MemoryWidth::Doubleword,
            op: AtomicMemoryOp::MinUnsigned,
            acquire: aq(raw),
            release: rl(raw),
        }),
        (0x1c, 0x3, _) => Ok(RiscvInstruction::AtomicMemory {
            rd: rd(raw),
            rs1: rs1(raw),
            rs2: rs2(raw),
            width: MemoryWidth::Doubleword,
            op: AtomicMemoryOp::MaxUnsigned,
            acquire: aq(raw),
            release: rl(raw),
        }),
        _ => Err(RiscvError::UnknownEncoding { raw }),
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

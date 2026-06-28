use crate::error::RiscvError;

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

    pub(crate) const fn from_field(index: u32) -> Self {
        Self(index as u8)
    }

    pub const fn index(self) -> u8 {
        self.0
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FloatRegister(u8);

impl FloatRegister {
    pub fn new(index: u8) -> Result<Self, RiscvError> {
        if index < 32 {
            Ok(Self(index))
        } else {
            Err(RiscvError::InvalidRegister { index })
        }
    }

    pub(crate) const fn from_field(index: u32) -> Self {
        Self(index as u8)
    }

    pub const fn index(self) -> u8 {
        self.0
    }
}

pub const RISCV_VECTOR_REGISTER_BYTES: usize = 16;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VectorRegister(u8);

impl VectorRegister {
    pub fn new(index: u8) -> Result<Self, RiscvError> {
        if index < 32 {
            Ok(Self(index))
        } else {
            Err(RiscvError::InvalidRegister { index })
        }
    }

    pub(crate) const fn from_field(index: u32) -> Self {
        Self(index as u8)
    }

    pub const fn index(self) -> u8 {
        self.0
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

impl MemoryWidth {
    pub const fn bytes(self) -> usize {
        match self {
            Self::Byte => 1,
            Self::Halfword => 2,
            Self::Word => 4,
            Self::Doubleword => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryResponseWritebackTarget {
    Integer(Register),
    Float(FloatRegister),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryResponseWriteback {
    target: MemoryResponseWritebackTarget,
    value: u64,
}

impl MemoryResponseWriteback {
    pub const fn new(register: Register, value: u64) -> Self {
        Self {
            target: MemoryResponseWritebackTarget::Integer(register),
            value,
        }
    }

    pub const fn new_float(register: FloatRegister, value: u64) -> Self {
        Self {
            target: MemoryResponseWritebackTarget::Float(register),
            value,
        }
    }

    pub const fn target(self) -> MemoryResponseWritebackTarget {
        self.target
    }

    pub const fn integer_register(self) -> Option<Register> {
        match self.target {
            MemoryResponseWritebackTarget::Integer(register) => Some(register),
            MemoryResponseWritebackTarget::Float(_) => None,
        }
    }

    pub fn expect_integer_register(self) -> Register {
        match self.target {
            MemoryResponseWritebackTarget::Integer(register) => register,
            MemoryResponseWritebackTarget::Float(_) => {
                panic!("floating-point writeback has no integer register")
            }
        }
    }

    pub const fn value(self) -> u64 {
        self.value
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryResponseError {
    ShortData { expected: usize, actual: usize },
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

    pub(crate) const fn from_bits(bits: u32) -> Self {
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
    FloatLoad {
        rd: FloatRegister,
        address: u64,
        width: MemoryWidth,
    },
    VectorLoadUnitStride {
        vd: VectorRegister,
        address: u64,
        width: MemoryWidth,
        byte_len: usize,
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
    FloatStore {
        address: u64,
        width: MemoryWidth,
        value: u64,
    },
    VectorStoreUnitStride {
        address: u64,
        width: MemoryWidth,
        data: Vec<u8>,
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
            Self::Load { .. }
            | Self::FloatLoad { .. }
            | Self::VectorLoadUnitStride { .. }
            | Self::Store { .. }
            | Self::FloatStore { .. }
            | Self::VectorStoreUnitStride { .. } => RiscvMemoryOrdering::none(),
        }
    }

    pub fn read_response_writeback(
        &self,
        data: &[u8],
    ) -> Result<Option<MemoryResponseWriteback>, MemoryResponseError> {
        let Some((register, width, signed)) = self.read_response_target() else {
            return Ok(None);
        };
        let value = read_response_value(data, width, signed)?;
        Ok(Some(match register {
            MemoryResponseWritebackTarget::Integer(register) => {
                MemoryResponseWriteback::new(register, value)
            }
            MemoryResponseWritebackTarget::Float(register) => {
                MemoryResponseWriteback::new_float(register, float_response_value(width, value))
            }
        }))
    }

    fn read_response_target(&self) -> Option<(MemoryResponseWritebackTarget, MemoryWidth, bool)> {
        match self {
            Self::Load {
                rd, width, signed, ..
            } => Some((MemoryResponseWritebackTarget::Integer(*rd), *width, *signed)),
            Self::FloatLoad { rd, width, .. } => {
                Some((MemoryResponseWritebackTarget::Float(*rd), *width, false))
            }
            Self::LoadReserved { rd, width, .. } | Self::AtomicMemory { rd, width, .. } => Some((
                MemoryResponseWritebackTarget::Integer(*rd),
                *width,
                *width == MemoryWidth::Word,
            )),
            Self::StoreConditional { .. }
            | Self::Store { .. }
            | Self::FloatStore { .. }
            | Self::VectorLoadUnitStride { .. }
            | Self::VectorStoreUnitStride { .. } => None,
        }
    }
}

const fn float_response_value(width: MemoryWidth, value: u64) -> u64 {
    match width {
        MemoryWidth::Word => 0xffff_ffff_0000_0000 | (value & 0xffff_ffff),
        _ => value,
    }
}

fn aq_rl_ordering(acquire: bool, release: bool) -> RiscvMemoryOrdering {
    RiscvMemoryOrdering::new(
        release.then_some(RiscvFenceSet::memory()),
        acquire.then_some(RiscvFenceSet::memory()),
    )
}

fn read_response_value(
    data: &[u8],
    width: MemoryWidth,
    signed: bool,
) -> Result<u64, MemoryResponseError> {
    let expected = width.bytes();
    let bytes = data.get(..expected).ok_or(MemoryResponseError::ShortData {
        expected,
        actual: data.len(),
    })?;
    let raw = bytes.iter().enumerate().fold(0u64, |value, (shift, byte)| {
        value | (u64::from(*byte) << (shift * 8))
    });
    if !signed || width == MemoryWidth::Doubleword {
        return Ok(raw);
    }

    let bits = expected as u32 * 8;
    let sign_bit = 1u64 << (bits - 1);
    if raw & sign_bit == 0 {
        Ok(raw)
    } else {
        Ok(raw | (!0u64 << bits))
    }
}

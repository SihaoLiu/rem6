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

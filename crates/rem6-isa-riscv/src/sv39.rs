use std::error::Error;
use std::fmt;

const PTE_VALID_BIT: u64 = 1 << 0;
const PTE_READ_BIT: u64 = 1 << 1;
const PTE_WRITE_BIT: u64 = 1 << 2;
const PTE_EXECUTE_BIT: u64 = 1 << 3;
const PTE_USER_BIT: u64 = 1 << 4;
const PTE_GLOBAL_BIT: u64 = 1 << 5;
const PTE_ACCESSED_BIT: u64 = 1 << 6;
const PTE_DIRTY_BIT: u64 = 1 << 7;
const PTE_PPN_SHIFT: u64 = 10;
const PTE_PPN_MASK: u64 = (1_u64 << 44) - 1;
const PTE_RESERVED_BITS_MASK: u64 = u64::MAX << 54;
const PAGE_SHIFT: u64 = 12;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RiscvSv39AccessKind {
    InstructionFetch,
    Load,
    Store,
    Atomic,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RiscvSv39Pte {
    raw: u64,
}

impl RiscvSv39Pte {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    pub const fn raw(self) -> u64 {
        self.raw
    }

    pub const fn valid(self) -> bool {
        self.raw & PTE_VALID_BIT != 0
    }

    pub const fn readable(self) -> bool {
        self.raw & PTE_READ_BIT != 0
    }

    pub const fn writable(self) -> bool {
        self.raw & PTE_WRITE_BIT != 0
    }

    pub const fn executable(self) -> bool {
        self.raw & PTE_EXECUTE_BIT != 0
    }

    pub const fn user(self) -> bool {
        self.raw & PTE_USER_BIT != 0
    }

    pub const fn global(self) -> bool {
        self.raw & PTE_GLOBAL_BIT != 0
    }

    pub const fn accessed(self) -> bool {
        self.raw & PTE_ACCESSED_BIT != 0
    }

    pub const fn dirty(self) -> bool {
        self.raw & PTE_DIRTY_BIT != 0
    }

    pub const fn is_leaf(self) -> bool {
        self.readable() || self.executable()
    }

    pub const fn physical_page_number(self) -> u64 {
        (self.raw >> PTE_PPN_SHIFT) & PTE_PPN_MASK
    }

    pub const fn physical_address_base(self) -> u64 {
        self.physical_page_number() << PAGE_SHIFT
    }

    pub const fn validate(self) -> Result<(), RiscvSv39PageFault> {
        if !self.valid() {
            return Err(RiscvSv39PageFault::InvalidEntry);
        }

        let reserved_bits = self.raw & PTE_RESERVED_BITS_MASK;
        if reserved_bits != 0 {
            return Err(RiscvSv39PageFault::ReservedBitsSet {
                bits: reserved_bits,
            });
        }

        if self.writable() && !self.readable() {
            return Err(RiscvSv39PageFault::ReservedPermissionEncoding);
        }

        Ok(())
    }

    pub const fn validate_leaf_access(
        self,
        access: RiscvSv39AccessKind,
    ) -> Result<(), RiscvSv39PageFault> {
        match self.validate() {
            Ok(()) => {}
            Err(error) => return Err(error),
        }

        if !self.is_leaf() {
            return Err(RiscvSv39PageFault::NonLeaf);
        }

        if !self.permits(access) {
            return Err(RiscvSv39PageFault::PermissionDenied { access });
        }

        if !self.accessed() {
            return Err(RiscvSv39PageFault::AccessedBitClear);
        }

        if access.requires_dirty() && !self.dirty() {
            return Err(RiscvSv39PageFault::DirtyBitClear);
        }

        Ok(())
    }

    const fn permits(self, access: RiscvSv39AccessKind) -> bool {
        match access {
            RiscvSv39AccessKind::InstructionFetch => self.executable(),
            RiscvSv39AccessKind::Load => self.readable(),
            RiscvSv39AccessKind::Store | RiscvSv39AccessKind::Atomic => self.writable(),
        }
    }
}

impl RiscvSv39AccessKind {
    const fn requires_dirty(self) -> bool {
        matches!(self, Self::Store | Self::Atomic)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvSv39PageFault {
    InvalidEntry,
    ReservedBitsSet { bits: u64 },
    ReservedPermissionEncoding,
    NonLeaf,
    PermissionDenied { access: RiscvSv39AccessKind },
    AccessedBitClear,
    DirtyBitClear,
}

impl fmt::Display for RiscvSv39PageFault {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEntry => write!(formatter, "RISC-V Sv39 PTE is not valid"),
            Self::ReservedBitsSet { bits } => {
                write!(formatter, "RISC-V Sv39 PTE has reserved bits {bits:#x}")
            }
            Self::ReservedPermissionEncoding => write!(
                formatter,
                "RISC-V Sv39 PTE has writable without readable permission"
            ),
            Self::NonLeaf => write!(formatter, "RISC-V Sv39 PTE is not a leaf entry"),
            Self::PermissionDenied { access } => {
                write!(formatter, "RISC-V Sv39 PTE denied {access:?} access")
            }
            Self::AccessedBitClear => write!(formatter, "RISC-V Sv39 PTE accessed bit is clear"),
            Self::DirtyBitClear => write!(formatter, "RISC-V Sv39 PTE dirty bit is clear"),
        }
    }
}

impl Error for RiscvSv39PageFault {}

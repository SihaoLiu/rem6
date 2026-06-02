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
const PTE_NONLEAF_RESERVED_ATTRIBUTES_MASK: u64 = PTE_USER_BIT | PTE_ACCESSED_BIT | PTE_DIRTY_BIT;
const PTE_SIZE: u64 = 8;
const PAGE_SHIFT: u64 = 12;
const PAGE_OFFSET_MASK: u64 = (1_u64 << PAGE_SHIFT) - 1;
const SV39_VIRTUAL_ADDRESS_BITS: u64 = 39;
const SV39_VIRTUAL_PAGE_NUMBER_BITS: u64 = 27;
const SV39_SIGN_BIT: u64 = 1 << (SV39_VIRTUAL_ADDRESS_BITS - 1);
const SV39_HIGH_BITS_MASK: u64 = u64::MAX << SV39_VIRTUAL_ADDRESS_BITS;
const SV39_VPN_MASK: u64 = (1_u64 << SV39_VIRTUAL_PAGE_NUMBER_BITS) - 1;
const SV39_LEVEL_BITS: u64 = 9;
const SV39_LEVEL_MASK: u64 = (1_u64 << SV39_LEVEL_BITS) - 1;
const SV39_PPN0_SHIFT: u64 = 0;
const SV39_PPN1_SHIFT: u64 = SV39_LEVEL_BITS;
const SV39_PPN2_SHIFT: u64 = SV39_LEVEL_BITS * 2;
const SV39_PPN2_MASK: u64 = (1_u64 << 26) - 1;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RiscvSv39AccessKind {
    InstructionFetch,
    Load,
    Store,
    Atomic,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RiscvSv39PageTableLevel {
    Level0,
    Level1,
    Level2,
}

impl RiscvSv39PageTableLevel {
    const fn number(self) -> u64 {
        match self {
            Self::Level0 => 0,
            Self::Level1 => 1,
            Self::Level2 => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RiscvSv39VirtualAddress {
    raw: u64,
}

impl RiscvSv39VirtualAddress {
    pub const fn new(raw: u64) -> Result<Self, RiscvSv39PageFault> {
        if !is_canonical_sv39_address(raw) {
            return Err(RiscvSv39PageFault::NonCanonicalVirtualAddress { address: raw });
        }
        Ok(Self { raw })
    }

    pub const fn raw(self) -> u64 {
        self.raw
    }

    pub const fn page_offset(self) -> u64 {
        self.raw & PAGE_OFFSET_MASK
    }

    pub const fn virtual_page_number(self) -> u32 {
        ((self.raw >> PAGE_SHIFT) & SV39_VPN_MASK) as u32
    }

    pub const fn vpn(self, level: RiscvSv39PageTableLevel) -> u16 {
        let shift = PAGE_SHIFT + (level.number() * SV39_LEVEL_BITS);
        ((self.raw >> shift) & SV39_LEVEL_MASK) as u16
    }

    pub const fn page_table_entry_address(
        self,
        table_ppn: u64,
        level: RiscvSv39PageTableLevel,
    ) -> Result<u64, RiscvSv39PageFault> {
        if table_ppn > PTE_PPN_MASK {
            return Err(RiscvSv39PageFault::PageTablePointerOutOfRange { ppn: table_ppn });
        }
        Ok((table_ppn << PAGE_SHIFT) + ((self.vpn(level) as u64) * PTE_SIZE))
    }
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

    pub const fn ppn(self, level: RiscvSv39PageTableLevel) -> u64 {
        match level {
            RiscvSv39PageTableLevel::Level0 => {
                (self.physical_page_number() >> SV39_PPN0_SHIFT) & SV39_LEVEL_MASK
            }
            RiscvSv39PageTableLevel::Level1 => {
                (self.physical_page_number() >> SV39_PPN1_SHIFT) & SV39_LEVEL_MASK
            }
            RiscvSv39PageTableLevel::Level2 => {
                (self.physical_page_number() >> SV39_PPN2_SHIFT) & SV39_PPN2_MASK
            }
        }
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

        let nonleaf_reserved_attributes = self.raw & PTE_NONLEAF_RESERVED_ATTRIBUTES_MASK;
        if !self.is_leaf() && nonleaf_reserved_attributes != 0 {
            return Err(RiscvSv39PageFault::ReservedNonLeafAttributes {
                bits: nonleaf_reserved_attributes,
            });
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

    pub const fn leaf_physical_address(
        self,
        virtual_address: RiscvSv39VirtualAddress,
        level: RiscvSv39PageTableLevel,
        access: RiscvSv39AccessKind,
    ) -> Result<u64, RiscvSv39PageFault> {
        match self.validate() {
            Ok(()) => {}
            Err(error) => return Err(error),
        }

        if !self.is_leaf() {
            return Err(RiscvSv39PageFault::NonLeaf);
        }

        if self.is_misaligned_superpage(level) {
            return Err(RiscvSv39PageFault::MisalignedSuperpage {
                level,
                ppn: self.physical_page_number(),
            });
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

        Ok(match level {
            RiscvSv39PageTableLevel::Level0 => {
                self.physical_address_base() | virtual_address.page_offset()
            }
            RiscvSv39PageTableLevel::Level1 => {
                (self.ppn(RiscvSv39PageTableLevel::Level2) << 30)
                    | (self.ppn(RiscvSv39PageTableLevel::Level1) << 21)
                    | ((virtual_address.vpn(RiscvSv39PageTableLevel::Level0) as u64) << 12)
                    | virtual_address.page_offset()
            }
            RiscvSv39PageTableLevel::Level2 => {
                (self.ppn(RiscvSv39PageTableLevel::Level2) << 30)
                    | ((virtual_address.vpn(RiscvSv39PageTableLevel::Level1) as u64) << 21)
                    | ((virtual_address.vpn(RiscvSv39PageTableLevel::Level0) as u64) << 12)
                    | virtual_address.page_offset()
            }
        })
    }

    const fn permits(self, access: RiscvSv39AccessKind) -> bool {
        match access {
            RiscvSv39AccessKind::InstructionFetch => self.executable(),
            RiscvSv39AccessKind::Load => self.readable(),
            RiscvSv39AccessKind::Store | RiscvSv39AccessKind::Atomic => self.writable(),
        }
    }

    const fn is_misaligned_superpage(self, level: RiscvSv39PageTableLevel) -> bool {
        match level {
            RiscvSv39PageTableLevel::Level0 => false,
            RiscvSv39PageTableLevel::Level1 => self.ppn(RiscvSv39PageTableLevel::Level0) != 0,
            RiscvSv39PageTableLevel::Level2 => {
                self.ppn(RiscvSv39PageTableLevel::Level0) != 0
                    || self.ppn(RiscvSv39PageTableLevel::Level1) != 0
            }
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
    NonCanonicalVirtualAddress {
        address: u64,
    },
    PageTablePointerOutOfRange {
        ppn: u64,
    },
    InvalidEntry,
    ReservedBitsSet {
        bits: u64,
    },
    ReservedNonLeafAttributes {
        bits: u64,
    },
    ReservedPermissionEncoding,
    NonLeaf,
    PermissionDenied {
        access: RiscvSv39AccessKind,
    },
    AccessedBitClear,
    DirtyBitClear,
    MisalignedSuperpage {
        level: RiscvSv39PageTableLevel,
        ppn: u64,
    },
}

impl fmt::Display for RiscvSv39PageFault {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonCanonicalVirtualAddress { address } => write!(
                formatter,
                "RISC-V Sv39 virtual address {address:#x} is not canonical"
            ),
            Self::PageTablePointerOutOfRange { ppn } => write!(
                formatter,
                "RISC-V Sv39 page-table pointer PPN {ppn:#x} is out of range"
            ),
            Self::InvalidEntry => write!(formatter, "RISC-V Sv39 PTE is not valid"),
            Self::ReservedBitsSet { bits } => {
                write!(formatter, "RISC-V Sv39 PTE has reserved bits {bits:#x}")
            }
            Self::ReservedNonLeafAttributes { bits } => write!(
                formatter,
                "RISC-V Sv39 non-leaf PTE has reserved attributes {bits:#x}"
            ),
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
            Self::MisalignedSuperpage { level, ppn } => write!(
                formatter,
                "RISC-V Sv39 {level:?} superpage PPN {ppn:#x} is not aligned"
            ),
        }
    }
}

impl Error for RiscvSv39PageFault {}

const fn is_canonical_sv39_address(address: u64) -> bool {
    let high_bits = address & SV39_HIGH_BITS_MASK;
    if address & SV39_SIGN_BIT == 0 {
        high_bits == 0
    } else {
        high_bits == SV39_HIGH_BITS_MASK
    }
}

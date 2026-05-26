use rem6_memory::{AccessSize, Address, AddressRange};

use crate::{PciError, PciFunctionAddress, PCI_BAR0_OFFSET, PCI_BAR_COUNT};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PciBarIndex(u8);

impl PciBarIndex {
    pub const fn new(value: u8) -> Result<Self, PciError> {
        if value as usize >= PCI_BAR_COUNT {
            return Err(PciError::InvalidBarIndex { index: value });
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }

    pub(crate) const fn config_offset(self) -> usize {
        PCI_BAR0_OFFSET + self.as_usize() * 4
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PciBarKind {
    Memory32 { prefetchable: bool },
    Memory64 { prefetchable: bool },
    LegacyIo { address: Address },
    Io,
}

impl PciBarKind {
    pub(crate) const fn flags(self) -> u32 {
        match self {
            Self::Memory32 { prefetchable } => {
                if prefetchable {
                    0x8
                } else {
                    0
                }
            }
            Self::Memory64 { prefetchable } => 0x4 | if prefetchable { 0x8 } else { 0 },
            Self::LegacyIo { .. } => 0,
            Self::Io => 0x1,
        }
    }

    const fn fixed_low_bits(self) -> u64 {
        match self {
            Self::Memory32 { .. } | Self::Memory64 { .. } => 0xf,
            Self::LegacyIo { .. } => 0x3,
            Self::Io => 0x3,
        }
    }

    pub(crate) const fn is_64_bit(self) -> bool {
        matches!(self, Self::Memory64 { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciBarSpec {
    index: PciBarIndex,
    kind: PciBarKind,
    size: AccessSize,
}

impl PciBarSpec {
    pub fn new(index: PciBarIndex, kind: PciBarKind, size: AccessSize) -> Result<Self, PciError> {
        if kind.is_64_bit() && index.as_usize() + 1 >= PCI_BAR_COUNT {
            return Err(PciError::InvalidBarPair { index });
        }
        let min_size = kind.fixed_low_bits() + 1;
        if size.bytes() < min_size || !size.bytes().is_power_of_two() {
            return Err(PciError::InvalidBarSize { index, kind, size });
        }
        if !kind.is_64_bit() && size.bytes() > u32::MAX as u64 {
            return Err(PciError::InvalidBarSize { index, kind, size });
        }
        Ok(Self { index, kind, size })
    }

    pub const fn index(self) -> PciBarIndex {
        self.index
    }

    pub const fn kind(self) -> PciBarKind {
        self.kind
    }

    pub const fn size(self) -> AccessSize {
        self.size
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciBarRange {
    index: PciBarIndex,
    kind: PciBarKind,
    range: AddressRange,
}

impl PciBarRange {
    pub fn new(
        index: PciBarIndex,
        kind: PciBarKind,
        base: Address,
        size: AccessSize,
    ) -> Result<Self, PciError> {
        Ok(Self {
            index,
            kind,
            range: AddressRange::new(base, size).map_err(PciError::Memory)?,
        })
    }

    pub const fn index(&self) -> PciBarIndex {
        self.index
    }

    pub const fn kind(&self) -> PciBarKind {
        self.kind
    }

    pub const fn range(&self) -> AddressRange {
        self.range
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PciHostAddressSpace {
    Io,
    Memory,
    PrefetchableMemory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciHostAddressBases {
    io_base: Address,
    memory_base: Address,
    prefetchable_memory_base: Address,
}

impl PciHostAddressBases {
    pub const fn new(
        io_base: Address,
        memory_base: Address,
        prefetchable_memory_base: Address,
    ) -> Self {
        Self {
            io_base,
            memory_base,
            prefetchable_memory_base,
        }
    }

    pub const fn zero() -> Self {
        Self {
            io_base: Address::new(0),
            memory_base: Address::new(0),
            prefetchable_memory_base: Address::new(0),
        }
    }

    pub const fn io_base(self) -> Address {
        self.io_base
    }

    pub const fn memory_base(self) -> Address {
        self.memory_base
    }

    pub const fn prefetchable_memory_base(self) -> Address {
        self.prefetchable_memory_base
    }

    pub(crate) const fn base_for_space(self, space: PciHostAddressSpace) -> Address {
        match space {
            PciHostAddressSpace::Io => self.io_base,
            PciHostAddressSpace::Memory => self.memory_base,
            PciHostAddressSpace::PrefetchableMemory => self.prefetchable_memory_base,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciHostBarRange {
    function: PciFunctionAddress,
    bar: PciBarIndex,
    space: PciHostAddressSpace,
    pci_range: AddressRange,
    host_range: AddressRange,
}

impl PciHostBarRange {
    pub fn new(
        function: PciFunctionAddress,
        bar: PciBarIndex,
        space: PciHostAddressSpace,
        pci_base: Address,
        host_base: Address,
        size: AccessSize,
    ) -> Result<Self, PciError> {
        Ok(Self {
            function,
            bar,
            space,
            pci_range: AddressRange::new(pci_base, size).map_err(PciError::Memory)?,
            host_range: AddressRange::new(host_base, size).map_err(PciError::Memory)?,
        })
    }

    pub const fn function(&self) -> PciFunctionAddress {
        self.function
    }

    pub const fn bar(&self) -> PciBarIndex {
        self.bar
    }

    pub const fn space(&self) -> PciHostAddressSpace {
        self.space
    }

    pub const fn pci_range(&self) -> AddressRange {
        self.pci_range
    }

    pub const fn host_range(&self) -> AddressRange {
        self.host_range
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PciBarState {
    Endpoint {
        spec: PciBarSpec,
        raw: u32,
        upper_raw: u32,
    },
    Upper {
        owner: PciBarIndex,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PciBarShape {
    Endpoint(PciBarSpec),
    Upper(PciBarIndex),
}

impl PciBarState {
    pub(crate) fn new(spec: PciBarSpec) -> Self {
        Self::Endpoint {
            spec,
            raw: spec.kind().flags(),
            upper_raw: 0,
        }
    }

    pub(crate) const fn upper(owner: PciBarIndex) -> Self {
        Self::Upper { owner }
    }

    pub(crate) const fn shape(&self) -> PciBarShape {
        match self {
            Self::Endpoint { spec, .. } => PciBarShape::Endpoint(*spec),
            Self::Upper { owner } => PciBarShape::Upper(*owner),
        }
    }

    pub(crate) const fn owner(&self) -> Option<PciBarIndex> {
        match self {
            Self::Upper { owner } => Some(*owner),
            Self::Endpoint { .. } => None,
        }
    }

    pub(crate) const fn raw(&self) -> Option<u32> {
        match self {
            Self::Endpoint { raw, .. } => Some(*raw),
            Self::Upper { .. } => None,
        }
    }

    pub(crate) const fn upper_raw(&self) -> Option<u32> {
        match self {
            Self::Endpoint { upper_raw, .. } => Some(*upper_raw),
            Self::Upper { .. } => None,
        }
    }

    pub(crate) const fn kind(&self) -> Option<PciBarKind> {
        match self {
            Self::Endpoint { spec, .. } => Some(spec.kind()),
            Self::Upper { .. } => None,
        }
    }

    pub(crate) fn write_lower(&mut self, value: u32) {
        let Self::Endpoint { spec, raw, .. } = self else {
            return;
        };
        if matches!(spec.kind(), PciBarKind::LegacyIo { .. }) {
            *raw = 0;
            return;
        }
        let mask = !((spec.size().bytes() - 1) as u32);
        *raw = (value & mask) | spec.kind().flags();
    }

    pub(crate) fn write_upper(&mut self, value: u32) {
        let Self::Endpoint {
            spec, upper_raw, ..
        } = self
        else {
            return;
        };
        let mask = !(((spec.size().bytes() - 1) >> 32) as u32);
        *upper_raw = value & mask;
    }

    pub(crate) fn range(&self) -> Result<PciBarRange, PciError> {
        let Self::Endpoint {
            spec,
            raw,
            upper_raw,
        } = self
        else {
            return Err(PciError::UpperBarRange);
        };
        let lower = match spec.kind() {
            PciBarKind::LegacyIo { address } => address.get(),
            kind => {
                let fixed_low_bits = kind.fixed_low_bits() as u32;
                (raw & !fixed_low_bits) as u64
            }
        };
        let upper = if spec.kind().is_64_bit() {
            (*upper_raw as u64) << 32
        } else {
            0
        };
        PciBarRange::new(
            spec.index(),
            spec.kind(),
            Address::new(upper | lower),
            spec.size(),
        )
    }
}

pub(crate) fn bar_index_for_offset(offset: crate::PciConfigOffset) -> Option<PciBarIndex> {
    let offset = offset.as_usize();
    if !(PCI_BAR0_OFFSET..PCI_BAR0_OFFSET + PCI_BAR_COUNT * 4).contains(&offset) {
        return None;
    }
    if !(offset - PCI_BAR0_OFFSET).is_multiple_of(4) {
        return None;
    }
    PciBarIndex::new(((offset - PCI_BAR0_OFFSET) / 4) as u8).ok()
}

pub(crate) const fn host_address_space(kind: PciBarKind) -> PciHostAddressSpace {
    match kind {
        PciBarKind::Memory32 { prefetchable: true } => PciHostAddressSpace::PrefetchableMemory,
        PciBarKind::Memory64 { prefetchable: true } => PciHostAddressSpace::PrefetchableMemory,
        PciBarKind::Memory32 {
            prefetchable: false,
        } => PciHostAddressSpace::Memory,
        PciBarKind::Memory64 {
            prefetchable: false,
        } => PciHostAddressSpace::Memory,
        PciBarKind::LegacyIo { .. } => PciHostAddressSpace::Io,
        PciBarKind::Io => PciHostAddressSpace::Io,
    }
}

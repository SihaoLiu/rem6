use rem6_memory::{AccessSize, Address, AddressRange};

use crate::{PciError, PciFunctionAddress, PCI_BAR0_OFFSET, PCI_BAR_COUNT};

const PCI_BAR_SNAPSHOT_MAGIC: &[u8; 8] = b"R6PCBAR1";
const PCI_BAR_SNAPSHOT_VERSION: u16 = 1;
const PCI_BAR_SNAPSHOT_ENDPOINT: u8 = 1;
const PCI_BAR_SNAPSHOT_UPPER: u8 = 2;
const PCI_BAR_KIND_MEMORY32: u8 = 1;
const PCI_BAR_KIND_MEMORY32_PREFETCHABLE: u8 = 2;
const PCI_BAR_KIND_MEMORY64: u8 = 3;
const PCI_BAR_KIND_MEMORY64_PREFETCHABLE: u8 = 4;
const PCI_BAR_KIND_LEGACY_IO: u8 = 5;
const PCI_BAR_KIND_IO: u8 = 6;
const U16_BYTES: usize = 2;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;

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

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(PCI_BAR_SNAPSHOT_MAGIC);
        write_u16(&mut payload, PCI_BAR_SNAPSHOT_VERSION);
        match self {
            Self::Endpoint {
                spec,
                raw,
                upper_raw,
            } => {
                payload.push(PCI_BAR_SNAPSHOT_ENDPOINT);
                payload.push(spec.index().get());
                payload.push(encode_bar_kind(spec.kind()));
                write_u32(&mut payload, *raw);
                write_u32(&mut payload, *upper_raw);
                write_u64(&mut payload, spec.size().bytes());
                write_u64(&mut payload, legacy_io_address(spec.kind()).get());
            }
            Self::Upper { owner } => {
                payload.push(PCI_BAR_SNAPSHOT_UPPER);
                payload.push(owner.get());
            }
        }
        payload
    }

    pub(crate) fn from_bytes(payload: &[u8]) -> Result<Self, PciError> {
        decode_bar_state(payload).ok_or(PciError::InvalidBarSnapshot)
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

fn decode_bar_state(payload: &[u8]) -> Option<PciBarState> {
    let mut cursor = 0;
    let magic = read_exact(payload, &mut cursor, PCI_BAR_SNAPSHOT_MAGIC.len())?;
    if magic != PCI_BAR_SNAPSHOT_MAGIC {
        return None;
    }
    if read_u16(payload, &mut cursor)? != PCI_BAR_SNAPSHOT_VERSION {
        return None;
    }
    let tag = read_u8(payload, &mut cursor)?;
    let state = match tag {
        PCI_BAR_SNAPSHOT_ENDPOINT => {
            let index = PciBarIndex::new(read_u8(payload, &mut cursor)?).ok()?;
            let kind_tag = read_u8(payload, &mut cursor)?;
            let raw = read_u32(payload, &mut cursor)?;
            let upper_raw = read_u32(payload, &mut cursor)?;
            let size = AccessSize::new(read_u64(payload, &mut cursor)?).ok()?;
            let legacy_address = Address::new(read_u64(payload, &mut cursor)?);
            if kind_tag != PCI_BAR_KIND_LEGACY_IO && legacy_address.get() != 0 {
                return None;
            }
            let kind = decode_bar_kind(kind_tag, legacy_address)?;
            let spec = PciBarSpec::new(index, kind, size).ok()?;
            if !bar_raw_state_is_valid(spec, raw, upper_raw) {
                return None;
            }
            PciBarState::Endpoint {
                spec,
                raw,
                upper_raw,
            }
        }
        PCI_BAR_SNAPSHOT_UPPER => {
            let owner = PciBarIndex::new(read_u8(payload, &mut cursor)?).ok()?;
            if owner.as_usize() + 1 >= PCI_BAR_COUNT {
                return None;
            }
            PciBarState::Upper { owner }
        }
        _ => return None,
    };
    if cursor != payload.len() {
        return None;
    }
    Some(state)
}

fn encode_bar_kind(kind: PciBarKind) -> u8 {
    match kind {
        PciBarKind::Memory32 {
            prefetchable: false,
        } => PCI_BAR_KIND_MEMORY32,
        PciBarKind::Memory32 { prefetchable: true } => PCI_BAR_KIND_MEMORY32_PREFETCHABLE,
        PciBarKind::Memory64 {
            prefetchable: false,
        } => PCI_BAR_KIND_MEMORY64,
        PciBarKind::Memory64 { prefetchable: true } => PCI_BAR_KIND_MEMORY64_PREFETCHABLE,
        PciBarKind::LegacyIo { .. } => PCI_BAR_KIND_LEGACY_IO,
        PciBarKind::Io => PCI_BAR_KIND_IO,
    }
}

fn decode_bar_kind(tag: u8, legacy_address: Address) -> Option<PciBarKind> {
    match tag {
        PCI_BAR_KIND_MEMORY32 => Some(PciBarKind::Memory32 {
            prefetchable: false,
        }),
        PCI_BAR_KIND_MEMORY32_PREFETCHABLE => Some(PciBarKind::Memory32 { prefetchable: true }),
        PCI_BAR_KIND_MEMORY64 => Some(PciBarKind::Memory64 {
            prefetchable: false,
        }),
        PCI_BAR_KIND_MEMORY64_PREFETCHABLE => Some(PciBarKind::Memory64 { prefetchable: true }),
        PCI_BAR_KIND_LEGACY_IO => Some(PciBarKind::LegacyIo {
            address: legacy_address,
        }),
        PCI_BAR_KIND_IO => Some(PciBarKind::Io),
        _ => None,
    }
}

fn legacy_io_address(kind: PciBarKind) -> Address {
    match kind {
        PciBarKind::LegacyIo { address } => address,
        _ => Address::new(0),
    }
}

fn bar_raw_state_is_valid(spec: PciBarSpec, raw: u32, upper_raw: u32) -> bool {
    match spec.kind() {
        PciBarKind::LegacyIo { .. } => raw == 0 && upper_raw == 0,
        kind => {
            let mask = !((spec.size().bytes() - 1) as u32);
            if raw != ((raw & mask) | kind.flags()) {
                return false;
            }
            if kind.is_64_bit() {
                let upper_mask = !(((spec.size().bytes() - 1) >> 32) as u32);
                upper_raw == (upper_raw & upper_mask)
            } else {
                upper_raw == 0
            }
        }
    }
}

fn write_u16(payload: &mut Vec<u8>, value: u16) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn read_u8(payload: &[u8], cursor: &mut usize) -> Option<u8> {
    let byte = *payload.get(*cursor)?;
    *cursor += 1;
    Some(byte)
}

fn read_u16(payload: &[u8], cursor: &mut usize) -> Option<u16> {
    let bytes = read_exact(payload, cursor, U16_BYTES)?;
    Some(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u32(payload: &[u8], cursor: &mut usize) -> Option<u32> {
    let bytes = read_exact(payload, cursor, U32_BYTES)?;
    Some(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u64(payload: &[u8], cursor: &mut usize) -> Option<u64> {
    let bytes = read_exact(payload, cursor, U64_BYTES)?;
    Some(u64::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_exact<'a>(payload: &'a [u8], cursor: &mut usize, length: usize) -> Option<&'a [u8]> {
    let end = cursor.checked_add(length)?;
    let bytes = payload.get(*cursor..end)?;
    *cursor = end;
    Some(bytes)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn memory64_bar() -> PciBarSpec {
        PciBarSpec::new(
            PciBarIndex::new(2).unwrap(),
            PciBarKind::Memory64 { prefetchable: true },
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn bar_state_codec_preserves_endpoint_and_upper_state() {
        let mut state = PciBarState::new(memory64_bar());
        state.write_lower(0x0000_2345);
        state.write_upper(0x0000_0001);
        let decoded = PciBarState::from_bytes(&state.to_bytes()).unwrap();

        assert_eq!(decoded, state);
        assert_eq!(
            decoded.range(),
            Ok(PciBarRange::new(
                PciBarIndex::new(2).unwrap(),
                PciBarKind::Memory64 { prefetchable: true },
                Address::new(0x1_0000_2000),
                AccessSize::new(0x2000).unwrap(),
            )
            .unwrap())
        );

        let upper = PciBarState::upper(PciBarIndex::new(2).unwrap());
        assert_eq!(PciBarState::from_bytes(&upper.to_bytes()), Ok(upper));
    }

    #[test]
    fn bar_state_codec_rejects_invalid_payloads() {
        let state = PciBarState::new(memory64_bar());
        let mut payload = state.to_bytes();

        assert_eq!(
            PciBarState::from_bytes(&payload[..payload.len() - 1]),
            Err(PciError::InvalidBarSnapshot)
        );

        payload.push(0);
        assert_eq!(
            PciBarState::from_bytes(&payload),
            Err(PciError::InvalidBarSnapshot)
        );

        let mut invalid_magic = state.to_bytes();
        invalid_magic[0] = 0;
        assert_eq!(
            PciBarState::from_bytes(&invalid_magic),
            Err(PciError::InvalidBarSnapshot)
        );

        let mut invalid_version = state.to_bytes();
        invalid_version[8] = 0xff;
        assert_eq!(
            PciBarState::from_bytes(&invalid_version),
            Err(PciError::InvalidBarSnapshot)
        );

        let mut invalid_raw_low_bits = state.to_bytes();
        invalid_raw_low_bits[13] = 0;
        assert_eq!(
            PciBarState::from_bytes(&invalid_raw_low_bits),
            Err(PciError::InvalidBarSnapshot)
        );

        let mut invalid_upper_raw = PciBarState::new(
            PciBarSpec::new(
                PciBarIndex::new(0).unwrap(),
                PciBarKind::Memory32 {
                    prefetchable: false,
                },
                AccessSize::new(0x1000).unwrap(),
            )
            .unwrap(),
        )
        .to_bytes();
        invalid_upper_raw[17] = 1;
        assert_eq!(
            PciBarState::from_bytes(&invalid_upper_raw),
            Err(PciError::InvalidBarSnapshot)
        );

        let mut invalid_legacy_address = state.to_bytes();
        invalid_legacy_address[29] = 1;
        assert_eq!(
            PciBarState::from_bytes(&invalid_legacy_address),
            Err(PciError::InvalidBarSnapshot)
        );

        let mut invalid_upper_owner = PciBarState::upper(PciBarIndex::new(2).unwrap()).to_bytes();
        invalid_upper_owner[11] = 5;
        assert_eq!(
            PciBarState::from_bytes(&invalid_upper_owner),
            Err(PciError::InvalidBarSnapshot)
        );
    }
}

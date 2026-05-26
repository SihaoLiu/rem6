use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_memory::{AccessSize, Address, AddressRange, MemoryError};

const PCI_CONFIG_SPACE_SIZE: usize = 256;
const PCI_VENDOR_ID_OFFSET: usize = 0x00;
const PCI_DEVICE_ID_OFFSET: usize = 0x02;
const PCI_COMMAND_OFFSET: usize = 0x04;
const PCI_CLASS_REVISION_OFFSET: usize = 0x08;
const PCI_HEADER_TYPE_OFFSET: usize = 0x0e;
const PCI_BAR0_OFFSET: usize = 0x10;
const PCI_INTERRUPT_LINE_OFFSET: usize = 0x3c;
const PCI_INTERRUPT_PIN_OFFSET: usize = 0x3d;
const PCI_TYPE0_HEADER_TYPE: u8 = 0x00;
const PCI_COMMAND_IO_SPACE: u16 = 0x0001;
const PCI_COMMAND_MEMORY_SPACE: u16 = 0x0002;
const PCI_BAR_COUNT: usize = 6;
const PCI_CONFIG_FUNCTIONS_PER_BUS: u64 = 256;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PciFunctionAddress {
    bus: u8,
    device: u8,
    function: u8,
}

impl PciFunctionAddress {
    pub const fn new(bus: u8, device: u8, function: u8) -> Result<Self, PciError> {
        if device >= 32 {
            return Err(PciError::InvalidDeviceNumber { device });
        }
        if function >= 8 {
            return Err(PciError::InvalidFunctionNumber { function });
        }
        Ok(Self {
            bus,
            device,
            function,
        })
    }

    pub const fn bus(self) -> u8 {
        self.bus
    }

    pub const fn device(self) -> u8 {
        self.device
    }

    pub const fn function(self) -> u8 {
        self.function
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciDeviceIdentity {
    vendor_id: u16,
    device_id: u16,
}

impl PciDeviceIdentity {
    pub const fn new(vendor_id: u16, device_id: u16) -> Self {
        Self {
            vendor_id,
            device_id,
        }
    }

    pub const fn vendor_id(self) -> u16 {
        self.vendor_id
    }

    pub const fn device_id(self) -> u16 {
        self.device_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciClassCode {
    class: u8,
    subclass: u8,
    prog_if: u8,
    revision: u8,
}

impl PciClassCode {
    pub const fn new(class: u8, subclass: u8, prog_if: u8, revision: u8) -> Self {
        Self {
            class,
            subclass,
            prog_if,
            revision,
        }
    }

    pub const fn class(self) -> u8 {
        self.class
    }

    pub const fn subclass(self) -> u8 {
        self.subclass
    }

    pub const fn prog_if(self) -> u8 {
        self.prog_if
    }

    pub const fn revision(self) -> u8 {
        self.revision
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PciConfigOffset(u16);

impl PciConfigOffset {
    pub const fn new(value: u16) -> Result<Self, PciError> {
        if value as usize >= PCI_CONFIG_SPACE_SIZE {
            return Err(PciError::InvalidConfigOffset { offset: value });
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u16 {
        self.0
    }

    const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

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

    const fn as_usize(self) -> usize {
        self.0 as usize
    }

    const fn config_offset(self) -> usize {
        PCI_BAR0_OFFSET + self.as_usize() * 4
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PciBarKind {
    Memory32 { prefetchable: bool },
    Io,
}

impl PciBarKind {
    const fn flags(self) -> u32 {
        match self {
            Self::Memory32 { prefetchable } => {
                if prefetchable {
                    0x8
                } else {
                    0
                }
            }
            Self::Io => 0x1,
        }
    }

    const fn fixed_low_bits(self) -> u64 {
        match self {
            Self::Memory32 { .. } => 0xf,
            Self::Io => 0x3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PciInterruptPin {
    None,
    IntA,
    IntB,
    IntC,
    IntD,
}

impl PciInterruptPin {
    const fn config_value(self) -> u8 {
        match self {
            Self::None => 0,
            Self::IntA => 1,
            Self::IntB => 2,
            Self::IntC => 3,
            Self::IntD => 4,
        }
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
        let min_size = kind.fixed_low_bits() + 1;
        if size.bytes() < min_size || !size.bytes().is_power_of_two() {
            return Err(PciError::InvalidBarSize { index, kind, size });
        }
        if size.bytes() > u32::MAX as u64 {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciEndpointConfig {
    function: PciFunctionAddress,
    identity: PciDeviceIdentity,
    class: PciClassCode,
    config: [u8; PCI_CONFIG_SPACE_SIZE],
    bars: [Option<PciBarState>; PCI_BAR_COUNT],
}

impl PciEndpointConfig {
    pub fn new(
        function: PciFunctionAddress,
        identity: PciDeviceIdentity,
        class: PciClassCode,
    ) -> Self {
        let mut config = [0; PCI_CONFIG_SPACE_SIZE];
        write_u16_at(&mut config, PCI_VENDOR_ID_OFFSET, identity.vendor_id());
        write_u16_at(&mut config, PCI_DEVICE_ID_OFFSET, identity.device_id());
        config[PCI_CLASS_REVISION_OFFSET] = class.revision();
        config[PCI_CLASS_REVISION_OFFSET + 1] = class.prog_if();
        config[PCI_CLASS_REVISION_OFFSET + 2] = class.subclass();
        config[PCI_CLASS_REVISION_OFFSET + 3] = class.class();
        config[PCI_HEADER_TYPE_OFFSET] = PCI_TYPE0_HEADER_TYPE;

        Self {
            function,
            identity,
            class,
            config,
            bars: std::array::from_fn(|_| None),
        }
    }

    pub fn with_interrupt(mut self, line: u8, pin: PciInterruptPin) -> Self {
        self.config[PCI_INTERRUPT_LINE_OFFSET] = line;
        self.config[PCI_INTERRUPT_PIN_OFFSET] = pin.config_value();
        self
    }

    pub const fn function(&self) -> PciFunctionAddress {
        self.function
    }

    pub const fn identity(&self) -> PciDeviceIdentity {
        self.identity
    }

    pub const fn class(&self) -> PciClassCode {
        self.class
    }

    pub fn install_bar(&mut self, spec: PciBarSpec) -> Result<(), PciError> {
        let index = spec.index().as_usize();
        if self.bars[index].is_some() {
            return Err(PciError::DuplicateBar {
                index: spec.index(),
            });
        }

        let state = PciBarState::new(spec);
        write_u32_at(&mut self.config, spec.index().config_offset(), state.raw);
        self.bars[index] = Some(state);
        Ok(())
    }

    pub fn read_config(
        &self,
        offset: PciConfigOffset,
        size: AccessSize,
    ) -> Result<Vec<u8>, PciError> {
        let span = config_span(offset, size)?;
        Ok(self.config[span.start..span.end].to_vec())
    }

    pub fn write_config(&mut self, offset: PciConfigOffset, data: &[u8]) -> Result<(), PciError> {
        let size = access_size_from_len(data.len())?;
        let span = config_span(offset, size)?;
        if let Some(index) = bar_index_for_offset(offset) {
            if data.len() != 4 {
                return Err(PciError::UnalignedBarAccess { offset, size });
            }
            return self.write_bar(index, u32::from_le_bytes(data.try_into().unwrap()));
        }

        match offset.as_usize() {
            PCI_COMMAND_OFFSET if data.len() == 2 => {
                self.config[span.start..span.end].copy_from_slice(data);
                Ok(())
            }
            PCI_COMMAND_OFFSET if data.len() == 4 => {
                self.config[PCI_COMMAND_OFFSET..PCI_COMMAND_OFFSET + 2].copy_from_slice(&data[..2]);
                Ok(())
            }
            PCI_INTERRUPT_LINE_OFFSET if data.len() == 1 => {
                self.config[PCI_INTERRUPT_LINE_OFFSET] = data[0];
                Ok(())
            }
            _ => Err(PciError::ReadOnlyConfigWrite { offset, size }),
        }
    }

    pub fn read_u32(&self, offset: PciConfigOffset) -> Result<u32, PciError> {
        let bytes = self.read_config(offset, AccessSize::new(4).map_err(PciError::Memory)?)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub fn write_u32(&mut self, offset: PciConfigOffset, value: u32) -> Result<(), PciError> {
        self.write_config(offset, &value.to_le_bytes())
    }

    pub fn active_bar_ranges(&self) -> Vec<PciBarRange> {
        self.bars
            .iter()
            .filter_map(|bar| {
                let bar = bar.as_ref()?;
                if !self.bar_enabled(bar.spec.kind()) {
                    return None;
                }
                bar.range().ok()
            })
            .collect()
    }

    pub fn snapshot(&self) -> PciEndpointConfigSnapshot {
        PciEndpointConfigSnapshot {
            function: self.function,
            identity: self.identity,
            class: self.class,
            config: self.config,
            bars: self.bars.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &PciEndpointConfigSnapshot) -> Result<(), PciError> {
        if self.function != snapshot.function {
            return Err(PciError::SnapshotFunctionMismatch {
                expected: self.function,
                actual: snapshot.function,
            });
        }
        if self.identity != snapshot.identity {
            return Err(PciError::SnapshotIdentityMismatch {
                expected: self.identity,
                actual: snapshot.identity,
            });
        }
        if self.class != snapshot.class {
            return Err(PciError::SnapshotClassMismatch {
                expected: self.class,
                actual: snapshot.class,
            });
        }
        for (index, (current, restored)) in self.bars.iter().zip(snapshot.bars.iter()).enumerate() {
            if current.as_ref().map(PciBarState::spec) != restored.as_ref().map(PciBarState::spec) {
                return Err(PciError::SnapshotBarMismatch {
                    index: PciBarIndex::new(index as u8).expect("snapshot bar index"),
                });
            }
        }

        self.config = snapshot.config;
        self.bars = snapshot.bars.clone();
        Ok(())
    }

    fn write_bar(&mut self, index: PciBarIndex, value: u32) -> Result<(), PciError> {
        let bar = self.bars[index.as_usize()]
            .as_mut()
            .ok_or(PciError::MissingBar { index })?;
        bar.write(value);
        write_u32_at(&mut self.config, index.config_offset(), bar.raw);
        Ok(())
    }

    fn command(&self) -> u16 {
        u16::from_le_bytes(
            self.config[PCI_COMMAND_OFFSET..PCI_COMMAND_OFFSET + 2]
                .try_into()
                .unwrap(),
        )
    }

    fn bar_enabled(&self, kind: PciBarKind) -> bool {
        let command = self.command();
        match kind {
            PciBarKind::Memory32 { .. } => command & PCI_COMMAND_MEMORY_SPACE != 0,
            PciBarKind::Io => command & PCI_COMMAND_IO_SPACE != 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciEndpointConfigSnapshot {
    function: PciFunctionAddress,
    identity: PciDeviceIdentity,
    class: PciClassCode,
    config: [u8; PCI_CONFIG_SPACE_SIZE],
    bars: [Option<PciBarState>; PCI_BAR_COUNT],
}

impl PciEndpointConfigSnapshot {
    pub const fn function(&self) -> PciFunctionAddress {
        self.function
    }

    pub const fn identity(&self) -> PciDeviceIdentity {
        self.identity
    }

    pub const fn class(&self) -> PciClassCode {
        self.class
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciConfigAperture {
    base: Address,
    bus_count: u8,
    device_bits: u8,
    range: AddressRange,
}

impl PciConfigAperture {
    pub fn cam(base: Address, bus_count: u8) -> Result<Self, PciError> {
        Self::new(base, bus_count, 8)
    }

    pub fn ecam(base: Address, bus_count: u8) -> Result<Self, PciError> {
        Self::new(base, bus_count, 12)
    }

    pub fn new(base: Address, bus_count: u8, device_bits: u8) -> Result<Self, PciError> {
        if bus_count == 0 {
            return Err(PciError::ZeroConfigBuses);
        }
        if !(8..=12).contains(&device_bits) {
            return Err(PciError::InvalidConfigDeviceBits { device_bits });
        }

        let slot_size = 1_u64 << device_bits;
        let bytes = (bus_count as u64)
            .checked_mul(PCI_CONFIG_FUNCTIONS_PER_BUS)
            .and_then(|slots| slots.checked_mul(slot_size))
            .ok_or(PciError::ConfigApertureSizeOverflow {
                bus_count,
                device_bits,
            })?;
        let size = AccessSize::new(bytes).map_err(PciError::Memory)?;
        let range = AddressRange::new(base, size).map_err(PciError::Memory)?;
        Ok(Self {
            base,
            bus_count,
            device_bits,
            range,
        })
    }

    pub const fn base(self) -> Address {
        self.base
    }

    pub const fn bus_count(self) -> u8 {
        self.bus_count
    }

    pub const fn device_bits(self) -> u8 {
        self.device_bits
    }

    pub const fn range(self) -> AddressRange {
        self.range
    }

    pub fn endpoint_config_range(
        self,
        function: PciFunctionAddress,
    ) -> Result<AddressRange, PciError> {
        self.validate_function(function)?;
        AddressRange::new(
            Address::new(self.function_base(function)?),
            AccessSize::new(1_u64 << self.device_bits).map_err(PciError::Memory)?,
        )
        .map_err(PciError::Memory)
    }

    pub fn config_address(
        self,
        function: PciFunctionAddress,
        offset: PciConfigOffset,
    ) -> Result<Address, PciError> {
        self.validate_function(function)?;
        self.function_base(function)?
            .checked_add(offset.get() as u64)
            .map(Address::new)
            .ok_or(PciError::ConfigApertureSizeOverflow {
                bus_count: self.bus_count,
                device_bits: self.device_bits,
            })
    }

    pub fn decode(self, address: Address) -> Result<PciDecodedConfigAddress, PciError> {
        if !self.range.contains(address) {
            return Err(PciError::ConfigAddressOutsideAperture {
                address,
                range: self.range,
            });
        }

        let relative = address.get() - self.base.get();
        let slot_offset_mask = (1_u64 << self.device_bits) - 1;
        let raw_offset = relative & slot_offset_mask;
        if raw_offset >= PCI_CONFIG_SPACE_SIZE as u64 {
            return Err(PciError::UnsupportedConfigAddressOffset {
                address,
                raw_offset,
                device_bits: self.device_bits,
            });
        }

        let slot = relative >> self.device_bits;
        let bus = (slot >> 8) as u8;
        let device = ((slot >> 3) & 0x1f) as u8;
        let function = (slot & 0x7) as u8;
        Ok(PciDecodedConfigAddress {
            function: PciFunctionAddress::new(bus, device, function)
                .expect("decoded PCI function address"),
            offset: PciConfigOffset::new(raw_offset as u16).expect("decoded PCI config offset"),
        })
    }

    fn validate_function(self, function: PciFunctionAddress) -> Result<(), PciError> {
        if function.bus() >= self.bus_count {
            return Err(PciError::FunctionOutsideAperture {
                function,
                bus_count: self.bus_count,
            });
        }
        Ok(())
    }

    fn function_base(self, function: PciFunctionAddress) -> Result<u64, PciError> {
        let slot = ((function.bus() as u64) << 8)
            | ((function.device() as u64) << 3)
            | function.function() as u64;
        self.base.get().checked_add(slot << self.device_bits).ok_or(
            PciError::ConfigApertureSizeOverflow {
                bus_count: self.bus_count,
                device_bits: self.device_bits,
            },
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciDecodedConfigAddress {
    function: PciFunctionAddress,
    offset: PciConfigOffset,
}

impl PciDecodedConfigAddress {
    pub const fn function(self) -> PciFunctionAddress {
        self.function
    }

    pub const fn offset(self) -> PciConfigOffset {
        self.offset
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciHostBridge {
    aperture: PciConfigAperture,
    endpoints: BTreeMap<PciFunctionAddress, PciEndpointConfig>,
}

impl PciHostBridge {
    pub fn new(aperture: PciConfigAperture) -> Self {
        Self {
            aperture,
            endpoints: BTreeMap::new(),
        }
    }

    pub const fn aperture(&self) -> PciConfigAperture {
        self.aperture
    }

    pub fn endpoint(&self, function: PciFunctionAddress) -> Option<&PciEndpointConfig> {
        self.endpoints.get(&function)
    }

    pub fn endpoint_config_range(
        &self,
        function: PciFunctionAddress,
    ) -> Result<AddressRange, PciError> {
        self.aperture.endpoint_config_range(function)
    }

    pub fn register_endpoint(&mut self, endpoint: PciEndpointConfig) -> Result<(), PciError> {
        let function = endpoint.function();
        self.aperture.validate_function(function)?;
        if self.endpoints.contains_key(&function) {
            return Err(PciError::DuplicateFunction { function });
        }
        self.endpoints.insert(function, endpoint);
        Ok(())
    }

    pub fn read_config_address(
        &self,
        address: Address,
        size: AccessSize,
    ) -> Result<Vec<u8>, PciError> {
        let decoded = self.aperture.decode(address)?;
        config_span(decoded.offset(), size)?;
        let Some(endpoint) = self.endpoint(decoded.function()) else {
            return Ok(vec![0xff; size.bytes() as usize]);
        };
        endpoint.read_config(decoded.offset(), size)
    }

    pub fn write_config_address(&mut self, address: Address, data: &[u8]) -> Result<(), PciError> {
        let decoded = self.aperture.decode(address)?;
        let size = access_size_from_len(data.len())?;
        config_span(decoded.offset(), size)?;
        let Some(endpoint) = self.endpoints.get_mut(&decoded.function()) else {
            return Ok(());
        };
        endpoint.write_config(decoded.offset(), data)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PciBarState {
    spec: PciBarSpec,
    raw: u32,
}

impl PciBarState {
    fn new(spec: PciBarSpec) -> Self {
        Self {
            spec,
            raw: spec.kind().flags(),
        }
    }

    const fn spec(&self) -> PciBarSpec {
        self.spec
    }

    fn write(&mut self, value: u32) {
        let mask = !((self.spec.size().bytes() as u32) - 1);
        self.raw = (value & mask) | self.spec.kind().flags();
    }

    fn range(&self) -> Result<PciBarRange, PciError> {
        let fixed_low_bits = self.spec.kind().fixed_low_bits() as u32;
        let base = self.raw & !fixed_low_bits;
        PciBarRange::new(
            self.spec.index(),
            self.spec.kind(),
            Address::new(base as u64),
            self.spec.size(),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConfigSpan {
    start: usize,
    end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PciError {
    InvalidDeviceNumber {
        device: u8,
    },
    InvalidFunctionNumber {
        function: u8,
    },
    InvalidConfigOffset {
        offset: u16,
    },
    InvalidConfigAccessSize {
        size: AccessSize,
    },
    ConfigAccessOutOfRange {
        offset: PciConfigOffset,
        size: AccessSize,
    },
    ZeroConfigBuses,
    InvalidConfigDeviceBits {
        device_bits: u8,
    },
    ConfigApertureSizeOverflow {
        bus_count: u8,
        device_bits: u8,
    },
    FunctionOutsideAperture {
        function: PciFunctionAddress,
        bus_count: u8,
    },
    ConfigAddressOutsideAperture {
        address: Address,
        range: AddressRange,
    },
    UnsupportedConfigAddressOffset {
        address: Address,
        raw_offset: u64,
        device_bits: u8,
    },
    DuplicateFunction {
        function: PciFunctionAddress,
    },
    ReadOnlyConfigWrite {
        offset: PciConfigOffset,
        size: AccessSize,
    },
    UnalignedBarAccess {
        offset: PciConfigOffset,
        size: AccessSize,
    },
    InvalidBarIndex {
        index: u8,
    },
    DuplicateBar {
        index: PciBarIndex,
    },
    MissingBar {
        index: PciBarIndex,
    },
    InvalidBarSize {
        index: PciBarIndex,
        kind: PciBarKind,
        size: AccessSize,
    },
    SnapshotFunctionMismatch {
        expected: PciFunctionAddress,
        actual: PciFunctionAddress,
    },
    SnapshotIdentityMismatch {
        expected: PciDeviceIdentity,
        actual: PciDeviceIdentity,
    },
    SnapshotClassMismatch {
        expected: PciClassCode,
        actual: PciClassCode,
    },
    SnapshotBarMismatch {
        index: PciBarIndex,
    },
    Memory(MemoryError),
}

impl fmt::Display for PciError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDeviceNumber { device } => {
                write!(f, "PCI device number {device} is outside 0..32")
            }
            Self::InvalidFunctionNumber { function } => {
                write!(f, "PCI function number {function} is outside 0..8")
            }
            Self::InvalidConfigOffset { offset } => {
                write!(f, "PCI config offset {offset:#x} is outside 256 bytes")
            }
            Self::InvalidConfigAccessSize { size } => {
                write!(
                    f,
                    "PCI config access size {} is not 1, 2, or 4 bytes",
                    size.bytes()
                )
            }
            Self::ConfigAccessOutOfRange { offset, size } => write!(
                f,
                "PCI config access at {:#x} for {} bytes crosses config space",
                offset.get(),
                size.bytes()
            ),
            Self::ZeroConfigBuses => write!(f, "PCI config aperture must cover at least one bus"),
            Self::InvalidConfigDeviceBits { device_bits } => write!(
                f,
                "PCI config aperture device bits {device_bits} are outside 8..=12"
            ),
            Self::ConfigApertureSizeOverflow {
                bus_count,
                device_bits,
            } => write!(
                f,
                "PCI config aperture for {bus_count} buses and {device_bits} device bits overflows"
            ),
            Self::FunctionOutsideAperture {
                function,
                bus_count,
            } => write!(
                f,
                "PCI function {:?} is outside config aperture bus count {}",
                function, bus_count
            ),
            Self::ConfigAddressOutsideAperture { address, range } => write!(
                f,
                "PCI config address {:#x} is outside aperture {:#x}..{:#x}",
                address.get(),
                range.start().get(),
                range.end().get()
            ),
            Self::UnsupportedConfigAddressOffset {
                address,
                raw_offset,
                device_bits,
            } => write!(
                f,
                "PCI config address {:#x} decodes to unsupported offset {:#x} with {} device bits",
                address.get(),
                raw_offset,
                device_bits
            ),
            Self::DuplicateFunction { function } => {
                write!(f, "PCI function {:?} is already registered", function)
            }
            Self::ReadOnlyConfigWrite { offset, size } => write!(
                f,
                "PCI config write at {:#x} for {} bytes targets read-only state",
                offset.get(),
                size.bytes()
            ),
            Self::UnalignedBarAccess { offset, size } => write!(
                f,
                "PCI BAR access at {:#x} for {} bytes must be a 32-bit BAR access",
                offset.get(),
                size.bytes()
            ),
            Self::InvalidBarIndex { index } => {
                write!(f, "PCI BAR index {index} is outside 0..6")
            }
            Self::DuplicateBar { index } => {
                write!(f, "PCI BAR {} is already installed", index.get())
            }
            Self::MissingBar { index } => {
                write!(f, "PCI BAR {} is not installed", index.get())
            }
            Self::InvalidBarSize { index, kind, size } => write!(
                f,
                "PCI BAR {} has invalid {:?} size {}",
                index.get(),
                kind,
                size.bytes()
            ),
            Self::SnapshotFunctionMismatch { expected, actual } => write!(
                f,
                "PCI snapshot function mismatch: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::SnapshotIdentityMismatch { expected, actual } => write!(
                f,
                "PCI snapshot identity mismatch: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::SnapshotClassMismatch { expected, actual } => write!(
                f,
                "PCI snapshot class mismatch: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::SnapshotBarMismatch { index } => {
                write!(
                    f,
                    "PCI snapshot BAR {} does not match this endpoint",
                    index.get()
                )
            }
            Self::Memory(error) => write!(f, "{error}"),
        }
    }
}

impl Error for PciError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            _ => None,
        }
    }
}

impl From<MemoryError> for PciError {
    fn from(value: MemoryError) -> Self {
        Self::Memory(value)
    }
}

fn config_span(offset: PciConfigOffset, size: AccessSize) -> Result<ConfigSpan, PciError> {
    validate_config_access_size(size)?;
    let start = offset.as_usize();
    let end = start
        .checked_add(size.bytes() as usize)
        .ok_or(PciError::ConfigAccessOutOfRange { offset, size })?;
    if end > PCI_CONFIG_SPACE_SIZE {
        return Err(PciError::ConfigAccessOutOfRange { offset, size });
    }
    Ok(ConfigSpan { start, end })
}

fn validate_config_access_size(size: AccessSize) -> Result<(), PciError> {
    match size.bytes() {
        1 | 2 | 4 => Ok(()),
        _ => Err(PciError::InvalidConfigAccessSize { size }),
    }
}

fn access_size_from_len(len: usize) -> Result<AccessSize, PciError> {
    let size = AccessSize::new(len as u64).map_err(PciError::Memory)?;
    validate_config_access_size(size)?;
    Ok(size)
}

fn bar_index_for_offset(offset: PciConfigOffset) -> Option<PciBarIndex> {
    let offset = offset.as_usize();
    if !(PCI_BAR0_OFFSET..PCI_BAR0_OFFSET + PCI_BAR_COUNT * 4).contains(&offset) {
        return None;
    }
    if !(offset - PCI_BAR0_OFFSET).is_multiple_of(4) {
        return None;
    }
    PciBarIndex::new(((offset - PCI_BAR0_OFFSET) / 4) as u8).ok()
}

fn write_u16_at(config: &mut [u8; PCI_CONFIG_SPACE_SIZE], offset: usize, value: u16) {
    config[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32_at(config: &mut [u8; PCI_CONFIG_SPACE_SIZE], offset: usize, value: u32) {
    config[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

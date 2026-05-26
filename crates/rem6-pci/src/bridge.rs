use rem6_memory::{AccessSize, Address, AddressRange};

use crate::{
    write_common_command, write_common_status, write_u16_at, write_u32_at, PciBarIndex, PciBarKind,
    PciBarRange, PciBarSpec, PciBarState, PciClassCode, PciConfigOffset, PciDeviceIdentity,
    PciError, PciFunctionAddress, PciHostAddressSpace, PciInterruptPin, PCI_COMMAND_IO_SPACE,
    PCI_COMMAND_MEMORY_SPACE, PCI_CONFIG_SPACE_SIZE, PCI_STATUS_OFFSET, PCI_TYPE1_HEADER_TYPE,
};

const PCI_VENDOR_ID_OFFSET: usize = 0x00;
const PCI_DEVICE_ID_OFFSET: usize = 0x02;
const PCI_COMMAND_OFFSET: usize = 0x04;
const PCI_CLASS_REVISION_OFFSET: usize = 0x08;
const PCI_CACHE_LINE_SIZE_OFFSET: usize = 0x0c;
const PCI_LATENCY_TIMER_OFFSET: usize = 0x0d;
const PCI_HEADER_TYPE_OFFSET: usize = 0x0e;
const PCI_BIST_OFFSET: usize = 0x0f;
const PCI_TYPE1_BAR0_OFFSET: usize = 0x10;
const PCI_TYPE1_BAR1_OFFSET: usize = 0x14;
const PCI_TYPE1_PRIMARY_BUS_OFFSET: usize = 0x18;
const PCI_TYPE1_SECONDARY_BUS_OFFSET: usize = 0x19;
const PCI_TYPE1_SUBORDINATE_BUS_OFFSET: usize = 0x1a;
const PCI_TYPE1_SECONDARY_LATENCY_OFFSET: usize = 0x1b;
const PCI_TYPE1_IO_BASE_OFFSET: usize = 0x1c;
const PCI_TYPE1_IO_LIMIT_OFFSET: usize = 0x1d;
const PCI_TYPE1_SECONDARY_STATUS_OFFSET: usize = 0x1e;
const PCI_TYPE1_MEMORY_BASE_OFFSET: usize = 0x20;
const PCI_TYPE1_MEMORY_LIMIT_OFFSET: usize = 0x22;
const PCI_TYPE1_PREFETCH_BASE_OFFSET: usize = 0x24;
const PCI_TYPE1_PREFETCH_LIMIT_OFFSET: usize = 0x26;
const PCI_TYPE1_PREFETCH_BASE_UPPER_OFFSET: usize = 0x28;
const PCI_TYPE1_PREFETCH_LIMIT_UPPER_OFFSET: usize = 0x2c;
const PCI_TYPE1_IO_BASE_UPPER_OFFSET: usize = 0x30;
const PCI_TYPE1_IO_LIMIT_UPPER_OFFSET: usize = 0x32;
const PCI_TYPE1_EXPANSION_ROM_OFFSET: usize = 0x38;
const PCI_INTERRUPT_LINE_OFFSET: usize = 0x3c;
const PCI_INTERRUPT_PIN_OFFSET: usize = 0x3d;
const PCI_TYPE1_BRIDGE_CONTROL_OFFSET: usize = 0x3e;
const PCI_BRIDGE_MEMORY_GRANULARITY: u64 = 0x0010_0000;
const PCI_BRIDGE_IO_GRANULARITY: u64 = 0x1000;
const PCI_TYPE1_EXPANSION_ROM_SIZE_PROBE: u32 = 0xffff_fffe;
const PCI_TYPE1_BAR_COUNT: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciBridgeBusRange {
    primary: u8,
    secondary: u8,
    subordinate: u8,
}

impl PciBridgeBusRange {
    pub const fn new(primary: u8, secondary: u8, subordinate: u8) -> Result<Self, PciError> {
        if secondary == 0 || secondary < primary || subordinate < secondary {
            return Err(PciError::InvalidBridgeBusRange {
                primary,
                secondary,
                subordinate,
            });
        }
        Ok(Self {
            primary,
            secondary,
            subordinate,
        })
    }

    pub const fn primary(self) -> u8 {
        self.primary
    }

    pub const fn secondary(self) -> u8 {
        self.secondary
    }

    pub const fn subordinate(self) -> u8 {
        self.subordinate
    }

    const fn contains(self, bus: u8) -> bool {
        self.secondary <= bus && bus <= self.subordinate
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciType1HeaderFields {
    expansion_rom: u32,
    interrupt_line: u8,
    interrupt_pin: PciInterruptPin,
    bridge_control: u16,
}

impl PciType1HeaderFields {
    pub const fn new(
        expansion_rom: u32,
        interrupt_line: u8,
        interrupt_pin: PciInterruptPin,
        bridge_control: u16,
    ) -> Self {
        Self {
            expansion_rom,
            interrupt_line,
            interrupt_pin,
            bridge_control,
        }
    }

    pub const fn expansion_rom(self) -> u32 {
        self.expansion_rom
    }

    pub const fn interrupt_line(self) -> u8 {
        self.interrupt_line
    }

    pub const fn interrupt_pin(self) -> PciInterruptPin {
        self.interrupt_pin
    }

    pub const fn bridge_control(self) -> u16 {
        self.bridge_control
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciBridgeConfig {
    function: PciFunctionAddress,
    identity: PciDeviceIdentity,
    class: PciClassCode,
    config: [u8; PCI_CONFIG_SPACE_SIZE],
    bars: [Option<PciBarState>; PCI_TYPE1_BAR_COUNT],
}

impl PciBridgeConfig {
    pub fn new(
        function: PciFunctionAddress,
        identity: PciDeviceIdentity,
        class: PciClassCode,
        bus_range: PciBridgeBusRange,
    ) -> Self {
        let mut config = [0; PCI_CONFIG_SPACE_SIZE];
        write_u16_at(&mut config, PCI_VENDOR_ID_OFFSET, identity.vendor_id());
        write_u16_at(&mut config, PCI_DEVICE_ID_OFFSET, identity.device_id());
        config[PCI_CLASS_REVISION_OFFSET] = class.revision();
        config[PCI_CLASS_REVISION_OFFSET + 1] = class.prog_if();
        config[PCI_CLASS_REVISION_OFFSET + 2] = class.subclass();
        config[PCI_CLASS_REVISION_OFFSET + 3] = class.class();
        config[PCI_HEADER_TYPE_OFFSET] = PCI_TYPE1_HEADER_TYPE;
        config[PCI_TYPE1_PRIMARY_BUS_OFFSET] = bus_range.primary();
        config[PCI_TYPE1_SECONDARY_BUS_OFFSET] = bus_range.secondary();
        config[PCI_TYPE1_SUBORDINATE_BUS_OFFSET] = bus_range.subordinate();

        Self {
            function,
            identity,
            class,
            config,
            bars: std::array::from_fn(|_| None),
        }
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

    pub fn with_type1_header(mut self, fields: PciType1HeaderFields) -> Self {
        write_u32_at(
            &mut self.config,
            PCI_TYPE1_EXPANSION_ROM_OFFSET,
            fields.expansion_rom(),
        );
        self.config[PCI_INTERRUPT_LINE_OFFSET] = fields.interrupt_line();
        self.config[PCI_INTERRUPT_PIN_OFFSET] = fields.interrupt_pin().config_value();
        write_u16_at(
            &mut self.config,
            PCI_TYPE1_BRIDGE_CONTROL_OFFSET,
            fields.bridge_control(),
        );
        self
    }

    pub fn install_bar(&mut self, spec: PciBarSpec) -> Result<(), PciError> {
        self.validate_type1_bar_index(spec.index())?;
        self.validate_bar_slot_free(spec.index())?;
        let upper_index = if spec.kind().is_64_bit() {
            let upper =
                PciBarIndex::new(spec.index().get() + 1).expect("validated 64-bit BAR pair");
            self.validate_type1_bar_index(upper)?;
            self.validate_bar_slot_free(upper)?;
            Some(upper)
        } else {
            None
        };

        let state = PciBarState::new(spec);
        write_u32_at(
            &mut self.config,
            spec.index().config_offset(),
            state.raw().expect("new PCI bridge BAR raw value"),
        );
        self.bars[spec.index().as_usize()] = Some(state);
        if let Some(upper) = upper_index {
            write_u32_at(&mut self.config, upper.config_offset(), 0);
            self.bars[upper.as_usize()] = Some(PciBarState::upper(spec.index()));
        }
        Ok(())
    }

    pub fn bus_range(&self) -> PciBridgeBusRange {
        PciBridgeBusRange::new(
            self.config[PCI_TYPE1_PRIMARY_BUS_OFFSET],
            self.config[PCI_TYPE1_SECONDARY_BUS_OFFSET],
            self.config[PCI_TYPE1_SUBORDINATE_BUS_OFFSET],
        )
        .expect("PCI bridge bus range is validated before config mutation")
    }

    pub fn routes_bus(&self, bus: u8) -> bool {
        self.bus_range().contains(bus)
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
        if let Some(index) = type1_bar_index_for_offset(offset) {
            if data.len() != 4 {
                return Err(PciError::UnalignedBarAccess { offset, size });
            }
            return self.write_bar(index, u32::from_le_bytes(data.try_into().unwrap()));
        }
        match (offset.as_usize(), data.len()) {
            (PCI_COMMAND_OFFSET, 2) => {
                write_common_command(
                    &mut self.config,
                    PCI_COMMAND_OFFSET,
                    u16::from_le_bytes(data.try_into().unwrap()),
                );
                Ok(())
            }
            (PCI_COMMAND_OFFSET, 4) => {
                write_common_command(
                    &mut self.config,
                    PCI_COMMAND_OFFSET,
                    u16::from_le_bytes([data[0], data[1]]),
                );
                Ok(())
            }
            (PCI_STATUS_OFFSET, 2) => {
                write_common_status(
                    &mut self.config,
                    u16::from_le_bytes(data.try_into().unwrap()),
                    0,
                );
                Ok(())
            }
            (PCI_CACHE_LINE_SIZE_OFFSET | PCI_LATENCY_TIMER_OFFSET, 1) => {
                self.config[span.start] = data[0];
                Ok(())
            }
            (PCI_BIST_OFFSET, 1) => {
                self.config[PCI_BIST_OFFSET] = data[0];
                Ok(())
            }
            (PCI_CACHE_LINE_SIZE_OFFSET, 2) => {
                self.config[span.start..span.end].copy_from_slice(data);
                Ok(())
            }
            (PCI_TYPE1_PRIMARY_BUS_OFFSET, 4) => self.write_bus_block(data),
            (PCI_TYPE1_PRIMARY_BUS_OFFSET, 1)
            | (PCI_TYPE1_SECONDARY_BUS_OFFSET, 1)
            | (PCI_TYPE1_SUBORDINATE_BUS_OFFSET, 1)
            | (PCI_TYPE1_SECONDARY_LATENCY_OFFSET, 1) => self.write_bus_byte(offset, data[0]),
            (PCI_TYPE1_IO_BASE_OFFSET, 1) | (PCI_TYPE1_IO_LIMIT_OFFSET, 1) => {
                self.config[span.start] = data[0];
                Ok(())
            }
            (PCI_TYPE1_SECONDARY_STATUS_OFFSET, 2)
            | (PCI_TYPE1_MEMORY_BASE_OFFSET, 2)
            | (PCI_TYPE1_MEMORY_LIMIT_OFFSET, 2)
            | (PCI_TYPE1_PREFETCH_BASE_OFFSET, 2)
            | (PCI_TYPE1_PREFETCH_LIMIT_OFFSET, 2)
            | (PCI_TYPE1_IO_BASE_UPPER_OFFSET, 2)
            | (PCI_TYPE1_IO_LIMIT_UPPER_OFFSET, 2)
            | (PCI_TYPE1_BRIDGE_CONTROL_OFFSET, 2) => {
                self.config[span.start..span.end].copy_from_slice(data);
                Ok(())
            }
            (PCI_TYPE1_MEMORY_BASE_OFFSET, 4) | (PCI_TYPE1_PREFETCH_BASE_OFFSET, 4) => {
                self.config[span.start..span.end].copy_from_slice(data);
                Ok(())
            }
            (PCI_TYPE1_PREFETCH_BASE_UPPER_OFFSET, 4)
            | (PCI_TYPE1_PREFETCH_LIMIT_UPPER_OFFSET, 4) => {
                self.config[span.start..span.end].copy_from_slice(data);
                Ok(())
            }
            (PCI_TYPE1_EXPANSION_ROM_OFFSET, 4) => {
                let value = u32::from_le_bytes(data.try_into().unwrap());
                let value = if value == PCI_TYPE1_EXPANSION_ROM_SIZE_PROBE {
                    u32::MAX
                } else {
                    value
                };
                write_u32_at(&mut self.config, PCI_TYPE1_EXPANSION_ROM_OFFSET, value);
                Ok(())
            }
            (PCI_INTERRUPT_LINE_OFFSET, 1) => {
                self.config[PCI_INTERRUPT_LINE_OFFSET] = data[0];
                Ok(())
            }
            _ => Err(PciError::ReadOnlyConfigWrite { offset, size }),
        }
    }

    pub fn allows_bar_range(&self, kind: PciBarKind, range: AddressRange) -> bool {
        self.window_for_kind(kind)
            .is_some_and(|window| window.contains_range(range))
    }

    pub fn active_bar_ranges(&self) -> Vec<PciBarRange> {
        self.bars
            .iter()
            .filter_map(|bar| {
                let bar = bar.as_ref()?;
                let kind = bar.kind()?;
                if !self.bar_enabled(kind) {
                    return None;
                }
                bar.range().ok()
            })
            .collect()
    }

    pub fn snapshot(&self) -> PciBridgeConfigSnapshot {
        PciBridgeConfigSnapshot {
            function: self.function,
            identity: self.identity,
            class: self.class,
            config: self.config,
            bars: self.bars.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &PciBridgeConfigSnapshot) -> Result<(), PciError> {
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
            if current.as_ref().map(PciBarState::shape) != restored.as_ref().map(PciBarState::shape)
            {
                return Err(PciError::SnapshotBarMismatch {
                    index: PciBarIndex::new(index as u8).expect("snapshot bridge BAR index"),
                });
            }
        }

        self.config = snapshot.config;
        self.bars = snapshot.bars.clone();
        Ok(())
    }

    fn write_bus_block(&mut self, data: &[u8]) -> Result<(), PciError> {
        let range = PciBridgeBusRange::new(data[0], data[1], data[2])?;
        self.config[PCI_TYPE1_PRIMARY_BUS_OFFSET] = range.primary();
        self.config[PCI_TYPE1_SECONDARY_BUS_OFFSET] = range.secondary();
        self.config[PCI_TYPE1_SUBORDINATE_BUS_OFFSET] = range.subordinate();
        self.config[PCI_TYPE1_SECONDARY_LATENCY_OFFSET] = data[3];
        Ok(())
    }

    fn write_bus_byte(&mut self, offset: PciConfigOffset, value: u8) -> Result<(), PciError> {
        let mut next = self.bus_range();
        match offset.as_usize() {
            PCI_TYPE1_PRIMARY_BUS_OFFSET => next.primary = value,
            PCI_TYPE1_SECONDARY_BUS_OFFSET => next.secondary = value,
            PCI_TYPE1_SUBORDINATE_BUS_OFFSET => next.subordinate = value,
            PCI_TYPE1_SECONDARY_LATENCY_OFFSET => {
                self.config[PCI_TYPE1_SECONDARY_LATENCY_OFFSET] = value;
                return Ok(());
            }
            _ => unreachable!("validated type-1 bus offset"),
        }
        let next = PciBridgeBusRange::new(next.primary, next.secondary, next.subordinate)?;
        self.config[PCI_TYPE1_PRIMARY_BUS_OFFSET] = next.primary();
        self.config[PCI_TYPE1_SECONDARY_BUS_OFFSET] = next.secondary();
        self.config[PCI_TYPE1_SUBORDINATE_BUS_OFFSET] = next.subordinate();
        Ok(())
    }

    fn write_bar(&mut self, index: PciBarIndex, value: u32) -> Result<(), PciError> {
        if let Some(owner) = self.bars[index.as_usize()]
            .as_ref()
            .and_then(PciBarState::owner)
        {
            let bar = self.bars[owner.as_usize()]
                .as_mut()
                .ok_or(PciError::MissingBar { index: owner })?;
            bar.write_upper(value);
            write_u32_at(
                &mut self.config,
                index.config_offset(),
                bar.upper_raw().expect("64-bit PCI bridge BAR upper value"),
            );
            return Ok(());
        }

        let Some(bar) = self.bars[index.as_usize()].as_mut() else {
            write_u32_at(&mut self.config, index.config_offset(), 0);
            return Ok(());
        };
        bar.write_lower(value);
        write_u32_at(
            &mut self.config,
            index.config_offset(),
            bar.raw().expect("PCI bridge BAR raw value"),
        );
        Ok(())
    }

    fn validate_bar_slot_free(&self, index: PciBarIndex) -> Result<(), PciError> {
        match self.bars[index.as_usize()].as_ref() {
            None => Ok(()),
            Some(PciBarState::Endpoint { .. }) => Err(PciError::DuplicateBar { index }),
            Some(PciBarState::Upper { owner }) => Err(PciError::ReservedBar {
                index,
                owner: *owner,
            }),
        }
    }

    fn validate_type1_bar_index(&self, index: PciBarIndex) -> Result<(), PciError> {
        if index.as_usize() >= PCI_TYPE1_BAR_COUNT {
            return Err(PciError::InvalidBridgeBarIndex { index });
        }
        Ok(())
    }

    fn bar_enabled(&self, kind: PciBarKind) -> bool {
        let command = self.read_u16(PCI_COMMAND_OFFSET);
        match kind {
            PciBarKind::Memory32 { .. } | PciBarKind::Memory64 { .. } => {
                command & PCI_COMMAND_MEMORY_SPACE != 0
            }
            PciBarKind::LegacyIo { .. } | PciBarKind::Io => command & PCI_COMMAND_IO_SPACE != 0,
        }
    }

    fn window_for_kind(&self, kind: PciBarKind) -> Option<AddressRange> {
        match bridge_space(kind) {
            PciHostAddressSpace::Memory => self.memory_window(),
            PciHostAddressSpace::PrefetchableMemory => self.prefetchable_memory_window(),
            PciHostAddressSpace::Io => self.io_window(),
        }
    }

    fn memory_window(&self) -> Option<AddressRange> {
        let base = self.read_u16(PCI_TYPE1_MEMORY_BASE_OFFSET);
        let limit = self.read_u16(PCI_TYPE1_MEMORY_LIMIT_OFFSET);
        bridge_window(
            u64::from(base & 0xfff0) << 16,
            (u64::from(limit & 0xfff0) << 16) | (PCI_BRIDGE_MEMORY_GRANULARITY - 1),
        )
    }

    fn prefetchable_memory_window(&self) -> Option<AddressRange> {
        let base_lower = u64::from(self.read_u16(PCI_TYPE1_PREFETCH_BASE_OFFSET) & 0xfff0) << 16;
        let limit_lower = (u64::from(self.read_u16(PCI_TYPE1_PREFETCH_LIMIT_OFFSET) & 0xfff0)
            << 16)
            | (PCI_BRIDGE_MEMORY_GRANULARITY - 1);
        let base =
            (u64::from(self.read_u32(PCI_TYPE1_PREFETCH_BASE_UPPER_OFFSET)) << 32) | base_lower;
        let limit =
            (u64::from(self.read_u32(PCI_TYPE1_PREFETCH_LIMIT_UPPER_OFFSET)) << 32) | limit_lower;
        bridge_window(base, limit)
    }

    fn io_window(&self) -> Option<AddressRange> {
        let base = ((u64::from(self.read_u16(PCI_TYPE1_IO_BASE_UPPER_OFFSET)) << 16)
            | u64::from(self.config[PCI_TYPE1_IO_BASE_OFFSET] & 0xf0))
            << 8;
        let limit = (((u64::from(self.read_u16(PCI_TYPE1_IO_LIMIT_UPPER_OFFSET)) << 16)
            | u64::from(self.config[PCI_TYPE1_IO_LIMIT_OFFSET] & 0xf0))
            << 8)
            | (PCI_BRIDGE_IO_GRANULARITY - 1);
        bridge_window(base, limit)
    }

    fn read_u16(&self, offset: usize) -> u16 {
        u16::from_le_bytes(self.config[offset..offset + 2].try_into().unwrap())
    }

    fn read_u32(&self, offset: usize) -> u32 {
        u32::from_le_bytes(self.config[offset..offset + 4].try_into().unwrap())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciBridgeConfigSnapshot {
    function: PciFunctionAddress,
    identity: PciDeviceIdentity,
    class: PciClassCode,
    config: [u8; PCI_CONFIG_SPACE_SIZE],
    bars: [Option<PciBarState>; PCI_TYPE1_BAR_COUNT],
}

impl PciBridgeConfigSnapshot {
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

fn type1_bar_index_for_offset(offset: PciConfigOffset) -> Option<PciBarIndex> {
    match offset.as_usize() {
        PCI_TYPE1_BAR0_OFFSET => Some(PciBarIndex::new(0).expect("type-1 BAR0 index")),
        PCI_TYPE1_BAR1_OFFSET => Some(PciBarIndex::new(1).expect("type-1 BAR1 index")),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConfigSpan {
    start: usize,
    end: usize,
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

fn bridge_window(base: u64, inclusive_limit: u64) -> Option<AddressRange> {
    if inclusive_limit < base {
        return None;
    }
    let size = inclusive_limit.checked_sub(base)?.checked_add(1)?;
    AddressRange::new(Address::new(base), AccessSize::new(size).ok()?).ok()
}

const fn bridge_space(kind: PciBarKind) -> PciHostAddressSpace {
    match kind {
        PciBarKind::Memory32 { prefetchable: true }
        | PciBarKind::Memory64 { prefetchable: true } => PciHostAddressSpace::PrefetchableMemory,
        PciBarKind::Memory32 {
            prefetchable: false,
        }
        | PciBarKind::Memory64 {
            prefetchable: false,
        } => PciHostAddressSpace::Memory,
        PciBarKind::LegacyIo { .. } => PciHostAddressSpace::Io,
        PciBarKind::Io => PciHostAddressSpace::Io,
    }
}

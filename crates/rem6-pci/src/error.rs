use std::error::Error;
use std::fmt;

use rem6_memory::{AccessSize, Address, AddressRange, MemoryError};

use crate::{
    PciBarIndex, PciBarKind, PciClassCode, PciConfigOffset, PciDeviceIdentity, PciFunctionAddress,
    PciInterruptPin, PciMsiMessage,
};

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
    HostAddressOverflow {
        base: Address,
        offset: Address,
    },
    OverlappingHostBarRange {
        existing_function: PciFunctionAddress,
        existing_bar: PciBarIndex,
        requested_function: PciFunctionAddress,
        requested_bar: PciBarIndex,
    },
    HostBarRangeNotForwarded {
        function: PciFunctionAddress,
        bar: PciBarIndex,
    },
    OverlappingCapability {
        existing_offset: PciConfigOffset,
        existing_size: AccessSize,
        requested_offset: PciConfigOffset,
        requested_size: AccessSize,
    },
    InvalidRawCapabilityOffset {
        offset: PciConfigOffset,
        size: AccessSize,
    },
    InvalidRawCapabilitySize {
        offset: PciConfigOffset,
        size: AccessSize,
    },
    SnapshotRawCapabilityMismatch,
    InvalidPowerManagementCapabilityOffset {
        offset: PciConfigOffset,
        size: AccessSize,
    },
    DuplicatePowerManagementCapability,
    SnapshotPowerManagementCapabilityMismatch,
    ReadOnlyPowerManagementCapabilityWrite {
        offset: PciConfigOffset,
        size: AccessSize,
    },
    UnalignedPowerManagementCapabilityWrite {
        offset: PciConfigOffset,
        size: AccessSize,
    },
    InvalidPciExpressCapabilityOffset {
        offset: PciConfigOffset,
        size: AccessSize,
    },
    DuplicatePciExpressCapability,
    SnapshotPciExpressCapabilityMismatch,
    ReadOnlyPciExpressCapabilityWrite {
        offset: PciConfigOffset,
        size: AccessSize,
    },
    UnalignedPciExpressCapabilityWrite {
        offset: PciConfigOffset,
        size: AccessSize,
    },
    InvalidBridgeBusRange {
        primary: u8,
        secondary: u8,
        subordinate: u8,
    },
    BridgePrimaryBusMismatch {
        function: PciFunctionAddress,
        primary: u8,
    },
    BridgeBusRangeOutsideAperture {
        secondary: u8,
        subordinate: u8,
        bus_count: u8,
    },
    InvalidBarPair {
        index: PciBarIndex,
    },
    InvalidBridgeBarIndex {
        index: PciBarIndex,
    },
    ReservedBar {
        index: PciBarIndex,
        owner: PciBarIndex,
    },
    UpperBarRange,
    ZeroLegacyInterruptLines,
    MissingLegacyInterruptPin {
        function: PciFunctionAddress,
    },
    LegacyInterruptLineOverflow {
        base: rem6_interrupt::InterruptLineId,
        index: u64,
    },
    DuplicateLegacyInterruptRoutingEntry {
        function: PciFunctionAddress,
        pin: PciInterruptPin,
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
    SnapshotMsiCapabilityMismatch,
    InvalidMsiCapabilityOffset {
        offset: PciConfigOffset,
        size: AccessSize,
    },
    InvalidMsiVectorCount {
        count: u8,
    },
    DuplicateMsiCapability,
    MissingMsiCapability {
        function: PciFunctionAddress,
    },
    InvalidMsiVector {
        vector: u8,
        vector_count: u8,
    },
    ReadOnlyMsiCapabilityWrite {
        offset: PciConfigOffset,
        size: AccessSize,
    },
    UnalignedMsiCapabilityWrite {
        offset: PciConfigOffset,
        size: AccessSize,
    },
    MsiEndpointMismatch {
        expected: PciFunctionAddress,
        actual: PciFunctionAddress,
    },
    MsiMessageMismatch {
        expected: PciMsiMessage,
        actual: PciMsiMessage,
    },
    SnapshotMsixCapabilityMismatch,
    InvalidMsixCapabilityOffset {
        offset: PciConfigOffset,
        size: AccessSize,
    },
    InvalidMsixVectorCount {
        count: u16,
    },
    OverlappingMsixRegions {
        table_bar: PciBarIndex,
        pba_bar: PciBarIndex,
    },
    DuplicateMsixCapability,
    MissingMsixCapability {
        function: PciFunctionAddress,
    },
    InvalidMsixVector {
        vector: u16,
        vector_count: u16,
    },
    MsixRegionAccessOutsideTable {
        address: Address,
        size: AccessSize,
    },
    ReadOnlyMsixPbaWrite {
        address: Address,
        size: AccessSize,
    },
    UnalignedMsixRegionAccess {
        address: Address,
        size: AccessSize,
    },
    MsixEndpointMismatch {
        expected: PciFunctionAddress,
        actual: PciFunctionAddress,
    },
    MsixMessageMismatch {
        expected: PciMsiMessage,
        actual: PciMsiMessage,
    },
    Interrupt(rem6_interrupt::InterruptError),
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
            Self::HostAddressOverflow { base, offset } => write!(
                f,
                "PCI host address base {:#x} plus PCI offset {:#x} overflows",
                base.get(),
                offset.get()
            ),
            Self::OverlappingHostBarRange {
                existing_function,
                existing_bar,
                requested_function,
                requested_bar,
            } => write!(
                f,
                "PCI host BAR range for {:?} BAR {} overlaps {:?} BAR {}",
                requested_function,
                requested_bar.get(),
                existing_function,
                existing_bar.get()
            ),
            Self::HostBarRangeNotForwarded { function, bar } => write!(
                f,
                "PCI host BAR range for {:?} BAR {} is not currently forwarded",
                function,
                bar.get()
            ),
            Self::OverlappingCapability {
                existing_offset,
                existing_size,
                requested_offset,
                requested_size,
            } => write!(
                f,
                "PCI capability at {:#x} for {} bytes overlaps capability at {:#x} for {} bytes",
                requested_offset.get(),
                requested_size.bytes(),
                existing_offset.get(),
                existing_size.bytes()
            ),
            Self::InvalidRawCapabilityOffset { offset, size } => write!(
                f,
                "PCI raw capability at {:#x} for {} bytes does not fit the writable capability area",
                offset.get(),
                size.bytes()
            ),
            Self::InvalidRawCapabilitySize { offset, size } => write!(
                f,
                "PCI raw capability at {:#x} has invalid size {}",
                offset.get(),
                size.bytes()
            ),
            Self::SnapshotRawCapabilityMismatch => {
                write!(f, "PCI snapshot raw capabilities do not match this endpoint")
            }
            Self::InvalidPowerManagementCapabilityOffset { offset, size } => write!(
                f,
                "PCI power-management capability at {:#x} for {} bytes does not fit the writable capability area",
                offset.get(),
                size.bytes()
            ),
            Self::DuplicatePowerManagementCapability => {
                write!(f, "PCI power-management capability is already installed")
            }
            Self::SnapshotPowerManagementCapabilityMismatch => write!(
                f,
                "PCI snapshot power-management capability does not match this endpoint"
            ),
            Self::ReadOnlyPowerManagementCapabilityWrite { offset, size } => write!(
                f,
                "PCI power-management capability write at {:#x} for {} bytes targets read-only state",
                offset.get(),
                size.bytes()
            ),
            Self::UnalignedPowerManagementCapabilityWrite { offset, size } => write!(
                f,
                "PCI power-management capability write at {:#x} for {} bytes targets an unsupported field width",
                offset.get(),
                size.bytes()
            ),
            Self::InvalidPciExpressCapabilityOffset { offset, size } => write!(
                f,
                "PCI Express capability at {:#x} for {} bytes does not fit the writable capability area",
                offset.get(),
                size.bytes()
            ),
            Self::DuplicatePciExpressCapability => {
                write!(f, "PCI Express capability is already installed")
            }
            Self::SnapshotPciExpressCapabilityMismatch => {
                write!(f, "PCI snapshot PCI Express capability does not match this endpoint")
            }
            Self::ReadOnlyPciExpressCapabilityWrite { offset, size } => write!(
                f,
                "PCI Express capability write at {:#x} for {} bytes targets read-only state",
                offset.get(),
                size.bytes()
            ),
            Self::UnalignedPciExpressCapabilityWrite { offset, size } => write!(
                f,
                "PCI Express capability write at {:#x} for {} bytes targets an unsupported field width",
                offset.get(),
                size.bytes()
            ),
            Self::InvalidBridgeBusRange {
                primary,
                secondary,
                subordinate,
            } => write!(
                f,
                "PCI bridge bus range primary {} secondary {} subordinate {} is invalid",
                primary, secondary, subordinate
            ),
            Self::BridgePrimaryBusMismatch { function, primary } => write!(
                f,
                "PCI bridge {:?} primary bus {} does not match its config bus",
                function, primary
            ),
            Self::BridgeBusRangeOutsideAperture {
                secondary,
                subordinate,
                bus_count,
            } => write!(
                f,
                "PCI bridge secondary bus {} subordinate bus {} exceed aperture bus count {}",
                secondary, subordinate, bus_count
            ),
            Self::InvalidBarPair { index } => {
                write!(f, "PCI BAR {} cannot start a 64-bit BAR pair", index.get())
            }
            Self::InvalidBridgeBarIndex { index } => {
                write!(
                    f,
                    "PCI bridge BAR {} is outside type-1 BAR0..BAR1",
                    index.get()
                )
            }
            Self::ReservedBar { index, owner } => write!(
                f,
                "PCI BAR {} is reserved as the upper half of BAR {}",
                index.get(),
                owner.get()
            ),
            Self::UpperBarRange => write!(f, "PCI upper BAR slots do not expose ranges"),
            Self::ZeroLegacyInterruptLines => {
                write!(f, "PCI legacy interrupt line count must be positive")
            }
            Self::MissingLegacyInterruptPin { function } => {
                write!(f, "PCI function {:?} has no legacy interrupt pin", function)
            }
            Self::LegacyInterruptLineOverflow { base, index } => write!(
                f,
                "PCI legacy interrupt line base {} plus index {} overflows",
                base.get(),
                index
            ),
            Self::DuplicateLegacyInterruptRoutingEntry { function, pin } => write!(
                f,
                "PCI legacy interrupt routing entry for function {:?} pin {:?} already exists",
                function, pin
            ),
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
            Self::SnapshotMsiCapabilityMismatch => {
                write!(f, "PCI snapshot MSI capability does not match this endpoint")
            }
            Self::InvalidMsiCapabilityOffset { offset, size } => write!(
                f,
                "PCI MSI capability at {:#x} for {} bytes does not fit the writable capability area",
                offset.get(),
                size.bytes()
            ),
            Self::InvalidMsiVectorCount { count } => {
                write!(f, "PCI MSI vector count {count} is not a power of two in 1..=32")
            }
            Self::DuplicateMsiCapability => write!(f, "PCI MSI capability is already installed"),
            Self::MissingMsiCapability { function } => {
                write!(f, "PCI function {:?} has no MSI capability", function)
            }
            Self::InvalidMsiVector {
                vector,
                vector_count,
            } => write!(
                f,
                "PCI MSI vector {} is outside configured vector count {}",
                vector, vector_count
            ),
            Self::ReadOnlyMsiCapabilityWrite { offset, size } => write!(
                f,
                "PCI MSI capability write at {:#x} for {} bytes targets read-only state",
                offset.get(),
                size.bytes()
            ),
            Self::UnalignedMsiCapabilityWrite { offset, size } => write!(
                f,
                "PCI MSI capability write at {:#x} for {} bytes targets an unsupported field width",
                offset.get(),
                size.bytes()
            ),
            Self::MsiEndpointMismatch { expected, actual } => write!(
                f,
                "PCI MSI endpoint mismatch: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::MsiMessageMismatch { expected, actual } => write!(
                f,
                "PCI MSI message mismatch: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::SnapshotMsixCapabilityMismatch => {
                write!(
                    f,
                    "PCI snapshot MSI-X capability does not match this endpoint"
                )
            }
            Self::InvalidMsixCapabilityOffset { offset, size } => write!(
                f,
                "PCI MSI-X capability at {:#x} for {} bytes does not fit the writable capability area",
                offset.get(),
                size.bytes()
            ),
            Self::InvalidMsixVectorCount { count } => {
                write!(f, "PCI MSI-X vector count {count} is outside 1..=2048")
            }
            Self::OverlappingMsixRegions { table_bar, pba_bar } => write!(
                f,
                "PCI MSI-X table BAR {} overlaps PBA BAR {}",
                table_bar.get(),
                pba_bar.get()
            ),
            Self::DuplicateMsixCapability => {
                write!(f, "PCI MSI-X capability is already installed")
            }
            Self::MissingMsixCapability { function } => {
                write!(f, "PCI function {:?} has no MSI-X capability", function)
            }
            Self::InvalidMsixVector {
                vector,
                vector_count,
            } => write!(
                f,
                "PCI MSI-X vector {} is outside configured vector count {}",
                vector, vector_count
            ),
            Self::MsixRegionAccessOutsideTable { address, size } => write!(
                f,
                "PCI MSI-X region access at {:#x} for {} bytes is outside table and PBA ranges",
                address.get(),
                size.bytes()
            ),
            Self::ReadOnlyMsixPbaWrite { address, size } => write!(
                f,
                "PCI MSI-X PBA write at {:#x} for {} bytes targets read-only state",
                address.get(),
                size.bytes()
            ),
            Self::UnalignedMsixRegionAccess { address, size } => write!(
                f,
                "PCI MSI-X region access at {:#x} for {} bytes has an unsupported alignment or size",
                address.get(),
                size.bytes()
            ),
            Self::MsixEndpointMismatch { expected, actual } => write!(
                f,
                "PCI MSI-X endpoint mismatch: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::MsixMessageMismatch { expected, actual } => write!(
                f,
                "PCI MSI-X message mismatch: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::Interrupt(error) => write!(f, "{error}"),
            Self::Memory(error) => write!(f, "{error}"),
        }
    }
}

impl Error for PciError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Interrupt(error) => Some(error),
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

impl From<rem6_interrupt::InterruptError> for PciError {
    fn from(value: rem6_interrupt::InterruptError) -> Self {
        Self::Interrupt(value)
    }
}

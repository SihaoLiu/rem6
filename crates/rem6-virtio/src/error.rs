use std::error::Error;
use std::fmt;

use rem6_memory::Address;
use rem6_mmio::{MmioError, MmioRequestId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VirtioError {
    TooManyQueues {
        count: usize,
    },
    InvalidQueueSize {
        index: u16,
        size: u16,
    },
    InvalidQueueRuntimeSize {
        index: u16,
        size: u16,
        max_size: u16,
    },
    EnabledQueueConfigWrite {
        index: u16,
    },
    InvalidQueueEnable {
        value: u16,
    },
    InvalidQueueReset {
        value: u16,
    },
    UnavailableQueue {
        index: u16,
    },
    InvalidNotifyMultiplier {
        multiplier: u32,
    },
    NoNotifyQueues,
    DuplicateNotifyQueue {
        index: u16,
    },
    NotifyAddressOverflow {
        queue: u16,
        notify_offset: u16,
        notify_off_multiplier: u32,
    },
    PartialNotifyWrite,
    NoQueueForNotifyAddress {
        address: Address,
    },
    NotifyValueMismatch {
        address: Address,
        value: u16,
    },
    EmptyDeviceConfig,
    DeviceConfigWritableMaskSizeMismatch {
        bytes: u64,
        mask: u64,
    },
    ReadOnlyDeviceConfigWrite {
        offset: u64,
    },
    InvalidBlockCapacity,
    InvalidBlockSizeMax {
        size_max: u32,
    },
    InvalidBlockSegMax {
        seg_max: u32,
    },
    InvalidBlockSize {
        block_size: u32,
    },
    InvalidBlockQueueCount {
        queues: u16,
    },
    BlockWritebackRequiresFlush,
    InvalidBlockBackendSize {
        bytes: u64,
    },
    BlockBackendCapacityMismatch {
        config_sectors: u64,
        backend_sectors: u64,
    },
    BlockStorageBackend {
        message: String,
    },
    InvalidBlockRequestDataLength {
        raw_type: u32,
        bytes: u64,
    },
    BlockRequestAddressOverflow {
        sector: u64,
        bytes: u64,
    },
    BlockRequestOutOfRange {
        sector: u64,
        bytes: u64,
        capacity_sectors: u64,
    },
    InvalidBlockDeviceId {
        bytes: usize,
    },
    DuplicateVirtioDescriptor {
        index: u16,
    },
    MissingVirtioDescriptor {
        index: u16,
    },
    VirtioDescriptorLoop {
        index: u16,
    },
    ShortVirtioBlockHeader {
        bytes: u64,
    },
    MissingVirtioBlockStatusDescriptor,
    InvalidVirtioBlockStatusDescriptor {
        index: u16,
        length: u32,
        writable: bool,
    },
    InvalidVirtioBlockReadableDescriptor {
        raw_type: u32,
        index: u16,
    },
    InvalidVirtioBlockWritableDescriptor {
        raw_type: u32,
        index: u16,
    },
    InvalidVirtioBlockDeviceIdOutput {
        bytes: u64,
    },
    VirtioBlockPayloadLengthOverflow {
        raw_type: u32,
    },
    InvalidBlockGeometry {
        cylinders: u16,
        heads: u8,
        sectors: u8,
    },
    InvalidBlockTopology {
        physical_block_exp: u8,
        alignment_offset: u8,
        min_io_size: u16,
        opt_io_size: u32,
    },
    InvalidBlockDiscardLimits {
        max_sectors: u32,
        max_segments: u32,
        sector_alignment: u32,
    },
    InvalidBlockWriteZeroesLimits {
        max_sectors: u32,
        max_segments: u32,
    },
    InvalidBlockSecureEraseLimits {
        max_sectors: u32,
        max_segments: u32,
        sector_alignment: u32,
    },
    ZeroPciCapabilityRegion {
        cfg_type: u8,
    },
    PciCapabilityOutOfConfig {
        offset: u16,
        length: u64,
    },
    InvalidNotifyCapabilityKind {
        cfg_type: u8,
    },
    ZeroSharedMemoryRegion {
        id: u8,
    },
    DuplicateSharedMemoryBar {
        bar: u8,
    },
    MissingSharedMemoryBar {
        id: u8,
        bar: u8,
    },
    SharedMemoryRegionAddressOverflow {
        id: u8,
        bar: u8,
        offset: u64,
        length: u64,
    },
    SharedMemoryCapabilityConfigBufferTooSmall {
        bytes: usize,
        required: usize,
    },
    SharedMemoryRegionOutOfBar {
        id: u8,
        bar: u8,
        offset: u64,
        length: u64,
        bar_length: u64,
    },
    DuplicateSharedMemoryId {
        id: u8,
    },
    OverlappingSharedMemoryRegion {
        first: u8,
        second: u8,
        bar: u8,
    },
    DuplicatePciTransportBar {
        bar: u8,
    },
    MissingPciTransportBar {
        cfg_type: u8,
        bar: u8,
    },
    PciTransportRegionAddressOverflow {
        cfg_type: u8,
        bar: u8,
        offset: u64,
        length: u64,
    },
    PciTransportRegionOutOfBar {
        cfg_type: u8,
        bar: u8,
        offset: u64,
        length: u64,
        bar_length: u64,
    },
    PciTransportRegionOffsetTooLarge {
        cfg_type: u8,
        bar: u8,
        offset: u64,
    },
    PciTransportDeviceRegionTooSmall {
        cfg_type: u8,
        bar: u8,
        declared_length: u64,
        device_length: u64,
    },
    OverlappingPciTransportRegion {
        first_cfg_type: u8,
        second_cfg_type: u8,
        bar: u8,
    },
    OverlappingPciTransportRuntimeDevice {
        first_cfg_type: u8,
        second_cfg_type: u8,
        bar: u8,
    },
    MissingPciTransportDeviceConfigDevice,
    UnexpectedPciTransportDeviceConfigDevice,
    PciTransportRuntimeConfig {
        message: String,
    },
    PciEndpointConfig {
        message: String,
    },
    InvalidPciIsrSnapshot,
}

impl VirtioError {
    pub(crate) fn memory(error: rem6_memory::MemoryError) -> Self {
        Self::PciTransportRuntimeConfig {
            message: error.to_string(),
        }
    }

    pub(crate) fn pci_endpoint(error: rem6_pci::PciError) -> Self {
        Self::PciEndpointConfig {
            message: error.to_string(),
        }
    }
}

impl fmt::Display for VirtioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyQueues { count } => {
                write!(formatter, "VirtIO common config has too many queues: {count}")
            }
            Self::InvalidQueueSize { index, size } => write!(
                formatter,
                "VirtIO queue {index} size {size} must be a nonzero power of two"
            ),
            Self::InvalidQueueRuntimeSize {
                index,
                size,
                max_size,
            } => write!(
                formatter,
                "VirtIO queue {index} size {size} must be a nonzero power of two no larger than {max_size}"
            ),
            Self::EnabledQueueConfigWrite { index } => {
                write!(formatter, "VirtIO queue {index} cannot be reconfigured while enabled")
            }
            Self::InvalidQueueEnable { value } => {
                write!(formatter, "VirtIO queue_enable write value {value} must be 1")
            }
            Self::InvalidQueueReset { value } => {
                write!(formatter, "VirtIO queue_reset write value {value} must be 0 or 1")
            }
            Self::UnavailableQueue { index } => {
                write!(formatter, "VirtIO selected unavailable queue {index}")
            }
            Self::InvalidNotifyMultiplier { multiplier } => write!(
                formatter,
                "VirtIO notify_off_multiplier {multiplier} must be 0 or an even power of two"
            ),
            Self::NoNotifyQueues => {
                write!(formatter, "VirtIO notify device must expose at least one queue")
            }
            Self::DuplicateNotifyQueue { index } => {
                write!(formatter, "VirtIO notify queue {index} is declared more than once")
            }
            Self::NotifyAddressOverflow {
                queue,
                notify_offset,
                notify_off_multiplier,
            } => write!(
                formatter,
                "VirtIO notify queue {queue} offset {notify_offset} overflows with notify_off_multiplier {notify_off_multiplier}"
            ),
            Self::PartialNotifyWrite => {
                write!(formatter, "VirtIO notify writes require a full 16-bit byte mask")
            }
            Self::NoQueueForNotifyAddress { address } => write!(
                formatter,
                "VirtIO notify address {:#x} has no queue",
                address.get()
            ),
            Self::NotifyValueMismatch { address, value } => write!(
                formatter,
                "VirtIO notify value {value} does not match any queue at address {:#x}",
                address.get()
            ),
            Self::EmptyDeviceConfig => {
                write!(formatter, "VirtIO device config must contain at least one byte")
            }
            Self::DeviceConfigWritableMaskSizeMismatch { bytes, mask } => write!(
                formatter,
                "VirtIO device config writable mask has {mask} bytes for {bytes} config bytes"
            ),
            Self::ReadOnlyDeviceConfigWrite { offset } => {
                write!(formatter, "VirtIO device config byte {offset} is read-only")
            }
            Self::InvalidBlockCapacity => {
                write!(formatter, "VirtIO block capacity must be nonzero")
            }
            Self::InvalidBlockSizeMax { size_max } => write!(
                formatter,
                "VirtIO block size_max {size_max} must be nonzero when offered"
            ),
            Self::InvalidBlockSegMax { seg_max } => write!(
                formatter,
                "VirtIO block seg_max {seg_max} must be nonzero when offered"
            ),
            Self::InvalidBlockSize { block_size } => write!(
                formatter,
                "VirtIO block size {block_size} must be a power of two at least 512 bytes"
            ),
            Self::InvalidBlockQueueCount { queues } => write!(
                formatter,
                "VirtIO block queue count {queues} must be nonzero when multiqueue is offered"
            ),
            Self::BlockWritebackRequiresFlush => write!(
                formatter,
                "VirtIO block writeback configuration requires the flush feature"
            ),
            Self::InvalidBlockBackendSize { bytes } => write!(
                formatter,
                "VirtIO block backend image has {bytes} bytes and must contain a nonzero number of 512-byte sectors"
            ),
            Self::BlockBackendCapacityMismatch {
                config_sectors,
                backend_sectors,
            } => write!(
                formatter,
                "VirtIO block config capacity {config_sectors} sectors does not match backend capacity {backend_sectors} sectors"
            ),
            Self::BlockStorageBackend { message } => {
                write!(formatter, "VirtIO block storage backend failed: {message}")
            }
            Self::InvalidBlockRequestDataLength { raw_type, bytes } => write!(
                formatter,
                "VirtIO block request type {raw_type} data length {bytes} must be a nonzero multiple of 512 bytes"
            ),
            Self::BlockRequestAddressOverflow { sector, bytes } => write!(
                formatter,
                "VirtIO block request at sector {sector} overflows with {bytes} bytes"
            ),
            Self::BlockRequestOutOfRange {
                sector,
                bytes,
                capacity_sectors,
            } => write!(
                formatter,
                "VirtIO block request at sector {sector} for {bytes} bytes exceeds capacity {capacity_sectors} sectors"
            ),
            Self::InvalidBlockDeviceId { bytes } => write!(
                formatter,
                "VirtIO block device id has {bytes} bytes but at most 20 bytes are allowed"
            ),
            Self::DuplicateVirtioDescriptor { index } => write!(
                formatter,
                "VirtIO split descriptor {index} is declared more than once"
            ),
            Self::MissingVirtioDescriptor { index } => write!(
                formatter,
                "VirtIO split descriptor chain references missing descriptor {index}"
            ),
            Self::VirtioDescriptorLoop { index } => write!(
                formatter,
                "VirtIO split descriptor chain contains a loop at descriptor {index}"
            ),
            Self::ShortVirtioBlockHeader { bytes } => write!(
                formatter,
                "VirtIO block descriptor chain header has {bytes} bytes but requires 16 bytes"
            ),
            Self::MissingVirtioBlockStatusDescriptor => write!(
                formatter,
                "VirtIO block descriptor chain is missing a writable status descriptor"
            ),
            Self::InvalidVirtioBlockStatusDescriptor {
                index,
                length,
                writable,
            } => write!(
                formatter,
                "VirtIO block status descriptor {index} must be writable and at least 1 byte, got length {length} writable {writable}"
            ),
            Self::InvalidVirtioBlockReadableDescriptor { raw_type, index } => write!(
                formatter,
                "VirtIO block request type {raw_type} descriptor {index} must be device-readable"
            ),
            Self::InvalidVirtioBlockWritableDescriptor { raw_type, index } => write!(
                formatter,
                "VirtIO block request type {raw_type} descriptor {index} must be device-writable"
            ),
            Self::InvalidVirtioBlockDeviceIdOutput { bytes } => write!(
                formatter,
                "VirtIO block get-id output has {bytes} writable bytes but device id requires 20 bytes"
            ),
            Self::VirtioBlockPayloadLengthOverflow { raw_type } => write!(
                formatter,
                "VirtIO block request type {raw_type} descriptor payload length overflows"
            ),
            Self::InvalidBlockGeometry {
                cylinders,
                heads,
                sectors,
            } => write!(
                formatter,
                "VirtIO block geometry is invalid: cylinders {cylinders}, heads {heads}, sectors {sectors}"
            ),
            Self::InvalidBlockTopology {
                physical_block_exp,
                alignment_offset,
                min_io_size,
                opt_io_size,
            } => write!(
                formatter,
                "VirtIO block topology is invalid: physical_block_exp {physical_block_exp}, alignment_offset {alignment_offset}, min_io_size {min_io_size}, opt_io_size {opt_io_size}"
            ),
            Self::InvalidBlockDiscardLimits {
                max_sectors,
                max_segments,
                sector_alignment,
            } => write!(
                formatter,
                "VirtIO block discard limits are invalid: max_sectors {max_sectors}, max_segments {max_segments}, sector_alignment {sector_alignment}"
            ),
            Self::InvalidBlockWriteZeroesLimits {
                max_sectors,
                max_segments,
            } => write!(
                formatter,
                "VirtIO block write zeroes limits are invalid: max_sectors {max_sectors}, max_segments {max_segments}"
            ),
            Self::InvalidBlockSecureEraseLimits {
                max_sectors,
                max_segments,
                sector_alignment,
            } => write!(
                formatter,
                "VirtIO block secure erase limits are invalid: max_sectors {max_sectors}, max_segments {max_segments}, sector_alignment {sector_alignment}"
            ),
            Self::ZeroPciCapabilityRegion { cfg_type } => {
                write!(formatter, "VirtIO PCI capability type {cfg_type} has zero length")
            }
            Self::PciCapabilityOutOfConfig { offset, length } => write!(
                formatter,
                "VirtIO PCI capability at {offset:#x} for {length} bytes exceeds configuration space"
            ),
            Self::InvalidNotifyCapabilityKind { cfg_type } => write!(
                formatter,
                "VirtIO notify PCI capability requires cfg_type 2, got {cfg_type}"
            ),
            Self::ZeroSharedMemoryRegion { id } => {
                write!(formatter, "VirtIO shared memory region id {id} has zero length")
            }
            Self::DuplicateSharedMemoryBar { bar } => {
                write!(
                    formatter,
                    "VirtIO shared memory BAR {bar} is declared more than once"
                )
            }
            Self::MissingSharedMemoryBar { id, bar } => write!(
                formatter,
                "VirtIO shared memory region id {id} references undeclared BAR {bar}"
            ),
            Self::SharedMemoryRegionAddressOverflow {
                id,
                bar,
                offset,
                length,
            } => write!(
                formatter,
                "VirtIO shared memory region id {id} in BAR {bar} offset {offset:#x} overflows with length {length:#x}"
            ),
            Self::SharedMemoryCapabilityConfigBufferTooSmall { bytes, required } => write!(
                formatter,
                "VirtIO shared memory PCI capability configuration buffer has {bytes} bytes but requires {required}"
            ),
            Self::SharedMemoryRegionOutOfBar {
                id,
                bar,
                offset,
                length,
                bar_length,
            } => write!(
                formatter,
                "VirtIO shared memory region id {id} offset {offset:#x} length {length:#x} must be contained within BAR {bar} length {bar_length:#x}"
            ),
            Self::DuplicateSharedMemoryId { id } => {
                write!(
                    formatter,
                    "VirtIO shared memory region id {id} is declared more than once"
                )
            }
            Self::OverlappingSharedMemoryRegion { first, second, bar } => write!(
                formatter,
                "VirtIO shared memory region id {second} overlaps id {first} in BAR {bar}"
            ),
            Self::DuplicatePciTransportBar { bar } => {
                write!(
                    formatter,
                    "VirtIO PCI transport BAR {bar} is declared more than once"
                )
            }
            Self::MissingPciTransportBar { cfg_type, bar } => write!(
                formatter,
                "VirtIO PCI capability type {cfg_type} references undeclared BAR {bar}"
            ),
            Self::PciTransportRegionAddressOverflow {
                cfg_type,
                bar,
                offset,
                length,
            } => write!(
                formatter,
                "VirtIO PCI capability type {cfg_type} in BAR {bar} offset {offset:#x} overflows with length {length:#x}"
            ),
            Self::PciTransportRegionOutOfBar {
                cfg_type,
                bar,
                offset,
                length,
                bar_length,
            } => write!(
                formatter,
                "VirtIO PCI capability type {cfg_type} offset {offset:#x} length {length:#x} must be contained within BAR {bar} length {bar_length:#x}"
            ),
            Self::PciTransportRegionOffsetTooLarge {
                cfg_type,
                bar,
                offset,
            } => write!(
                formatter,
                "VirtIO PCI capability type {cfg_type} in BAR {bar} offset {offset:#x} does not fit the 32-bit PCI capability field"
            ),
            Self::PciTransportDeviceRegionTooSmall {
                cfg_type,
                bar,
                declared_length,
                device_length,
            } => write!(
                formatter,
                "VirtIO PCI capability type {cfg_type} in BAR {bar} device length {device_length:#x} does not fit declared region length {declared_length:#x}"
            ),
            Self::OverlappingPciTransportRegion {
                first_cfg_type,
                second_cfg_type,
                bar,
            } => write!(
                formatter,
                "VirtIO PCI capability type {second_cfg_type} overlaps type {first_cfg_type} in BAR {bar}"
            ),
            Self::OverlappingPciTransportRuntimeDevice {
                first_cfg_type,
                second_cfg_type,
                bar,
            } => write!(
                formatter,
                "VirtIO PCI runtime device type {second_cfg_type} overlaps type {first_cfg_type} in BAR {bar}"
            ),
            Self::MissingPciTransportDeviceConfigDevice => write!(
                formatter,
                "VirtIO PCI transport declares a device-specific config region but no device was provided"
            ),
            Self::UnexpectedPciTransportDeviceConfigDevice => write!(
                formatter,
                "VirtIO PCI transport received a device-specific config device without a declared region"
            ),
            Self::PciTransportRuntimeConfig { message } => {
                write!(
                    formatter,
                    "VirtIO PCI transport runtime configuration failed: {message}"
                )
            }
            Self::PciEndpointConfig { message } => {
                write!(
                    formatter,
                    "VirtIO PCI endpoint configuration failed: {message}"
                )
            }
            Self::InvalidPciIsrSnapshot => {
                write!(formatter, "VirtIO PCI ISR snapshot payload is invalid")
            }
        }
    }
}

impl Error for VirtioError {}

pub(crate) fn virtio_device_error(request: MmioRequestId, error: VirtioError) -> MmioError {
    MmioError::DeviceError {
        request,
        message: error.to_string(),
    }
}

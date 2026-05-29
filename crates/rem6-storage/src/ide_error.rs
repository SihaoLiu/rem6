use std::error::Error;
use std::fmt::{self, Display, Formatter};

use rem6_pci::{PciFunctionAddress, PciInterruptPin};

use crate::ide::{IdeChannelId, IdeControllerBar, IdeDeviceId};
use crate::{IdeSnapshotError, StorageError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdeControllerError {
    InvalidDispatch {
        io_shift: u8,
        control_offset: u64,
    },
    InvalidDeviceSlot {
        slot: usize,
        expected: IdeDeviceId,
        actual: IdeDeviceId,
    },
    InvalidBarOffset {
        bar: IdeControllerBar,
        offset: u64,
        width: u8,
    },
    BarOffsetOverflow {
        bar: IdeControllerBar,
        offset: u64,
        addend: u64,
    },
    UnsupportedBarWidth {
        bar: IdeControllerBar,
        offset: u64,
        width: u8,
    },
    SnapshotChannelMismatch {
        channel: IdeChannelId,
    },
    SnapshotDeviceMismatch {
        channel: IdeChannelId,
        device: IdeDeviceId,
    },
    NoSelectedDevice {
        channel: IdeChannelId,
        device: IdeDeviceId,
    },
    InvalidBmiOffset {
        channel: IdeChannelId,
        offset: u8,
        width: u8,
    },
    DmaNotActive {
        channel: IdeChannelId,
    },
    DmaDirectionMismatch {
        channel: IdeChannelId,
    },
    InvalidPrdByteCount {
        channel: IdeChannelId,
        bytes: u64,
    },
    PrdTableMissingEnd {
        channel: IdeChannelId,
    },
    DmaByteCountMismatch {
        channel: IdeChannelId,
        expected_bytes: u64,
        prd_bytes: u64,
    },
    DmaByteCountOverflow {
        channel: IdeChannelId,
    },
    GuestAddressOverflow {
        address: u64,
        bytes: u64,
    },
    GuestMemory {
        operation: &'static str,
        address: u64,
        bytes: u64,
        capacity_bytes: u64,
    },
    GuestTransferSizeMismatch {
        expected_bytes: u64,
        actual_bytes: u64,
    },
    PciInterruptBindingMismatch {
        expected_function: PciFunctionAddress,
        actual_function: PciFunctionAddress,
        expected_pin: PciInterruptPin,
        actual_pin: PciInterruptPin,
    },
    PciEndpoint {
        source: rem6_pci::PciError,
    },
    DmaUnsupported {
        channel: IdeChannelId,
    },
    Disk {
        channel: IdeChannelId,
        source: IdeDiskError,
    },
}

impl Display for IdeControllerError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDispatch {
                io_shift,
                control_offset,
            } => write!(
                formatter,
                "invalid IDE controller dispatch io_shift {io_shift} control offset {control_offset:#x}"
            ),
            Self::InvalidDeviceSlot {
                slot,
                expected,
                actual,
            } => write!(
                formatter,
                "IDE controller disk slot {slot} expected {expected:?}, got {actual:?}"
            ),
            Self::InvalidBarOffset { bar, offset, width } => write!(
                formatter,
                "invalid IDE controller {bar:?} BAR offset {offset:#x} for {width}-byte access"
            ),
            Self::BarOffsetOverflow {
                bar,
                offset,
                addend,
            } => write!(
                formatter,
                "IDE controller {bar:?} BAR offset {offset:#x} overflows with addend {addend:#x}"
            ),
            Self::UnsupportedBarWidth { bar, offset, width } => write!(
                formatter,
                "IDE controller {bar:?} BAR offset {offset:#x} does not support {width}-byte access"
            ),
            Self::SnapshotChannelMismatch { channel } => write!(
                formatter,
                "IDE controller snapshot does not match {channel:?} channel"
            ),
            Self::SnapshotDeviceMismatch { channel, device } => write!(
                formatter,
                "IDE controller snapshot does not match {channel:?} {device:?}"
            ),
            Self::NoSelectedDevice { channel, device } => write!(
                formatter,
                "IDE controller {channel:?} channel has no selected {device:?} disk"
            ),
            Self::InvalidBmiOffset {
                channel,
                offset,
                width,
            } => write!(
                formatter,
                "invalid IDE controller {channel:?} BMI offset {offset:#x} for {width}-byte access"
            ),
            Self::DmaNotActive { channel } => {
                write!(formatter, "IDE controller {channel:?} channel DMA is not active")
            }
            Self::DmaDirectionMismatch { channel } => write!(
                formatter,
                "IDE controller {channel:?} BMI direction does not match the pending DMA command"
            ),
            Self::InvalidPrdByteCount { channel, bytes } => write!(
                formatter,
                "IDE controller {channel:?} PRD byte count {bytes} is not a supported sector multiple"
            ),
            Self::PrdTableMissingEnd { channel } => write!(
                formatter,
                "IDE controller {channel:?} PRD table has no end marker"
            ),
            Self::DmaByteCountMismatch {
                channel,
                expected_bytes,
                prd_bytes,
            } => write!(
                formatter,
                "IDE controller {channel:?} DMA expected {expected_bytes} bytes but PRDs describe {prd_bytes} bytes"
            ),
            Self::DmaByteCountOverflow { channel } => write!(
                formatter,
                "IDE controller {channel:?} DMA byte count overflows"
            ),
            Self::GuestAddressOverflow { address, bytes } => write!(
                formatter,
                "IDE controller guest address {address:#x} with {bytes} bytes overflows"
            ),
            Self::GuestMemory {
                operation,
                address,
                bytes,
                capacity_bytes,
            } => write!(
                formatter,
                "IDE controller guest memory {operation} at {address:#x} for {bytes} bytes exceeds capacity {capacity_bytes}"
            ),
            Self::GuestTransferSizeMismatch {
                expected_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "IDE controller guest memory returned {actual_bytes} bytes but expected {expected_bytes}"
            ),
            Self::PciInterruptBindingMismatch {
                expected_function,
                actual_function,
                expected_pin,
                actual_pin,
            } => write!(
                formatter,
                "IDE PCI interrupt binding expected {expected_function:?}/{expected_pin:?}, got {actual_function:?}/{actual_pin:?}"
            ),
            Self::PciEndpoint { source } => {
                write!(formatter, "IDE PCI endpoint error: {source}")
            }
            Self::DmaUnsupported { channel } => {
                write!(
                    formatter,
                    "IDE controller {channel:?} channel DMA is not implemented"
                )
            }
            Self::Disk { channel, source } => {
                write!(
                    formatter,
                    "IDE controller {channel:?} disk access failed: {source}"
                )
            }
        }
    }
}

impl Error for IdeControllerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::PciEndpoint { source } => Some(source),
            Self::Disk { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdeDiskError {
    InvalidCapacity {
        sectors: u64,
    },
    SnapshotDeviceMismatch {
        expected: IdeDeviceId,
        actual: IdeDeviceId,
    },
    InvalidTransferSnapshot {
        cursor: usize,
        payload_bytes: usize,
    },
    InvalidCommandOffset {
        offset: u8,
    },
    InvalidControlOffset {
        offset: u8,
    },
    ChsAccessUnsupported {
        command: u8,
        drive: u8,
    },
    UnsupportedCommand {
        command: u8,
    },
    DmaNotReady {
        command: u8,
    },
    DmaDirectionMismatch {
        command: u8,
    },
    DmaByteCountMismatch {
        expected_bytes: u64,
        actual_bytes: u64,
    },
    DataReadNotReady,
    DataWriteNotReady,
    TransferTooLarge {
        sectors: u64,
    },
    Storage(StorageError),
}

impl From<StorageError> for IdeDiskError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl From<IdeSnapshotError> for IdeDiskError {
    fn from(error: IdeSnapshotError) -> Self {
        match error {
            IdeSnapshotError::InvalidTransferSnapshot {
                cursor,
                payload_bytes,
            } => Self::InvalidTransferSnapshot {
                cursor,
                payload_bytes,
            },
        }
    }
}

impl Display for IdeDiskError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCapacity { sectors } => {
                write!(
                    formatter,
                    "IDE disk capacity {sectors} sectors is not supported"
                )
            }
            Self::SnapshotDeviceMismatch { expected, actual } => write!(
                formatter,
                "IDE disk snapshot expected {expected:?} but found {actual:?}"
            ),
            Self::InvalidTransferSnapshot {
                cursor,
                payload_bytes,
            } => write!(
                formatter,
                "IDE disk transfer snapshot cursor {cursor} is invalid for {payload_bytes} payload bytes"
            ),
            Self::InvalidCommandOffset { offset } => {
                write!(formatter, "invalid IDE command register offset {offset:#x}")
            }
            Self::InvalidControlOffset { offset } => {
                write!(formatter, "invalid IDE control register offset {offset:#x}")
            }
            Self::ChsAccessUnsupported { command, drive } => write!(
                formatter,
                "IDE command {command:#x} requested CHS access with drive register {drive:#x}"
            ),
            Self::UnsupportedCommand { command } => {
                write!(formatter, "unsupported IDE command {command:#x}")
            }
            Self::DmaNotReady { command } => {
                write!(formatter, "IDE command {command:#x} has no pending DMA transfer")
            }
            Self::DmaDirectionMismatch { command } => write!(
                formatter,
                "IDE command {command:#x} DMA direction does not match the requested transfer"
            ),
            Self::DmaByteCountMismatch {
                expected_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "IDE DMA expected {expected_bytes} bytes but received {actual_bytes} bytes"
            ),
            Self::DataReadNotReady => {
                write!(formatter, "IDE data register is not ready for read")
            }
            Self::DataWriteNotReady => {
                write!(formatter, "IDE data register is not ready for write")
            }
            Self::TransferTooLarge { sectors } => {
                write!(formatter, "IDE transfer of {sectors} sectors is too large")
            }
            Self::Storage(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for IdeDiskError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            _ => None,
        }
    }
}

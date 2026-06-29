use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AddressRange, MemoryError};

pub(crate) fn invalid_elf(reason: BootElfError) -> BootError {
    BootError::InvalidElf { reason }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootError {
    EmptySegment {
        start: Address,
    },
    InvalidElf {
        reason: BootElfError,
    },
    OverlappingSegment {
        existing: AddressRange,
        requested: AddressRange,
    },
    Memory(MemoryError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootElfError {
    BadMagic,
    NoLoadableSegments,
    SegmentFileRangeOutOfBounds {
        segment: u16,
        offset: u64,
        size: u64,
        image_size: u64,
    },
    SegmentFileSizeExceedsMemorySize {
        segment: u16,
        file_size: u64,
        memory_size: u64,
    },
    SegmentMemorySizeTooLarge {
        segment: u16,
        memory_size: u64,
    },
    SegmentMemoryRangeOverflow {
        segment: u16,
        physical: u64,
        memory_size: u64,
    },
    InterpreterFileRangeOutOfBounds {
        segment: u16,
        offset: u64,
        size: u64,
        image_size: u64,
    },
    InvalidInterpreterPath {
        segment: u16,
    },
    ProgramHeaderTableOutOfBounds {
        offset: u64,
        size: u64,
        image_size: u64,
    },
    SectionHeaderTableOutOfBounds {
        offset: u64,
        size: u64,
        image_size: u64,
    },
    SectionDataRangeOutOfBounds {
        offset: u64,
        size: u64,
        image_size: u64,
    },
    TruncatedField {
        offset: u64,
        size: u64,
        image_size: u64,
    },
    UnsupportedClass {
        class: u8,
    },
    UnsupportedEncoding {
        encoding: u8,
    },
    UnsupportedHeaderSize {
        expected: u16,
        actual: u16,
    },
    UnsupportedProgramHeaderSize {
        expected: u16,
        actual: u16,
    },
    UnsupportedSectionHeaderSize {
        expected: u16,
        actual: u16,
    },
    UnsupportedVersion {
        version: u8,
    },
}

impl fmt::Display for BootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySegment { start } => {
                write!(formatter, "boot segment at {:#x} is empty", start.get())
            }
            Self::InvalidElf { reason } => write!(formatter, "invalid ELF image: {reason}"),
            Self::OverlappingSegment {
                existing,
                requested,
            } => write!(
                formatter,
                "boot segment {:#x}..{:#x} overlaps existing segment {:#x}..{:#x}",
                requested.start().get(),
                requested.end().get(),
                existing.start().get(),
                existing.end().get()
            ),
            Self::Memory(error) => write!(formatter, "{error}"),
        }
    }
}

impl fmt::Display for BootElfError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => write!(formatter, "bad ELF magic"),
            Self::NoLoadableSegments => write!(formatter, "ELF image has no loadable segments"),
            Self::SegmentFileRangeOutOfBounds {
                segment,
                offset,
                size,
                image_size,
            } => write!(
                formatter,
                "ELF segment {segment} file range {offset:#x}+{size:#x} exceeds image size {image_size:#x}"
            ),
            Self::SegmentFileSizeExceedsMemorySize {
                segment,
                file_size,
                memory_size,
            } => write!(
                formatter,
                "ELF segment {segment} file size {file_size:#x} exceeds memory size {memory_size:#x}"
            ),
            Self::SegmentMemorySizeTooLarge {
                segment,
                memory_size,
            } => write!(
                formatter,
                "ELF segment {segment} memory size {memory_size:#x} is too large"
            ),
            Self::SegmentMemoryRangeOverflow {
                segment,
                physical,
                memory_size,
            } => write!(
                formatter,
                "ELF segment {segment} memory range {physical:#x}+{memory_size:#x} overflows"
            ),
            Self::InterpreterFileRangeOutOfBounds {
                segment,
                offset,
                size,
                image_size,
            } => write!(
                formatter,
                "ELF interpreter segment {segment} file range {offset:#x}+{size:#x} exceeds image size {image_size:#x}"
            ),
            Self::InvalidInterpreterPath { segment } => {
                write!(formatter, "ELF interpreter segment {segment} has an invalid path")
            }
            Self::ProgramHeaderTableOutOfBounds {
                offset,
                size,
                image_size,
            } => write!(
                formatter,
                "ELF program header table {offset:#x}+{size:#x} exceeds image size {image_size:#x}"
            ),
            Self::SectionHeaderTableOutOfBounds {
                offset,
                size,
                image_size,
            } => write!(
                formatter,
                "ELF section header table {offset:#x}+{size:#x} exceeds image size {image_size:#x}"
            ),
            Self::SectionDataRangeOutOfBounds {
                offset,
                size,
                image_size,
            } => write!(
                formatter,
                "ELF section data {offset:#x}+{size:#x} exceeds image size {image_size:#x}"
            ),
            Self::TruncatedField {
                offset,
                size,
                image_size,
            } => write!(
                formatter,
                "ELF field {offset:#x}+{size:#x} exceeds image size {image_size:#x}"
            ),
            Self::UnsupportedClass { class } => {
                write!(formatter, "unsupported ELF class {class}")
            }
            Self::UnsupportedEncoding { encoding } => {
                write!(formatter, "unsupported ELF data encoding {encoding}")
            }
            Self::UnsupportedHeaderSize { expected, actual } => write!(
                formatter,
                "unsupported ELF header size {actual}, expected {expected}"
            ),
            Self::UnsupportedProgramHeaderSize { expected, actual } => write!(
                formatter,
                "unsupported ELF program header size {actual}, expected {expected}"
            ),
            Self::UnsupportedSectionHeaderSize { expected, actual } => write!(
                formatter,
                "unsupported ELF section header size {actual}, expected {expected}"
            ),
            Self::UnsupportedVersion { version } => {
                write!(formatter, "unsupported ELF version {version}")
            }
        }
    }
}

impl Error for BootElfError {}

impl Error for BootError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidElf { reason } => Some(reason),
            Self::Memory(error) => Some(error),
            _ => None,
        }
    }
}

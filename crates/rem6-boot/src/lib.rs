use std::error::Error;
use std::fmt;

use rem6_memory::{
    AccessSize, Address, AddressRange, CacheLineLayout, LineMemoryStore, MemoryError,
    MemoryTargetId, PartitionedMemoryStore,
};

const ELF64_HEADER_SIZE: usize = 64;
const ELF64_PROGRAM_HEADER_SIZE: u16 = 56;
const ELF32_HEADER_SIZE: usize = 52;
const ELF32_PROGRAM_HEADER_SIZE: u16 = 32;
const ELF_CLASS_32: u8 = 1;
const ELF_CLASS_64: u8 = 2;
const ELF_DATA_LITTLE: u8 = 1;
const ELF_VERSION_CURRENT: u8 = 1;
const PT_LOAD: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootImage {
    entry: Address,
    segments: Vec<BootSegment>,
}

impl BootImage {
    pub const fn new(entry: Address) -> Self {
        Self {
            entry,
            segments: Vec::new(),
        }
    }

    pub const fn entry(&self) -> Address {
        self.entry
    }

    pub fn from_elf(bytes: &[u8]) -> Result<Self, BootError> {
        parse_elf(bytes)
    }

    pub fn from_elf64_le(bytes: &[u8]) -> Result<Self, BootError> {
        parse_elf64_le(bytes)
    }

    pub fn from_elf32_le(bytes: &[u8]) -> Result<Self, BootError> {
        parse_elf32_le(bytes)
    }

    pub fn segments(&self) -> &[BootSegment] {
        &self.segments
    }

    pub fn add_segment(mut self, start: Address, data: Vec<u8>) -> Result<Self, BootError> {
        let segment = BootSegment::new(start, data)?;
        if let Some(existing) = self
            .segments
            .iter()
            .find(|existing| existing.range().overlaps(segment.range()))
        {
            return Err(BootError::OverlappingSegment {
                existing: existing.range(),
                requested: segment.range(),
            });
        }

        self.segments.push(segment);
        self.segments.sort_by_key(|segment| segment.range().start());
        Ok(self)
    }

    pub fn load_into_line_store(
        &self,
        store: &mut LineMemoryStore,
    ) -> Result<BootLoadReport, BootError> {
        let mut writes = Vec::new();
        let layout = store.layout();
        for segment in &self.segments {
            load_segment(segment, layout, &mut writes, |line, offset, bytes| {
                let mut line_data = store.line_data(line).unwrap_or_else(|| zero_line(layout));
                let start = offset as usize;
                line_data[start..start + bytes.len()].copy_from_slice(bytes);
                store
                    .insert_line(line, line_data)
                    .map_err(BootError::Memory)
            })?;
        }

        Ok(BootLoadReport::new(self.entry, writes))
    }

    pub fn load_into_partitioned_store(
        &self,
        store: &mut PartitionedMemoryStore,
        target: MemoryTargetId,
    ) -> Result<BootLoadReport, BootError> {
        let layout = store.partition_layout(target).map_err(BootError::Memory)?;
        let mut writes = Vec::new();
        for segment in &self.segments {
            load_segment(segment, layout, &mut writes, |line, offset, bytes| {
                let mut line_data = store
                    .line_data(target, line)
                    .unwrap_or_else(|_| zero_line(layout));
                let start = offset as usize;
                line_data[start..start + bytes.len()].copy_from_slice(bytes);
                store
                    .insert_line(target, line, line_data)
                    .map_err(BootError::Memory)
            })?;
        }

        Ok(BootLoadReport::new(self.entry, writes))
    }

    pub fn load_into_partitioned_store_by_address(
        &self,
        store: &mut PartitionedMemoryStore,
    ) -> Result<BootLoadReport, BootError> {
        let mut writes = Vec::new();
        for segment in &self.segments {
            load_segment_by_address(segment, store, &mut writes)?;
        }

        Ok(BootLoadReport::new(self.entry, writes))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootSegment {
    range: AddressRange,
    data: Vec<u8>,
}

impl BootSegment {
    pub fn new(start: Address, data: Vec<u8>) -> Result<Self, BootError> {
        if data.is_empty() {
            return Err(BootError::EmptySegment { start });
        }
        let size = AccessSize::new(data.len() as u64).map_err(BootError::Memory)?;
        let range = AddressRange::new(start, size).map_err(BootError::Memory)?;
        Ok(Self { range, data })
    }

    pub const fn range(&self) -> AddressRange {
        self.range
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootLineWrite {
    line: Address,
    offset: u64,
    bytes: u64,
}

impl BootLineWrite {
    pub const fn new(line: Address, offset: u64, bytes: u64) -> Self {
        Self {
            line,
            offset,
            bytes,
        }
    }

    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootLoadReport {
    entry: Address,
    writes: Vec<BootLineWrite>,
}

impl BootLoadReport {
    pub const fn new(entry: Address, writes: Vec<BootLineWrite>) -> Self {
        Self { entry, writes }
    }

    pub const fn entry(&self) -> Address {
        self.entry
    }

    pub fn writes(&self) -> &[BootLineWrite] {
        &self.writes
    }
}

fn load_segment<F>(
    segment: &BootSegment,
    layout: CacheLineLayout,
    writes: &mut Vec<BootLineWrite>,
    mut write_line: F,
) -> Result<(), BootError>
where
    F: FnMut(Address, u64, &[u8]) -> Result<(), BootError>,
{
    let mut cursor = segment.range().start().get();
    let end = segment.range().end().get();
    let mut data_offset = 0usize;

    while cursor < end {
        let address = Address::new(cursor);
        let line = layout.line_address(address);
        let line_offset = layout.line_offset(address);
        let available = layout.bytes() - line_offset;
        let remaining = end - cursor;
        let bytes = available.min(remaining);
        let next_data_offset = data_offset + bytes as usize;
        write_line(
            line,
            line_offset,
            &segment.data()[data_offset..next_data_offset],
        )?;
        writes.push(BootLineWrite::new(line, line_offset, bytes));
        cursor += bytes;
        data_offset = next_data_offset;
    }

    Ok(())
}

fn load_segment_by_address(
    segment: &BootSegment,
    store: &mut PartitionedMemoryStore,
    writes: &mut Vec<BootLineWrite>,
) -> Result<(), BootError> {
    let mut cursor = segment.range().start().get();
    let end = segment.range().end().get();
    let mut data_offset = 0usize;

    while cursor < end {
        let address = Address::new(cursor);
        let (target, region) = partitioned_target_at(store, address)?;
        let layout = store.partition_layout(target).map_err(BootError::Memory)?;
        let line = layout.line_address(address);
        let line_offset = layout.line_offset(address);
        let available_in_line = layout.bytes() - line_offset;
        let available_in_region = region.end().get() - cursor;
        let remaining = end - cursor;
        let bytes = available_in_line.min(available_in_region).min(remaining);
        let next_data_offset = data_offset + bytes as usize;

        let mut line_data = store
            .line_data(target, line)
            .unwrap_or_else(|_| zero_line(layout));
        let start = line_offset as usize;
        line_data[start..start + bytes as usize]
            .copy_from_slice(&segment.data()[data_offset..next_data_offset]);
        store
            .insert_line(target, line, line_data)
            .map_err(BootError::Memory)?;
        writes.push(BootLineWrite::new(line, line_offset, bytes));

        cursor += bytes;
        data_offset = next_data_offset;
    }

    Ok(())
}

fn partitioned_target_at(
    store: &PartitionedMemoryStore,
    address: Address,
) -> Result<(MemoryTargetId, AddressRange), BootError> {
    store
        .regions()
        .iter()
        .find_map(|(target, region)| region.contains(address).then_some((*target, *region)))
        .ok_or(BootError::Memory(MemoryError::UnmappedAddress { address }))
}

fn zero_line(layout: CacheLineLayout) -> Vec<u8> {
    vec![0; layout.bytes() as usize]
}

fn parse_elf(bytes: &[u8]) -> Result<BootImage, BootError> {
    match detect_elf_class(bytes)? {
        ELF_CLASS_32 => parse_elf32_le(bytes),
        ELF_CLASS_64 => parse_elf64_le(bytes),
        class => Err(invalid_elf(BootElfError::UnsupportedClass { class })),
    }
}

fn parse_elf64_le(bytes: &[u8]) -> Result<BootImage, BootError> {
    validate_elf_ident(bytes, ELF_CLASS_64)?;
    let header_size = read_u16(bytes, 52)?;
    if header_size as usize != ELF64_HEADER_SIZE {
        return Err(invalid_elf(BootElfError::UnsupportedHeaderSize {
            expected: ELF64_HEADER_SIZE as u16,
            actual: header_size,
        }));
    }

    let program_header_size = read_u16(bytes, 54)?;
    if program_header_size != ELF64_PROGRAM_HEADER_SIZE {
        return Err(invalid_elf(BootElfError::UnsupportedProgramHeaderSize {
            expected: ELF64_PROGRAM_HEADER_SIZE,
            actual: program_header_size,
        }));
    }

    let entry = Address::new(read_u64(bytes, 24)?);
    let program_header_offset = read_u64(bytes, 32)?;
    let program_header_count = read_u16(bytes, 56)?;
    let table_size = (program_header_size as u64)
        .checked_mul(program_header_count as u64)
        .ok_or_else(|| {
            invalid_elf(BootElfError::ProgramHeaderTableOutOfBounds {
                offset: program_header_offset,
                size: u64::MAX,
                image_size: bytes.len() as u64,
            })
        })?;
    checked_file_range(bytes, program_header_offset, table_size).map_err(|_| {
        invalid_elf(BootElfError::ProgramHeaderTableOutOfBounds {
            offset: program_header_offset,
            size: table_size,
            image_size: bytes.len() as u64,
        })
    })?;

    let mut image = BootImage::new(entry);
    let mut loaded_segments = 0usize;
    for index in 0..program_header_count {
        let header_offset = program_header_offset + index as u64 * program_header_size as u64;
        let kind = read_u32_at_u64(bytes, header_offset)?;
        if kind != PT_LOAD {
            continue;
        }

        let file_offset = read_u64_at_u64(bytes, header_offset + 8)?;
        let physical = read_u64_at_u64(bytes, header_offset + 24)?;
        let file_size = read_u64_at_u64(bytes, header_offset + 32)?;
        let memory_size = read_u64_at_u64(bytes, header_offset + 40)?;
        if memory_size == 0 {
            continue;
        }
        if file_size > memory_size {
            return Err(invalid_elf(
                BootElfError::SegmentFileSizeExceedsMemorySize {
                    segment: index,
                    file_size,
                    memory_size,
                },
            ));
        }

        let file_range = checked_file_range(bytes, file_offset, file_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentFileRangeOutOfBounds {
                segment: index,
                offset: file_offset,
                size: file_size,
                image_size: bytes.len() as u64,
            })
        })?;
        let memory_len = usize::try_from(memory_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemorySizeTooLarge {
                segment: index,
                memory_size,
            })
        })?;
        let memory_access_size = AccessSize::new(memory_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment: index,
                physical,
                memory_size,
            })
        })?;
        AddressRange::new(Address::new(physical), memory_access_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment: index,
                physical,
                memory_size,
            })
        })?;
        let file_len = usize::try_from(file_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentFileRangeOutOfBounds {
                segment: index,
                offset: file_offset,
                size: file_size,
                image_size: bytes.len() as u64,
            })
        })?;

        let mut data = vec![0; memory_len];
        data[..file_len].copy_from_slice(file_range);
        image = image.add_segment(Address::new(physical), data)?;
        loaded_segments += 1;
    }

    if loaded_segments == 0 {
        return Err(invalid_elf(BootElfError::NoLoadableSegments));
    }

    Ok(image)
}

fn parse_elf32_le(bytes: &[u8]) -> Result<BootImage, BootError> {
    validate_elf_ident(bytes, ELF_CLASS_32)?;
    let header_size = read_u16(bytes, 40)?;
    if header_size as usize != ELF32_HEADER_SIZE {
        return Err(invalid_elf(BootElfError::UnsupportedHeaderSize {
            expected: ELF32_HEADER_SIZE as u16,
            actual: header_size,
        }));
    }

    let program_header_size = read_u16(bytes, 42)?;
    if program_header_size != ELF32_PROGRAM_HEADER_SIZE {
        return Err(invalid_elf(BootElfError::UnsupportedProgramHeaderSize {
            expected: ELF32_PROGRAM_HEADER_SIZE,
            actual: program_header_size,
        }));
    }

    let entry = Address::new(u64::from(read_u32(bytes, 24)?));
    let program_header_offset = u64::from(read_u32(bytes, 28)?);
    let program_header_count = read_u16(bytes, 44)?;
    let table_size = (program_header_size as u64)
        .checked_mul(program_header_count as u64)
        .ok_or_else(|| {
            invalid_elf(BootElfError::ProgramHeaderTableOutOfBounds {
                offset: program_header_offset,
                size: u64::MAX,
                image_size: bytes.len() as u64,
            })
        })?;
    checked_file_range(bytes, program_header_offset, table_size).map_err(|_| {
        invalid_elf(BootElfError::ProgramHeaderTableOutOfBounds {
            offset: program_header_offset,
            size: table_size,
            image_size: bytes.len() as u64,
        })
    })?;

    let mut image = BootImage::new(entry);
    let mut loaded_segments = 0usize;
    for index in 0..program_header_count {
        let header_offset = program_header_offset + index as u64 * program_header_size as u64;
        let kind = read_u32_at_u64(bytes, header_offset)?;
        if kind != PT_LOAD {
            continue;
        }

        let file_offset = u64::from(read_u32_at_u64(bytes, header_offset + 4)?);
        let physical = u64::from(read_u32_at_u64(bytes, header_offset + 12)?);
        let file_size = u64::from(read_u32_at_u64(bytes, header_offset + 16)?);
        let memory_size = u64::from(read_u32_at_u64(bytes, header_offset + 20)?);
        if memory_size == 0 {
            continue;
        }
        if file_size > memory_size {
            return Err(invalid_elf(
                BootElfError::SegmentFileSizeExceedsMemorySize {
                    segment: index,
                    file_size,
                    memory_size,
                },
            ));
        }

        let file_range = checked_file_range(bytes, file_offset, file_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentFileRangeOutOfBounds {
                segment: index,
                offset: file_offset,
                size: file_size,
                image_size: bytes.len() as u64,
            })
        })?;
        let memory_end = physical.checked_add(memory_size).ok_or_else(|| {
            invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment: index,
                physical,
                memory_size,
            })
        })?;
        if memory_end > u64::from(u32::MAX) + 1 {
            return Err(invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment: index,
                physical,
                memory_size,
            }));
        }
        let memory_len = usize::try_from(memory_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemorySizeTooLarge {
                segment: index,
                memory_size,
            })
        })?;
        let memory_access_size = AccessSize::new(memory_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment: index,
                physical,
                memory_size,
            })
        })?;
        AddressRange::new(Address::new(physical), memory_access_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentMemoryRangeOverflow {
                segment: index,
                physical,
                memory_size,
            })
        })?;
        let file_len = usize::try_from(file_size).map_err(|_| {
            invalid_elf(BootElfError::SegmentFileRangeOutOfBounds {
                segment: index,
                offset: file_offset,
                size: file_size,
                image_size: bytes.len() as u64,
            })
        })?;

        let mut data = vec![0; memory_len];
        data[..file_len].copy_from_slice(file_range);
        image = image.add_segment(Address::new(physical), data)?;
        loaded_segments += 1;
    }

    if loaded_segments == 0 {
        return Err(invalid_elf(BootElfError::NoLoadableSegments));
    }

    Ok(image)
}

fn validate_elf_ident(bytes: &[u8], expected_class: u8) -> Result<(), BootError> {
    let class = detect_elf_class(bytes)?;
    if class != expected_class {
        return Err(invalid_elf(BootElfError::UnsupportedClass { class }));
    }
    Ok(())
}

fn detect_elf_class(bytes: &[u8]) -> Result<u8, BootError> {
    let ident = read_exact(bytes, 0, 16)?;
    if &ident[0..4] != b"\x7fELF" {
        return Err(invalid_elf(BootElfError::BadMagic));
    }
    let class = ident[4];
    if !matches!(class, ELF_CLASS_32 | ELF_CLASS_64) {
        return Err(invalid_elf(BootElfError::UnsupportedClass { class }));
    }
    if ident[5] != ELF_DATA_LITTLE {
        return Err(invalid_elf(BootElfError::UnsupportedEncoding {
            encoding: ident[5],
        }));
    }
    if ident[6] != ELF_VERSION_CURRENT {
        return Err(invalid_elf(BootElfError::UnsupportedVersion {
            version: ident[6],
        }));
    }
    Ok(class)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, BootError> {
    let data = read_exact(bytes, offset, 2)?;
    Ok(u16::from_le_bytes([data[0], data[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, BootError> {
    let data = read_exact(bytes, offset, 4)?;
    Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
}

fn read_u32_at_u64(bytes: &[u8], offset: u64) -> Result<u32, BootError> {
    let offset = usize::try_from(offset).map_err(|_| {
        invalid_elf(BootElfError::TruncatedField {
            offset,
            size: 4,
            image_size: bytes.len() as u64,
        })
    })?;
    let data = read_exact(bytes, offset, 4)?;
    Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, BootError> {
    let data = read_exact(bytes, offset, 8)?;
    Ok(u64::from_le_bytes([
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
    ]))
}

fn read_u64_at_u64(bytes: &[u8], offset: u64) -> Result<u64, BootError> {
    let offset = usize::try_from(offset).map_err(|_| {
        invalid_elf(BootElfError::TruncatedField {
            offset,
            size: 8,
            image_size: bytes.len() as u64,
        })
    })?;
    read_u64(bytes, offset)
}

fn read_exact(bytes: &[u8], offset: usize, size: usize) -> Result<&[u8], BootError> {
    bytes
        .get(offset..offset.saturating_add(size))
        .ok_or_else(|| {
            invalid_elf(BootElfError::TruncatedField {
                offset: offset as u64,
                size: size as u64,
                image_size: bytes.len() as u64,
            })
        })
}

fn checked_file_range(bytes: &[u8], offset: u64, size: u64) -> Result<&[u8], ()> {
    let end = offset.checked_add(size).ok_or(())?;
    let start = usize::try_from(offset).map_err(|_| ())?;
    let end = usize::try_from(end).map_err(|_| ())?;
    bytes.get(start..end).ok_or(())
}

fn invalid_elf(reason: BootElfError) -> BootError {
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
    ProgramHeaderTableOutOfBounds {
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
            Self::ProgramHeaderTableOutOfBounds {
                offset,
                size,
                image_size,
            } => write!(
                formatter,
                "ELF program header table {offset:#x}+{size:#x} exceeds image size {image_size:#x}"
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

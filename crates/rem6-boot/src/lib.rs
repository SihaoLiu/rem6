use std::error::Error;
use std::fmt;

use rem6_memory::{
    AccessSize, Address, AddressRange, CacheLineLayout, LineMemoryStore, MemoryError,
    MemoryTargetId, PartitionedMemoryStore,
};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootError {
    EmptySegment {
        start: Address,
    },
    OverlappingSegment {
        existing: AddressRange,
        requested: AddressRange,
    },
    Memory(MemoryError),
}

impl fmt::Display for BootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySegment { start } => {
                write!(formatter, "boot segment at {:#x} is empty", start.get())
            }
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

impl Error for BootError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            _ => None,
        }
    }
}

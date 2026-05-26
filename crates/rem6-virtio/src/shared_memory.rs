use std::collections::BTreeMap;

use rem6_memory::{AccessSize, Address, AddressRange};

use crate::VirtioError;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VirtioPciBarIndex(u8);

impl VirtioPciBarIndex {
    pub const fn new(value: u8) -> Option<Self> {
        if value < 6 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VirtioPciSharedMemoryId(u8);

impl VirtioPciSharedMemoryId {
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioPciSharedMemoryRegionSpec {
    id: VirtioPciSharedMemoryId,
    bar: VirtioPciBarIndex,
    offset: u64,
    length: u64,
}

impl VirtioPciSharedMemoryRegionSpec {
    pub fn new(
        id: VirtioPciSharedMemoryId,
        bar: VirtioPciBarIndex,
        offset: u64,
        length: u64,
    ) -> Result<Self, VirtioError> {
        if length == 0 {
            return Err(VirtioError::ZeroSharedMemoryRegion { id: id.get() });
        }
        Ok(Self {
            id,
            bar,
            offset,
            length,
        })
    }

    pub const fn id(self) -> VirtioPciSharedMemoryId {
        self.id
    }

    pub const fn bar(self) -> VirtioPciBarIndex {
        self.bar
    }

    pub const fn offset(self) -> u64 {
        self.offset
    }

    pub const fn length(self) -> u64 {
        self.length
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioPciSharedMemoryRegion {
    id: VirtioPciSharedMemoryId,
    bar: VirtioPciBarIndex,
    range: AddressRange,
}

impl VirtioPciSharedMemoryRegion {
    pub fn new(
        id: VirtioPciSharedMemoryId,
        bar: VirtioPciBarIndex,
        offset: u64,
        length: u64,
    ) -> Result<Self, VirtioError> {
        if length == 0 {
            return Err(VirtioError::ZeroSharedMemoryRegion { id: id.get() });
        }
        let range = AddressRange::new(Address::new(offset), AccessSize::new(length).unwrap())
            .map_err(|_| VirtioError::SharedMemoryRegionAddressOverflow {
                id: id.get(),
                bar: bar.get(),
                offset,
                length,
            })?;
        Ok(Self { id, bar, range })
    }

    pub const fn id(self) -> VirtioPciSharedMemoryId {
        self.id
    }

    pub const fn bar(self) -> VirtioPciBarIndex {
        self.bar
    }

    pub const fn range(self) -> AddressRange {
        self.range
    }

    pub const fn offset(self) -> u64 {
        self.range.start().get()
    }

    pub const fn length(self) -> u64 {
        self.range.size().bytes()
    }

    pub const fn cap64_fields(self) -> VirtioPciSharedMemoryCap64Fields {
        VirtioPciSharedMemoryCap64Fields::from_region(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioPciSharedMemoryCap64Fields {
    bar: VirtioPciBarIndex,
    id: VirtioPciSharedMemoryId,
    offset: u32,
    offset_hi: u32,
    length: u32,
    length_hi: u32,
}

impl VirtioPciSharedMemoryCap64Fields {
    pub const fn new(
        bar: VirtioPciBarIndex,
        id: VirtioPciSharedMemoryId,
        offset: u32,
        offset_hi: u32,
        length: u32,
        length_hi: u32,
    ) -> Self {
        Self {
            bar,
            id,
            offset,
            offset_hi,
            length,
            length_hi,
        }
    }

    pub const fn from_region(region: VirtioPciSharedMemoryRegion) -> Self {
        Self {
            bar: region.bar,
            id: region.id,
            offset: region.offset() as u32,
            offset_hi: (region.offset() >> 32) as u32,
            length: region.length() as u32,
            length_hi: (region.length() >> 32) as u32,
        }
    }

    pub const fn bar(self) -> VirtioPciBarIndex {
        self.bar
    }

    pub const fn id(self) -> VirtioPciSharedMemoryId {
        self.id
    }

    pub const fn offset(self) -> u32 {
        self.offset
    }

    pub const fn offset_hi(self) -> u32 {
        self.offset_hi
    }

    pub const fn length(self) -> u32 {
        self.length
    }

    pub const fn length_hi(self) -> u32 {
        self.length_hi
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioPciSharedMemoryRegistry {
    regions: Vec<VirtioPciSharedMemoryRegion>,
}

impl VirtioPciSharedMemoryRegistry {
    pub fn new(
        bars: impl IntoIterator<Item = (VirtioPciBarIndex, AccessSize)>,
        regions: impl IntoIterator<Item = VirtioPciSharedMemoryRegionSpec>,
    ) -> Result<Self, VirtioError> {
        let bars = collect_bars(bars)?;
        let mut regions = regions
            .into_iter()
            .map(|spec| checked_region(&bars, spec))
            .collect::<Result<Vec<_>, _>>()?;
        regions.sort_by_key(|region| (region.id(), region.bar(), region.offset()));
        validate_unique_ids(&regions)?;
        validate_non_overlapping(&regions)?;
        Ok(Self { regions })
    }

    pub fn regions(&self) -> &[VirtioPciSharedMemoryRegion] {
        &self.regions
    }

    pub fn region(&self, id: VirtioPciSharedMemoryId) -> Option<&VirtioPciSharedMemoryRegion> {
        self.regions.iter().find(|region| region.id == id)
    }

    pub fn regions_for_bar(&self, bar: VirtioPciBarIndex) -> Vec<VirtioPciSharedMemoryRegion> {
        self.regions
            .iter()
            .copied()
            .filter(|region| region.bar == bar)
            .collect()
    }

    pub fn cap64_fields(
        &self,
        id: VirtioPciSharedMemoryId,
    ) -> Option<VirtioPciSharedMemoryCap64Fields> {
        self.region(id).map(|region| region.cap64_fields())
    }
}

fn collect_bars(
    bars: impl IntoIterator<Item = (VirtioPciBarIndex, AccessSize)>,
) -> Result<BTreeMap<VirtioPciBarIndex, AccessSize>, VirtioError> {
    let mut by_bar = BTreeMap::new();
    for (bar, size) in bars {
        if by_bar.insert(bar, size).is_some() {
            return Err(VirtioError::DuplicateSharedMemoryBar { bar: bar.get() });
        }
    }
    Ok(by_bar)
}

fn checked_region(
    bars: &BTreeMap<VirtioPciBarIndex, AccessSize>,
    spec: VirtioPciSharedMemoryRegionSpec,
) -> Result<VirtioPciSharedMemoryRegion, VirtioError> {
    let bar_size = bars
        .get(&spec.bar)
        .ok_or_else(|| VirtioError::MissingSharedMemoryBar {
            id: spec.id.get(),
            bar: spec.bar.get(),
        })?;
    let end = spec.offset.checked_add(spec.length).ok_or_else(|| {
        VirtioError::SharedMemoryRegionOutOfBar {
            id: spec.id.get(),
            bar: spec.bar.get(),
            offset: spec.offset,
            length: spec.length,
            bar_length: bar_size.bytes(),
        }
    })?;
    if end > bar_size.bytes() {
        return Err(VirtioError::SharedMemoryRegionOutOfBar {
            id: spec.id.get(),
            bar: spec.bar.get(),
            offset: spec.offset,
            length: spec.length,
            bar_length: bar_size.bytes(),
        });
    }
    VirtioPciSharedMemoryRegion::new(spec.id, spec.bar, spec.offset, spec.length)
}

fn validate_unique_ids(regions: &[VirtioPciSharedMemoryRegion]) -> Result<(), VirtioError> {
    for (index, region) in regions.iter().enumerate() {
        if regions
            .iter()
            .skip(index + 1)
            .any(|other| other.id == region.id)
        {
            return Err(VirtioError::DuplicateSharedMemoryId {
                id: region.id.get(),
            });
        }
    }
    Ok(())
}

fn validate_non_overlapping(regions: &[VirtioPciSharedMemoryRegion]) -> Result<(), VirtioError> {
    for (index, region) in regions.iter().enumerate() {
        if let Some(other) = regions
            .iter()
            .skip(index + 1)
            .find(|other| other.bar == region.bar && other.range.overlaps(region.range))
        {
            return Err(VirtioError::OverlappingSharedMemoryRegion {
                first: region.id.get(),
                second: other.id.get(),
                bar: region.bar.get(),
            });
        }
    }
    Ok(())
}

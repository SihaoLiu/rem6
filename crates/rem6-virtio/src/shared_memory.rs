use std::collections::BTreeMap;

use rem6_memory::{AccessSize, Address, AddressRange};

use crate::{
    pci_capability::{
        raw_capability_spec, validate_pci_capability_span, write_u32_le, VirtioPciBarIndex,
        VirtioPciCapabilityKind, VirtioPciCapabilityOffset, PCI_CONFIG_SPACE_SIZE,
        PCI_VENDOR_SPECIFIC_CAPABILITY_ID, VIRTIO_PCI_CAP64_SIZE,
    },
    VirtioError,
};
use rem6_pci::PciRawCapabilitySpec;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioPciSharedMemoryCapabilityEntry {
    offset: VirtioPciCapabilityOffset,
    next: Option<VirtioPciCapabilityOffset>,
    region: VirtioPciSharedMemoryRegion,
}

impl VirtioPciSharedMemoryCapabilityEntry {
    pub const fn new(
        offset: VirtioPciCapabilityOffset,
        next: Option<VirtioPciCapabilityOffset>,
        region: VirtioPciSharedMemoryRegion,
    ) -> Self {
        Self {
            offset,
            next,
            region,
        }
    }

    pub const fn offset(self) -> VirtioPciCapabilityOffset {
        self.offset
    }

    pub const fn next(self) -> Option<VirtioPciCapabilityOffset> {
        self.next
    }

    pub const fn region(self) -> VirtioPciSharedMemoryRegion {
        self.region
    }

    pub fn bytes(self) -> [u8; VIRTIO_PCI_CAP64_SIZE as usize] {
        let fields = self.region.cap64_fields();
        let mut bytes = [0; VIRTIO_PCI_CAP64_SIZE as usize];
        bytes[0] = PCI_VENDOR_SPECIFIC_CAPABILITY_ID;
        bytes[1] = self.next.map_or(0, VirtioPciCapabilityOffset::get);
        bytes[2] = VIRTIO_PCI_CAP64_SIZE;
        bytes[3] = VirtioPciCapabilityKind::SharedMemoryConfig.cfg_type();
        bytes[4] = fields.bar().get();
        bytes[5] = fields.id().get();
        write_u32_le(&mut bytes, 8, fields.offset());
        write_u32_le(&mut bytes, 12, fields.length());
        write_u32_le(&mut bytes, 16, fields.offset_hi());
        write_u32_le(&mut bytes, 20, fields.length_hi());
        bytes
    }

    pub fn raw_capability_spec(self) -> PciRawCapabilitySpec {
        raw_capability_spec(self.offset, self.bytes())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioPciSharedMemoryCapabilities {
    entries: Vec<VirtioPciSharedMemoryCapabilityEntry>,
}

impl VirtioPciSharedMemoryCapabilities {
    pub fn new(
        start: VirtioPciCapabilityOffset,
        registry: &VirtioPciSharedMemoryRegistry,
    ) -> Result<Self, VirtioError> {
        let mut entries = Vec::with_capacity(registry.regions().len());
        let mut raw_offset = u16::from(start.get());
        for (index, region) in registry.regions().iter().copied().enumerate() {
            validate_pci_capability_span(raw_offset, VIRTIO_PCI_CAP64_SIZE)?;
            let offset = VirtioPciCapabilityOffset::new(raw_offset as u8).ok_or(
                VirtioError::PciCapabilityOutOfConfig {
                    offset: raw_offset,
                    length: u64::from(VIRTIO_PCI_CAP64_SIZE),
                },
            )?;
            let next = if index + 1 == registry.regions().len() {
                None
            } else {
                let next_offset = raw_offset + u16::from(VIRTIO_PCI_CAP64_SIZE);
                validate_pci_capability_span(next_offset, VIRTIO_PCI_CAP64_SIZE)?;
                Some(VirtioPciCapabilityOffset::new(next_offset as u8).ok_or(
                    VirtioError::PciCapabilityOutOfConfig {
                        offset: next_offset,
                        length: u64::from(VIRTIO_PCI_CAP64_SIZE),
                    },
                )?)
            };
            entries.push(VirtioPciSharedMemoryCapabilityEntry::new(
                offset, next, region,
            ));
            raw_offset += u16::from(VIRTIO_PCI_CAP64_SIZE);
        }
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[VirtioPciSharedMemoryCapabilityEntry] {
        &self.entries
    }

    pub fn first_offset(&self) -> Option<VirtioPciCapabilityOffset> {
        self.entries.first().map(|entry| entry.offset())
    }

    pub fn entry_for_region(
        &self,
        id: VirtioPciSharedMemoryId,
    ) -> Option<&VirtioPciSharedMemoryCapabilityEntry> {
        self.entries.iter().find(|entry| entry.region().id() == id)
    }

    pub fn entry_at(
        &self,
        offset: VirtioPciCapabilityOffset,
    ) -> Option<&VirtioPciSharedMemoryCapabilityEntry> {
        self.entries.iter().find(|entry| entry.offset() == offset)
    }

    pub fn write_into_config(&self, config: &mut [u8]) -> Result<(), VirtioError> {
        let required = self.required_config_bytes();
        if config.len() < required {
            return Err(VirtioError::SharedMemoryCapabilityConfigBufferTooSmall {
                bytes: config.len(),
                required,
            });
        }
        for entry in &self.entries {
            let start = usize::from(entry.offset().get());
            let end = start + usize::from(VIRTIO_PCI_CAP64_SIZE);
            config[start..end].copy_from_slice(&entry.bytes());
        }
        Ok(())
    }

    pub fn config_image(&self) -> [u8; PCI_CONFIG_SPACE_SIZE as usize] {
        let mut config = [0; PCI_CONFIG_SPACE_SIZE as usize];
        self.write_into_config(&mut config)
            .expect("validated shared memory capabilities fit config image");
        config
    }

    fn required_config_bytes(&self) -> usize {
        self.entries.last().map_or(0, |entry| {
            entry.offset().get() as usize + usize::from(VIRTIO_PCI_CAP64_SIZE)
        })
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

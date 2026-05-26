use rem6_memory::AccessSize;

use crate::{
    PciConfigOffset, PciEndpointConfig, PciError, PCI_CAPABILITY_PTR_OFFSET, PCI_CONFIG_SPACE_SIZE,
    PCI_STATUS_CAPABILITY_LIST, PCI_STATUS_OFFSET,
};

const PCI_CAPABILITY_MIN_OFFSET: u16 = 0x40;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciRawCapabilitySpec {
    offset: PciConfigOffset,
    bytes: Vec<u8>,
    size: AccessSize,
}

impl PciRawCapabilitySpec {
    pub fn new(offset: PciConfigOffset, bytes: impl Into<Vec<u8>>) -> Result<Self, PciError> {
        let mut bytes = bytes.into();
        let size = AccessSize::new(bytes.len() as u64).map_err(PciError::Memory)?;
        if bytes.len() < 2 {
            return Err(PciError::InvalidRawCapabilitySize { offset, size });
        }
        let raw_offset = offset.get();
        let end = u64::from(raw_offset) + size.bytes();
        if raw_offset < PCI_CAPABILITY_MIN_OFFSET
            || !raw_offset.is_multiple_of(4)
            || end > PCI_CONFIG_SPACE_SIZE as u64
        {
            return Err(PciError::InvalidRawCapabilityOffset { offset, size });
        }
        bytes[1] = 0;
        Ok(Self {
            offset,
            bytes,
            size,
        })
    }

    pub const fn offset(&self) -> PciConfigOffset {
        self.offset
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn size(&self) -> AccessSize {
        self.size
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PciRawCapabilityState {
    spec: PciRawCapabilitySpec,
}

impl PciRawCapabilityState {
    pub(crate) const fn new(spec: PciRawCapabilitySpec) -> Self {
        Self { spec }
    }

    pub(crate) const fn spec(&self) -> &PciRawCapabilitySpec {
        &self.spec
    }

    pub(crate) fn install_into(&self, config: &mut [u8]) {
        let start = self.spec.offset().as_usize();
        let end = start + self.spec.bytes().len();
        config[start..end].copy_from_slice(self.spec.bytes());
    }
}

impl PciEndpointConfig {
    pub fn install_raw_capability(&mut self, spec: PciRawCapabilitySpec) -> Result<(), PciError> {
        self.register_capability_region(spec.offset(), spec.size().bytes())?;
        let state = PciRawCapabilityState::new(spec);
        state.install_into(&mut self.config);
        self.raw_capabilities.push(state);
        self.rebuild_capability_list();
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PciEndpointCapabilityList {
    regions: Vec<PciCapabilityRegion>,
}

impl PciEndpointCapabilityList {
    pub(crate) fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    pub(crate) fn register(&mut self, offset: PciConfigOffset, size: u64) -> Result<(), PciError> {
        let requested_size = AccessSize::new(size).map_err(PciError::Memory)?;
        let requested = PciCapabilityRegion {
            offset,
            size: requested_size,
        };
        if let Some(existing) = self
            .regions
            .iter()
            .find(|existing| existing.overlaps(requested))
        {
            return Err(PciError::OverlappingCapability {
                existing_offset: existing.offset(),
                existing_size: existing.size(),
                requested_offset: requested.offset(),
                requested_size: requested.size(),
            });
        }
        self.regions.push(requested);
        Ok(())
    }

    pub(crate) fn rebuild(&self, config: &mut [u8]) {
        if self.regions.is_empty() {
            config[PCI_CAPABILITY_PTR_OFFSET] = 0;
            config[PCI_STATUS_OFFSET] &= !PCI_STATUS_CAPABILITY_LIST;
            return;
        }

        config[PCI_STATUS_OFFSET] |= PCI_STATUS_CAPABILITY_LIST;
        config[PCI_CAPABILITY_PTR_OFFSET] = self.regions[0].offset().get() as u8;
        for index in 0..self.regions.len() {
            let offset = self.regions[index].offset().as_usize();
            let next = self
                .regions
                .get(index + 1)
                .map_or(0, |capability| capability.offset().get() as u8);
            config[offset + 1] = next;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PciCapabilityRegion {
    offset: PciConfigOffset,
    size: AccessSize,
}

impl PciCapabilityRegion {
    const fn offset(self) -> PciConfigOffset {
        self.offset
    }

    const fn size(self) -> AccessSize {
        self.size
    }

    fn end(self) -> u64 {
        u64::from(self.offset.get()) + self.size.bytes()
    }

    fn overlaps(self, other: Self) -> bool {
        u64::from(self.offset.get()) < other.end() && u64::from(other.offset.get()) < self.end()
    }
}

use rem6_memory::AccessSize;

use crate::{
    PciConfigOffset, PciError, PCI_CAPABILITY_PTR_OFFSET, PCI_STATUS_CAPABILITY_LIST,
    PCI_STATUS_OFFSET,
};

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

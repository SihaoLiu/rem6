use rem6_memory::AccessSize;

use crate::{write_u16_at, PciConfigOffset, PciError, PCI_CONFIG_SPACE_SIZE};

const PCI_CAPABILITY_MIN_OFFSET: u16 = 0x40;
const PCI_PM_CAPABILITY_ID: u8 = 0x01;
const PCI_PM_CAPABILITY_SIZE: u64 = 0x06;
const PCI_PM_CAPABILITIES_OFFSET: u16 = 0x02;
const PCI_PM_CONTROL_STATUS_OFFSET: u16 = 0x04;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciPowerManagementCapabilitySpec {
    offset: PciConfigOffset,
    capabilities: u16,
    initial_control_status: u16,
}

impl PciPowerManagementCapabilitySpec {
    pub fn new(
        offset: PciConfigOffset,
        capabilities: u16,
        initial_control_status: u16,
    ) -> Result<Self, PciError> {
        let size = AccessSize::new(PCI_PM_CAPABILITY_SIZE).map_err(PciError::Memory)?;
        let raw_offset = offset.get();
        let end = u64::from(raw_offset) + size.bytes();
        if raw_offset < PCI_CAPABILITY_MIN_OFFSET
            || !raw_offset.is_multiple_of(4)
            || end > PCI_CONFIG_SPACE_SIZE as u64
        {
            return Err(PciError::InvalidPowerManagementCapabilityOffset { offset, size });
        }

        Ok(Self {
            offset,
            capabilities,
            initial_control_status,
        })
    }

    pub const fn offset(self) -> PciConfigOffset {
        self.offset
    }

    pub const fn capabilities(self) -> u16 {
        self.capabilities
    }

    pub const fn initial_control_status(self) -> u16 {
        self.initial_control_status
    }

    pub const fn size(self) -> u64 {
        PCI_PM_CAPABILITY_SIZE
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PciPowerManagementCapabilityState {
    spec: PciPowerManagementCapabilitySpec,
    control_status: u16,
}

impl PciPowerManagementCapabilityState {
    pub(crate) const fn new(spec: PciPowerManagementCapabilitySpec) -> Self {
        Self {
            spec,
            control_status: spec.initial_control_status(),
        }
    }

    pub(crate) const fn spec(&self) -> PciPowerManagementCapabilitySpec {
        self.spec
    }

    pub(crate) fn install_into(&self, config: &mut [u8; PCI_CONFIG_SPACE_SIZE]) {
        let base = self.spec.offset().as_usize();
        config[base] = PCI_PM_CAPABILITY_ID;
        config[base + 1] = 0;
        write_u16_at(
            config,
            base + PCI_PM_CAPABILITIES_OFFSET as usize,
            self.spec.capabilities(),
        );
        write_u16_at(
            config,
            base + PCI_PM_CONTROL_STATUS_OFFSET as usize,
            self.control_status,
        );
    }

    pub(crate) fn contains(&self, offset: PciConfigOffset, size: AccessSize) -> bool {
        let start = offset.get() as u64;
        let end = start + size.bytes();
        let cap_start = self.spec.offset().get() as u64;
        let cap_end = cap_start + PCI_PM_CAPABILITY_SIZE;
        start >= cap_start && end <= cap_end
    }

    pub(crate) fn write_config(
        &mut self,
        offset: PciConfigOffset,
        data: &[u8],
        config: &mut [u8; PCI_CONFIG_SPACE_SIZE],
    ) -> Result<(), PciError> {
        let size = AccessSize::new(data.len() as u64).map_err(PciError::Memory)?;
        let relative = offset.get() - self.spec.offset().get();
        match (relative, data.len()) {
            (PCI_PM_CONTROL_STATUS_OFFSET, 2) => {
                self.control_status = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.control_status);
                Ok(())
            }
            (PCI_PM_CONTROL_STATUS_OFFSET, _) => {
                Err(PciError::UnalignedPowerManagementCapabilityWrite { offset, size })
            }
            _ => Err(PciError::ReadOnlyPowerManagementCapabilityWrite { offset, size }),
        }
    }
}

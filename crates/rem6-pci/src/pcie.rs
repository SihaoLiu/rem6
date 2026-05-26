use rem6_memory::AccessSize;

use crate::{write_u16_at, write_u32_at, PciConfigOffset, PciError, PCI_CONFIG_SPACE_SIZE};

const PCI_CAPABILITY_MIN_OFFSET: u16 = 0x40;
const PCI_EXPRESS_CAPABILITY_ID: u8 = 0x10;
const PCI_EXPRESS_CAPABILITY_SIZE: u64 = 0x3c;
const PCI_EXPRESS_CAPABILITY_OFFSET: u16 = 0x02;
const PCI_EXPRESS_DEVICE_CAPABILITIES_OFFSET: u16 = 0x04;
const PCI_EXPRESS_DEVICE_CONTROL_OFFSET: u16 = 0x08;
const PCI_EXPRESS_DEVICE_STATUS_OFFSET: u16 = 0x0a;
const PCI_EXPRESS_LINK_CAPABILITIES_OFFSET: u16 = 0x0c;
const PCI_EXPRESS_LINK_CONTROL_OFFSET: u16 = 0x10;
const PCI_EXPRESS_LINK_STATUS_OFFSET: u16 = 0x12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciExpressDeviceCapabilitySpec {
    capabilities: u32,
    initial_control: u16,
    initial_status: u16,
}

impl PciExpressDeviceCapabilitySpec {
    pub const fn new(capabilities: u32, initial_control: u16, initial_status: u16) -> Self {
        Self {
            capabilities,
            initial_control,
            initial_status,
        }
    }

    pub const fn capabilities(self) -> u32 {
        self.capabilities
    }

    pub const fn initial_control(self) -> u16 {
        self.initial_control
    }

    pub const fn initial_status(self) -> u16 {
        self.initial_status
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciExpressLinkCapabilitySpec {
    capabilities: u32,
    initial_control: u16,
    initial_status: u16,
}

impl PciExpressLinkCapabilitySpec {
    pub const fn new(capabilities: u32, initial_control: u16, initial_status: u16) -> Self {
        Self {
            capabilities,
            initial_control,
            initial_status,
        }
    }

    pub const fn capabilities(self) -> u32 {
        self.capabilities
    }

    pub const fn initial_control(self) -> u16 {
        self.initial_control
    }

    pub const fn initial_status(self) -> u16 {
        self.initial_status
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciExpressCapabilitySpec {
    offset: PciConfigOffset,
    capability: u16,
    device: PciExpressDeviceCapabilitySpec,
    link: PciExpressLinkCapabilitySpec,
}

impl PciExpressCapabilitySpec {
    pub fn new(
        offset: PciConfigOffset,
        capability: u16,
        device: PciExpressDeviceCapabilitySpec,
        link: PciExpressLinkCapabilitySpec,
    ) -> Result<Self, PciError> {
        let size = AccessSize::new(PCI_EXPRESS_CAPABILITY_SIZE).map_err(PciError::Memory)?;
        let raw_offset = offset.get();
        let end = u64::from(raw_offset) + size.bytes();
        if raw_offset < PCI_CAPABILITY_MIN_OFFSET
            || !raw_offset.is_multiple_of(4)
            || end > PCI_CONFIG_SPACE_SIZE as u64
        {
            return Err(PciError::InvalidPciExpressCapabilityOffset { offset, size });
        }

        Ok(Self {
            offset,
            capability,
            device,
            link,
        })
    }

    pub const fn offset(self) -> PciConfigOffset {
        self.offset
    }

    pub const fn capability(self) -> u16 {
        self.capability
    }

    pub const fn device_capabilities(self) -> u32 {
        self.device.capabilities()
    }

    pub const fn initial_device_control(self) -> u16 {
        self.device.initial_control()
    }

    pub const fn initial_device_status(self) -> u16 {
        self.device.initial_status()
    }

    pub const fn link_capabilities(self) -> u32 {
        self.link.capabilities()
    }

    pub const fn initial_link_control(self) -> u16 {
        self.link.initial_control()
    }

    pub const fn initial_link_status(self) -> u16 {
        self.link.initial_status()
    }

    pub const fn size(self) -> u64 {
        PCI_EXPRESS_CAPABILITY_SIZE
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PciExpressCapabilityState {
    spec: PciExpressCapabilitySpec,
    device_control: u16,
    device_status: u16,
    link_control: u16,
    link_status: u16,
}

impl PciExpressCapabilityState {
    pub(crate) const fn new(spec: PciExpressCapabilitySpec) -> Self {
        Self {
            spec,
            device_control: spec.initial_device_control(),
            device_status: spec.initial_device_status(),
            link_control: spec.initial_link_control(),
            link_status: spec.initial_link_status(),
        }
    }

    pub(crate) const fn spec(&self) -> PciExpressCapabilitySpec {
        self.spec
    }

    pub(crate) fn install_into(&self, config: &mut [u8; PCI_CONFIG_SPACE_SIZE]) {
        let base = self.spec.offset().as_usize();
        config[base] = PCI_EXPRESS_CAPABILITY_ID;
        config[base + 1] = 0;
        write_u16_at(
            config,
            base + PCI_EXPRESS_CAPABILITY_OFFSET as usize,
            self.spec.capability(),
        );
        write_u32_at(
            config,
            base + PCI_EXPRESS_DEVICE_CAPABILITIES_OFFSET as usize,
            self.spec.device_capabilities(),
        );
        write_u16_at(
            config,
            base + PCI_EXPRESS_DEVICE_CONTROL_OFFSET as usize,
            self.device_control,
        );
        write_u16_at(
            config,
            base + PCI_EXPRESS_DEVICE_STATUS_OFFSET as usize,
            self.device_status,
        );
        write_u32_at(
            config,
            base + PCI_EXPRESS_LINK_CAPABILITIES_OFFSET as usize,
            self.spec.link_capabilities(),
        );
        write_u16_at(
            config,
            base + PCI_EXPRESS_LINK_CONTROL_OFFSET as usize,
            self.link_control,
        );
        write_u16_at(
            config,
            base + PCI_EXPRESS_LINK_STATUS_OFFSET as usize,
            self.link_status,
        );
    }

    pub(crate) fn contains(&self, offset: PciConfigOffset, size: AccessSize) -> bool {
        let start = offset.get() as u64;
        let end = start + size.bytes();
        let cap_start = self.spec.offset().get() as u64;
        let cap_end = cap_start + PCI_EXPRESS_CAPABILITY_SIZE;
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
            (PCI_EXPRESS_DEVICE_CONTROL_OFFSET, 2) => {
                self.device_control = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.device_control);
                Ok(())
            }
            (PCI_EXPRESS_DEVICE_STATUS_OFFSET, 2) => {
                self.device_status = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.device_status);
                Ok(())
            }
            (PCI_EXPRESS_LINK_CONTROL_OFFSET, 2) => {
                self.link_control = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.link_control);
                Ok(())
            }
            (PCI_EXPRESS_LINK_STATUS_OFFSET, 2) => {
                self.link_status = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.link_status);
                Ok(())
            }
            (PCI_EXPRESS_DEVICE_CONTROL_OFFSET, _)
            | (PCI_EXPRESS_DEVICE_STATUS_OFFSET, _)
            | (PCI_EXPRESS_LINK_CONTROL_OFFSET, _)
            | (PCI_EXPRESS_LINK_STATUS_OFFSET, _) => {
                Err(PciError::UnalignedPciExpressCapabilityWrite { offset, size })
            }
            _ => Err(PciError::ReadOnlyPciExpressCapabilityWrite { offset, size }),
        }
    }
}

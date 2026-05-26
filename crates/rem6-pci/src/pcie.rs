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
const PCI_EXPRESS_SLOT_CAPABILITIES_OFFSET: u16 = 0x14;
const PCI_EXPRESS_SLOT_CONTROL_OFFSET: u16 = 0x18;
const PCI_EXPRESS_SLOT_STATUS_OFFSET: u16 = 0x1a;
const PCI_EXPRESS_ROOT_CONTROL_OFFSET: u16 = 0x1c;
const PCI_EXPRESS_ROOT_CAPABILITIES_OFFSET: u16 = 0x1e;
const PCI_EXPRESS_ROOT_STATUS_OFFSET: u16 = 0x20;
const PCI_EXPRESS_DEVICE_CAPABILITIES2_OFFSET: u16 = 0x24;
const PCI_EXPRESS_DEVICE_CONTROL2_OFFSET: u16 = 0x28;
const PCI_EXPRESS_DEVICE_STATUS2_OFFSET: u16 = 0x2a;
const PCI_EXPRESS_LINK_CAPABILITIES2_OFFSET: u16 = 0x2c;
const PCI_EXPRESS_LINK_CONTROL2_OFFSET: u16 = 0x30;
const PCI_EXPRESS_LINK_STATUS2_OFFSET: u16 = 0x32;
const PCI_EXPRESS_SLOT_CAPABILITIES2_OFFSET: u16 = 0x34;
const PCI_EXPRESS_SLOT_CONTROL2_OFFSET: u16 = 0x38;
const PCI_EXPRESS_SLOT_STATUS2_OFFSET: u16 = 0x3a;

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
pub struct PciExpressSlotCapabilitySpec {
    capabilities: u32,
    initial_control: u16,
    initial_status: u16,
}

impl PciExpressSlotCapabilitySpec {
    pub const fn new(capabilities: u32, initial_control: u16, initial_status: u16) -> Self {
        Self {
            capabilities,
            initial_control,
            initial_status,
        }
    }

    pub const fn empty() -> Self {
        Self {
            capabilities: 0,
            initial_control: 0,
            initial_status: 0,
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

impl Default for PciExpressSlotCapabilitySpec {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciExpressRootCapabilitySpec {
    capabilities: u16,
    initial_control: u16,
    initial_status: u32,
}

impl PciExpressRootCapabilitySpec {
    pub const fn new(capabilities: u16, initial_control: u16, initial_status: u32) -> Self {
        Self {
            capabilities,
            initial_control,
            initial_status,
        }
    }

    pub const fn empty() -> Self {
        Self {
            capabilities: 0,
            initial_control: 0,
            initial_status: 0,
        }
    }

    pub const fn capabilities(self) -> u16 {
        self.capabilities
    }

    pub const fn initial_control(self) -> u16 {
        self.initial_control
    }

    pub const fn initial_status(self) -> u32 {
        self.initial_status
    }
}

impl Default for PciExpressRootCapabilitySpec {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciExpressCapability2Spec {
    device: PciExpressDeviceCapabilitySpec,
    link: PciExpressLinkCapabilitySpec,
    slot: PciExpressSlotCapabilitySpec,
}

impl PciExpressCapability2Spec {
    pub const fn new(
        device: PciExpressDeviceCapabilitySpec,
        link: PciExpressLinkCapabilitySpec,
        slot: PciExpressSlotCapabilitySpec,
    ) -> Self {
        Self { device, link, slot }
    }

    pub const fn empty() -> Self {
        Self {
            device: PciExpressDeviceCapabilitySpec::new(0, 0, 0),
            link: PciExpressLinkCapabilitySpec::new(0, 0, 0),
            slot: PciExpressSlotCapabilitySpec::empty(),
        }
    }

    pub const fn device(self) -> PciExpressDeviceCapabilitySpec {
        self.device
    }

    pub const fn link(self) -> PciExpressLinkCapabilitySpec {
        self.link
    }

    pub const fn slot(self) -> PciExpressSlotCapabilitySpec {
        self.slot
    }
}

impl Default for PciExpressCapability2Spec {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciExpressCapabilitySpec {
    offset: PciConfigOffset,
    capability: u16,
    device: PciExpressDeviceCapabilitySpec,
    link: PciExpressLinkCapabilitySpec,
    slot: PciExpressSlotCapabilitySpec,
    root: PciExpressRootCapabilitySpec,
    capability2: PciExpressCapability2Spec,
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
            slot: PciExpressSlotCapabilitySpec::empty(),
            root: PciExpressRootCapabilitySpec::empty(),
            capability2: PciExpressCapability2Spec::empty(),
        })
    }

    pub const fn with_slot(mut self, slot: PciExpressSlotCapabilitySpec) -> Self {
        self.slot = slot;
        self
    }

    pub const fn with_root(mut self, root: PciExpressRootCapabilitySpec) -> Self {
        self.root = root;
        self
    }

    pub const fn with_capability2(mut self, capability2: PciExpressCapability2Spec) -> Self {
        self.capability2 = capability2;
        self
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

    pub const fn slot_capabilities(self) -> u32 {
        self.slot.capabilities()
    }

    pub const fn initial_slot_control(self) -> u16 {
        self.slot.initial_control()
    }

    pub const fn initial_slot_status(self) -> u16 {
        self.slot.initial_status()
    }

    pub const fn root_capabilities(self) -> u16 {
        self.root.capabilities()
    }

    pub const fn initial_root_control(self) -> u16 {
        self.root.initial_control()
    }

    pub const fn initial_root_status(self) -> u32 {
        self.root.initial_status()
    }

    pub const fn device_capabilities2(self) -> u32 {
        self.capability2.device().capabilities()
    }

    pub const fn initial_device_control2(self) -> u16 {
        self.capability2.device().initial_control()
    }

    pub const fn initial_device_status2(self) -> u16 {
        self.capability2.device().initial_status()
    }

    pub const fn link_capabilities2(self) -> u32 {
        self.capability2.link().capabilities()
    }

    pub const fn initial_link_control2(self) -> u16 {
        self.capability2.link().initial_control()
    }

    pub const fn initial_link_status2(self) -> u16 {
        self.capability2.link().initial_status()
    }

    pub const fn slot_capabilities2(self) -> u32 {
        self.capability2.slot().capabilities()
    }

    pub const fn initial_slot_control2(self) -> u16 {
        self.capability2.slot().initial_control()
    }

    pub const fn initial_slot_status2(self) -> u16 {
        self.capability2.slot().initial_status()
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
    slot_control: u16,
    slot_status: u16,
    root_control: u16,
    root_status: u32,
    device_control2: u16,
    device_status2: u16,
    link_control2: u16,
    link_status2: u16,
    slot_control2: u16,
    slot_status2: u16,
}

impl PciExpressCapabilityState {
    pub(crate) const fn new(spec: PciExpressCapabilitySpec) -> Self {
        Self {
            spec,
            device_control: spec.initial_device_control(),
            device_status: spec.initial_device_status(),
            link_control: spec.initial_link_control(),
            link_status: spec.initial_link_status(),
            slot_control: spec.initial_slot_control(),
            slot_status: spec.initial_slot_status(),
            root_control: spec.initial_root_control(),
            root_status: spec.initial_root_status(),
            device_control2: spec.initial_device_control2(),
            device_status2: spec.initial_device_status2(),
            link_control2: spec.initial_link_control2(),
            link_status2: spec.initial_link_status2(),
            slot_control2: spec.initial_slot_control2(),
            slot_status2: spec.initial_slot_status2(),
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
        write_u32_at(
            config,
            base + PCI_EXPRESS_SLOT_CAPABILITIES_OFFSET as usize,
            self.spec.slot_capabilities(),
        );
        write_u16_at(
            config,
            base + PCI_EXPRESS_SLOT_CONTROL_OFFSET as usize,
            self.slot_control,
        );
        write_u16_at(
            config,
            base + PCI_EXPRESS_SLOT_STATUS_OFFSET as usize,
            self.slot_status,
        );
        write_u16_at(
            config,
            base + PCI_EXPRESS_ROOT_CONTROL_OFFSET as usize,
            self.root_control,
        );
        write_u16_at(
            config,
            base + PCI_EXPRESS_ROOT_CAPABILITIES_OFFSET as usize,
            self.spec.root_capabilities(),
        );
        write_u32_at(
            config,
            base + PCI_EXPRESS_ROOT_STATUS_OFFSET as usize,
            self.root_status,
        );
        write_u32_at(
            config,
            base + PCI_EXPRESS_DEVICE_CAPABILITIES2_OFFSET as usize,
            self.spec.device_capabilities2(),
        );
        write_u16_at(
            config,
            base + PCI_EXPRESS_DEVICE_CONTROL2_OFFSET as usize,
            self.device_control2,
        );
        write_u16_at(
            config,
            base + PCI_EXPRESS_DEVICE_STATUS2_OFFSET as usize,
            self.device_status2,
        );
        write_u32_at(
            config,
            base + PCI_EXPRESS_LINK_CAPABILITIES2_OFFSET as usize,
            self.spec.link_capabilities2(),
        );
        write_u16_at(
            config,
            base + PCI_EXPRESS_LINK_CONTROL2_OFFSET as usize,
            self.link_control2,
        );
        write_u16_at(
            config,
            base + PCI_EXPRESS_LINK_STATUS2_OFFSET as usize,
            self.link_status2,
        );
        write_u32_at(
            config,
            base + PCI_EXPRESS_SLOT_CAPABILITIES2_OFFSET as usize,
            self.spec.slot_capabilities2(),
        );
        write_u16_at(
            config,
            base + PCI_EXPRESS_SLOT_CONTROL2_OFFSET as usize,
            self.slot_control2,
        );
        write_u16_at(
            config,
            base + PCI_EXPRESS_SLOT_STATUS2_OFFSET as usize,
            self.slot_status2,
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
            (PCI_EXPRESS_SLOT_CONTROL_OFFSET, 2) => {
                self.slot_control = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.slot_control);
                Ok(())
            }
            (PCI_EXPRESS_SLOT_STATUS_OFFSET, 2) => {
                self.slot_status = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.slot_status);
                Ok(())
            }
            (PCI_EXPRESS_ROOT_CONTROL_OFFSET, 2) => {
                self.root_control = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.root_control);
                Ok(())
            }
            (PCI_EXPRESS_ROOT_STATUS_OFFSET, 4) => {
                self.root_status = u32::from_le_bytes(data.try_into().unwrap());
                write_u32_at(config, offset.as_usize(), self.root_status);
                Ok(())
            }
            (PCI_EXPRESS_DEVICE_CONTROL2_OFFSET, 2) => {
                self.device_control2 = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.device_control2);
                Ok(())
            }
            (PCI_EXPRESS_DEVICE_STATUS2_OFFSET, 2) => {
                self.device_status2 = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.device_status2);
                Ok(())
            }
            (PCI_EXPRESS_LINK_CONTROL2_OFFSET, 2) => {
                self.link_control2 = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.link_control2);
                Ok(())
            }
            (PCI_EXPRESS_LINK_STATUS2_OFFSET, 2) => {
                self.link_status2 = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.link_status2);
                Ok(())
            }
            (PCI_EXPRESS_SLOT_CONTROL2_OFFSET, 2) => {
                self.slot_control2 = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.slot_control2);
                Ok(())
            }
            (PCI_EXPRESS_SLOT_STATUS2_OFFSET, 2) => {
                self.slot_status2 = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.slot_status2);
                Ok(())
            }
            (PCI_EXPRESS_DEVICE_CONTROL_OFFSET, _)
            | (PCI_EXPRESS_DEVICE_STATUS_OFFSET, _)
            | (PCI_EXPRESS_LINK_CONTROL_OFFSET, _)
            | (PCI_EXPRESS_LINK_STATUS_OFFSET, _)
            | (PCI_EXPRESS_SLOT_CONTROL_OFFSET, _)
            | (PCI_EXPRESS_SLOT_STATUS_OFFSET, _)
            | (PCI_EXPRESS_ROOT_CONTROL_OFFSET, _)
            | (PCI_EXPRESS_ROOT_STATUS_OFFSET, _)
            | (PCI_EXPRESS_DEVICE_CONTROL2_OFFSET, _)
            | (PCI_EXPRESS_DEVICE_STATUS2_OFFSET, _)
            | (PCI_EXPRESS_LINK_CONTROL2_OFFSET, _)
            | (PCI_EXPRESS_LINK_STATUS2_OFFSET, _)
            | (PCI_EXPRESS_SLOT_CONTROL2_OFFSET, _)
            | (PCI_EXPRESS_SLOT_STATUS2_OFFSET, _) => {
                Err(PciError::UnalignedPciExpressCapabilityWrite { offset, size })
            }
            _ => Err(PciError::ReadOnlyPciExpressCapabilityWrite { offset, size }),
        }
    }
}

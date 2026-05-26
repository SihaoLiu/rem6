use crate::VirtioError;

pub(crate) const PCI_CONFIG_SPACE_SIZE: u16 = 0x100;
pub(crate) const PCI_VENDOR_SPECIFIC_CAPABILITY_ID: u8 = 0x09;
pub(crate) const VIRTIO_PCI_CAP_SIZE: u8 = 0x10;
pub(crate) const VIRTIO_PCI_NOTIFY_CAP_SIZE: u8 = 0x14;
pub(crate) const VIRTIO_PCI_CAP64_SIZE: u8 = 0x18;

const PCI_CAPABILITY_MIN_OFFSET: u8 = 0x40;

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
pub struct VirtioPciCapabilityOffset(u8);

impl VirtioPciCapabilityOffset {
    pub const fn new(value: u8) -> Option<Self> {
        if value >= PCI_CAPABILITY_MIN_OFFSET && value.is_multiple_of(4) {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtioPciCapabilityKind {
    CommonConfig,
    NotifyConfig,
    IsrConfig,
    DeviceConfig,
    PciConfig,
    SharedMemoryConfig,
    VendorConfig,
}

impl VirtioPciCapabilityKind {
    pub const fn cfg_type(self) -> u8 {
        match self {
            Self::CommonConfig => 1,
            Self::NotifyConfig => 2,
            Self::IsrConfig => 3,
            Self::DeviceConfig => 4,
            Self::PciConfig => 5,
            Self::SharedMemoryConfig => 8,
            Self::VendorConfig => 9,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioPciCapabilityEntry {
    offset: VirtioPciCapabilityOffset,
    next: Option<VirtioPciCapabilityOffset>,
    kind: VirtioPciCapabilityKind,
    bar: VirtioPciBarIndex,
    id: u8,
    region_offset: u32,
    length: u32,
}

impl VirtioPciCapabilityEntry {
    pub fn new(
        offset: VirtioPciCapabilityOffset,
        next: Option<VirtioPciCapabilityOffset>,
        kind: VirtioPciCapabilityKind,
        bar: VirtioPciBarIndex,
        id: u8,
        region_offset: u32,
        length: u32,
    ) -> Result<Self, VirtioError> {
        if length == 0 {
            return Err(VirtioError::ZeroPciCapabilityRegion {
                cfg_type: kind.cfg_type(),
            });
        }
        validate_pci_capability_span(u16::from(offset.get()), VIRTIO_PCI_CAP_SIZE)?;
        Ok(Self {
            offset,
            next,
            kind,
            bar,
            id,
            region_offset,
            length,
        })
    }

    pub const fn offset(self) -> VirtioPciCapabilityOffset {
        self.offset
    }

    pub const fn next(self) -> Option<VirtioPciCapabilityOffset> {
        self.next
    }

    pub const fn kind(self) -> VirtioPciCapabilityKind {
        self.kind
    }

    pub const fn bar(self) -> VirtioPciBarIndex {
        self.bar
    }

    pub const fn id(self) -> u8 {
        self.id
    }

    pub const fn region_offset(self) -> u32 {
        self.region_offset
    }

    pub const fn length(self) -> u32 {
        self.length
    }

    pub fn bytes(self) -> [u8; VIRTIO_PCI_CAP_SIZE as usize] {
        let mut bytes = [0; VIRTIO_PCI_CAP_SIZE as usize];
        self.write_base_bytes(&mut bytes, VIRTIO_PCI_CAP_SIZE);
        bytes
    }

    fn write_base_bytes(self, bytes: &mut [u8], cap_len: u8) {
        bytes[0] = PCI_VENDOR_SPECIFIC_CAPABILITY_ID;
        bytes[1] = self.next.map_or(0, VirtioPciCapabilityOffset::get);
        bytes[2] = cap_len;
        bytes[3] = self.kind.cfg_type();
        bytes[4] = self.bar.get();
        bytes[5] = self.id;
        write_u32_le(bytes, 8, self.region_offset);
        write_u32_le(bytes, 12, self.length);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioPciNotifyCapabilityEntry {
    base: VirtioPciCapabilityEntry,
    notify_off_multiplier: u32,
}

impl VirtioPciNotifyCapabilityEntry {
    pub fn new(
        base: VirtioPciCapabilityEntry,
        notify_off_multiplier: u32,
    ) -> Result<Self, VirtioError> {
        if base.kind() != VirtioPciCapabilityKind::NotifyConfig {
            return Err(VirtioError::InvalidNotifyCapabilityKind {
                cfg_type: base.kind().cfg_type(),
            });
        }
        validate_pci_capability_span(u16::from(base.offset().get()), VIRTIO_PCI_NOTIFY_CAP_SIZE)?;
        Ok(Self {
            base,
            notify_off_multiplier,
        })
    }

    pub const fn base(self) -> VirtioPciCapabilityEntry {
        self.base
    }

    pub const fn notify_off_multiplier(self) -> u32 {
        self.notify_off_multiplier
    }

    pub fn bytes(self) -> [u8; VIRTIO_PCI_NOTIFY_CAP_SIZE as usize] {
        let mut bytes = [0; VIRTIO_PCI_NOTIFY_CAP_SIZE as usize];
        self.base
            .write_base_bytes(&mut bytes, VIRTIO_PCI_NOTIFY_CAP_SIZE);
        write_u32_le(&mut bytes, 16, self.notify_off_multiplier);
        bytes
    }
}

pub(crate) fn validate_pci_capability_span(offset: u16, length: u8) -> Result<(), VirtioError> {
    if offset + u16::from(length) > PCI_CONFIG_SPACE_SIZE {
        return Err(VirtioError::PciCapabilityOutOfConfig {
            offset,
            length: u64::from(length),
        });
    }
    Ok(())
}

pub(crate) fn write_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

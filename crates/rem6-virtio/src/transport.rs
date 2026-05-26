use std::collections::BTreeMap;

use rem6_memory::AccessSize;
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarSpec, PciClassCode, PciDeviceIdentity, PciEndpointConfig,
    PciFunctionAddress, PciRawCapabilitySpec,
};

use crate::{
    pci_capability::{VIRTIO_PCI_CAP_SIZE, VIRTIO_PCI_NOTIFY_CAP_SIZE},
    validate_notify_multiplier, VirtioError, VirtioPciBarIndex, VirtioPciCapabilityEntry,
    VirtioPciCapabilityKind, VirtioPciCapabilityOffset, VirtioPciNotifyCapabilityEntry,
    VirtioPciSharedMemoryCapabilities, VirtioPciSharedMemoryRegistry,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioPciTransportBarSpec {
    bar: VirtioPciBarIndex,
    kind: PciBarKind,
    size: AccessSize,
}

impl VirtioPciTransportBarSpec {
    pub const fn new(bar: VirtioPciBarIndex, kind: PciBarKind, size: AccessSize) -> Self {
        Self { bar, kind, size }
    }

    pub const fn bar(self) -> VirtioPciBarIndex {
        self.bar
    }

    pub const fn kind(self) -> PciBarKind {
        self.kind
    }

    pub const fn size(self) -> AccessSize {
        self.size
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioPciTransportRegion {
    bar: VirtioPciBarIndex,
    offset: u64,
    length: u32,
}

impl VirtioPciTransportRegion {
    pub const fn new(bar: VirtioPciBarIndex, offset: u64, length: u32) -> Self {
        Self {
            bar,
            offset,
            length,
        }
    }

    pub const fn bar(self) -> VirtioPciBarIndex {
        self.bar
    }

    pub const fn offset(self) -> u64 {
        self.offset
    }

    pub const fn length(self) -> u32 {
        self.length
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioPciNotifyRegion {
    region: VirtioPciTransportRegion,
    notify_off_multiplier: u32,
}

impl VirtioPciNotifyRegion {
    pub fn new(
        region: VirtioPciTransportRegion,
        notify_off_multiplier: u32,
    ) -> Result<Self, VirtioError> {
        validate_notify_multiplier(notify_off_multiplier)?;
        Ok(Self {
            region,
            notify_off_multiplier,
        })
    }

    pub const fn region(self) -> VirtioPciTransportRegion {
        self.region
    }

    pub const fn notify_off_multiplier(self) -> u32 {
        self.notify_off_multiplier
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioPciTransportEndpointSpec {
    function: PciFunctionAddress,
    identity: PciDeviceIdentity,
    class: PciClassCode,
}

impl VirtioPciTransportEndpointSpec {
    pub const fn new(
        function: PciFunctionAddress,
        identity: PciDeviceIdentity,
        class: PciClassCode,
    ) -> Self {
        Self {
            function,
            identity,
            class,
        }
    }

    pub const fn function(self) -> PciFunctionAddress {
        self.function
    }

    pub const fn identity(self) -> PciDeviceIdentity {
        self.identity
    }

    pub const fn class(self) -> PciClassCode {
        self.class
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioPciModernTransportSpec {
    endpoint: VirtioPciTransportEndpointSpec,
    capability_start: VirtioPciCapabilityOffset,
    bars: Vec<VirtioPciTransportBarSpec>,
    common: VirtioPciTransportRegion,
    notify: VirtioPciNotifyRegion,
    isr: VirtioPciTransportRegion,
    device_config: Option<VirtioPciTransportRegion>,
    shared_memory: Option<VirtioPciSharedMemoryRegistry>,
}

impl VirtioPciModernTransportSpec {
    pub fn new(
        endpoint: VirtioPciTransportEndpointSpec,
        capability_start: VirtioPciCapabilityOffset,
        bars: impl IntoIterator<Item = VirtioPciTransportBarSpec>,
        common: VirtioPciTransportRegion,
        notify: VirtioPciNotifyRegion,
        isr: VirtioPciTransportRegion,
    ) -> Self {
        Self {
            endpoint,
            capability_start,
            bars: bars.into_iter().collect(),
            common,
            notify,
            isr,
            device_config: None,
            shared_memory: None,
        }
    }

    pub fn with_device_config(mut self, region: VirtioPciTransportRegion) -> Self {
        self.device_config = Some(region);
        self
    }

    pub fn with_shared_memory(mut self, registry: VirtioPciSharedMemoryRegistry) -> Self {
        self.shared_memory = Some(registry);
        self
    }

    pub fn build_endpoint(&self) -> Result<PciEndpointConfig, VirtioError> {
        let bars = self.checked_bars()?;
        self.validate_regions(&bars)?;

        let mut endpoint = PciEndpointConfig::new(
            self.endpoint.function(),
            self.endpoint.identity(),
            self.endpoint.class(),
        );
        for bar in bars.values().copied() {
            let spec = PciBarSpec::new(pci_bar_index(bar.bar()), bar.kind(), bar.size())
                .map_err(VirtioError::pci_endpoint)?;
            endpoint
                .install_bar(spec)
                .map_err(VirtioError::pci_endpoint)?;
        }
        for capability in self.raw_capabilities()? {
            endpoint
                .install_raw_capability(capability)
                .map_err(VirtioError::pci_endpoint)?;
        }
        Ok(endpoint)
    }

    fn checked_bars(
        &self,
    ) -> Result<BTreeMap<VirtioPciBarIndex, VirtioPciTransportBarSpec>, VirtioError> {
        let mut bars = BTreeMap::new();
        for bar in self.bars.iter().copied() {
            if bars.insert(bar.bar(), bar).is_some() {
                return Err(VirtioError::DuplicatePciTransportBar {
                    bar: bar.bar().get(),
                });
            }
        }
        Ok(bars)
    }

    fn validate_regions(
        &self,
        bars: &BTreeMap<VirtioPciBarIndex, VirtioPciTransportBarSpec>,
    ) -> Result<(), VirtioError> {
        let regions = self.region_instances();
        for region in &regions {
            region.validate_against_bars(bars)?;
        }
        for (index, first) in regions.iter().enumerate() {
            for second in regions.iter().skip(index + 1) {
                if first.overlaps(*second)? {
                    return Err(VirtioError::OverlappingPciTransportRegion {
                        first_cfg_type: first.cfg_type,
                        second_cfg_type: second.cfg_type,
                        bar: first.bar.get(),
                    });
                }
            }
        }
        Ok(())
    }

    fn region_instances(&self) -> Vec<TransportRegionInstance> {
        let mut regions = vec![
            TransportRegionInstance::new(VirtioPciCapabilityKind::CommonConfig, self.common),
            TransportRegionInstance::new(
                VirtioPciCapabilityKind::NotifyConfig,
                self.notify.region(),
            ),
            TransportRegionInstance::new(VirtioPciCapabilityKind::IsrConfig, self.isr),
        ];
        if let Some(device_config) = self.device_config {
            regions.push(TransportRegionInstance::new(
                VirtioPciCapabilityKind::DeviceConfig,
                device_config,
            ));
        }
        if let Some(shared_memory) = &self.shared_memory {
            regions.extend(
                shared_memory
                    .regions()
                    .iter()
                    .map(|region| TransportRegionInstance {
                        cfg_type: VirtioPciCapabilityKind::SharedMemoryConfig.cfg_type(),
                        bar: region.bar(),
                        offset: region.offset(),
                        length: region.length(),
                    }),
            );
        }
        regions
    }

    fn raw_capabilities(&self) -> Result<Vec<PciRawCapabilitySpec>, VirtioError> {
        let common_offset = self.capability_start;
        let notify_offset = next_capability_offset(common_offset, VIRTIO_PCI_CAP_SIZE)?;
        let isr_offset = next_capability_offset(notify_offset, VIRTIO_PCI_NOTIFY_CAP_SIZE)?;
        let after_isr = next_capability_offset(isr_offset, VIRTIO_PCI_CAP_SIZE)?;
        let device_offset = if self.device_config.is_some() {
            Some(after_isr)
        } else {
            None
        };
        let shared_start = if device_offset.is_some() {
            next_capability_offset(after_isr, VIRTIO_PCI_CAP_SIZE)?
        } else {
            after_isr
        };
        let has_shared = self
            .shared_memory
            .as_ref()
            .is_some_and(|registry| !registry.regions().is_empty());
        let isr_next = device_offset.or(has_shared.then_some(shared_start));
        let device_next = has_shared.then_some(shared_start);

        let mut capabilities = Vec::new();
        capabilities.push(
            VirtioPciCapabilityEntry::new(
                common_offset,
                Some(notify_offset),
                VirtioPciCapabilityKind::CommonConfig,
                self.common.bar(),
                0,
                region_offset_u32(VirtioPciCapabilityKind::CommonConfig, self.common)?,
                self.common.length(),
            )?
            .raw_capability_spec(),
        );
        capabilities.push(
            VirtioPciNotifyCapabilityEntry::new(
                VirtioPciCapabilityEntry::new(
                    notify_offset,
                    Some(isr_offset),
                    VirtioPciCapabilityKind::NotifyConfig,
                    self.notify.region().bar(),
                    0,
                    region_offset_u32(VirtioPciCapabilityKind::NotifyConfig, self.notify.region())?,
                    self.notify.region().length(),
                )?,
                self.notify.notify_off_multiplier(),
            )?
            .raw_capability_spec(),
        );
        capabilities.push(
            VirtioPciCapabilityEntry::new(
                isr_offset,
                isr_next,
                VirtioPciCapabilityKind::IsrConfig,
                self.isr.bar(),
                0,
                region_offset_u32(VirtioPciCapabilityKind::IsrConfig, self.isr)?,
                self.isr.length(),
            )?
            .raw_capability_spec(),
        );
        if let Some(device_config) = self.device_config {
            capabilities.push(
                VirtioPciCapabilityEntry::new(
                    device_offset.expect("device capability offset"),
                    device_next,
                    VirtioPciCapabilityKind::DeviceConfig,
                    device_config.bar(),
                    0,
                    region_offset_u32(VirtioPciCapabilityKind::DeviceConfig, device_config)?,
                    device_config.length(),
                )?
                .raw_capability_spec(),
            );
        }
        if has_shared {
            let shared = VirtioPciSharedMemoryCapabilities::new(
                shared_start,
                self.shared_memory.as_ref().expect("shared memory registry"),
            )?;
            capabilities.extend(
                shared
                    .entries()
                    .iter()
                    .copied()
                    .map(|entry| entry.raw_capability_spec()),
            );
        }
        Ok(capabilities)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TransportRegionInstance {
    cfg_type: u8,
    bar: VirtioPciBarIndex,
    offset: u64,
    length: u64,
}

impl TransportRegionInstance {
    fn new(kind: VirtioPciCapabilityKind, region: VirtioPciTransportRegion) -> Self {
        Self {
            cfg_type: kind.cfg_type(),
            bar: region.bar(),
            offset: region.offset(),
            length: u64::from(region.length()),
        }
    }

    fn validate_against_bars(
        self,
        bars: &BTreeMap<VirtioPciBarIndex, VirtioPciTransportBarSpec>,
    ) -> Result<(), VirtioError> {
        if self.length == 0 {
            return Err(VirtioError::ZeroPciCapabilityRegion {
                cfg_type: self.cfg_type,
            });
        }
        let Some(bar) = bars.get(&self.bar) else {
            return Err(VirtioError::MissingPciTransportBar {
                cfg_type: self.cfg_type,
                bar: self.bar.get(),
            });
        };
        let end = self.end()?;
        if end > bar.size().bytes() {
            return Err(VirtioError::PciTransportRegionOutOfBar {
                cfg_type: self.cfg_type,
                bar: self.bar.get(),
                offset: self.offset,
                length: self.length,
                bar_length: bar.size().bytes(),
            });
        }
        Ok(())
    }

    fn end(self) -> Result<u64, VirtioError> {
        self.offset
            .checked_add(self.length)
            .ok_or(VirtioError::PciTransportRegionAddressOverflow {
                cfg_type: self.cfg_type,
                bar: self.bar.get(),
                offset: self.offset,
                length: self.length,
            })
    }

    fn overlaps(self, other: Self) -> Result<bool, VirtioError> {
        Ok(self.bar == other.bar && self.offset < other.end()? && other.offset < self.end()?)
    }
}

fn next_capability_offset(
    offset: VirtioPciCapabilityOffset,
    size: u8,
) -> Result<VirtioPciCapabilityOffset, VirtioError> {
    let next = u16::from(offset.get()) + u16::from(size);
    VirtioPciCapabilityOffset::new(next as u8).ok_or(VirtioError::PciCapabilityOutOfConfig {
        offset: next,
        length: 1,
    })
}

fn region_offset_u32(
    kind: VirtioPciCapabilityKind,
    region: VirtioPciTransportRegion,
) -> Result<u32, VirtioError> {
    u32::try_from(region.offset()).map_err(|_| VirtioError::PciTransportRegionOffsetTooLarge {
        cfg_type: kind.cfg_type(),
        bar: region.bar().get(),
        offset: region.offset(),
    })
}

fn pci_bar_index(bar: VirtioPciBarIndex) -> PciBarIndex {
    PciBarIndex::new(bar.get()).expect("validated VirtIO PCI BAR index")
}

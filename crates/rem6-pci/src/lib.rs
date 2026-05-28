use rem6_interrupt::InterruptLineId;
use rem6_memory::AccessSize;

mod bar;
mod bridge;
mod capability;
mod common;
mod error;
mod host;
mod interrupt;
mod mmio;
mod msi;
mod msix;
mod pcie;
mod pm;

use bar::{bar_index_for_offset, PciBarState};
use capability::PciEndpointCapabilityList;
use common::{write_common_command, write_common_status, write_u16_at, write_u32_at};

pub use bar::{
    PciBarIndex, PciBarKind, PciBarRange, PciBarSpec, PciHostAddressBases, PciHostAddressSpace,
    PciHostBarRange,
};
pub use bridge::{
    PciBridgeBusRange, PciBridgeConfig, PciBridgeConfigSnapshot, PciType1HeaderFields,
};
pub use capability::PciRawCapabilitySpec;
pub use error::PciError;
pub use host::{
    PciConfigAperture, PciDecodedConfigAddress, PciHostBridge, PciHostBridgeSnapshot,
    PciHostBridgeTopologySnapshot,
};
pub use interrupt::{
    PciLegacyInterruptMapper, PciLegacyInterruptPath, PciLegacyInterruptPolicy,
    PciLegacyInterruptPort, PciLegacyInterruptRoute, PciLegacyInterruptRouter,
    PciLegacyInterruptRouterSnapshot, PciLegacyInterruptRoutingEntry,
    PciLegacyInterruptRoutingTable, PciLegacyInterruptRoutingTableSnapshot,
};
pub use mmio::{PciBarMmioDevice, PciConfigMmioDevice};
pub use msi::{PciMsiCapabilitySpec, PciMsiMessage, PciMsiPort, PciMsiRoute};
pub use msix::{PciMsixCapabilitySpec, PciMsixPort, PciMsixRoute};
pub use pcie::{
    PciExpressCapability2Spec, PciExpressCapabilitySpec, PciExpressDeviceCapabilitySpec,
    PciExpressLinkCapabilitySpec, PciExpressRootCapabilitySpec, PciExpressSlotCapabilitySpec,
};
pub use pm::PciPowerManagementCapabilitySpec;

pub(crate) const PCI_CONFIG_SPACE_SIZE: usize = 256;
const PCI_VENDOR_ID_OFFSET: usize = 0x00;
const PCI_DEVICE_ID_OFFSET: usize = 0x02;
const PCI_COMMAND_OFFSET: usize = 0x04;
pub(crate) const PCI_STATUS_OFFSET: usize = 0x06;
const PCI_CLASS_REVISION_OFFSET: usize = 0x08;
const PCI_CACHE_LINE_SIZE_OFFSET: usize = 0x0c;
const PCI_LATENCY_TIMER_OFFSET: usize = 0x0d;
const PCI_HEADER_TYPE_OFFSET: usize = 0x0e;
const PCI_BIST_OFFSET: usize = 0x0f;
pub(crate) const PCI_BAR0_OFFSET: usize = 0x10;
const PCI_CARD_BUS_CIS_OFFSET: usize = 0x28;
const PCI_SUBSYSTEM_VENDOR_ID_OFFSET: usize = 0x2c;
const PCI_SUBSYSTEM_ID_OFFSET: usize = 0x2e;
const PCI_EXPANSION_ROM_OFFSET: usize = 0x30;
pub(crate) const PCI_CAPABILITY_PTR_OFFSET: usize = 0x34;
const PCI_INTERRUPT_LINE_OFFSET: usize = 0x3c;
const PCI_INTERRUPT_PIN_OFFSET: usize = 0x3d;
const PCI_MINIMUM_GRANT_OFFSET: usize = 0x3e;
const PCI_MAXIMUM_LATENCY_OFFSET: usize = 0x3f;
const PCI_TYPE0_HEADER_TYPE: u8 = 0x00;
pub(crate) const PCI_TYPE1_HEADER_TYPE: u8 = 0x01;
pub(crate) const PCI_STATUS_CAPABILITY_LIST: u8 = 0x10;
const PCI_COMMAND_IO_SPACE: u16 = 0x0001;
const PCI_COMMAND_MEMORY_SPACE: u16 = 0x0002;
pub(crate) const PCI_BAR_COUNT: usize = 6;
const PCI_CONFIG_FUNCTIONS_PER_BUS: u64 = 256;
const PCI_EXPANSION_ROM_SIZE_PROBE: u32 = 0xffff_fffe;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PciFunctionAddress {
    bus: u8,
    device: u8,
    function: u8,
}

impl PciFunctionAddress {
    pub const fn new(bus: u8, device: u8, function: u8) -> Result<Self, PciError> {
        if device >= 32 {
            return Err(PciError::InvalidDeviceNumber { device });
        }
        if function >= 8 {
            return Err(PciError::InvalidFunctionNumber { function });
        }
        Ok(Self {
            bus,
            device,
            function,
        })
    }

    pub const fn bus(self) -> u8 {
        self.bus
    }

    pub const fn device(self) -> u8 {
        self.device
    }

    pub const fn function(self) -> u8 {
        self.function
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciDeviceIdentity {
    vendor_id: u16,
    device_id: u16,
}

impl PciDeviceIdentity {
    pub const fn new(vendor_id: u16, device_id: u16) -> Self {
        Self {
            vendor_id,
            device_id,
        }
    }

    pub const fn vendor_id(self) -> u16 {
        self.vendor_id
    }

    pub const fn device_id(self) -> u16 {
        self.device_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciClassCode {
    class: u8,
    subclass: u8,
    prog_if: u8,
    revision: u8,
}

impl PciClassCode {
    pub const fn new(class: u8, subclass: u8, prog_if: u8, revision: u8) -> Self {
        Self {
            class,
            subclass,
            prog_if,
            revision,
        }
    }

    pub const fn class(self) -> u8 {
        self.class
    }

    pub const fn subclass(self) -> u8 {
        self.subclass
    }

    pub const fn prog_if(self) -> u8 {
        self.prog_if
    }

    pub const fn revision(self) -> u8 {
        self.revision
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciType0HeaderFields {
    cardbus_cis: u32,
    subsystem_vendor_id: u16,
    subsystem_id: u16,
    expansion_rom: u32,
    minimum_grant: u8,
    maximum_latency: u8,
}

impl PciType0HeaderFields {
    pub const fn new(
        cardbus_cis: u32,
        subsystem_vendor_id: u16,
        subsystem_id: u16,
        expansion_rom: u32,
        minimum_grant: u8,
        maximum_latency: u8,
    ) -> Self {
        Self {
            cardbus_cis,
            subsystem_vendor_id,
            subsystem_id,
            expansion_rom,
            minimum_grant,
            maximum_latency,
        }
    }

    pub const fn cardbus_cis(self) -> u32 {
        self.cardbus_cis
    }

    pub const fn subsystem_vendor_id(self) -> u16 {
        self.subsystem_vendor_id
    }

    pub const fn subsystem_id(self) -> u16 {
        self.subsystem_id
    }

    pub const fn expansion_rom(self) -> u32 {
        self.expansion_rom
    }

    pub const fn minimum_grant(self) -> u8 {
        self.minimum_grant
    }

    pub const fn maximum_latency(self) -> u8 {
        self.maximum_latency
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PciConfigOffset(u16);

impl PciConfigOffset {
    pub const fn new(value: u16) -> Result<Self, PciError> {
        if value as usize >= PCI_CONFIG_SPACE_SIZE {
            return Err(PciError::InvalidConfigOffset { offset: value });
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u16 {
        self.0
    }

    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PciInterruptPin {
    None,
    IntA,
    IntB,
    IntC,
    IntD,
}

impl PciInterruptPin {
    pub const fn from_config_value(value: u8) -> Result<Self, PciError> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::IntA),
            2 => Ok(Self::IntB),
            3 => Ok(Self::IntC),
            4 => Ok(Self::IntD),
            _ => Err(PciError::InvalidLegacyInterruptPinValue { value }),
        }
    }

    pub const fn config_value(self) -> u8 {
        match self {
            Self::None => 0,
            Self::IntA => 1,
            Self::IntB => 2,
            Self::IntC => 3,
            Self::IntD => 4,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciEndpointConfig {
    function: PciFunctionAddress,
    identity: PciDeviceIdentity,
    class: PciClassCode,
    config: [u8; PCI_CONFIG_SPACE_SIZE],
    bars: [Option<PciBarState>; PCI_BAR_COUNT],
    capabilities: PciEndpointCapabilityList,
    raw_capabilities: Vec<capability::PciRawCapabilityState>,
    pm: Option<pm::PciPowerManagementCapabilityState>,
    pcie: Option<pcie::PciExpressCapabilityState>,
    msi: Option<msi::PciMsiCapabilityState>,
    msix: Option<msix::PciMsixCapabilityState>,
}

impl PciEndpointConfig {
    pub fn new(
        function: PciFunctionAddress,
        identity: PciDeviceIdentity,
        class: PciClassCode,
    ) -> Self {
        let mut config = [0; PCI_CONFIG_SPACE_SIZE];
        write_u16_at(&mut config, PCI_VENDOR_ID_OFFSET, identity.vendor_id());
        write_u16_at(&mut config, PCI_DEVICE_ID_OFFSET, identity.device_id());
        config[PCI_CLASS_REVISION_OFFSET] = class.revision();
        config[PCI_CLASS_REVISION_OFFSET + 1] = class.prog_if();
        config[PCI_CLASS_REVISION_OFFSET + 2] = class.subclass();
        config[PCI_CLASS_REVISION_OFFSET + 3] = class.class();
        config[PCI_HEADER_TYPE_OFFSET] = PCI_TYPE0_HEADER_TYPE;

        Self {
            function,
            identity,
            class,
            config,
            bars: std::array::from_fn(|_| None),
            capabilities: PciEndpointCapabilityList::new(),
            raw_capabilities: Vec::new(),
            pm: None,
            pcie: None,
            msi: None,
            msix: None,
        }
    }

    pub fn with_interrupt(mut self, line: u8, pin: PciInterruptPin) -> Self {
        self.config[PCI_INTERRUPT_LINE_OFFSET] = line;
        self.config[PCI_INTERRUPT_PIN_OFFSET] = pin.config_value();
        self
    }

    pub fn with_type0_header(mut self, fields: PciType0HeaderFields) -> Self {
        write_u32_at(
            &mut self.config,
            PCI_CARD_BUS_CIS_OFFSET,
            fields.cardbus_cis(),
        );
        write_u16_at(
            &mut self.config,
            PCI_SUBSYSTEM_VENDOR_ID_OFFSET,
            fields.subsystem_vendor_id(),
        );
        write_u16_at(
            &mut self.config,
            PCI_SUBSYSTEM_ID_OFFSET,
            fields.subsystem_id(),
        );
        write_u32_at(
            &mut self.config,
            PCI_EXPANSION_ROM_OFFSET,
            fields.expansion_rom(),
        );
        self.config[PCI_MINIMUM_GRANT_OFFSET] = fields.minimum_grant();
        self.config[PCI_MAXIMUM_LATENCY_OFFSET] = fields.maximum_latency();
        self
    }

    pub const fn function(&self) -> PciFunctionAddress {
        self.function
    }

    pub const fn identity(&self) -> PciDeviceIdentity {
        self.identity
    }

    pub const fn class(&self) -> PciClassCode {
        self.class
    }

    pub const fn legacy_interrupt_line(&self) -> u8 {
        self.config[PCI_INTERRUPT_LINE_OFFSET]
    }

    pub fn legacy_interrupt_pin(&self) -> Result<PciInterruptPin, PciError> {
        PciInterruptPin::from_config_value(self.config[PCI_INTERRUPT_PIN_OFFSET])
    }

    pub fn legacy_interrupt_path(&self) -> Result<PciLegacyInterruptPath, PciError> {
        PciLegacyInterruptPath::new(self.function, self.legacy_interrupt_pin()?)
    }

    pub fn assign_legacy_interrupt_line(&mut self, line: InterruptLineId) -> Result<(), PciError> {
        let line = u8::try_from(line.get())
            .map_err(|_| PciError::LegacyInterruptConfigLineOverflow { line })?;
        self.config[PCI_INTERRUPT_LINE_OFFSET] = line;
        Ok(())
    }

    pub fn install_bar(&mut self, spec: PciBarSpec) -> Result<(), PciError> {
        let index = spec.index().as_usize();
        self.validate_bar_slot_free(spec.index())?;
        let upper_index = if spec.kind().is_64_bit() {
            let upper =
                PciBarIndex::new(spec.index().get() + 1).expect("validated 64-bit BAR pair");
            self.validate_bar_slot_free(upper)?;
            Some(upper)
        } else {
            None
        };

        let state = PciBarState::new(spec);
        write_u32_at(
            &mut self.config,
            spec.index().config_offset(),
            state.raw().expect("new PCI BAR endpoint raw value"),
        );
        self.bars[index] = Some(state);
        if let Some(upper) = upper_index {
            write_u32_at(&mut self.config, upper.config_offset(), 0);
            self.bars[upper.as_usize()] = Some(PciBarState::upper(spec.index()));
        }
        Ok(())
    }

    pub fn install_msi_capability(&mut self, spec: PciMsiCapabilitySpec) -> Result<(), PciError> {
        if self.msi.is_some() {
            return Err(PciError::DuplicateMsiCapability);
        }
        self.register_capability_region(spec.offset(), spec.size())?;
        let state = msi::PciMsiCapabilityState::new(spec);
        state.install_into(&mut self.config);
        self.msi = Some(state);
        self.rebuild_capability_list();
        Ok(())
    }

    pub fn install_pm_capability(
        &mut self,
        spec: PciPowerManagementCapabilitySpec,
    ) -> Result<(), PciError> {
        if self.pm.is_some() {
            return Err(PciError::DuplicatePowerManagementCapability);
        }
        self.register_capability_region(spec.offset(), spec.size())?;
        let state = pm::PciPowerManagementCapabilityState::new(spec);
        state.install_into(&mut self.config);
        self.pm = Some(state);
        self.rebuild_capability_list();
        Ok(())
    }

    pub fn install_pcie_capability(
        &mut self,
        spec: PciExpressCapabilitySpec,
    ) -> Result<(), PciError> {
        if self.pcie.is_some() {
            return Err(PciError::DuplicatePciExpressCapability);
        }
        self.register_capability_region(spec.offset(), spec.size())?;
        let state = pcie::PciExpressCapabilityState::new(spec);
        state.install_into(&mut self.config);
        self.pcie = Some(state);
        self.rebuild_capability_list();
        Ok(())
    }

    pub fn read_config(
        &self,
        offset: PciConfigOffset,
        size: AccessSize,
    ) -> Result<Vec<u8>, PciError> {
        let span = config_span(offset, size)?;
        Ok(self.config[span.start..span.end].to_vec())
    }

    pub fn write_config(&mut self, offset: PciConfigOffset, data: &[u8]) -> Result<(), PciError> {
        let size = access_size_from_len(data.len())?;
        let span = config_span(offset, size)?;
        if let Some(index) = bar_index_for_offset(offset) {
            if data.len() != 4 {
                return Err(PciError::UnalignedBarAccess { offset, size });
            }
            return self.write_bar(index, u32::from_le_bytes(data.try_into().unwrap()));
        }
        if let Some(pm) = self.pm.as_mut() {
            if pm.contains(offset, size) {
                return pm.write_config(offset, data, &mut self.config);
            }
        }
        if let Some(pcie) = self.pcie.as_mut() {
            if pcie.contains(offset, size) {
                return pcie.write_config(offset, data, &mut self.config);
            }
        }
        if let Some(msi) = self.msi.as_mut() {
            if msi.contains(offset, size) {
                return msi.write_config(offset, data, &mut self.config);
            }
        }
        if let Some(msix) = self.msix.as_mut() {
            if msix.contains(offset, size) {
                return msix.write_config(offset, data, &mut self.config);
            }
        }

        match offset.as_usize() {
            PCI_COMMAND_OFFSET if data.len() == 2 => {
                write_common_command(
                    &mut self.config,
                    PCI_COMMAND_OFFSET,
                    u16::from_le_bytes(data.try_into().unwrap()),
                );
                Ok(())
            }
            PCI_COMMAND_OFFSET if data.len() == 4 => {
                write_common_command(
                    &mut self.config,
                    PCI_COMMAND_OFFSET,
                    u16::from_le_bytes([data[0], data[1]]),
                );
                Ok(())
            }
            PCI_STATUS_OFFSET if data.len() == 2 => {
                write_common_status(
                    &mut self.config,
                    u16::from_le_bytes(data.try_into().unwrap()),
                    PCI_STATUS_CAPABILITY_LIST as u16,
                );
                Ok(())
            }
            PCI_CACHE_LINE_SIZE_OFFSET | PCI_LATENCY_TIMER_OFFSET if data.len() == 1 => {
                self.config[span.start] = data[0];
                Ok(())
            }
            PCI_BIST_OFFSET if data.len() == 1 => {
                self.config[PCI_BIST_OFFSET] = data[0];
                Ok(())
            }
            PCI_CACHE_LINE_SIZE_OFFSET if data.len() == 2 => {
                self.config[span.start..span.end].copy_from_slice(data);
                Ok(())
            }
            PCI_INTERRUPT_LINE_OFFSET if data.len() == 1 => {
                self.config[PCI_INTERRUPT_LINE_OFFSET] = data[0];
                Ok(())
            }
            PCI_EXPANSION_ROM_OFFSET if data.len() == 4 => {
                let value = u32::from_le_bytes(data.try_into().unwrap());
                let value = if value == PCI_EXPANSION_ROM_SIZE_PROBE {
                    u32::MAX
                } else {
                    value
                };
                write_u32_at(&mut self.config, PCI_EXPANSION_ROM_OFFSET, value);
                Ok(())
            }
            PCI_MINIMUM_GRANT_OFFSET | PCI_MAXIMUM_LATENCY_OFFSET if data.len() == 1 => Ok(()),
            _ => Err(PciError::ReadOnlyConfigWrite { offset, size }),
        }
    }

    pub fn read_u32(&self, offset: PciConfigOffset) -> Result<u32, PciError> {
        let bytes = self.read_config(offset, AccessSize::new(4).map_err(PciError::Memory)?)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub fn write_u32(&mut self, offset: PciConfigOffset, value: u32) -> Result<(), PciError> {
        self.write_config(offset, &value.to_le_bytes())
    }

    pub fn active_bar_ranges(&self) -> Vec<PciBarRange> {
        self.bars
            .iter()
            .filter_map(|bar| {
                let bar = bar.as_ref()?;
                let kind = bar.kind()?;
                if !self.bar_enabled(kind) {
                    return None;
                }
                bar.range().ok()
            })
            .collect()
    }

    pub fn msi_message(&self, vector: u8) -> Result<Option<PciMsiMessage>, PciError> {
        let state = self.msi.as_ref().ok_or(PciError::MissingMsiCapability {
            function: self.function,
        })?;
        state.message(self.function, vector)
    }

    pub fn snapshot(&self) -> PciEndpointConfigSnapshot {
        PciEndpointConfigSnapshot {
            function: self.function,
            identity: self.identity,
            class: self.class,
            config: self.config,
            bars: self.bars.clone(),
            capabilities: self.capabilities.clone(),
            raw_capabilities: self.raw_capabilities.clone(),
            pm: self.pm.clone(),
            pcie: self.pcie.clone(),
            msi: self.msi.clone(),
            msix: self.msix.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &PciEndpointConfigSnapshot) -> Result<(), PciError> {
        if self.function != snapshot.function {
            return Err(PciError::SnapshotFunctionMismatch {
                expected: self.function,
                actual: snapshot.function,
            });
        }
        if self.identity != snapshot.identity {
            return Err(PciError::SnapshotIdentityMismatch {
                expected: self.identity,
                actual: snapshot.identity,
            });
        }
        if self.class != snapshot.class {
            return Err(PciError::SnapshotClassMismatch {
                expected: self.class,
                actual: snapshot.class,
            });
        }
        for (index, (current, restored)) in self.bars.iter().zip(snapshot.bars.iter()).enumerate() {
            if current.as_ref().map(PciBarState::shape) != restored.as_ref().map(PciBarState::shape)
            {
                return Err(PciError::SnapshotBarMismatch {
                    index: PciBarIndex::new(index as u8).expect("snapshot bar index"),
                });
            }
        }
        if self.msi.as_ref().map(msi::PciMsiCapabilityState::spec)
            != snapshot.msi.as_ref().map(msi::PciMsiCapabilityState::spec)
        {
            return Err(PciError::SnapshotMsiCapabilityMismatch);
        }
        if self.msix.as_ref().map(msix::PciMsixCapabilityState::spec)
            != snapshot
                .msix
                .as_ref()
                .map(msix::PciMsixCapabilityState::spec)
        {
            return Err(PciError::SnapshotMsixCapabilityMismatch);
        }
        if self
            .raw_capabilities
            .iter()
            .map(capability::PciRawCapabilityState::spec)
            .ne(snapshot
                .raw_capabilities
                .iter()
                .map(capability::PciRawCapabilityState::spec))
        {
            return Err(PciError::SnapshotRawCapabilityMismatch);
        }
        if self
            .pm
            .as_ref()
            .map(pm::PciPowerManagementCapabilityState::spec)
            != snapshot
                .pm
                .as_ref()
                .map(pm::PciPowerManagementCapabilityState::spec)
        {
            return Err(PciError::SnapshotPowerManagementCapabilityMismatch);
        }
        if self
            .pcie
            .as_ref()
            .map(pcie::PciExpressCapabilityState::spec)
            != snapshot
                .pcie
                .as_ref()
                .map(pcie::PciExpressCapabilityState::spec)
        {
            return Err(PciError::SnapshotPciExpressCapabilityMismatch);
        }

        self.config = snapshot.config;
        self.bars = snapshot.bars.clone();
        self.capabilities = snapshot.capabilities.clone();
        self.raw_capabilities = snapshot.raw_capabilities.clone();
        self.pm = snapshot.pm.clone();
        self.pcie = snapshot.pcie.clone();
        self.msi = snapshot.msi.clone();
        self.msix = snapshot.msix.clone();
        Ok(())
    }

    fn write_bar(&mut self, index: PciBarIndex, value: u32) -> Result<(), PciError> {
        if let Some(owner) = self.bars[index.as_usize()]
            .as_ref()
            .and_then(PciBarState::owner)
        {
            let bar = self.bars[owner.as_usize()]
                .as_mut()
                .ok_or(PciError::MissingBar { index: owner })?;
            bar.write_upper(value);
            write_u32_at(
                &mut self.config,
                index.config_offset(),
                bar.upper_raw().expect("64-bit PCI BAR upper value"),
            );
            return Ok(());
        }

        let bar = self.bars[index.as_usize()]
            .as_mut()
            .ok_or(PciError::MissingBar { index })?;
        bar.write_lower(value);
        write_u32_at(
            &mut self.config,
            index.config_offset(),
            bar.raw().expect("PCI BAR endpoint raw value"),
        );
        Ok(())
    }

    fn validate_bar_slot_free(&self, index: PciBarIndex) -> Result<(), PciError> {
        match self.bars[index.as_usize()].as_ref() {
            None => Ok(()),
            Some(PciBarState::Endpoint { .. }) => Err(PciError::DuplicateBar { index }),
            Some(PciBarState::Upper { owner }) => Err(PciError::ReservedBar {
                index,
                owner: *owner,
            }),
        }
    }

    pub(crate) fn register_capability_region(
        &mut self,
        offset: PciConfigOffset,
        size: u64,
    ) -> Result<(), PciError> {
        self.capabilities.register(offset, size)
    }

    pub(crate) fn rebuild_capability_list(&mut self) {
        self.capabilities.rebuild(&mut self.config);
    }

    fn command(&self) -> u16 {
        u16::from_le_bytes(
            self.config[PCI_COMMAND_OFFSET..PCI_COMMAND_OFFSET + 2]
                .try_into()
                .unwrap(),
        )
    }

    fn bar_enabled(&self, kind: PciBarKind) -> bool {
        let command = self.command();
        match kind {
            PciBarKind::Memory32 { .. } | PciBarKind::Memory64 { .. } => {
                command & PCI_COMMAND_MEMORY_SPACE != 0
            }
            PciBarKind::LegacyIo { .. } | PciBarKind::Io => command & PCI_COMMAND_IO_SPACE != 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciEndpointConfigSnapshot {
    function: PciFunctionAddress,
    identity: PciDeviceIdentity,
    class: PciClassCode,
    config: [u8; PCI_CONFIG_SPACE_SIZE],
    bars: [Option<PciBarState>; PCI_BAR_COUNT],
    capabilities: PciEndpointCapabilityList,
    raw_capabilities: Vec<capability::PciRawCapabilityState>,
    pm: Option<pm::PciPowerManagementCapabilityState>,
    pcie: Option<pcie::PciExpressCapabilityState>,
    msi: Option<msi::PciMsiCapabilityState>,
    msix: Option<msix::PciMsixCapabilityState>,
}

impl PciEndpointConfigSnapshot {
    pub const fn function(&self) -> PciFunctionAddress {
        self.function
    }

    pub const fn identity(&self) -> PciDeviceIdentity {
        self.identity
    }

    pub const fn class(&self) -> PciClassCode {
        self.class
    }

    pub fn power_management_payload(&self) -> Option<Vec<u8>> {
        self.pm
            .as_ref()
            .map(pm::PciPowerManagementCapabilityState::to_bytes)
    }

    pub fn validate_power_management_payload(&self, payload: &[u8]) -> Result<(), PciError> {
        let decoded = pm::PciPowerManagementCapabilityState::from_bytes(payload)?;
        if self.pm.as_ref() == Some(&decoded) {
            Ok(())
        } else {
            Err(PciError::SnapshotPowerManagementCapabilityMismatch)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConfigSpan {
    start: usize,
    end: usize,
}

fn config_span(offset: PciConfigOffset, size: AccessSize) -> Result<ConfigSpan, PciError> {
    validate_config_access_size(size)?;
    let start = offset.as_usize();
    let end = start
        .checked_add(size.bytes() as usize)
        .ok_or(PciError::ConfigAccessOutOfRange { offset, size })?;
    if end > PCI_CONFIG_SPACE_SIZE {
        return Err(PciError::ConfigAccessOutOfRange { offset, size });
    }
    Ok(ConfigSpan { start, end })
}

fn validate_config_access_size(size: AccessSize) -> Result<(), PciError> {
    match size.bytes() {
        1 | 2 | 4 => Ok(()),
        _ => Err(PciError::InvalidConfigAccessSize { size }),
    }
}

fn access_size_from_len(len: usize) -> Result<AccessSize, PciError> {
    let size = AccessSize::new(len as u64).map_err(PciError::Memory)?;
    validate_config_access_size(size)?;
    Ok(size)
}

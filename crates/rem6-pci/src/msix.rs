use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptError, InterruptLineChannel, InterruptLinePort, InterruptRoute,
    InterruptSourceId,
};
use rem6_kernel::{ParallelSchedulerContext, PartitionEventId, SchedulerContext, Tick};
use rem6_memory::{AccessSize, Address};

use crate::{
    write_u16_at, write_u32_at, PciBarIndex, PciConfigOffset, PciEndpointConfig, PciError,
    PciFunctionAddress, PciMsiMessage, PCI_CAPABILITY_PTR_OFFSET, PCI_CONFIG_SPACE_SIZE,
    PCI_STATUS_CAPABILITY_LIST, PCI_STATUS_OFFSET,
};

const PCI_CAPABILITY_MIN_OFFSET: u16 = 0x40;
const PCI_MSIX_CAPABILITY_ID: u8 = 0x11;
const PCI_MSIX_CAPABILITY_SIZE: u64 = 0x0c;
const PCI_MSIX_CONTROL_OFFSET: u16 = 0x02;
const PCI_MSIX_TABLE_OFFSET: u16 = 0x04;
const PCI_MSIX_PBA_OFFSET: u16 = 0x08;
const PCI_MSIX_ENABLE_BIT: u16 = 1 << 15;
const PCI_MSIX_FUNCTION_MASK_BIT: u16 = 1 << 14;
const PCI_MSIX_TABLE_ENTRY_BYTES: u64 = 16;
const PCI_MSIX_PBA_ENTRY_BYTES: u64 = 8;
const PCI_MSIX_MAX_VECTORS: u16 = 2048;
const PCI_MSIX_VECTOR_MASK_BIT: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciMsixCapabilitySpec {
    offset: PciConfigOffset,
    vector_count: u16,
    table_bar: PciBarIndex,
    table_offset: Address,
    pba_bar: PciBarIndex,
    pba_offset: Address,
}

impl PciMsixCapabilitySpec {
    pub fn new(
        offset: PciConfigOffset,
        vector_count: u16,
        table_bar: PciBarIndex,
        table_offset: Address,
        pba_bar: PciBarIndex,
        pba_offset: Address,
    ) -> Result<Self, PciError> {
        let size = msix_capability_size();
        let raw_offset = offset.get();
        let end = u64::from(raw_offset) + size.bytes();
        if raw_offset < PCI_CAPABILITY_MIN_OFFSET
            || !raw_offset.is_multiple_of(4)
            || end > PCI_CONFIG_SPACE_SIZE as u64
        {
            return Err(PciError::InvalidMsixCapabilityOffset { offset, size });
        }
        if vector_count == 0 || vector_count > PCI_MSIX_MAX_VECTORS {
            return Err(PciError::InvalidMsixVectorCount {
                count: vector_count,
            });
        }
        if !table_offset.get().is_multiple_of(8) || !pba_offset.get().is_multiple_of(8) {
            return Err(PciError::InvalidMsixCapabilityOffset { offset, size });
        }
        let spec = Self {
            offset,
            vector_count,
            table_bar,
            table_offset,
            pba_bar,
            pba_offset,
        };
        if table_bar == pba_bar && spec.regions_overlap() {
            return Err(PciError::OverlappingMsixRegions { table_bar, pba_bar });
        }
        Ok(spec)
    }

    pub const fn offset(self) -> PciConfigOffset {
        self.offset
    }

    pub const fn vector_count(self) -> u16 {
        self.vector_count
    }

    pub const fn table_bar(self) -> PciBarIndex {
        self.table_bar
    }

    pub const fn table_offset(self) -> Address {
        self.table_offset
    }

    pub const fn pba_bar(self) -> PciBarIndex {
        self.pba_bar
    }

    pub const fn pba_offset(self) -> Address {
        self.pba_offset
    }

    fn table_size(self) -> u64 {
        u64::from(self.vector_count) * PCI_MSIX_TABLE_ENTRY_BYTES
    }

    fn pba_size(self) -> u64 {
        u64::from(self.vector_count).div_ceil(64) * PCI_MSIX_PBA_ENTRY_BYTES
    }

    fn table_end(self) -> u64 {
        self.table_offset
            .get()
            .checked_add(self.table_size())
            .expect("validated MSI-X table size")
    }

    fn pba_end(self) -> u64 {
        self.pba_offset
            .get()
            .checked_add(self.pba_size())
            .expect("validated MSI-X PBA size")
    }

    fn regions_overlap(self) -> bool {
        self.table_offset.get() < self.pba_end() && self.pba_offset.get() < self.table_end()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PciMsixCapabilityState {
    spec: PciMsixCapabilitySpec,
    enabled: bool,
    function_mask: bool,
    table: Vec<[u8; PCI_MSIX_TABLE_ENTRY_BYTES as usize]>,
    pba: Vec<u64>,
}

impl PciMsixCapabilityState {
    pub(crate) fn new(spec: PciMsixCapabilitySpec) -> Self {
        Self {
            spec,
            enabled: false,
            function_mask: false,
            table: vec![[0; PCI_MSIX_TABLE_ENTRY_BYTES as usize]; spec.vector_count() as usize],
            pba: vec![0; spec.vector_count().div_ceil(64) as usize],
        }
    }

    pub(crate) const fn spec(&self) -> PciMsixCapabilitySpec {
        self.spec
    }

    pub(crate) fn install_into(&self, config: &mut [u8; PCI_CONFIG_SPACE_SIZE]) {
        let base = self.spec.offset().as_usize();
        config[PCI_CAPABILITY_PTR_OFFSET] = self.spec.offset().get() as u8;
        config[PCI_STATUS_OFFSET] |= PCI_STATUS_CAPABILITY_LIST;
        config[base] = PCI_MSIX_CAPABILITY_ID;
        config[base + 1] = 0;
        write_u16_at(
            config,
            base + PCI_MSIX_CONTROL_OFFSET as usize,
            self.control(),
        );
        write_u32_at(
            config,
            base + PCI_MSIX_TABLE_OFFSET as usize,
            self.table_register(),
        );
        write_u32_at(
            config,
            base + PCI_MSIX_PBA_OFFSET as usize,
            self.pba_register(),
        );
    }

    pub(crate) fn contains(&self, offset: PciConfigOffset, size: AccessSize) -> bool {
        let start = offset.get() as u64;
        let end = start + size.bytes();
        let cap_start = self.spec.offset().get() as u64;
        let cap_end = cap_start + PCI_MSIX_CAPABILITY_SIZE;
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
            (PCI_MSIX_CONTROL_OFFSET, 2) => {
                self.write_control(u16::from_le_bytes(data.try_into().unwrap()));
                write_u16_at(config, offset.as_usize(), self.control());
                Ok(())
            }
            (PCI_MSIX_CONTROL_OFFSET, _) => Err(PciError::UnalignedMsixRegionAccess {
                address: Address::new(offset.get() as u64),
                size,
            }),
            _ => Err(PciError::ReadOnlyConfigWrite { offset, size }),
        }
    }

    pub(crate) fn read_region(
        &self,
        address: Address,
        size: AccessSize,
    ) -> Result<Vec<u8>, PciError> {
        if let Some((vector, field_offset)) = self.table_location(address, size)? {
            let entry = self.table[vector as usize];
            let start = field_offset as usize;
            let end = start + size.bytes() as usize;
            return Ok(entry[start..end].to_vec());
        }
        if let Some(word) = self.pba_location(address, size)? {
            return Ok(self.pba[word].to_le_bytes().to_vec());
        }
        Err(PciError::MsixRegionAccessOutsideTable { address, size })
    }

    pub(crate) fn write_region(&mut self, address: Address, data: &[u8]) -> Result<(), PciError> {
        let size = AccessSize::new(data.len() as u64).map_err(PciError::Memory)?;
        if let Some((vector, field_offset)) = self.table_location(address, size)? {
            let value = u32::from_le_bytes(
                data.try_into()
                    .map_err(|_| PciError::UnalignedMsixRegionAccess { address, size })?,
            );
            self.write_table_field(vector, field_offset, value);
            return Ok(());
        }
        if self.pba_location(address, size)?.is_some() {
            return Err(PciError::ReadOnlyMsixPbaWrite { address, size });
        }
        Err(PciError::MsixRegionAccessOutsideTable { address, size })
    }

    pub(crate) fn message(
        &self,
        function: PciFunctionAddress,
        vector: u16,
    ) -> Result<Option<PciMsiMessage>, PciError> {
        self.validate_vector(vector)?;
        if !self.enabled || self.function_mask || self.vector_masked(vector) {
            return Ok(None);
        }
        Ok(Some(self.entry_message(function, vector)))
    }

    pub(crate) fn delivery_message(
        &mut self,
        function: PciFunctionAddress,
        vector: u16,
    ) -> Result<Option<PciMsiMessage>, PciError> {
        self.validate_vector(vector)?;
        if !self.enabled {
            return Ok(None);
        }
        if self.function_mask || self.vector_masked(vector) {
            self.queue_pending(vector)?;
            return Ok(None);
        }
        Ok(Some(self.entry_message(function, vector)))
    }

    pub(crate) fn queue_pending(&mut self, vector: u16) -> Result<(), PciError> {
        self.validate_vector(vector)?;
        let word = (vector / 64) as usize;
        let bit = u64::from(vector % 64);
        self.pba[word] |= 1_u64 << bit;
        Ok(())
    }

    pub(crate) fn clear_pending(&mut self, vector: u16) -> Result<(), PciError> {
        self.validate_vector(vector)?;
        let word = (vector / 64) as usize;
        let bit = u64::from(vector % 64);
        self.pba[word] &= !(1_u64 << bit);
        Ok(())
    }

    fn write_control(&mut self, value: u16) {
        self.enabled = value & PCI_MSIX_ENABLE_BIT != 0;
        self.function_mask = value & PCI_MSIX_FUNCTION_MASK_BIT != 0;
    }

    fn control(&self) -> u16 {
        let mut control = self.spec.vector_count() - 1;
        if self.function_mask {
            control |= PCI_MSIX_FUNCTION_MASK_BIT;
        }
        if self.enabled {
            control |= PCI_MSIX_ENABLE_BIT;
        }
        control
    }

    fn table_register(&self) -> u32 {
        ((self.spec.table_offset().get() as u32) & !0x7) | u32::from(self.spec.table_bar().get())
    }

    fn pba_register(&self) -> u32 {
        ((self.spec.pba_offset().get() as u32) & !0x7) | u32::from(self.spec.pba_bar().get())
    }

    fn table_location(
        &self,
        address: Address,
        size: AccessSize,
    ) -> Result<Option<(u16, u64)>, PciError> {
        if size.bytes() != 4 && size.bytes() != 8 && size.bytes() != 16 {
            return Err(PciError::UnalignedMsixRegionAccess { address, size });
        }
        let start = address.get();
        let end = start + size.bytes();
        if start < self.spec.table_offset().get() || end > self.spec.table_end() {
            return Ok(None);
        }
        let offset = start - self.spec.table_offset().get();
        if !offset.is_multiple_of(4) {
            return Err(PciError::UnalignedMsixRegionAccess { address, size });
        }
        let vector = (offset / PCI_MSIX_TABLE_ENTRY_BYTES) as u16;
        let field_offset = offset % PCI_MSIX_TABLE_ENTRY_BYTES;
        if field_offset + size.bytes() > PCI_MSIX_TABLE_ENTRY_BYTES {
            return Err(PciError::UnalignedMsixRegionAccess { address, size });
        }
        Ok(Some((vector, field_offset)))
    }

    fn pba_location(&self, address: Address, size: AccessSize) -> Result<Option<usize>, PciError> {
        if size.bytes() != PCI_MSIX_PBA_ENTRY_BYTES {
            return Ok(None);
        }
        let start = address.get();
        let end = start + size.bytes();
        if start < self.spec.pba_offset().get() || end > self.spec.pba_end() {
            return Ok(None);
        }
        let offset = start - self.spec.pba_offset().get();
        if !offset.is_multiple_of(PCI_MSIX_PBA_ENTRY_BYTES) {
            return Err(PciError::UnalignedMsixRegionAccess { address, size });
        }
        Ok(Some((offset / PCI_MSIX_PBA_ENTRY_BYTES) as usize))
    }

    fn write_table_field(&mut self, vector: u16, field_offset: u64, value: u32) {
        let value = match field_offset {
            0 => value & !0x3,
            12 => value & PCI_MSIX_VECTOR_MASK_BIT,
            _ => value,
        };
        let entry = &mut self.table[vector as usize];
        let start = field_offset as usize;
        entry[start..start + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn vector_masked(&self, vector: u16) -> bool {
        let entry = self.table[vector as usize];
        u32::from_le_bytes(entry[12..16].try_into().unwrap()) & PCI_MSIX_VECTOR_MASK_BIT != 0
    }

    fn entry_message(&self, function: PciFunctionAddress, vector: u16) -> PciMsiMessage {
        let entry = self.table[vector as usize];
        let lower = u32::from_le_bytes(entry[0..4].try_into().unwrap()) & !0x3;
        let upper = u32::from_le_bytes(entry[4..8].try_into().unwrap());
        let data = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as u16;
        PciMsiMessage::new(
            function,
            vector as u8,
            Address::new((u64::from(upper) << 32) | u64::from(lower)),
            data,
        )
    }

    fn validate_vector(&self, vector: u16) -> Result<(), PciError> {
        if vector >= self.spec.vector_count() {
            return Err(PciError::InvalidMsixVector {
                vector,
                vector_count: self.spec.vector_count(),
            });
        }
        Ok(())
    }
}

impl PciEndpointConfig {
    pub fn install_msix_capability(&mut self, spec: PciMsixCapabilitySpec) -> Result<(), PciError> {
        if self.msix.is_some() {
            return Err(PciError::DuplicateMsixCapability);
        }
        let state = PciMsixCapabilityState::new(spec);
        state.install_into(&mut self.config);
        self.msix = Some(state);
        Ok(())
    }

    pub fn read_msix_region(
        &self,
        address: Address,
        size: AccessSize,
    ) -> Result<Vec<u8>, PciError> {
        let state = self.msix.as_ref().ok_or(PciError::MissingMsixCapability {
            function: self.function,
        })?;
        state.read_region(address, size)
    }

    pub fn write_msix_region(&mut self, address: Address, data: &[u8]) -> Result<(), PciError> {
        let state = self.msix.as_mut().ok_or(PciError::MissingMsixCapability {
            function: self.function,
        })?;
        state.write_region(address, data)
    }

    pub fn msix_message(&self, vector: u16) -> Result<Option<PciMsiMessage>, PciError> {
        let state = self.msix.as_ref().ok_or(PciError::MissingMsixCapability {
            function: self.function,
        })?;
        state.message(self.function, vector)
    }

    pub fn queue_msix_pending(&mut self, vector: u16) -> Result<(), PciError> {
        let state = self.msix.as_mut().ok_or(PciError::MissingMsixCapability {
            function: self.function,
        })?;
        state.queue_pending(vector)
    }

    pub fn clear_msix_pending(&mut self, vector: u16) -> Result<(), PciError> {
        let state = self.msix.as_mut().ok_or(PciError::MissingMsixCapability {
            function: self.function,
        })?;
        state.clear_pending(vector)
    }

    fn msix_delivery_message(&mut self, vector: u16) -> Result<Option<PciMsiMessage>, PciError> {
        let state = self.msix.as_mut().ok_or(PciError::MissingMsixCapability {
            function: self.function,
        })?;
        state.delivery_message(self.function, vector)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciMsixRoute {
    function: PciFunctionAddress,
    vector: u16,
    message: PciMsiMessage,
    interrupt_route: InterruptRoute,
    signal_latency: Tick,
}

impl PciMsixRoute {
    pub fn new(
        function: PciFunctionAddress,
        vector: u16,
        message: PciMsiMessage,
        interrupt_route: InterruptRoute,
        signal_latency: Tick,
    ) -> Result<Self, PciError> {
        if message.function() != function {
            return Err(PciError::MsixEndpointMismatch {
                expected: function,
                actual: message.function(),
            });
        }
        if u16::from(message.vector()) != vector {
            return Err(PciError::InvalidMsixVector {
                vector: u16::from(message.vector()),
                vector_count: vector.saturating_add(1),
            });
        }
        InterruptLineChannel::new(interrupt_route, signal_latency).map_err(PciError::Interrupt)?;
        Ok(Self {
            function,
            vector,
            message,
            interrupt_route,
            signal_latency,
        })
    }

    pub const fn function(self) -> PciFunctionAddress {
        self.function
    }

    pub const fn vector(self) -> u16 {
        self.vector
    }

    pub const fn message(self) -> PciMsiMessage {
        self.message
    }

    pub const fn interrupt_route(self) -> InterruptRoute {
        self.interrupt_route
    }

    pub const fn signal_latency(self) -> Tick {
        self.signal_latency
    }

    fn channel(self) -> Result<InterruptLineChannel, PciError> {
        InterruptLineChannel::new(self.interrupt_route, self.signal_latency)
            .map_err(PciError::Interrupt)
    }
}

#[derive(Clone, Debug)]
pub struct PciMsixPort {
    route: PciMsixRoute,
    port: InterruptLinePort,
}

impl PciMsixPort {
    pub fn new(
        route: PciMsixRoute,
        controller: Arc<Mutex<InterruptController>>,
    ) -> Result<Self, PciError> {
        Ok(Self {
            route,
            port: InterruptLinePort::new(route.channel()?, controller),
        })
    }

    pub const fn route(&self) -> PciMsixRoute {
        self.route
    }

    pub fn controller(&self) -> Arc<Mutex<InterruptController>> {
        self.port.controller()
    }

    pub fn delivery_errors(&self) -> Arc<Mutex<Vec<InterruptError>>> {
        self.port.delivery_errors()
    }

    pub fn send(
        &self,
        endpoint: &mut PciEndpointConfig,
        context: &mut SchedulerContext<'_>,
        source: InterruptSourceId,
    ) -> Result<Option<PartitionEventId>, PciError> {
        self.validate_endpoint(endpoint)?;
        let Some(message) = endpoint.msix_delivery_message(self.route.vector())? else {
            return Ok(None);
        };
        self.validate_message(message)?;
        self.port
            .assert(context, source)
            .map(Some)
            .map_err(PciError::Interrupt)
    }

    pub fn send_parallel(
        &self,
        endpoint: &mut PciEndpointConfig,
        context: &mut ParallelSchedulerContext<'_>,
        source: InterruptSourceId,
    ) -> Result<Option<PartitionEventId>, PciError> {
        self.validate_endpoint(endpoint)?;
        let Some(message) = endpoint.msix_delivery_message(self.route.vector())? else {
            return Ok(None);
        };
        self.validate_message(message)?;
        self.port
            .assert_parallel(context, source)
            .map(Some)
            .map_err(PciError::Interrupt)
    }

    fn validate_endpoint(&self, endpoint: &PciEndpointConfig) -> Result<(), PciError> {
        if endpoint.function() != self.route.function() {
            return Err(PciError::MsixEndpointMismatch {
                expected: self.route.function(),
                actual: endpoint.function(),
            });
        }
        Ok(())
    }

    fn validate_message(&self, actual: PciMsiMessage) -> Result<(), PciError> {
        if actual != self.route.message() {
            return Err(PciError::MsixMessageMismatch {
                expected: self.route.message(),
                actual,
            });
        }
        Ok(())
    }
}

fn msix_capability_size() -> AccessSize {
    AccessSize::new(PCI_MSIX_CAPABILITY_SIZE).expect("MSI-X capability size is nonzero")
}

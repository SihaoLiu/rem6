use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptError, InterruptLineChannel, InterruptLinePort, InterruptRoute,
    InterruptSourceId,
};
use rem6_kernel::{ParallelSchedulerContext, PartitionEventId, SchedulerContext, Tick};
use rem6_memory::{AccessSize, Address};

use crate::{
    write_u16_at, write_u32_at, PciConfigOffset, PciEndpointConfig, PciError, PciFunctionAddress,
    PCI_CONFIG_SPACE_SIZE,
};

const PCI_CAPABILITY_MIN_OFFSET: u16 = 0x40;
const PCI_MSI_CAPABILITY_ID: u8 = 0x05;
const PCI_MSI_CAPABILITY_SIZE: u64 = 0x18;
const PCI_MSI_CONTROL_OFFSET: u16 = 0x02;
const PCI_MSI_ADDRESS_OFFSET: u16 = 0x04;
const PCI_MSI_UPPER_ADDRESS_OFFSET: u16 = 0x08;
const PCI_MSI_DATA_OFFSET: u16 = 0x0c;
const PCI_MSI_MASK_OFFSET: u16 = 0x10;
const PCI_MSI_PENDING_OFFSET: u16 = 0x14;
const PCI_MSI_ENABLE_BIT: u16 = 1 << 0;
const PCI_MSI_MULTIPLE_CAPABLE_SHIFT: u16 = 1;
const PCI_MSI_MULTIPLE_ENABLE_SHIFT: u16 = 4;
const PCI_MSI_64_BIT_CAPABLE_BIT: u16 = 1 << 7;
const PCI_MSI_PER_VECTOR_MASK_BIT: u16 = 1 << 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciMsiCapabilitySpec {
    offset: PciConfigOffset,
    vector_count: u8,
    supports_64_bit: bool,
    per_vector_masking: bool,
}

impl PciMsiCapabilitySpec {
    pub fn new(
        offset: PciConfigOffset,
        vector_count: u8,
        supports_64_bit: bool,
        per_vector_masking: bool,
    ) -> Result<Self, PciError> {
        let size = msi_capability_size();
        let raw_offset = offset.get();
        let end = u64::from(raw_offset) + size.bytes();
        if raw_offset < PCI_CAPABILITY_MIN_OFFSET
            || !raw_offset.is_multiple_of(4)
            || end > PCI_CONFIG_SPACE_SIZE as u64
        {
            return Err(PciError::InvalidMsiCapabilityOffset { offset, size });
        }
        if vector_count == 0 || vector_count > 32 || !vector_count.is_power_of_two() {
            return Err(PciError::InvalidMsiVectorCount {
                count: vector_count,
            });
        }

        Ok(Self {
            offset,
            vector_count,
            supports_64_bit,
            per_vector_masking,
        })
    }

    pub const fn offset(self) -> PciConfigOffset {
        self.offset
    }

    pub const fn vector_count(self) -> u8 {
        self.vector_count
    }

    pub const fn supports_64_bit(self) -> bool {
        self.supports_64_bit
    }

    pub const fn per_vector_masking(self) -> bool {
        self.per_vector_masking
    }

    pub const fn size(self) -> u64 {
        PCI_MSI_CAPABILITY_SIZE
    }

    const fn multiple_message_capable_bits(self) -> u8 {
        self.vector_count.trailing_zeros() as u8
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciMsiMessage {
    function: PciFunctionAddress,
    vector: u8,
    address: Address,
    data: u16,
}

impl PciMsiMessage {
    pub const fn new(
        function: PciFunctionAddress,
        vector: u8,
        address: Address,
        data: u16,
    ) -> Self {
        Self {
            function,
            vector,
            address,
            data,
        }
    }

    pub const fn function(self) -> PciFunctionAddress {
        self.function
    }

    pub const fn vector(self) -> u8 {
        self.vector
    }

    pub const fn address(self) -> Address {
        self.address
    }

    pub const fn data(self) -> u16 {
        self.data
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PciMsiCapabilityState {
    spec: PciMsiCapabilitySpec,
    enabled: bool,
    enabled_vector_bits: u8,
    address: u64,
    data: u16,
    mask_bits: u32,
    pending_bits: u32,
}

impl PciMsiCapabilityState {
    pub(crate) const fn new(spec: PciMsiCapabilitySpec) -> Self {
        Self {
            spec,
            enabled: false,
            enabled_vector_bits: 0,
            address: 0,
            data: 0,
            mask_bits: 0,
            pending_bits: 0,
        }
    }

    pub(crate) const fn spec(&self) -> PciMsiCapabilitySpec {
        self.spec
    }

    pub(crate) fn install_into(&self, config: &mut [u8; PCI_CONFIG_SPACE_SIZE]) {
        let base = self.spec.offset().as_usize();
        config[base] = PCI_MSI_CAPABILITY_ID;
        config[base + 1] = 0;
        write_u16_at(
            config,
            base + PCI_MSI_CONTROL_OFFSET as usize,
            self.control(),
        );
        write_u32_at(config, base + PCI_MSI_ADDRESS_OFFSET as usize, 0);
        write_u32_at(config, base + PCI_MSI_UPPER_ADDRESS_OFFSET as usize, 0);
        write_u16_at(config, base + PCI_MSI_DATA_OFFSET as usize, 0);
        write_u32_at(config, base + PCI_MSI_MASK_OFFSET as usize, 0);
        write_u32_at(config, base + PCI_MSI_PENDING_OFFSET as usize, 0);
    }

    pub(crate) fn contains(&self, offset: PciConfigOffset, size: AccessSize) -> bool {
        let start = offset.get() as u64;
        let end = start + size.bytes();
        let cap_start = self.spec.offset().get() as u64;
        let cap_end = cap_start + PCI_MSI_CAPABILITY_SIZE;
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
            (PCI_MSI_CONTROL_OFFSET, 2) => {
                self.write_control(u16::from_le_bytes(data.try_into().unwrap()));
                write_u16_at(config, offset.as_usize(), self.control());
                Ok(())
            }
            (PCI_MSI_ADDRESS_OFFSET, 4) => {
                let value = u32::from_le_bytes(data.try_into().unwrap()) & !0x3;
                self.address = (self.address & 0xffff_ffff_0000_0000) | u64::from(value);
                write_u32_at(config, offset.as_usize(), value);
                Ok(())
            }
            (PCI_MSI_UPPER_ADDRESS_OFFSET, 4) if self.spec.supports_64_bit() => {
                let value = u32::from_le_bytes(data.try_into().unwrap());
                self.address = (self.address & 0x0000_0000_ffff_ffff) | (u64::from(value) << 32);
                write_u32_at(config, offset.as_usize(), value);
                Ok(())
            }
            (PCI_MSI_DATA_OFFSET, 2) => {
                self.data = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.data);
                Ok(())
            }
            (PCI_MSI_MASK_OFFSET, 4) if self.spec.per_vector_masking() => {
                self.mask_bits = u32::from_le_bytes(data.try_into().unwrap());
                write_u32_at(config, offset.as_usize(), self.mask_bits);
                Ok(())
            }
            (PCI_MSI_CONTROL_OFFSET, _)
            | (PCI_MSI_ADDRESS_OFFSET, _)
            | (PCI_MSI_UPPER_ADDRESS_OFFSET, _)
            | (PCI_MSI_DATA_OFFSET, _)
            | (PCI_MSI_MASK_OFFSET, _) => {
                Err(PciError::UnalignedMsiCapabilityWrite { offset, size })
            }
            _ => Err(PciError::ReadOnlyMsiCapabilityWrite { offset, size }),
        }
    }

    pub(crate) fn message(
        &self,
        function: PciFunctionAddress,
        vector: u8,
    ) -> Result<Option<PciMsiMessage>, PciError> {
        if vector >= self.spec.vector_count() {
            return Err(PciError::InvalidMsiVector {
                vector,
                vector_count: self.spec.vector_count(),
            });
        }
        let enabled_vectors = 1_u8 << self.enabled_vector_bits;
        if !self.enabled || vector >= enabled_vectors || self.masked(vector) {
            return Ok(None);
        }

        Ok(Some(PciMsiMessage::new(
            function,
            vector,
            Address::new(self.address),
            self.data.wrapping_add(u16::from(vector)),
        )))
    }

    fn write_control(&mut self, value: u16) {
        self.enabled = value & PCI_MSI_ENABLE_BIT != 0;
        let requested = ((value >> PCI_MSI_MULTIPLE_ENABLE_SHIFT) & 0x7) as u8;
        self.enabled_vector_bits = requested.min(self.spec.multiple_message_capable_bits());
    }

    fn control(&self) -> u16 {
        let mut control =
            (self.spec.multiple_message_capable_bits() as u16) << PCI_MSI_MULTIPLE_CAPABLE_SHIFT;
        if self.enabled {
            control |= PCI_MSI_ENABLE_BIT;
        }
        control |= (self.enabled_vector_bits as u16) << PCI_MSI_MULTIPLE_ENABLE_SHIFT;
        if self.spec.supports_64_bit() {
            control |= PCI_MSI_64_BIT_CAPABLE_BIT;
        }
        if self.spec.per_vector_masking() {
            control |= PCI_MSI_PER_VECTOR_MASK_BIT;
        }
        control
    }

    fn masked(&self, vector: u8) -> bool {
        self.spec.per_vector_masking() && (self.mask_bits & (1_u32 << vector)) != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciMsiRoute {
    function: PciFunctionAddress,
    vector: u8,
    message: PciMsiMessage,
    interrupt_route: InterruptRoute,
    signal_latency: Tick,
}

impl PciMsiRoute {
    pub fn new(
        function: PciFunctionAddress,
        vector: u8,
        message: PciMsiMessage,
        interrupt_route: InterruptRoute,
        signal_latency: Tick,
    ) -> Result<Self, PciError> {
        if message.function() != function {
            return Err(PciError::MsiEndpointMismatch {
                expected: function,
                actual: message.function(),
            });
        }
        if message.vector() != vector {
            return Err(PciError::InvalidMsiVector {
                vector: message.vector(),
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

    pub const fn vector(self) -> u8 {
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
pub struct PciMsiPort {
    route: PciMsiRoute,
    port: InterruptLinePort,
}

impl PciMsiPort {
    pub fn new(
        route: PciMsiRoute,
        controller: Arc<Mutex<InterruptController>>,
    ) -> Result<Self, PciError> {
        Ok(Self {
            route,
            port: InterruptLinePort::new(route.channel()?, controller),
        })
    }

    pub const fn route(&self) -> PciMsiRoute {
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
        endpoint: &PciEndpointConfig,
        context: &mut SchedulerContext<'_>,
        source: InterruptSourceId,
    ) -> Result<Option<PartitionEventId>, PciError> {
        self.validate_endpoint(endpoint)?;
        let Some(message) = endpoint.msi_message(self.route.vector())? else {
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
        endpoint: &PciEndpointConfig,
        context: &mut ParallelSchedulerContext<'_>,
        source: InterruptSourceId,
    ) -> Result<Option<PartitionEventId>, PciError> {
        self.validate_endpoint(endpoint)?;
        let Some(message) = endpoint.msi_message(self.route.vector())? else {
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
            return Err(PciError::MsiEndpointMismatch {
                expected: self.route.function(),
                actual: endpoint.function(),
            });
        }
        Ok(())
    }

    fn validate_message(&self, actual: PciMsiMessage) -> Result<(), PciError> {
        if actual != self.route.message() {
            return Err(PciError::MsiMessageMismatch {
                expected: self.route.message(),
                actual,
            });
        }
        Ok(())
    }
}

fn msi_capability_size() -> AccessSize {
    AccessSize::new(PCI_MSI_CAPABILITY_SIZE).expect("MSI capability size is nonzero")
}

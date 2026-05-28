use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptError, InterruptLineChannel, InterruptLinePort, InterruptRoute,
    InterruptSourceId,
};
use rem6_kernel::{ParallelSchedulerContext, PartitionEventId, SchedulerContext, Tick};
use rem6_memory::{AccessSize, Address};

use crate::{
    write_u16_at, write_u32_at, PciBarIndex, PciConfigOffset, PciEndpointConfig, PciError,
    PciFunctionAddress, PciMsiMessage, PCI_CONFIG_SPACE_SIZE,
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
const PCI_MSIX_SNAPSHOT_MAGIC: &[u8; 8] = b"R6PCMX01";
const PCI_MSIX_SNAPSHOT_VERSION: u16 = 1;
const PCI_MSIX_SNAPSHOT_ENABLED: u8 = 1 << 0;
const PCI_MSIX_SNAPSHOT_FUNCTION_MASK: u8 = 1 << 1;
const U16_BYTES: usize = 2;
const U64_BYTES: usize = 8;

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

    pub const fn size(self) -> u64 {
        PCI_MSIX_CAPABILITY_SIZE
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

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(PCI_MSIX_SNAPSHOT_MAGIC);
        write_u16(&mut payload, PCI_MSIX_SNAPSHOT_VERSION);
        write_u16(&mut payload, self.spec.offset().get());
        write_u16(&mut payload, self.spec.vector_count());
        payload.push(self.spec.table_bar().get());
        payload.push(self.spec.pba_bar().get());
        payload.push(self.snapshot_flags());
        write_u64(&mut payload, self.spec.table_offset().get());
        write_u64(&mut payload, self.spec.pba_offset().get());
        for entry in &self.table {
            payload.extend_from_slice(entry);
        }
        for word in &self.pba {
            write_u64(&mut payload, *word);
        }
        payload
    }

    pub(crate) fn from_bytes(payload: &[u8]) -> Result<Self, PciError> {
        decode_state(payload).ok_or(PciError::InvalidMsixCapabilitySnapshot)
    }

    pub(crate) fn install_into(&self, config: &mut [u8; PCI_CONFIG_SPACE_SIZE]) {
        let base = self.spec.offset().as_usize();
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

    fn snapshot_flags(&self) -> u8 {
        let mut flags = 0;
        if self.enabled {
            flags |= PCI_MSIX_SNAPSHOT_ENABLED;
        }
        if self.function_mask {
            flags |= PCI_MSIX_SNAPSHOT_FUNCTION_MASK;
        }
        flags
    }
}

fn decode_state(payload: &[u8]) -> Option<PciMsixCapabilityState> {
    let mut cursor = 0;
    let magic = read_exact(payload, &mut cursor, PCI_MSIX_SNAPSHOT_MAGIC.len())?;
    if magic != PCI_MSIX_SNAPSHOT_MAGIC {
        return None;
    }
    if read_u16(payload, &mut cursor)? != PCI_MSIX_SNAPSHOT_VERSION {
        return None;
    }
    let offset = PciConfigOffset::new(read_u16(payload, &mut cursor)?).ok()?;
    let vector_count = read_u16(payload, &mut cursor)?;
    let table_bar = PciBarIndex::new(read_u8(payload, &mut cursor)?).ok()?;
    let pba_bar = PciBarIndex::new(read_u8(payload, &mut cursor)?).ok()?;
    let flags = read_u8(payload, &mut cursor)?;
    let known_flags = PCI_MSIX_SNAPSHOT_ENABLED | PCI_MSIX_SNAPSHOT_FUNCTION_MASK;
    if flags & !known_flags != 0 {
        return None;
    }
    let table_offset = Address::new(read_u64(payload, &mut cursor)?);
    let pba_offset = Address::new(read_u64(payload, &mut cursor)?);
    let spec = PciMsixCapabilitySpec::new(
        offset,
        vector_count,
        table_bar,
        table_offset,
        pba_bar,
        pba_offset,
    )
    .ok()?;
    let mut table = Vec::with_capacity(vector_count as usize);
    for _ in 0..vector_count {
        let entry = read_msix_table_entry(payload, &mut cursor)?;
        if !msix_table_entry_is_valid(&entry) {
            return None;
        }
        table.push(entry);
    }
    let pba_words = vector_count.div_ceil(64) as usize;
    let mut pba = Vec::with_capacity(pba_words);
    for _ in 0..pba_words {
        pba.push(read_u64(payload, &mut cursor)?);
    }
    if cursor != payload.len() || !pba_tail_bits_are_valid(vector_count, &pba) {
        return None;
    }
    Some(PciMsixCapabilityState {
        spec,
        enabled: flags & PCI_MSIX_SNAPSHOT_ENABLED != 0,
        function_mask: flags & PCI_MSIX_SNAPSHOT_FUNCTION_MASK != 0,
        table,
        pba,
    })
}

fn msix_table_entry_is_valid(entry: &[u8; PCI_MSIX_TABLE_ENTRY_BYTES as usize]) -> bool {
    let lower = u32::from_le_bytes(entry[0..4].try_into().unwrap());
    let vector_control = u32::from_le_bytes(entry[12..16].try_into().unwrap());
    lower & 0x3 == 0 && vector_control & !PCI_MSIX_VECTOR_MASK_BIT == 0
}

fn pba_tail_bits_are_valid(vector_count: u16, pba: &[u64]) -> bool {
    let trailing = vector_count % 64;
    if trailing == 0 {
        return true;
    }
    let Some(last) = pba.last() else {
        return false;
    };
    let valid_mask = (1_u64 << trailing) - 1;
    last & !valid_mask == 0
}

fn write_u16(payload: &mut Vec<u8>, value: u16) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn read_u8(payload: &[u8], cursor: &mut usize) -> Option<u8> {
    let byte = *payload.get(*cursor)?;
    *cursor += 1;
    Some(byte)
}

fn read_u16(payload: &[u8], cursor: &mut usize) -> Option<u16> {
    let bytes = read_exact(payload, cursor, U16_BYTES)?;
    Some(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u64(payload: &[u8], cursor: &mut usize) -> Option<u64> {
    let bytes = read_exact(payload, cursor, U64_BYTES)?;
    Some(u64::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_msix_table_entry(
    payload: &[u8],
    cursor: &mut usize,
) -> Option<[u8; PCI_MSIX_TABLE_ENTRY_BYTES as usize]> {
    let bytes = read_exact(payload, cursor, PCI_MSIX_TABLE_ENTRY_BYTES as usize)?;
    Some(bytes.try_into().unwrap())
}

fn read_exact<'a>(payload: &'a [u8], cursor: &mut usize, length: usize) -> Option<&'a [u8]> {
    let end = cursor.checked_add(length)?;
    let bytes = payload.get(*cursor..end)?;
    *cursor = end;
    Some(bytes)
}

impl PciEndpointConfig {
    pub fn install_msix_capability(&mut self, spec: PciMsixCapabilitySpec) -> Result<(), PciError> {
        if self.msix.is_some() {
            return Err(PciError::DuplicateMsixCapability);
        }
        self.register_capability_region(spec.offset(), spec.size())?;
        let state = PciMsixCapabilityState::new(spec);
        state.install_into(&mut self.config);
        self.msix = Some(state);
        self.rebuild_capability_list();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn msix_spec(offset: u16) -> PciMsixCapabilitySpec {
        PciMsixCapabilitySpec::new(
            PciConfigOffset::new(offset).unwrap(),
            4,
            PciBarIndex::new(2).unwrap(),
            Address::new(0x100),
            PciBarIndex::new(2).unwrap(),
            Address::new(0x180),
        )
        .unwrap()
    }

    #[test]
    fn msix_capability_state_codec_preserves_table_and_pending_bits() {
        let spec = msix_spec(0x70);
        let mut state = PciMsixCapabilityState::new(spec);
        let mut config = [0; PCI_CONFIG_SPACE_SIZE];
        state.install_into(&mut config);
        state
            .write_config(
                PciConfigOffset::new(0x72).unwrap(),
                &0xc000_u16.to_le_bytes(),
                &mut config,
            )
            .unwrap();
        state
            .write_region(Address::new(0x120), &0xfee0_0123_u32.to_le_bytes())
            .unwrap();
        state
            .write_region(Address::new(0x124), &0x0000_0001_u32.to_le_bytes())
            .unwrap();
        state
            .write_region(Address::new(0x128), &0x0060_u32.to_le_bytes())
            .unwrap();
        state
            .write_region(Address::new(0x12c), &1_u32.to_le_bytes())
            .unwrap();
        state.queue_pending(2).unwrap();

        let decoded = PciMsixCapabilityState::from_bytes(&state.to_bytes()).unwrap();
        let mut decoded_config = [0; PCI_CONFIG_SPACE_SIZE];
        decoded.install_into(&mut decoded_config);

        assert_eq!(decoded, state);
        assert_eq!(&decoded_config[0x70..0x7c], &config[0x70..0x7c]);
        assert_eq!(
            decoded.read_region(Address::new(0x120), AccessSize::new(16).unwrap()),
            state.read_region(Address::new(0x120), AccessSize::new(16).unwrap())
        );
        assert_eq!(
            decoded.read_region(Address::new(0x180), AccessSize::new(8).unwrap()),
            state.read_region(Address::new(0x180), AccessSize::new(8).unwrap())
        );
    }

    #[test]
    fn msix_capability_state_codec_rejects_invalid_payloads() {
        let state = PciMsixCapabilityState::new(msix_spec(0x70));
        let mut payload = state.to_bytes();

        assert_eq!(
            PciMsixCapabilityState::from_bytes(&payload[..payload.len() - 1]),
            Err(PciError::InvalidMsixCapabilitySnapshot)
        );

        payload.push(0);
        assert_eq!(
            PciMsixCapabilityState::from_bytes(&payload),
            Err(PciError::InvalidMsixCapabilitySnapshot)
        );

        let mut invalid_version = state.to_bytes();
        invalid_version[8] = 0xff;
        assert_eq!(
            PciMsixCapabilityState::from_bytes(&invalid_version),
            Err(PciError::InvalidMsixCapabilitySnapshot)
        );

        let mut invalid_flags = state.to_bytes();
        invalid_flags[16] = 0x80;
        assert_eq!(
            PciMsixCapabilityState::from_bytes(&invalid_flags),
            Err(PciError::InvalidMsixCapabilitySnapshot)
        );

        let mut invalid_bar = state.to_bytes();
        invalid_bar[14] = 7;
        assert_eq!(
            PciMsixCapabilityState::from_bytes(&invalid_bar),
            Err(PciError::InvalidMsixCapabilitySnapshot)
        );

        let mut invalid_vector_control = state.to_bytes();
        invalid_vector_control[45] = 2;
        assert_eq!(
            PciMsixCapabilityState::from_bytes(&invalid_vector_control),
            Err(PciError::InvalidMsixCapabilitySnapshot)
        );

        let mut invalid_pending_tail = state.to_bytes();
        invalid_pending_tail[97] = 0x10;
        assert_eq!(
            PciMsixCapabilityState::from_bytes(&invalid_pending_tail),
            Err(PciError::InvalidMsixCapabilitySnapshot)
        );
    }
}

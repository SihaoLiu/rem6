use std::collections::BTreeMap;

use rem6_interrupt::InterruptLineId;
use rem6_memory::{AccessSize, Address, AddressRange};

use crate::bar::{host_address_space, PciHostAddressBases};
use crate::{
    PciBarRange, PciBridgeConfig, PciBridgeConfigSnapshot, PciConfigOffset, PciEndpointConfig,
    PciEndpointConfigSnapshot, PciError, PciFunctionAddress, PciHostBarRange,
    PciLegacyInterruptPath, PCI_CONFIG_FUNCTIONS_PER_BUS, PCI_CONFIG_SPACE_SIZE,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciConfigAperture {
    base: Address,
    bus_count: u8,
    device_bits: u8,
    range: AddressRange,
}

impl PciConfigAperture {
    pub fn cam(base: Address, bus_count: u8) -> Result<Self, PciError> {
        Self::new(base, bus_count, 8)
    }

    pub fn ecam(base: Address, bus_count: u8) -> Result<Self, PciError> {
        Self::new(base, bus_count, 12)
    }

    pub fn new(base: Address, bus_count: u8, device_bits: u8) -> Result<Self, PciError> {
        if bus_count == 0 {
            return Err(PciError::ZeroConfigBuses);
        }
        if !(8..=12).contains(&device_bits) {
            return Err(PciError::InvalidConfigDeviceBits { device_bits });
        }

        let slot_size = 1_u64 << device_bits;
        let bytes = (bus_count as u64)
            .checked_mul(PCI_CONFIG_FUNCTIONS_PER_BUS)
            .and_then(|slots| slots.checked_mul(slot_size))
            .ok_or(PciError::ConfigApertureSizeOverflow {
                bus_count,
                device_bits,
            })?;
        let size = AccessSize::new(bytes).map_err(PciError::Memory)?;
        let range = AddressRange::new(base, size).map_err(PciError::Memory)?;
        Ok(Self {
            base,
            bus_count,
            device_bits,
            range,
        })
    }

    pub const fn base(self) -> Address {
        self.base
    }

    pub const fn bus_count(self) -> u8 {
        self.bus_count
    }

    pub const fn device_bits(self) -> u8 {
        self.device_bits
    }

    pub const fn range(self) -> AddressRange {
        self.range
    }

    pub fn endpoint_config_range(
        self,
        function: PciFunctionAddress,
    ) -> Result<AddressRange, PciError> {
        self.validate_function(function)?;
        AddressRange::new(
            Address::new(self.function_base(function)?),
            AccessSize::new(1_u64 << self.device_bits).map_err(PciError::Memory)?,
        )
        .map_err(PciError::Memory)
    }

    pub fn config_address(
        self,
        function: PciFunctionAddress,
        offset: PciConfigOffset,
    ) -> Result<Address, PciError> {
        self.validate_function(function)?;
        self.function_base(function)?
            .checked_add(offset.get() as u64)
            .map(Address::new)
            .ok_or(PciError::ConfigApertureSizeOverflow {
                bus_count: self.bus_count,
                device_bits: self.device_bits,
            })
    }

    pub fn decode(self, address: Address) -> Result<PciDecodedConfigAddress, PciError> {
        if !self.range.contains(address) {
            return Err(PciError::ConfigAddressOutsideAperture {
                address,
                range: self.range,
            });
        }

        let relative = address.get() - self.base.get();
        let slot_offset_mask = (1_u64 << self.device_bits) - 1;
        let raw_offset = relative & slot_offset_mask;
        if raw_offset >= PCI_CONFIG_SPACE_SIZE as u64 {
            return Err(PciError::UnsupportedConfigAddressOffset {
                address,
                raw_offset,
                device_bits: self.device_bits,
            });
        }

        let slot = relative >> self.device_bits;
        let bus = (slot >> 8) as u8;
        let device = ((slot >> 3) & 0x1f) as u8;
        let function = (slot & 0x7) as u8;
        Ok(PciDecodedConfigAddress {
            function: PciFunctionAddress::new(bus, device, function)
                .expect("decoded PCI function address"),
            offset: PciConfigOffset::new(raw_offset as u16).expect("decoded PCI config offset"),
        })
    }

    fn validate_function(self, function: PciFunctionAddress) -> Result<(), PciError> {
        if function.bus() >= self.bus_count {
            return Err(PciError::FunctionOutsideAperture {
                function,
                bus_count: self.bus_count,
            });
        }
        Ok(())
    }

    fn function_base(self, function: PciFunctionAddress) -> Result<u64, PciError> {
        let slot = ((function.bus() as u64) << 8)
            | ((function.device() as u64) << 3)
            | function.function() as u64;
        self.base.get().checked_add(slot << self.device_bits).ok_or(
            PciError::ConfigApertureSizeOverflow {
                bus_count: self.bus_count,
                device_bits: self.device_bits,
            },
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciDecodedConfigAddress {
    function: PciFunctionAddress,
    offset: PciConfigOffset,
}

impl PciDecodedConfigAddress {
    pub const fn function(self) -> PciFunctionAddress {
        self.function
    }

    pub const fn offset(self) -> PciConfigOffset {
        self.offset
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciHostBridgeSnapshot {
    aperture: PciConfigAperture,
    address_bases: PciHostAddressBases,
    bridges: BTreeMap<PciFunctionAddress, PciBridgeConfigSnapshot>,
    endpoints: BTreeMap<PciFunctionAddress, PciEndpointConfigSnapshot>,
}

impl PciHostBridgeSnapshot {
    pub const fn aperture(&self) -> PciConfigAperture {
        self.aperture
    }

    pub const fn address_bases(&self) -> PciHostAddressBases {
        self.address_bases
    }

    pub fn bridges(&self) -> &BTreeMap<PciFunctionAddress, PciBridgeConfigSnapshot> {
        &self.bridges
    }

    pub fn endpoints(&self) -> &BTreeMap<PciFunctionAddress, PciEndpointConfigSnapshot> {
        &self.endpoints
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciHostBridge {
    aperture: PciConfigAperture,
    address_bases: PciHostAddressBases,
    bridges: BTreeMap<PciFunctionAddress, PciBridgeConfig>,
    endpoints: BTreeMap<PciFunctionAddress, PciEndpointConfig>,
}

impl PciHostBridge {
    pub fn new(aperture: PciConfigAperture) -> Self {
        Self::with_address_bases(aperture, PciHostAddressBases::zero())
    }

    pub fn with_address_bases(
        aperture: PciConfigAperture,
        address_bases: PciHostAddressBases,
    ) -> Self {
        Self {
            aperture,
            address_bases,
            bridges: BTreeMap::new(),
            endpoints: BTreeMap::new(),
        }
    }

    pub const fn aperture(&self) -> PciConfigAperture {
        self.aperture
    }

    pub const fn address_bases(&self) -> PciHostAddressBases {
        self.address_bases
    }

    pub fn endpoint(&self, function: PciFunctionAddress) -> Option<&PciEndpointConfig> {
        self.endpoints.get(&function)
    }

    pub fn bridge(&self, function: PciFunctionAddress) -> Option<&PciBridgeConfig> {
        self.bridges.get(&function)
    }

    pub fn endpoint_mut(&mut self, function: PciFunctionAddress) -> Option<&mut PciEndpointConfig> {
        self.endpoints.get_mut(&function)
    }

    pub fn endpoint_config_range(
        &self,
        function: PciFunctionAddress,
    ) -> Result<AddressRange, PciError> {
        self.aperture.endpoint_config_range(function)
    }

    pub fn legacy_interrupt_path(
        &self,
        function: PciFunctionAddress,
    ) -> Result<PciLegacyInterruptPath, PciError> {
        let endpoint = self
            .endpoint(function)
            .ok_or(PciError::MissingEndpoint { function })?;
        let mut path = endpoint.legacy_interrupt_path()?;
        let mut bus = function.bus();

        while bus != 0 {
            let bridge = self
                .parent_bridge_for_bus(bus)
                .ok_or(PciError::MissingBridgePath { function, bus })?;
            path = path.with_upstream_bridge(bridge.function());
            bus = bridge.function().bus();
        }

        Ok(path)
    }

    pub fn assign_legacy_interrupt_line(
        &mut self,
        function: PciFunctionAddress,
        line: InterruptLineId,
    ) -> Result<(), PciError> {
        self.endpoint_mut(function)
            .ok_or(PciError::MissingEndpoint { function })?
            .assign_legacy_interrupt_line(line)
    }

    pub fn snapshot(&self) -> PciHostBridgeSnapshot {
        PciHostBridgeSnapshot {
            aperture: self.aperture,
            address_bases: self.address_bases,
            bridges: self
                .bridges
                .iter()
                .map(|(function, bridge)| (*function, bridge.snapshot()))
                .collect(),
            endpoints: self
                .endpoints
                .iter()
                .map(|(function, endpoint)| (*function, endpoint.snapshot()))
                .collect(),
        }
    }

    pub fn restore(&mut self, snapshot: &PciHostBridgeSnapshot) -> Result<(), PciError> {
        if self.aperture != snapshot.aperture
            || self.address_bases != snapshot.address_bases
            || self.bridges.keys().ne(snapshot.bridges.keys())
            || self.endpoints.keys().ne(snapshot.endpoints.keys())
        {
            return Err(PciError::SnapshotHostBridgeMismatch);
        }

        let mut restored_bridges = BTreeMap::new();
        for (function, bridge) in &self.bridges {
            let bridge_snapshot = snapshot
                .bridges
                .get(function)
                .ok_or(PciError::SnapshotHostBridgeMismatch)?;
            let mut restored = bridge.clone();
            restored.restore(bridge_snapshot)?;
            restored_bridges.insert(*function, restored);
        }

        let mut restored_endpoints = BTreeMap::new();
        for (function, endpoint) in &self.endpoints {
            let endpoint_snapshot = snapshot
                .endpoints
                .get(function)
                .ok_or(PciError::SnapshotHostBridgeMismatch)?;
            let mut restored = endpoint.clone();
            restored.restore(endpoint_snapshot)?;
            restored_endpoints.insert(*function, restored);
        }

        self.bridges = restored_bridges;
        self.endpoints = restored_endpoints;
        Ok(())
    }

    pub fn register_endpoint(&mut self, endpoint: PciEndpointConfig) -> Result<(), PciError> {
        let function = endpoint.function();
        self.aperture.validate_function(function)?;
        if self.endpoints.contains_key(&function) || self.bridges.contains_key(&function) {
            return Err(PciError::DuplicateFunction { function });
        }
        self.endpoints.insert(function, endpoint);
        Ok(())
    }

    pub fn register_bridge(&mut self, bridge: PciBridgeConfig) -> Result<(), PciError> {
        let function = bridge.function();
        self.aperture.validate_function(function)?;
        if bridge.bus_range().primary() != function.bus() {
            return Err(PciError::BridgePrimaryBusMismatch {
                function,
                primary: bridge.bus_range().primary(),
            });
        }
        if bridge.bus_range().subordinate() >= self.aperture.bus_count() {
            return Err(PciError::BridgeBusRangeOutsideAperture {
                secondary: bridge.bus_range().secondary(),
                subordinate: bridge.bus_range().subordinate(),
                bus_count: self.aperture.bus_count(),
            });
        }
        if self.endpoints.contains_key(&function) || self.bridges.contains_key(&function) {
            return Err(PciError::DuplicateFunction { function });
        }
        self.bridges.insert(function, bridge);
        Ok(())
    }

    pub fn read_config_address(
        &self,
        address: Address,
        size: AccessSize,
    ) -> Result<Vec<u8>, PciError> {
        let decoded = self.aperture.decode(address)?;
        crate::config_span(decoded.offset(), size)?;
        if let Some(bridge) = self.bridge(decoded.function()) {
            return bridge.read_config(decoded.offset(), size);
        }
        if !self.function_config_accessible(decoded.function()) {
            return Ok(vec![0xff; size.bytes() as usize]);
        }
        let Some(endpoint) = self.endpoint(decoded.function()) else {
            return Ok(vec![0xff; size.bytes() as usize]);
        };
        endpoint.read_config(decoded.offset(), size)
    }

    pub fn write_config_address(&mut self, address: Address, data: &[u8]) -> Result<(), PciError> {
        let decoded = self.aperture.decode(address)?;
        let size = crate::access_size_from_len(data.len())?;
        crate::config_span(decoded.offset(), size)?;
        if let Some(bridge) = self.bridges.get_mut(&decoded.function()) {
            return bridge.write_config(decoded.offset(), data);
        }
        if !self.function_config_accessible(decoded.function()) {
            return Ok(());
        }
        let Some(endpoint) = self.endpoints.get_mut(&decoded.function()) else {
            return Ok(());
        };
        endpoint.write_config(decoded.offset(), data)
    }

    pub fn active_host_bar_ranges(&self) -> Result<Vec<PciHostBarRange>, PciError> {
        let mut ranges = Vec::new();
        for (function, bridge) in &self.bridges {
            for range in bridge.active_bar_ranges() {
                if !self.host_forwards_bar_range(*function, &range) {
                    continue;
                }
                self.push_host_bar_range(&mut ranges, *function, range)?;
            }
        }
        for (function, endpoint) in &self.endpoints {
            for range in endpoint.active_bar_ranges() {
                if !self.host_forwards_bar_range(*function, &range) {
                    continue;
                }
                self.push_host_bar_range(&mut ranges, *function, range)?;
            }
        }
        Ok(ranges)
    }

    fn function_config_accessible(&self, function: PciFunctionAddress) -> bool {
        function.bus() == 0 || self.bridge_for_bus(function.bus()).is_some()
    }

    fn bridge_for_bus(&self, bus: u8) -> Option<&PciBridgeConfig> {
        self.bridges.values().find(|bridge| bridge.routes_bus(bus))
    }

    fn parent_bridge_for_bus(&self, bus: u8) -> Option<&PciBridgeConfig> {
        self.bridges
            .values()
            .filter(|bridge| bridge.function().bus() < bus && bridge.routes_bus(bus))
            .max_by_key(|bridge| (bridge.bus_range().secondary(), bridge.function()))
    }

    fn host_forwards_bar_range(&self, function: PciFunctionAddress, range: &PciBarRange) -> bool {
        if function.bus() == 0 {
            return true;
        }
        self.bridge_for_bus(function.bus())
            .is_some_and(|bridge| bridge.allows_bar_range(range.kind(), range.range()))
    }

    fn push_host_bar_range(
        &self,
        ranges: &mut Vec<PciHostBarRange>,
        function: PciFunctionAddress,
        range: PciBarRange,
    ) -> Result<(), PciError> {
        let space = host_address_space(range.kind());
        let host_base = checked_base_plus_offset(
            self.address_bases.base_for_space(space),
            range.range().start(),
        )?;
        let host_range = PciHostBarRange::new(
            function,
            range.index(),
            space,
            range.range().start(),
            host_base,
            range.range().size(),
        )?;
        if let Some(existing) = ranges.iter().find(|existing: &&PciHostBarRange| {
            existing.space() == host_range.space()
                && existing.host_range().overlaps(host_range.host_range())
        }) {
            return Err(PciError::OverlappingHostBarRange {
                existing_function: existing.function(),
                existing_bar: existing.bar(),
                requested_function: host_range.function(),
                requested_bar: host_range.bar(),
            });
        }
        ranges.push(host_range);
        Ok(())
    }
}

fn checked_base_plus_offset(base: Address, offset: Address) -> Result<Address, PciError> {
    base.get()
        .checked_add(offset.get())
        .map(Address::new)
        .ok_or(PciError::HostAddressOverflow { base, offset })
}

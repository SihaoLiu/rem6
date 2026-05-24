use rem6_memory::{AccessSize, Address, AddressRange, CacheLineLayout, MemoryError};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CpuDataLineLayoutRange {
    range: AddressRange,
    line_layout: CacheLineLayout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuDataConfig {
    endpoint: TransportEndpointId,
    route: MemoryRouteId,
    line_layout: CacheLineLayout,
    address_line_layouts: Vec<CpuDataLineLayoutRange>,
}

impl CpuDataConfig {
    pub fn new(
        endpoint: TransportEndpointId,
        route: MemoryRouteId,
        line_layout: CacheLineLayout,
    ) -> Self {
        Self {
            endpoint,
            route,
            line_layout,
            address_line_layouts: Vec::new(),
        }
    }

    pub fn with_line_layout_range(
        mut self,
        range: AddressRange,
        line_layout: CacheLineLayout,
    ) -> Self {
        self.address_line_layouts
            .push(CpuDataLineLayoutRange { range, line_layout });
        self
    }

    pub fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub const fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub const fn line_layout(&self) -> CacheLineLayout {
        self.line_layout
    }

    pub fn line_layout_for_access(
        &self,
        address: Address,
        size: AccessSize,
    ) -> Result<CacheLineLayout, MemoryError> {
        let range = AddressRange::new(address, size)?;
        Ok(self.line_layout_for_range(range))
    }

    pub fn line_layout_for_range(&self, range: AddressRange) -> CacheLineLayout {
        self.address_line_layouts
            .iter()
            .find(|candidate| candidate.range.contains_range(range))
            .map(|candidate| candidate.line_layout)
            .unwrap_or(self.line_layout)
    }
}

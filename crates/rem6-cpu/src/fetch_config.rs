use rem6_memory::{AccessSize, Address, AddressRange, CacheLineLayout, MemoryError};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CpuFetchLineLayoutRange {
    range: AddressRange,
    line_layout: CacheLineLayout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuFetchConfig {
    endpoint: TransportEndpointId,
    route: MemoryRouteId,
    line_layout: CacheLineLayout,
    width: AccessSize,
    address_line_layouts: Vec<CpuFetchLineLayoutRange>,
}

impl CpuFetchConfig {
    pub fn new(
        endpoint: TransportEndpointId,
        route: MemoryRouteId,
        line_layout: CacheLineLayout,
        width: AccessSize,
    ) -> Self {
        Self {
            endpoint,
            route,
            line_layout,
            width,
            address_line_layouts: Vec::new(),
        }
    }

    pub fn with_line_layout_range(
        mut self,
        range: AddressRange,
        line_layout: CacheLineLayout,
    ) -> Self {
        self.add_line_layout_range(range, line_layout);
        self
    }

    pub fn add_line_layout_range(&mut self, range: AddressRange, line_layout: CacheLineLayout) {
        self.address_line_layouts
            .push(CpuFetchLineLayoutRange { range, line_layout });
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

    pub const fn width(&self) -> AccessSize {
        self.width
    }

    pub fn line_layout_for_fetch(&self, pc: Address) -> Result<CacheLineLayout, MemoryError> {
        let range = AddressRange::new(pc, self.width)?;
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

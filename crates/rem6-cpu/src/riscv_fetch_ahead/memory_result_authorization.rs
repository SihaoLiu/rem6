use rem6_isa_riscv::Register;
use rem6_memory::{AccessSize, Address, AddressRange};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3MemoryResultWindowRoute {
    Memory,
    Mmio,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3MemoryResultWindowRole {
    Head,
    YoungerRead,
    YoungerBufferedEffect,
}

impl O3MemoryResultWindowRole {
    pub(crate) const fn is_younger(self) -> bool {
        matches!(self, Self::YoungerRead | Self::YoungerBufferedEffect)
    }

    pub(crate) const fn is_buffered_effect(self) -> bool {
        matches!(self, Self::YoungerBufferedEffect)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3MemoryResultWindowAuthorization {
    integer_destination: Option<Register>,
    route: O3MemoryResultWindowRoute,
    physical_range: AddressRange,
    role: O3MemoryResultWindowRole,
}

impl O3MemoryResultWindowAuthorization {
    pub(in crate::riscv_fetch_ahead) const fn new(
        integer_destination: Option<Register>,
        route: O3MemoryResultWindowRoute,
        physical_range: AddressRange,
        role: O3MemoryResultWindowRole,
    ) -> Self {
        Self {
            integer_destination,
            route,
            physical_range,
            role,
        }
    }

    pub(crate) const fn integer_destination(self) -> Option<Register> {
        self.integer_destination
    }

    pub(crate) const fn role(self) -> O3MemoryResultWindowRole {
        self.role
    }

    pub(crate) const fn route(self) -> O3MemoryResultWindowRoute {
        self.route
    }

    pub(crate) const fn physical_range(self) -> AddressRange {
        self.physical_range
    }

    pub(crate) fn matches(
        self,
        route: O3MemoryResultWindowRoute,
        physical_address: Address,
        size: AccessSize,
    ) -> bool {
        self.route == route
            && AddressRange::new(physical_address, size)
                .is_ok_and(|range| range == self.physical_range)
    }
}

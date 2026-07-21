use rem6_isa_riscv::{Immediate, MemoryWidth, Register};
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
    YoungerDependentRead,
    YoungerBufferedEffect,
}

impl O3MemoryResultWindowRole {
    pub(crate) const fn is_younger(self) -> bool {
        matches!(
            self,
            Self::YoungerRead | Self::YoungerDependentRead | Self::YoungerBufferedEffect
        )
    }

    pub(crate) const fn is_buffered_effect(self) -> bool {
        matches!(self, Self::YoungerBufferedEffect)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3MemoryResultWindowAddressAuthority {
    ResolvedRange(AddressRange),
    DependentSource {
        register: Register,
        width: MemoryWidth,
        immediate: Immediate,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3MemoryResultWindowAuthorization {
    integer_destination: Option<Register>,
    route: O3MemoryResultWindowRoute,
    address_authority: O3MemoryResultWindowAddressAuthority,
    role: O3MemoryResultWindowRole,
}

impl O3MemoryResultWindowAuthorization {
    pub(in crate::riscv_fetch_ahead) const fn resolved(
        integer_destination: Option<Register>,
        route: O3MemoryResultWindowRoute,
        physical_range: AddressRange,
        role: O3MemoryResultWindowRole,
    ) -> Self {
        Self {
            integer_destination,
            route,
            address_authority: O3MemoryResultWindowAddressAuthority::ResolvedRange(physical_range),
            role,
        }
    }

    pub(in crate::riscv_fetch_ahead) const fn dependent(
        integer_destination: Register,
        register: Register,
        width: MemoryWidth,
        immediate: Immediate,
    ) -> Self {
        Self {
            integer_destination: Some(integer_destination),
            route: O3MemoryResultWindowRoute::Memory,
            address_authority: O3MemoryResultWindowAddressAuthority::DependentSource {
                register,
                width,
                immediate,
            },
            role: O3MemoryResultWindowRole::YoungerDependentRead,
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

    pub(crate) const fn resolved_range(self) -> Option<AddressRange> {
        match self.address_authority {
            O3MemoryResultWindowAddressAuthority::ResolvedRange(range) => Some(range),
            O3MemoryResultWindowAddressAuthority::DependentSource { .. } => None,
        }
    }

    pub(crate) const fn dependent_source(self) -> Option<(Register, MemoryWidth, Immediate)> {
        match self.address_authority {
            O3MemoryResultWindowAddressAuthority::ResolvedRange(_) => None,
            O3MemoryResultWindowAddressAuthority::DependentSource {
                register,
                width,
                immediate,
            } => Some((register, width, immediate)),
        }
    }

    pub(crate) fn matches_resolved_range(
        self,
        route: O3MemoryResultWindowRoute,
        physical_address: Address,
        size: AccessSize,
    ) -> bool {
        self.route == route
            && self.resolved_range().is_some_and(|physical_range| {
                AddressRange::new(physical_address, size).is_ok_and(|range| range == physical_range)
            })
    }
}

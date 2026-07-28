use rem6_isa_riscv::{Immediate, MemoryWidth, Register};
use rem6_memory::{AccessSize, Address, AddressRange};

#[path = "memory_result_authorization/translated.rs"]
mod translated;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3MemoryResultWindowRoute {
    Memory,
    Mmio,
    Translated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3MemoryResultWindowRole {
    Head,
    YoungerRead,
    YoungerDependentRead,
    YoungerDependentEffect,
    YoungerBufferedEffect,
}

impl O3MemoryResultWindowRole {
    pub(crate) const fn is_younger(self) -> bool {
        !matches!(self, Self::Head)
    }

    pub(crate) const fn is_buffered_effect(self) -> bool {
        matches!(self, Self::YoungerBufferedEffect)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3MemoryResultWindowAddressAuthority {
    ResolvedRange(AddressRange),
    TranslatedRange {
        virtual_range: AddressRange,
        physical_range: Option<AddressRange>,
        target: Option<O3MemoryResultWindowRoute>,
    },
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

    pub(crate) const fn restored_completed_fp_load(physical_range: AddressRange) -> Self {
        Self {
            integer_destination: None,
            route: O3MemoryResultWindowRoute::Memory,
            address_authority: O3MemoryResultWindowAddressAuthority::ResolvedRange(physical_range),
            role: O3MemoryResultWindowRole::Head,
        }
    }

    pub(in crate::riscv_fetch_ahead) const fn dependent(
        integer_destination: Option<Register>,
        register: Register,
        width: MemoryWidth,
        immediate: Immediate,
    ) -> Self {
        Self {
            integer_destination,
            route: O3MemoryResultWindowRoute::Memory,
            address_authority: O3MemoryResultWindowAddressAuthority::DependentSource {
                register,
                width,
                immediate,
            },
            role: if integer_destination.is_some() {
                O3MemoryResultWindowRole::YoungerDependentRead
            } else {
                O3MemoryResultWindowRole::YoungerDependentEffect
            },
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
            O3MemoryResultWindowAddressAuthority::TranslatedRange { physical_range, .. } => {
                physical_range
            }
            O3MemoryResultWindowAddressAuthority::DependentSource { .. } => None,
        }
    }

    pub(crate) const fn dependent_source(self) -> Option<(Register, MemoryWidth, Immediate)> {
        match self.address_authority {
            O3MemoryResultWindowAddressAuthority::ResolvedRange(_)
            | O3MemoryResultWindowAddressAuthority::TranslatedRange { .. } => None,
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
        let O3MemoryResultWindowAddressAuthority::ResolvedRange(physical_range) =
            self.address_authority
        else {
            return false;
        };
        self.route == route
            && AddressRange::new(physical_address, size).is_ok_and(|range| range == physical_range)
    }
}

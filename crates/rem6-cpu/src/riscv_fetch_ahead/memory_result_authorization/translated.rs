use super::*;

impl O3MemoryResultWindowAuthorization {
    pub(in crate::riscv_fetch_ahead) const fn translated_unbound(
        integer_destination: Option<Register>,
        virtual_range: AddressRange,
        role: O3MemoryResultWindowRole,
    ) -> Self {
        Self {
            integer_destination,
            route: O3MemoryResultWindowRoute::Translated,
            address_authority: O3MemoryResultWindowAddressAuthority::TranslatedRange {
                virtual_range,
                physical_range: None,
                target: None,
            },
            role,
        }
    }

    pub(crate) fn bind_translated(
        &mut self,
        virtual_address: Address,
        physical_address: Address,
        size: AccessSize,
    ) -> bool {
        let Ok(virtual_range) = AddressRange::new(virtual_address, size) else {
            return false;
        };
        let Ok(physical_range) = AddressRange::new(physical_address, size) else {
            return false;
        };
        let O3MemoryResultWindowAddressAuthority::TranslatedRange {
            virtual_range: authorized_virtual_range,
            physical_range: authorized_physical_range,
            ..
        } = &mut self.address_authority
        else {
            return false;
        };
        if *authorized_virtual_range != virtual_range {
            return false;
        }
        match authorized_physical_range {
            Some(range) => *range == physical_range,
            slot @ None => {
                *slot = Some(physical_range);
                true
            }
        }
    }

    pub(crate) fn bind_target(&mut self, route: O3MemoryResultWindowRoute) -> bool {
        if route == O3MemoryResultWindowRoute::Translated {
            return false;
        }
        let O3MemoryResultWindowAddressAuthority::TranslatedRange { target, .. } =
            &mut self.address_authority
        else {
            return false;
        };
        match target {
            Some(bound) => *bound == route,
            slot @ None => {
                *slot = Some(route);
                true
            }
        }
    }

    pub(crate) const fn is_translated(self) -> bool {
        matches!(
            self.address_authority,
            O3MemoryResultWindowAddressAuthority::TranslatedRange { .. }
        )
    }

    pub(crate) const fn virtual_range(self) -> Option<AddressRange> {
        match self.address_authority {
            O3MemoryResultWindowAddressAuthority::TranslatedRange { virtual_range, .. } => {
                Some(virtual_range)
            }
            O3MemoryResultWindowAddressAuthority::ResolvedRange(_)
            | O3MemoryResultWindowAddressAuthority::DependentSource { .. } => None,
        }
    }

    pub(crate) fn matches_virtual_range(self, address: Address, size: AccessSize) -> bool {
        self.virtual_range().is_some_and(|virtual_range| {
            AddressRange::new(address, size).is_ok_and(|range| range == virtual_range)
        })
    }

    pub(crate) fn matches_bound_target(
        self,
        route: O3MemoryResultWindowRoute,
        physical_address: Address,
        size: AccessSize,
    ) -> bool {
        let O3MemoryResultWindowAddressAuthority::TranslatedRange {
            physical_range: Some(physical_range),
            target: Some(target),
            ..
        } = self.address_authority
        else {
            return false;
        };
        target == route
            && AddressRange::new(physical_address, size).is_ok_and(|range| range == physical_range)
    }
}

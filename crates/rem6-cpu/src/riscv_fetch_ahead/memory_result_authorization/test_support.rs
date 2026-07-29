use super::*;

impl O3MemoryResultWindowAuthorization {
    pub(crate) const fn resolved_for_test(
        integer_destination: Option<Register>,
        route: O3MemoryResultWindowRoute,
        physical_range: AddressRange,
        role: O3MemoryResultWindowRole,
    ) -> Self {
        Self::resolved(integer_destination, route, physical_range, role)
    }

    pub(crate) const fn dependent_for_test(
        integer_destination: Option<Register>,
        register: Register,
        width: MemoryWidth,
        immediate: Immediate,
    ) -> Self {
        Self::dependent(integer_destination, register, width, immediate)
    }
}

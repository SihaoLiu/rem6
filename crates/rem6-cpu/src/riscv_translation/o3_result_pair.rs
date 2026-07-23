use rem6_memory::{AccessSize, Address, MemoryRequestId};

use crate::{
    riscv_fetch_ahead::O3MemoryResultWindowRoute, riscv_translation::TranslatedDataAccess,
    RiscvCoreState,
};

impl RiscvCoreState {
    pub(crate) fn translated_result_authorization_is_pending(
        &self,
        fetch_request: MemoryRequestId,
        virtual_address: Address,
        size: AccessSize,
    ) -> bool {
        self.memory_result_window_authorizations
            .get(&fetch_request)
            .copied()
            .is_some_and(|authorization| {
                authorization.route() == O3MemoryResultWindowRoute::Translated
                    && authorization.is_translated()
                    && authorization.resolved_range().is_none()
                    && authorization.matches_virtual_range(virtual_address, size)
            })
    }

    pub(crate) fn bind_translated_result_range(
        &mut self,
        translated: &TranslatedDataAccess,
    ) -> bool {
        let Some(authorization) = self
            .memory_result_window_authorizations
            .get_mut(&translated.fetch_request)
        else {
            return true;
        };
        authorization.bind_translated(
            translated.virtual_address,
            translated.physical_address,
            translated.size,
        )
    }

    pub(crate) fn bind_translated_result_target(
        &mut self,
        fetch_request: MemoryRequestId,
        route: O3MemoryResultWindowRoute,
    ) -> bool {
        let Some(authorization) = self
            .memory_result_window_authorizations
            .get_mut(&fetch_request)
        else {
            return true;
        };
        authorization.bind_target(route)
    }
}

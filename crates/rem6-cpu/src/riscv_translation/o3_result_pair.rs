use std::collections::BTreeSet;

use rem6_memory::{AccessSize, Address, MemoryRequestId};

use crate::{
    riscv_fetch_ahead::O3MemoryResultWindowRoute, riscv_translation::TranslatedDataAccess,
    RiscvCoreState,
};

impl RiscvCoreState {
    pub(crate) fn has_unbound_translated_result_state(&self) -> bool {
        !self.pending_data_translations.is_empty()
            || !self.ready_translated_data.is_empty()
            || !self.memory_result_window_authorizations.is_empty()
            || !self.translated_scalar_load_window_fetches.is_empty()
    }

    pub(crate) fn discard_translated_result_pair_from(&mut self, fetch_request: MemoryRequestId) {
        let owns_suffix = |request: MemoryRequestId| {
            request.agent() == fetch_request.agent()
                && request.sequence() >= fetch_request.sequence()
        };
        let mut affected = BTreeSet::from([fetch_request]);
        affected.extend(
            self.memory_result_window_authorizations
                .keys()
                .copied()
                .filter(|request| owns_suffix(*request)),
        );
        affected.extend(
            self.pending_data_translations
                .values()
                .map(|pending| pending.fetch_request)
                .filter(|request| owns_suffix(*request)),
        );
        affected.extend(
            self.ready_translated_data
                .keys()
                .copied()
                .filter(|request| owns_suffix(*request)),
        );
        affected.extend(
            self.translated_scalar_load_window_fetches
                .iter()
                .copied()
                .filter(|request| owns_suffix(*request)),
        );

        let translation_ids = self
            .pending_data_translations
            .iter()
            .filter_map(|(translation, pending)| {
                owns_suffix(pending.fetch_request).then_some(*translation)
            })
            .collect::<Vec<_>>();
        if let Some(frontend) = self.data_translation.as_mut() {
            for translation in translation_ids {
                frontend.discard_pending(translation);
            }
        }

        self.memory_result_window_authorizations
            .retain(|request, _| !owns_suffix(*request));

        for request in affected {
            self.abort_deferred_o3_live_data_access_execution(request);
        }
        self.pending_data_translations
            .retain(|_, pending| !owns_suffix(pending.fetch_request));
        self.ready_translated_data
            .retain(|request, _| !owns_suffix(*request));
        self.translated_scalar_load_window_fetches
            .retain(|request| !owns_suffix(*request));
    }

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

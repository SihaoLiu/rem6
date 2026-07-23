use std::collections::BTreeSet;

use super::*;
use crate::riscv_fetch_ahead::O3MemoryResultWindowRoute;

impl RiscvCoreState {
    pub(crate) fn has_unbound_translated_result_state(&self) -> bool {
        !self.pending_data_translations.is_empty()
            || !self.ready_translated_data.is_empty()
            || !self.memory_result_window_authorizations.is_empty()
            || !self.translated_scalar_load_window_fetches.is_empty()
    }

    pub(crate) fn translated_result_pair_retry_wake_tick(&self, now: Tick) -> Option<Tick> {
        (self.pending_trap.is_none()
            && self.outstanding_data.is_empty()
            && !self.ready_translated_data.is_empty()
            && !self.memory_result_window_authorizations.is_empty()
            && !self.o3_runtime.has_live_data_access())
        .then(|| now.checked_add(1))
        .flatten()
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

        for request in &affected {
            let Some(execution) = self.data_access_execution(*request).cloned() else {
                continue;
            };
            if self
                .o3_runtime
                .discard_live_staged_suffix_for_fetch_identity(
                    execution.fetch_pc(),
                    execution.instruction(),
                    &[*request],
                )
            {
                break;
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

impl RiscvCore {
    fn translated_result_pair_prepare_result<T>(
        &self,
        fetch_request: MemoryRequestId,
        tick: Tick,
        result: Result<T, RiscvCpuError>,
    ) -> Result<T, RiscvCpuError> {
        if result.is_err() {
            let mut state = self.state.lock().expect("riscv core lock");
            if state
                .memory_result_window_authorizations
                .get(&fetch_request)
                .is_some_and(|authorization| authorization.is_translated())
            {
                state.discard_translated_result_pair_from(fetch_request);
                state.abort_prepared_data_issue(fetch_request, tick);
            }
        }
        result
    }

    fn reject_uncacheable_translated_result_memory_target(
        &self,
        translated: &TranslatedDataAccess,
        tick: Tick,
    ) -> Result<(), RiscvCpuError> {
        let is_authorized = self
            .state
            .lock()
            .expect("riscv core lock")
            .memory_result_window_authorizations
            .get(&translated.fetch_request)
            .is_some_and(|authorization| authorization.is_translated());
        if !is_authorized {
            return Ok(());
        }
        let uncacheable_result = self
            .state
            .lock()
            .expect("riscv core lock")
            .pma
            .is_uncacheable(translated.physical_address.get(), translated.size.bytes())
            .map_err(|error| RiscvCpuError::DataPmaAccess {
                fetch: translated.fetch_request,
                error,
            });
        let uncacheable = self.translated_result_pair_prepare_result(
            translated.fetch_request,
            tick,
            uncacheable_result,
        )?;
        if uncacheable {
            return self.translated_result_pair_prepare_result(
                translated.fetch_request,
                tick,
                Err(RiscvCpuError::TranslatedResultAuthorizationMismatch {
                    fetch: translated.fetch_request,
                }),
            );
        }
        Ok(())
    }

    pub(super) fn prepare_ready_translated_mmio_data_access(
        &self,
        scheduler: &PartitionedScheduler,
        bus: &MmioBus,
    ) -> Result<Option<OutstandingDataAccess>, RiscvCpuError> {
        let tick = scheduler.now();
        let translated = {
            let state = self.state.lock().expect("riscv core lock");
            let Some(fetch_request) = ready_translated_fetch_request(&state) else {
                return Ok(None);
            };
            state
                .ready_translated_data
                .get(&fetch_request)
                .expect("selected ready data translation exists")
                .clone()
        };
        self.translated_result_pair_prepare_result(
            translated.fetch_request,
            tick,
            self.check_pmp_data_access(
                translated.fetch_request,
                &translated.access,
                translated.size,
                translated.physical_address,
            ),
        )?;
        self.translated_result_pair_prepare_result(
            translated.fetch_request,
            tick,
            self.check_pma_data_access(
                translated.fetch_request,
                &translated.access,
                translated.size,
                translated.physical_address,
                translated.request_byte_offset,
            ),
        )?;
        let route_probe = self.translated_result_pair_prepare_result(
            translated.fetch_request,
            tick,
            MmioRequest::read(
                MmioRequestId::new(translated.request_id.sequence()),
                translated.physical_address,
                translated.size,
            )
            .map_err(RiscvCpuError::Mmio),
        )?;
        let route = match bus.route_for(self.core.partition(), &route_probe) {
            Ok(route) => route,
            Err(MmioError::UnmappedAddress { .. }) => return Ok(None),
            Err(error) => {
                return self.translated_result_pair_prepare_result(
                    translated.fetch_request,
                    tick,
                    Err(RiscvCpuError::Mmio(error)),
                );
            }
        };
        if route.source_partition() != self.core.partition() {
            return self.translated_result_pair_prepare_result(
                translated.fetch_request,
                tick,
                Err(RiscvCpuError::MmioRoutePartitionMismatch {
                    expected: self.core.partition(),
                    actual: route.source_partition(),
                }),
            );
        }
        self.translated_result_pair_prepare_result(
            translated.fetch_request,
            tick,
            riscv_data_access::validate_parallel_mmio_route(
                route,
                tick,
                scheduler.min_remote_delay(),
                scheduler.partition_count(),
            )
            .map_err(|error| RiscvCpuError::Mmio(MmioError::Scheduler(error))),
        )?;
        let bound_target = self
            .state
            .lock()
            .expect("riscv core lock")
            .bind_translated_result_target(
                translated.fetch_request,
                O3MemoryResultWindowRoute::Mmio,
            );
        if !bound_target {
            return self.translated_result_pair_prepare_result(
                translated.fetch_request,
                tick,
                Err(RiscvCpuError::TranslatedResultAuthorizationMismatch {
                    fetch: translated.fetch_request,
                }),
            );
        }
        self.state
            .lock()
            .expect("riscv core lock")
            .ready_translated_data
            .remove(&translated.fetch_request)
            .expect("selected ready data translation exists");

        Ok(Some(OutstandingDataAccess {
            tick,
            partition: self.core.partition(),
            target: RiscvDataAccessTarget::Mmio { route },
            request_id: translated.request_id,
            fetch_request: translated.fetch_request,
            access: translated.access,
            size: translated.size,
            physical_address: translated.physical_address,
            request_byte_offset: translated.request_byte_offset,
            line_layout: None,
            forwarded_load_data: None,
            store_load_forwarding_plan: None,
        }))
    }

    pub(super) fn prepare_translated_data_access(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
        translated: TranslatedDataAccess,
    ) -> Result<OutstandingDataAccess, RiscvCpuError> {
        let data_result = self
            .state
            .lock()
            .expect("riscv core lock")
            .data
            .clone()
            .ok_or(RiscvCpuError::MissingDataConfig {
                fetch: translated.fetch_request,
            });
        let data = self.translated_result_pair_prepare_result(
            translated.fetch_request,
            tick,
            data_result,
        )?;
        let route = self.translated_result_pair_prepare_result(
            translated.fetch_request,
            tick,
            transport
                .route(data.route())
                .ok_or(RiscvCpuError::Transport(TransportError::UnknownRoute {
                    route: data.route(),
                })),
        )?;
        if route.source_partition() != self.core.partition() {
            return self.translated_result_pair_prepare_result(
                translated.fetch_request,
                tick,
                Err(RiscvCpuError::DataRoutePartitionMismatch {
                    route: data.route(),
                    expected: self.core.partition(),
                    actual: route.source_partition(),
                }),
            );
        }
        if route.source() != data.endpoint() {
            return self.translated_result_pair_prepare_result(
                translated.fetch_request,
                tick,
                Err(RiscvCpuError::DataRouteEndpointMismatch {
                    route: data.route(),
                    expected: data.endpoint().clone(),
                    actual: route.source().clone(),
                }),
            );
        }
        self.translated_result_pair_prepare_result(
            translated.fetch_request,
            tick,
            self.check_pmp_data_access(
                translated.fetch_request,
                &translated.access,
                translated.size,
                translated.physical_address,
            ),
        )?;
        self.translated_result_pair_prepare_result(
            translated.fetch_request,
            tick,
            self.check_pma_data_access(
                translated.fetch_request,
                &translated.access,
                translated.size,
                translated.physical_address,
                translated.request_byte_offset,
            ),
        )?;
        self.reject_uncacheable_translated_result_memory_target(&translated, tick)?;
        let line_layout = self.translated_result_pair_prepare_result(
            translated.fetch_request,
            tick,
            data.line_layout_for_access(translated.physical_address, translated.size)
                .map_err(RiscvCpuError::Memory),
        )?;
        let line_offset = line_layout.line_offset(translated.physical_address);
        if line_offset + translated.size.bytes() > line_layout.bytes()
            && !supports_translated_cross_line_data_access(
                &translated.access,
                translated.virtual_address,
                translated.physical_address,
                translated.size,
                line_layout,
            )
        {
            return self.translated_result_pair_prepare_result(
                translated.fetch_request,
                tick,
                Err(RiscvCpuError::DataAccessCrossesLine {
                    address: translated.physical_address,
                    size: translated.size,
                    line_size: line_layout.bytes(),
                }),
            );
        }
        let bound_target = self
            .state
            .lock()
            .expect("riscv core lock")
            .bind_translated_result_target(
                translated.fetch_request,
                O3MemoryResultWindowRoute::Memory,
            );
        if !bound_target {
            return self.translated_result_pair_prepare_result(
                translated.fetch_request,
                tick,
                Err(RiscvCpuError::TranslatedResultAuthorizationMismatch {
                    fetch: translated.fetch_request,
                }),
            );
        }

        Ok(OutstandingDataAccess {
            tick,
            partition: self.core.partition(),
            target: RiscvDataAccessTarget::Memory {
                route: data.route(),
                endpoint: data.endpoint().clone(),
            },
            request_id: translated.request_id,
            fetch_request: translated.fetch_request,
            access: translated.access,
            size: translated.size,
            physical_address: translated.physical_address,
            request_byte_offset: translated.request_byte_offset,
            line_layout: Some(line_layout),
            forwarded_load_data: None,
            store_load_forwarding_plan: None,
        })
    }
}

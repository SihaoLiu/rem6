use super::*;

impl IssuedDataAccess {
    pub(super) fn store_conditional_failure_completion(&self, tick: Tick) -> RiscvDataCompletion {
        RiscvDataCompletion::store_conditional_failed(
            self.fetch_request,
            self.access.clone(),
            self.physical_address,
            self.size,
            self.request_byte_offset,
            tick,
        )
    }
}

impl RiscvCore {
    fn record_local_store_conditional_failure_issue(&self, issue: OutstandingDataAccess) {
        self.record_data_issue_state(issue, false);
    }

    pub(crate) fn schedule_store_conditional_failure(
        &self,
        scheduler: &mut PartitionedScheduler,
        issue: OutstandingDataAccess,
    ) -> Result<PartitionEventId, RiscvCpuError> {
        let request_id = issue.request_id;
        let core = self.clone();
        let event = scheduler
            .schedule_at(self.partition(), scheduler.now(), move |context| {
                core.record_store_conditional_failure(request_id, context.now());
            })
            .map_err(RiscvCpuError::Scheduler)?;
        self.record_local_store_conditional_failure_issue(issue);
        Ok(event)
    }

    pub(crate) fn schedule_store_conditional_failure_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        issue: OutstandingDataAccess,
    ) -> Result<PartitionEventId, RiscvCpuError> {
        let request_id = issue.request_id;
        let core = self.clone();
        let event = scheduler
            .schedule_parallel_at(self.partition(), scheduler.now(), move |context| {
                core.record_store_conditional_failure(request_id, context.now());
            })
            .map_err(RiscvCpuError::Scheduler)?;
        self.record_local_store_conditional_failure_issue(issue);
        Ok(event)
    }

    pub(crate) fn store_conditional_fails(&self, issue: &OutstandingDataAccess) -> bool {
        if !matches!(issue.access, MemoryAccessKind::StoreConditional { .. }) {
            return false;
        }
        let expected = RiscvLoadReservation::new(issue.physical_address, issue.size);
        self.state.lock().expect("riscv core lock").reservation != Some(expected)
    }

    fn record_store_conditional_failure(&self, request_id: MemoryRequestId, tick: Tick) {
        let mut state = self.state.lock().expect("riscv core lock");
        if state.pending_callback_error.is_some() {
            return;
        }
        let Some(access) = state.outstanding_data.remove(&request_id) else {
            return;
        };
        if !matches!(&access.access, MemoryAccessKind::StoreConditional { .. }) {
            debug_assert!(false, "store-conditional failure for non-SC access");
            return;
        }
        let completion = access.store_conditional_failure_completion(tick);
        apply_data_completion(
            &mut state,
            self.id(),
            &completion,
            "store-conditional failure has no response data",
        );
        riscv_checker::sync_checker_hart(&mut state);
        let completed_event =
            record_data_retire_cycle(&mut state, &access, tick, completion.data_event_kind());
        if let Err(error) =
            record_o3_data_access_outcome(&mut state, &access, completed_event, tick, None, None)
        {
            record_callback_error(&mut state, error);
            return;
        }
        state
            .data_events
            .push(RiscvDataAccessEvent::conditional_failed(
                access.record(tick),
            ));
    }
}

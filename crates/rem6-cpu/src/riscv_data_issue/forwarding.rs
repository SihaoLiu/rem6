use rem6_isa_riscv::MemoryAccessKind;
use rem6_kernel::{PartitionEventId, PartitionedScheduler, Tick};
use rem6_memory::MemoryRequestId;

use super::{
    deferred_o3_scalar_load_writeback, mark_data_access_event_kind, record_o3_data_access_outcome,
    OutstandingDataAccess,
};
use crate::{
    o3_runtime::O3StoreLoadForwardingPlan, RiscvCore, RiscvCpuError, RiscvDataAccessEvent,
    RiscvDataAccessEventKind,
};

impl RiscvCore {
    pub(super) fn scalar_load_forwarding_plan(
        &self,
        fetch_request: MemoryRequestId,
        access: &MemoryAccessKind,
    ) -> Option<O3StoreLoadForwardingPlan> {
        let state = self.state.lock().expect("riscv core lock");
        if state.data_translation.is_some() {
            return None;
        }
        let execution = state
            .events
            .iter()
            .find(|event| event.fetch().request_id() == fetch_request)?;
        state
            .o3_runtime
            .scalar_load_forwarding_plan(execution.instruction(), access)
    }

    pub(crate) fn schedule_forwarded_load_completion(
        &self,
        scheduler: &mut PartitionedScheduler,
        issue: OutstandingDataAccess,
    ) -> Result<PartitionEventId, RiscvCpuError> {
        let request_id = issue.request_id;
        let data = issue
            .forwarded_load_data
            .clone()
            .expect("forwarded load carries completion data");
        let core = self.clone();
        let event = scheduler
            .schedule_at(self.partition(), scheduler.now(), move |context| {
                core.record_forwarded_load_completion(request_id, context.now(), data);
            })
            .map_err(RiscvCpuError::Scheduler)?;
        self.record_data_issue(issue);
        Ok(event)
    }

    pub(crate) fn schedule_forwarded_load_completion_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        issue: OutstandingDataAccess,
    ) -> Result<PartitionEventId, RiscvCpuError> {
        let request_id = issue.request_id;
        let data = issue
            .forwarded_load_data
            .clone()
            .expect("forwarded load carries completion data");
        let core = self.clone();
        let event = scheduler
            .schedule_parallel_at(self.partition(), scheduler.now(), move |context| {
                core.record_forwarded_load_completion(request_id, context.now(), data);
            })
            .map_err(RiscvCpuError::Scheduler)?;
        self.record_data_issue(issue);
        Ok(event)
    }

    fn record_forwarded_load_completion(
        &self,
        request_id: MemoryRequestId,
        tick: Tick,
        data: Vec<u8>,
    ) {
        let mut state = self.state.lock().expect("riscv core lock");
        let Some(access) = state.outstanding_data.remove(&request_id) else {
            return;
        };
        if !matches!(access.access, MemoryAccessKind::Load { .. }) {
            debug_assert!(false, "forwarded completion requires a scalar load");
            return;
        }
        assert!(
            deferred_o3_scalar_load_writeback(&state, &access),
            "forwarded scalar load must defer architectural writeback"
        );
        let completed_event =
            mark_data_access_event_kind(&mut state, &access, RiscvDataAccessEventKind::Completed);
        record_o3_data_access_outcome(
            &mut state,
            &access,
            completed_event,
            tick,
            Some(&data),
            true,
        );
        state.data_events.push(RiscvDataAccessEvent::completed(
            access.record(tick),
            Some(data),
        ));
    }
}

impl OutstandingDataAccess {
    pub(super) fn has_forwarded_load_data(&self) -> bool {
        self.forwarded_load_data.is_some()
    }
}

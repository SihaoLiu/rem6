use rem6_kernel::{PartitionedScheduler, SchedulerContext};
use rem6_transport::{MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome};

use crate::{RiscvCore, RiscvCoreDriveAction, RiscvCpuError};

impl RiscvCore {
    #[allow(clippy::too_many_arguments)]
    pub fn drive_next_action<F, D>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        fetch_responder: F,
        data_responder: D,
    ) -> Result<Option<RiscvCoreDriveAction>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        D: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        if !self.is_hart_started() {
            return Ok(None);
        }
        if self.has_pending_data_access() {
            return Ok(None);
        }
        if self.has_pending_trap() {
            return Ok(None);
        }
        self.sync_in_order_fetch_state()?;
        if self.core.has_pending_fetch() {
            if !self.can_retire_completed_fetch_while_fetch_pending()? {
                return Ok(None);
            }
            if let Some(event) = self.execute_next_completed_fetch()? {
                return Ok(Some(RiscvCoreDriveAction::InstructionExecuted(Box::new(
                    event,
                ))));
            }
            return Ok(None);
        }

        if let Some(decision) = self.next_fetch_ahead_before_retire() {
            self.set_fetch_ahead_pc(decision.pc());
            let event =
                self.issue_next_fetch(scheduler, transport, fetch_trace, fetch_responder)?;
            self.record_fetch_ahead_speculation(&decision);
            return Ok(Some(RiscvCoreDriveAction::FetchIssued { event }));
        }

        if let Some(event) = self.execute_next_completed_fetch()? {
            return Ok(Some(RiscvCoreDriveAction::InstructionExecuted(Box::new(
                event,
            ))));
        }

        if let Some(event) =
            self.issue_next_data_access(scheduler, transport, data_trace, data_responder)?
        {
            return Ok(Some(RiscvCoreDriveAction::DataAccessIssued { event }));
        }

        let event = self.issue_next_fetch(scheduler, transport, fetch_trace, fetch_responder)?;
        Ok(Some(RiscvCoreDriveAction::FetchIssued { event }))
    }
}

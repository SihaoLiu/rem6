use rem6_kernel::{PartitionedScheduler, SchedulerContext};
use rem6_transport::{MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome};

use crate::riscv_in_order_drive::{RiscvInOrderDriveStatus, RiscvInOrderFetchAdmission};
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
        if self.pending_data_access_blocks_new_work() {
            return Ok(None);
        }
        if self.has_pending_trap() {
            return Ok(None);
        }
        if let Some(event) =
            self.issue_next_data_access(scheduler, transport, data_trace, data_responder)?
        {
            return Ok(Some(RiscvCoreDriveAction::DataAccessIssued { event }));
        }
        self.sync_in_order_fetch_state()?;
        if self.core.has_pending_fetch() {
            if !self.can_retire_completed_fetch_while_fetch_pending()? {
                self.record_in_order_fetch_wait_stall_cycle()?;
                return Ok(None);
            }
            if let Some(action) = self.drive_next_completed_fetch_serial_action(scheduler)? {
                return Ok(Some(action));
            }
            if self.live_retire_gate_blocks_new_work() {
                return Ok(None);
            }
            self.record_in_order_fetch_wait_stall_cycle()?;
            return Ok(None);
        }

        let detailed_o3_fetch = self.detailed_o3_window_prefers_fetch_ahead();
        let pending_o3_scalar_memory_retirement = self.has_pending_o3_scalar_memory_retirement();
        if !detailed_o3_fetch && pending_o3_scalar_memory_retirement {
            return Ok(None);
        }
        let fetch_admission = if detailed_o3_fetch {
            RiscvInOrderFetchAdmission::Admitted
        } else {
            self.in_order_fetch_admission()
        };

        if fetch_admission.allows_fetch() {
            if let Some(decision) = self.next_fetch_ahead_before_retire() {
                let fetch_ahead = self.prepare_fetch_ahead_speculation(&decision)?;
                self.set_fetch_ahead_pc(decision.pc());
                let event = self.issue_next_fetch_with_prepared_fetch_ahead(
                    scheduler,
                    transport,
                    fetch_trace,
                    fetch_responder,
                    fetch_ahead,
                )?;
                return Ok(Some(RiscvCoreDriveAction::FetchIssued { event }));
            }
        }

        if !detailed_o3_fetch {
            match self.schedule_next_completed_fetch_pipeline_cycle_serial(scheduler)? {
                RiscvInOrderDriveStatus::Scheduled(event) => {
                    return Ok(Some(RiscvCoreDriveAction::PipelineCycleScheduled { event }));
                }
                RiscvInOrderDriveStatus::Pending => return Ok(None),
                RiscvInOrderDriveStatus::Ready if self.live_retire_gate_blocks_new_work() => {
                    return self.drive_next_completed_fetch_serial_action(scheduler);
                }
                RiscvInOrderDriveStatus::Unavailable if self.live_retire_gate_blocks_new_work() => {
                    return Ok(None);
                }
                RiscvInOrderDriveStatus::Unavailable | RiscvInOrderDriveStatus::Ready => {}
                RiscvInOrderDriveStatus::Reserved { .. } => {
                    unreachable!("pipeline reservation is scheduled before returning")
                }
            }
        }

        if let Some(action) = self.drive_next_completed_fetch_serial_action(scheduler)? {
            return Ok(Some(action));
        }
        if self.live_retire_gate_blocks_new_work() {
            return Ok(None);
        }
        if !fetch_admission.allows_fetch() {
            return Ok(None);
        }

        let event = self.issue_next_fetch(scheduler, transport, fetch_trace, fetch_responder)?;
        Ok(Some(RiscvCoreDriveAction::FetchIssued { event }))
    }
}

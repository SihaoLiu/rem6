use rem6_kernel::{PartitionEventId, PartitionId, PartitionedScheduler, Tick};

use crate::{ExecutionMode, ExecutionModeTarget, GuestEventId, GuestEventKind, SystemError};

use super::RiscvTrapEventPort;

impl RiscvTrapEventPort {
    pub fn schedule_host_execution_mode_switch_event_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        source: PartitionId,
        source_tick: Tick,
        target: ExecutionModeTarget,
        mode: ExecutionMode,
    ) -> Result<PartitionEventId, SystemError> {
        self.schedule_host_control_event_kind_parallel(
            scheduler,
            event,
            source,
            source_tick,
            GuestEventKind::ExecutionModeSwitch { target, mode },
        )
    }
}

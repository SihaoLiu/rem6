use rem6_cpu::RiscvO3LiveCheckpointProfile;

use super::*;

impl RiscvCoreCheckpointBank {
    pub(crate) fn pending_live_retire_gate_wakes(
        &self,
    ) -> Vec<(SchedulerInstanceId, PendingEventSnapshot)> {
        self.ports
            .values()
            .flat_map(|port| port.core().checkpoint_owned_live_retire_gate_wakes())
            .collect()
    }

    pub(crate) fn pending_o3_writeback_wakes(
        &self,
    ) -> Vec<(SchedulerInstanceId, PendingEventSnapshot)> {
        self.ports
            .values()
            .flat_map(|port| port.core().owned_o3_writeback_wakes())
            .collect()
    }

    pub(crate) fn live_o3_scheduler_restores(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<RiscvO3LiveSchedulerRestore>, RiscvCoreCheckpointError> {
        let mut restores = Vec::new();
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            if let Some(live) = record.o3_live_checkpoint() {
                restores.push(RiscvO3LiveSchedulerRestore {
                    component: port.component.clone(),
                    core: port.core.clone(),
                    wake: live.wake,
                });
            }
        }
        Ok(restores)
    }

    pub(crate) fn has_pending_data_address_live_checkpoint(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<bool, RiscvCoreCheckpointError> {
        for port in self.ports.values() {
            if port
                .decode_from(registry)?
                .o3_live_checkpoint()
                .is_some_and(|live| {
                    live.profile == RiscvO3LiveCheckpointProfile::PendingDataAddress
                })
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

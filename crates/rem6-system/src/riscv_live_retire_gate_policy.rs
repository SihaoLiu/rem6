use rem6_cpu::RiscvCluster;

use crate::{trap_event, ExecutionMode, RiscvSystemRunDriver, SystemError};

impl RiscvSystemRunDriver {
    pub(crate) fn snapshot_live_retire_gate_policy(
        &self,
        cluster: &RiscvCluster,
    ) -> Result<(), SystemError> {
        let policies = {
            let controller = self.trap_port.controller();
            let controller = controller.lock().expect("system host controller lock");
            cluster
                .core_ids()
                .into_iter()
                .map(|cpu| {
                    let target = trap_event::execution_mode_target_for_cpu(cpu);
                    let detailed = controller
                        .executor()
                        .execution_mode(&target)
                        .is_some_and(|mode| mode == ExecutionMode::Detailed);
                    (cpu, detailed)
                })
                .collect::<Vec<_>>()
        };

        for (cpu, detailed) in policies {
            cluster
                .core(cpu)
                .map_err(SystemError::RiscvCluster)?
                .set_detailed_live_retire_gate_enabled(detailed);
        }
        Ok(())
    }
}

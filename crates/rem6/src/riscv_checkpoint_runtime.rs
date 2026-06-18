use std::sync::{Arc, Mutex};

use rem6_checkpoint::CheckpointComponentId;
use rem6_cpu::RiscvCluster;
use rem6_system::{RiscvCoreCheckpointBank, RiscvCoreCheckpointPort, SystemHostController};

use crate::{execute_error, Rem6CliError};

pub(crate) fn attach_cli_riscv_checkpoint_bank(
    controller: &Arc<Mutex<SystemHostController>>,
    cluster: &RiscvCluster,
) -> Result<(), Rem6CliError> {
    let ports = cluster
        .core_ids()
        .into_iter()
        .map(|cpu| {
            let component =
                CheckpointComponentId::new(format!("cpu{}", cpu.get())).map_err(execute_error)?;
            let core = cluster.core(cpu).map_err(execute_error)?;
            Ok(RiscvCoreCheckpointPort::new(component, core))
        })
        .collect::<Result<Vec<_>, Rem6CliError>>()?;
    let bank = RiscvCoreCheckpointBank::new(ports).map_err(execute_error)?;
    controller
        .lock()
        .map_err(|error| execute_error(format!("host controller lock poisoned: {error}")))?
        .executor_mut()
        .attach_riscv_checkpoint_bank(bank)
        .map_err(execute_error)
}

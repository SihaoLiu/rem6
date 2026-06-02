use rem6_checkpoint::{CheckpointComponentId, CheckpointError};

use crate::{
    SinicFifoCheckpointBank, SinicFifoCheckpointPort, SinicRegisterCheckpointBank,
    SinicRegisterCheckpointPort, SystemError,
};

use super::{RiscvTopologySystem, RiscvTopologySystemError};

impl RiscvTopologySystem {
    pub fn with_sinic_register_checkpoint_port(
        mut self,
        port: SinicRegisterCheckpointPort,
    ) -> Result<Self, RiscvTopologySystemError> {
        let component = port.component().clone();
        if self
            .sinic_register_checkpoint_ports
            .contains_key(&component)
        {
            return Err(sinic_duplicate_component(component));
        }
        if let Some(host) = self.host.as_ref() {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_sinic_register_checkpoint_port(port.clone())
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        self.sinic_register_checkpoint_ports.insert(component, port);
        Ok(self)
    }

    pub fn sinic_register_checkpoint_components(&self) -> Vec<CheckpointComponentId> {
        self.sinic_register_checkpoint_ports
            .keys()
            .cloned()
            .collect()
    }

    pub fn with_sinic_fifo_checkpoint_port(
        mut self,
        port: SinicFifoCheckpointPort,
    ) -> Result<Self, RiscvTopologySystemError> {
        let component = port.component().clone();
        if self.sinic_fifo_checkpoint_ports.contains_key(&component) {
            return Err(sinic_duplicate_component(component));
        }
        if let Some(host) = self.host.as_ref() {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_sinic_fifo_checkpoint_port(port.clone())
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        self.sinic_fifo_checkpoint_ports.insert(component, port);
        Ok(self)
    }

    pub fn sinic_fifo_checkpoint_components(&self) -> Vec<CheckpointComponentId> {
        self.sinic_fifo_checkpoint_ports.keys().cloned().collect()
    }

    pub(super) fn attach_sinic_checkpoint_to_host(
        &mut self,
    ) -> Result<(), RiscvTopologySystemError> {
        let Some(host) = self.host.as_ref() else {
            return Ok(());
        };
        if !self.sinic_register_checkpoint_ports.is_empty() {
            let bank = SinicRegisterCheckpointBank::new(
                self.sinic_register_checkpoint_ports.values().cloned(),
            )
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_sinic_register_checkpoint_bank(bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        if !self.sinic_fifo_checkpoint_ports.is_empty() {
            let bank =
                SinicFifoCheckpointBank::new(self.sinic_fifo_checkpoint_ports.values().cloned())
                    .map_err(SystemError::Checkpoint)
                    .map_err(RiscvTopologySystemError::System)?;
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_sinic_fifo_checkpoint_bank(bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        Ok(())
    }
}

fn sinic_duplicate_component(component: CheckpointComponentId) -> RiscvTopologySystemError {
    RiscvTopologySystemError::System(SystemError::Checkpoint(
        CheckpointError::DuplicateComponent { component },
    ))
}

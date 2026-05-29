use rem6_checkpoint::{CheckpointComponentId, CheckpointError};

use crate::{StorageImageCheckpointBank, StorageImageCheckpointPort, SystemError};

use super::{RiscvTopologySystem, RiscvTopologySystemError};

impl RiscvTopologySystem {
    pub fn with_storage_image_checkpoint_port(
        mut self,
        port: StorageImageCheckpointPort,
    ) -> Result<Self, RiscvTopologySystemError> {
        let component = port.component().clone();
        if self.storage_checkpoint_ports.contains_key(&component) {
            return Err(storage_duplicate_component(component));
        }
        if let Some(host) = self.host.as_ref() {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_storage_image_checkpoint_port(port.clone())
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        self.storage_checkpoint_ports.insert(component, port);
        Ok(self)
    }

    pub fn storage_image_checkpoint_components(&self) -> Vec<CheckpointComponentId> {
        self.storage_checkpoint_ports.keys().cloned().collect()
    }

    pub(super) fn attach_storage_checkpoint_to_host(
        &mut self,
    ) -> Result<(), RiscvTopologySystemError> {
        let Some(host) = self.host.as_ref() else {
            return Ok(());
        };
        if self.storage_checkpoint_ports.is_empty() {
            return Ok(());
        }
        let bank = StorageImageCheckpointBank::new(self.storage_checkpoint_ports.values().cloned())
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
        host.controller
            .lock()
            .expect("topology host controller lock")
            .executor_mut()
            .attach_storage_image_checkpoint_bank(bank)
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
        Ok(())
    }
}

fn storage_duplicate_component(component: CheckpointComponentId) -> RiscvTopologySystemError {
    RiscvTopologySystemError::System(SystemError::Checkpoint(
        CheckpointError::DuplicateComponent { component },
    ))
}

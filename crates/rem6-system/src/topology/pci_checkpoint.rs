use rem6_checkpoint::{CheckpointComponentId, CheckpointError};

use crate::{
    PciHostCheckpointBank, PciHostCheckpointPort, PciLegacyInterruptRouterCheckpointBank,
    PciLegacyInterruptRouterCheckpointPort, SystemError,
};

use super::{RiscvTopologySystem, RiscvTopologySystemError};

impl RiscvTopologySystem {
    pub fn with_pci_host_checkpoint_port(
        mut self,
        port: PciHostCheckpointPort,
    ) -> Result<Self, RiscvTopologySystemError> {
        let component = port.component().clone();
        if self.pci_host_checkpoint_ports.contains_key(&component) {
            return Err(pci_duplicate_component(component));
        }
        if let Some(host) = self.host.as_ref() {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_pci_host_checkpoint_port(port.clone())
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        self.pci_host_checkpoint_ports.insert(component, port);
        Ok(self)
    }

    pub fn pci_host_checkpoint_components(&self) -> Vec<CheckpointComponentId> {
        self.pci_host_checkpoint_ports.keys().cloned().collect()
    }

    pub fn with_pci_legacy_interrupt_router_checkpoint_port(
        mut self,
        port: PciLegacyInterruptRouterCheckpointPort,
    ) -> Result<Self, RiscvTopologySystemError> {
        let component = port.component().clone();
        if self
            .pci_legacy_interrupt_router_checkpoint_ports
            .contains_key(&component)
        {
            return Err(pci_duplicate_component(component));
        }
        if let Some(host) = self.host.as_ref() {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_pci_legacy_interrupt_router_checkpoint_port(port.clone())
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        self.pci_legacy_interrupt_router_checkpoint_ports
            .insert(component, port);
        Ok(self)
    }

    pub fn pci_legacy_interrupt_router_checkpoint_components(&self) -> Vec<CheckpointComponentId> {
        self.pci_legacy_interrupt_router_checkpoint_ports
            .keys()
            .cloned()
            .collect()
    }

    pub(super) fn attach_pci_checkpoint_to_host(&mut self) -> Result<(), RiscvTopologySystemError> {
        let Some(host) = self.host.as_ref() else {
            return Ok(());
        };
        if !self.pci_host_checkpoint_ports.is_empty() {
            let bank = PciHostCheckpointBank::new(self.pci_host_checkpoint_ports.values().cloned())
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_pci_host_checkpoint_bank(bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        if !self.pci_legacy_interrupt_router_checkpoint_ports.is_empty() {
            let bank = PciLegacyInterruptRouterCheckpointBank::new(
                self.pci_legacy_interrupt_router_checkpoint_ports
                    .values()
                    .cloned(),
            )
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_pci_legacy_interrupt_router_checkpoint_bank(bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        Ok(())
    }
}

fn pci_duplicate_component(component: CheckpointComponentId) -> RiscvTopologySystemError {
    RiscvTopologySystemError::System(SystemError::Checkpoint(
        CheckpointError::DuplicateComponent { component },
    ))
}

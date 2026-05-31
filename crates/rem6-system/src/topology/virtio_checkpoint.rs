use rem6_checkpoint::{CheckpointComponentId, CheckpointError};

use crate::{
    SystemError, VirtioPciCommonCheckpointBank, VirtioPciCommonCheckpointPort,
    VirtioPciDeviceConfigCheckpointBank, VirtioPciDeviceConfigCheckpointPort,
    VirtioPciIsrCheckpointBank, VirtioPciIsrCheckpointPort, VirtioPciNotifyCheckpointBank,
    VirtioPciNotifyCheckpointPort, VirtioSplitQueueCheckpointBank, VirtioSplitQueueCheckpointPort,
};

use super::{RiscvTopologySystem, RiscvTopologySystemError};

impl RiscvTopologySystem {
    pub fn with_virtio_split_queue_checkpoint_port(
        mut self,
        port: VirtioSplitQueueCheckpointPort,
    ) -> Result<Self, RiscvTopologySystemError> {
        let component = port.component().clone();
        if self
            .virtio_split_queue_checkpoint_ports
            .contains_key(&component)
        {
            return Err(virtio_duplicate_component(component));
        }
        if let Some(host) = self.host.as_ref() {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_virtio_split_queue_checkpoint_port(port.clone())
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        self.virtio_split_queue_checkpoint_ports
            .insert(component, port);
        Ok(self)
    }

    pub fn virtio_split_queue_checkpoint_components(&self) -> Vec<CheckpointComponentId> {
        self.virtio_split_queue_checkpoint_ports
            .keys()
            .cloned()
            .collect()
    }

    pub fn with_virtio_pci_common_checkpoint_port(
        mut self,
        port: VirtioPciCommonCheckpointPort,
    ) -> Result<Self, RiscvTopologySystemError> {
        let component = port.component().clone();
        if self
            .virtio_pci_common_checkpoint_ports
            .contains_key(&component)
        {
            return Err(virtio_duplicate_component(component));
        }
        if let Some(host) = self.host.as_ref() {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_virtio_pci_common_checkpoint_port(port.clone())
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        self.virtio_pci_common_checkpoint_ports
            .insert(component, port);
        Ok(self)
    }

    pub fn virtio_pci_common_checkpoint_components(&self) -> Vec<CheckpointComponentId> {
        self.virtio_pci_common_checkpoint_ports
            .keys()
            .cloned()
            .collect()
    }

    pub fn with_virtio_pci_notify_checkpoint_port(
        mut self,
        port: VirtioPciNotifyCheckpointPort,
    ) -> Result<Self, RiscvTopologySystemError> {
        let component = port.component().clone();
        if self
            .virtio_pci_notify_checkpoint_ports
            .contains_key(&component)
        {
            return Err(virtio_duplicate_component(component));
        }
        if let Some(host) = self.host.as_ref() {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_virtio_pci_notify_checkpoint_port(port.clone())
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        self.virtio_pci_notify_checkpoint_ports
            .insert(component, port);
        Ok(self)
    }

    pub fn virtio_pci_notify_checkpoint_components(&self) -> Vec<CheckpointComponentId> {
        self.virtio_pci_notify_checkpoint_ports
            .keys()
            .cloned()
            .collect()
    }

    pub fn with_virtio_pci_isr_checkpoint_port(
        mut self,
        port: VirtioPciIsrCheckpointPort,
    ) -> Result<Self, RiscvTopologySystemError> {
        let component = port.component().clone();
        if self
            .virtio_pci_isr_checkpoint_ports
            .contains_key(&component)
        {
            return Err(virtio_duplicate_component(component));
        }
        if let Some(host) = self.host.as_ref() {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_virtio_pci_isr_checkpoint_port(port.clone())
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        self.virtio_pci_isr_checkpoint_ports.insert(component, port);
        Ok(self)
    }

    pub fn virtio_pci_isr_checkpoint_components(&self) -> Vec<CheckpointComponentId> {
        self.virtio_pci_isr_checkpoint_ports
            .keys()
            .cloned()
            .collect()
    }

    pub fn with_virtio_pci_device_config_checkpoint_port(
        mut self,
        port: VirtioPciDeviceConfigCheckpointPort,
    ) -> Result<Self, RiscvTopologySystemError> {
        let component = port.component().clone();
        if self
            .virtio_pci_device_config_checkpoint_ports
            .contains_key(&component)
        {
            return Err(virtio_duplicate_component(component));
        }
        if let Some(host) = self.host.as_ref() {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_virtio_pci_device_config_checkpoint_port(port.clone())
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        self.virtio_pci_device_config_checkpoint_ports
            .insert(component, port);
        Ok(self)
    }

    pub fn virtio_pci_device_config_checkpoint_components(&self) -> Vec<CheckpointComponentId> {
        self.virtio_pci_device_config_checkpoint_ports
            .keys()
            .cloned()
            .collect()
    }

    pub(super) fn attach_virtio_pci_checkpoint_to_host(
        &mut self,
    ) -> Result<(), RiscvTopologySystemError> {
        let Some(host) = self.host.as_ref() else {
            return Ok(());
        };
        if !self.virtio_split_queue_checkpoint_ports.is_empty() {
            let bank = VirtioSplitQueueCheckpointBank::new(
                self.virtio_split_queue_checkpoint_ports.values().cloned(),
            )
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_virtio_split_queue_checkpoint_bank(bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        if !self.virtio_pci_common_checkpoint_ports.is_empty() {
            let bank = VirtioPciCommonCheckpointBank::new(
                self.virtio_pci_common_checkpoint_ports.values().cloned(),
            )
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_virtio_pci_common_checkpoint_bank(bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        if !self.virtio_pci_notify_checkpoint_ports.is_empty() {
            let bank = VirtioPciNotifyCheckpointBank::new(
                self.virtio_pci_notify_checkpoint_ports.values().cloned(),
            )
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_virtio_pci_notify_checkpoint_bank(bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        if !self.virtio_pci_isr_checkpoint_ports.is_empty() {
            let bank = VirtioPciIsrCheckpointBank::new(
                self.virtio_pci_isr_checkpoint_ports.values().cloned(),
            )
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_virtio_pci_isr_checkpoint_bank(bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        if !self.virtio_pci_device_config_checkpoint_ports.is_empty() {
            let bank = VirtioPciDeviceConfigCheckpointBank::new(
                self.virtio_pci_device_config_checkpoint_ports
                    .values()
                    .cloned(),
            )
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_virtio_pci_device_config_checkpoint_bank(bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }
        Ok(())
    }
}

fn virtio_duplicate_component(component: CheckpointComponentId) -> RiscvTopologySystemError {
    RiscvTopologySystemError::System(SystemError::Checkpoint(
        CheckpointError::DuplicateComponent { component },
    ))
}

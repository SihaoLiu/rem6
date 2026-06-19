use rem6_memory::PartitionedMemoryStore;
use rem6_workload::{WorkloadError, WorkloadLinuxBootHandoff};

use super::payload::load_payload_at;
use super::{RiscvWorkloadReplay, RiscvWorkloadReplayError};

impl RiscvWorkloadReplay {
    pub(super) fn load_linux_device_tree_payload(
        &self,
        store: &mut PartitionedMemoryStore,
    ) -> Result<(), RiscvWorkloadReplayError> {
        let Some(handoff) = self.plan.linux_boot_handoff() else {
            return Ok(());
        };
        let Some(resource) = handoff.device_tree_resource() else {
            return Ok(());
        };
        let payload = self
            .resolved_resources
            .as_ref()
            .and_then(|resources| resources.linux_device_tree_data(handoff))
            .ok_or_else(|| {
                RiscvWorkloadReplayError::Workload(WorkloadError::MissingResourcePayload {
                    resource: resource.clone(),
                })
            })?;

        load_payload_at(store, handoff.dtb_addr(), payload)
    }

    pub(super) fn debug_console_input_payload(
        &self,
        handoff: &WorkloadLinuxBootHandoff,
    ) -> Result<Option<Vec<u8>>, RiscvWorkloadReplayError> {
        let Some(resource) = handoff.debug_console_input_resource() else {
            return Ok(None);
        };
        let payload = self
            .resolved_resources
            .as_ref()
            .and_then(|resources| resources.linux_debug_console_input_data(handoff))
            .ok_or_else(|| {
                RiscvWorkloadReplayError::Workload(WorkloadError::MissingResourcePayload {
                    resource: resource.clone(),
                })
            })?;
        Ok(Some(payload.to_vec()))
    }

    pub(super) fn load_linux_initrd_payload(
        &self,
        store: &mut PartitionedMemoryStore,
    ) -> Result<(), RiscvWorkloadReplayError> {
        let Some(handoff) = self.plan.linux_boot_handoff() else {
            return Ok(());
        };
        let Some(initrd) = handoff.initrd() else {
            return Ok(());
        };
        let payload = self
            .resolved_resources
            .as_ref()
            .and_then(|resources| resources.linux_initrd_data(handoff))
            .ok_or_else(|| {
                RiscvWorkloadReplayError::Workload(WorkloadError::MissingResourcePayload {
                    resource: initrd.resource().clone(),
                })
            })?;

        load_payload_at(store, initrd.start(), payload)
    }
}

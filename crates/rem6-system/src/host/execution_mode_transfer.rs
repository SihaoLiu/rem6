use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_kernel::{PendingEventSnapshot, SchedulerInstanceId, Tick};

use crate::scheduler_checkpoint::{
    remove_scheduler_checkpoint_chunk, SchedulerCheckpointBankGuard, SchedulerCheckpointContext,
    SchedulerCheckpointOwnedEvent,
};
use crate::{ExecutionModeTarget, HostActionRecord, SchedulerCheckpointBank, SystemError};

use super::{
    ExecutionModeSwitchStateTransfer, SystemActionExecutor,
    EXECUTION_MODE_SWITCH_STATE_TRANSFER_LABEL_PREFIX,
};

impl SystemActionExecutor {
    pub fn attach_scheduler_checkpoint_bank(
        &mut self,
        scheduler_checkpoints: SchedulerCheckpointBank,
    ) -> Result<(), CheckpointError> {
        let components = scheduler_checkpoints.components();
        let mut staged_checkpoints = self.checkpoints.clone();
        for component in &components {
            if self
                .borrowed_scheduler_checkpoint_components
                .contains(component)
            {
                remove_scheduler_checkpoint_chunk(&mut staged_checkpoints, component);
            }
        }
        scheduler_checkpoints.register_all(&mut staged_checkpoints)?;
        self.checkpoints = staged_checkpoints;
        self.scheduler_checkpoints = Some(scheduler_checkpoints);
        for component in components {
            self.borrowed_scheduler_checkpoint_components
                .remove(&component);
        }
        Ok(())
    }

    pub(super) fn capture_execution_mode_switch_state_transfer_with_scheduler(
        &mut self,
        record: &HostActionRecord,
        target: &ExecutionModeTarget,
        mut scheduler_checkpoint: Option<&mut SchedulerCheckpointContext<'_>>,
        scheduler_checkpoint_bank: Option<&SchedulerCheckpointBankGuard<'_>>,
    ) -> Result<Option<ExecutionModeSwitchStateTransfer>, SystemError> {
        let has_state_transfer_banks = self.has_execution_mode_switch_state_transfer_banks();
        if !has_state_transfer_banks && scheduler_checkpoint.is_none() {
            return Ok(None);
        }

        let mut staged_checkpoints = self.checkpoints.clone();
        let captured_borrowed_scheduler = self
            .capture_attached_checkpoint_banks_into_with_scheduler(
                &mut staged_checkpoints,
                record.tick(),
                scheduler_checkpoint.as_deref_mut(),
                scheduler_checkpoint_bank,
            )?;
        if !has_state_transfer_banks && !captured_borrowed_scheduler {
            return Ok(None);
        }
        self.capture_execution_modes_into(&mut staged_checkpoints)?;

        let manifest = staged_checkpoints
            .capture(
                execution_mode_switch_state_transfer_label(target, record.tick()),
                record.tick(),
            )
            .map_err(SystemError::Checkpoint)?;
        self.checkpoints = staged_checkpoints;
        self.captured_manifests
            .insert(manifest.label().to_string(), manifest.clone());
        let checker = self
            .riscv_checkpoints
            .as_ref()
            .and_then(|checkpoints| checkpoints.checker_summary_for_target(target));

        Ok(Some(ExecutionModeSwitchStateTransfer::from_manifest(
            &manifest, target, checker,
        )))
    }

    pub(super) fn capture_attached_checkpoint_banks_into_with_scheduler(
        &mut self,
        staged_checkpoints: &mut CheckpointRegistry,
        tick: Tick,
        mut scheduler_checkpoint: Option<&mut SchedulerCheckpointContext<'_>>,
        scheduler_checkpoint_bank: Option<&SchedulerCheckpointBankGuard<'_>>,
    ) -> Result<bool, SystemError> {
        for component in &self.borrowed_scheduler_checkpoint_components {
            if self
                .scheduler_checkpoints
                .as_ref()
                .is_some_and(|bank| bank.has_component(component))
            {
                continue;
            }
            remove_scheduler_checkpoint_chunk(staged_checkpoints, component);
        }
        if let Some(scheduler_checkpoint) = scheduler_checkpoint.as_ref() {
            scheduler_checkpoint.remove_checkpoint_chunk(staged_checkpoints);
        }
        let borrowed_scheduler_is_attached = match (
            self.scheduler_checkpoints.as_ref(),
            scheduler_checkpoint.as_ref(),
        ) {
            (Some(bank), Some(scheduler)) => bank
                .validate_borrowed_scheduler(scheduler)
                .map_err(SystemError::SchedulerCheckpoint)?,
            _ => false,
        };
        let owned_scheduler_events = self.owned_scheduler_checkpoint_events();
        let capture_borrowed_scheduler = scheduler_checkpoint.as_ref().is_some_and(|scheduler| {
            borrowed_scheduler_is_attached
                || scheduler.has_pending_discard_claim(&owned_scheduler_events)
        });
        let borrowed_scheduler_component = capture_borrowed_scheduler
            .then(|| scheduler_checkpoint.as_ref().unwrap().component().clone());
        if let Some(scheduler_checkpoint_bank) = scheduler_checkpoint_bank {
            scheduler_checkpoint_bank
                .validate_quiescent_capture_with_owned_events(&owned_scheduler_events)
                .map_err(SystemError::SchedulerCheckpoint)?;
        }
        if capture_borrowed_scheduler {
            let scheduler_checkpoint = scheduler_checkpoint
                .as_deref_mut()
                .expect("borrowed scheduler capture is present");
            scheduler_checkpoint
                .validate_capture(&owned_scheduler_events)
                .map_err(SystemError::SchedulerCheckpoint)?;
        }
        if let Some(accelerator_checkpoints) = &self.accelerator_checkpoints {
            accelerator_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(msi_bank_checkpoints) = &self.msi_bank_checkpoints {
            msi_bank_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::MsiBankCheckpoint)?;
        }
        if let Some(fabric_checkpoints) = &self.fabric_checkpoints {
            fabric_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::FabricCheckpoint)?;
        }
        if let Some(gpu_checkpoints) = &self.gpu_checkpoints {
            gpu_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(riscv_checkpoints) = &self.riscv_checkpoints {
            riscv_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(scheduler_checkpoint_bank) = scheduler_checkpoint_bank {
            scheduler_checkpoint_bank
                .capture_all_into_with_owned_events(staged_checkpoints, &owned_scheduler_events)
                .map_err(SystemError::SchedulerCheckpoint)?;
        }
        if capture_borrowed_scheduler {
            let scheduler_checkpoint =
                scheduler_checkpoint.expect("borrowed scheduler capture is present");
            scheduler_checkpoint
                .capture_into(staged_checkpoints, &owned_scheduler_events)
                .map_err(SystemError::SchedulerCheckpoint)?;
        }
        if let Some(component) = borrowed_scheduler_component {
            self.track_borrowed_scheduler_checkpoint_component(component);
        }
        if let Some(memory_checkpoints) = &self.memory_checkpoints {
            memory_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(storage_image_checkpoints) = &self.storage_image_checkpoints {
            storage_image_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::StorageCheckpoint)?;
        }
        if let Some(guest_fd_checkpoints) = &self.guest_fd_checkpoints {
            guest_fd_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(guest_futex_checkpoints) = &self.guest_futex_checkpoints {
            guest_futex_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(guest_wait_checkpoints) = &self.guest_wait_checkpoints {
            guest_wait_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(ide_controller_checkpoints) = &self.ide_controller_checkpoints {
            ide_controller_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::StorageCheckpoint)?;
        }
        if let Some(sinic_register_checkpoints) = &self.sinic_register_checkpoints {
            sinic_register_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(sinic_fifo_checkpoints) = &self.sinic_fifo_checkpoints {
            sinic_fifo_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(dram_memory_checkpoints) = &self.dram_memory_checkpoints {
            dram_memory_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(readfile_checkpoints) = &self.readfile_checkpoints {
            readfile_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(interrupt_controller_checkpoints) = &self.interrupt_controller_checkpoints {
            interrupt_controller_checkpoints
                .capture_all_into(staged_checkpoints, tick)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(clint_checkpoints) = &self.clint_checkpoints {
            clint_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(timer_checkpoints) = &self.timer_checkpoints {
            timer_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(uart_checkpoints) = &self.uart_checkpoints {
            uart_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(pl011_uart_checkpoints) = &self.pl011_uart_checkpoints {
            pl011_uart_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(plic_checkpoints) = &self.plic_checkpoints {
            plic_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(pl031_checkpoints) = &self.pl031_checkpoints {
            pl031_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(sp804_checkpoints) = &self.sp804_checkpoints {
            sp804_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(sp805_checkpoints) = &self.sp805_checkpoints {
            sp805_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(cpu_local_timer_checkpoints) = &self.cpu_local_timer_checkpoints {
            cpu_local_timer_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(rtc_checkpoints) = &self.rtc_checkpoints {
            rtc_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(pci_host_checkpoints) = &self.pci_host_checkpoints {
            pci_host_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::PciHostCheckpoint)?;
        }
        if let Some(pci_legacy_interrupt_router_checkpoints) =
            &self.pci_legacy_interrupt_router_checkpoints
        {
            pci_legacy_interrupt_router_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::Checkpoint)?;
        }
        if let Some(virtio_split_queue_checkpoints) = &self.virtio_split_queue_checkpoints {
            virtio_split_queue_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::VirtioCheckpoint)?;
        }
        if let Some(virtio_pci_isr_checkpoints) = &self.virtio_pci_isr_checkpoints {
            virtio_pci_isr_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::VirtioPciIsrCheckpoint)?;
        }
        if let Some(virtio_pci_common_checkpoints) = &self.virtio_pci_common_checkpoints {
            virtio_pci_common_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::VirtioPciCommonCheckpoint)?;
        }
        if let Some(virtio_pci_notify_checkpoints) = &self.virtio_pci_notify_checkpoints {
            virtio_pci_notify_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::VirtioPciNotifyCheckpoint)?;
        }
        if let Some(virtio_pci_device_config_checkpoints) =
            &self.virtio_pci_device_config_checkpoints
        {
            virtio_pci_device_config_checkpoints
                .capture_all_into(staged_checkpoints)
                .map_err(SystemError::VirtioPciDeviceConfigCheckpoint)?;
        }
        Ok(capture_borrowed_scheduler)
    }

    pub(super) fn owned_scheduler_checkpoint_events(&self) -> Vec<SchedulerCheckpointOwnedEvent> {
        let mut events = self.scheduler_checkpoint_control_events.clone();
        events.extend(
            self.riscv_checkpoints
                .as_ref()
                .into_iter()
                .flat_map(|checkpoints| checkpoints.pending_live_retire_gate_wakes())
                .map(|(scheduler, event)| {
                    SchedulerCheckpointOwnedEvent::discard_on_restore(scheduler, event)
                }),
        );
        events
    }

    pub(crate) fn register_scheduler_checkpoint_control_event(
        &mut self,
        scheduler: SchedulerInstanceId,
        event: PendingEventSnapshot,
    ) {
        let event = SchedulerCheckpointOwnedEvent::preserve_on_restore(scheduler, event);
        if !self
            .scheduler_checkpoint_control_events
            .iter()
            .copied()
            .any(|candidate| candidate.same_identity(event))
        {
            self.scheduler_checkpoint_control_events.push(event);
        }
    }

    pub(crate) fn retain_scheduler_checkpoint_control_events(
        &mut self,
        scheduler: SchedulerInstanceId,
        snapshot: &rem6_kernel::SchedulerSnapshot,
    ) {
        self.scheduler_checkpoint_control_events
            .retain(|event| event.retain_for_scheduler(scheduler, snapshot));
    }

    pub(crate) fn retain_attached_scheduler_checkpoint_control_events(
        &mut self,
        scheduler_checkpoint_bank: Option<&SchedulerCheckpointBankGuard<'_>>,
    ) {
        if let Some(scheduler_checkpoint_bank) = scheduler_checkpoint_bank {
            scheduler_checkpoint_bank
                .retain_pending_owned_events(&mut self.scheduler_checkpoint_control_events);
        }
    }

    fn has_execution_mode_switch_state_transfer_banks(&self) -> bool {
        self.accelerator_checkpoints.is_some()
            || self.scheduler_checkpoints.is_some()
            || self.msi_bank_checkpoints.is_some()
            || self.fabric_checkpoints.is_some()
            || self.gpu_checkpoints.is_some()
            || self.riscv_checkpoints.is_some()
            || self.memory_checkpoints.is_some()
            || self.dram_memory_checkpoints.is_some()
            || self.storage_image_checkpoints.is_some()
            || self.guest_fd_checkpoints.is_some()
            || self.guest_futex_checkpoints.is_some()
            || self.guest_wait_checkpoints.is_some()
            || self.ide_controller_checkpoints.is_some()
            || self.sinic_register_checkpoints.is_some()
            || self.sinic_fifo_checkpoints.is_some()
            || self.readfile_checkpoints.is_some()
            || self.interrupt_controller_checkpoints.is_some()
            || self.clint_checkpoints.is_some()
            || self.timer_checkpoints.is_some()
            || self.uart_checkpoints.is_some()
            || self.pl011_uart_checkpoints.is_some()
            || self.plic_checkpoints.is_some()
            || self.pl031_checkpoints.is_some()
            || self.sp804_checkpoints.is_some()
            || self.sp805_checkpoints.is_some()
            || self.cpu_local_timer_checkpoints.is_some()
            || self.rtc_checkpoints.is_some()
            || self.pci_host_checkpoints.is_some()
            || self.pci_legacy_interrupt_router_checkpoints.is_some()
            || self.virtio_split_queue_checkpoints.is_some()
            || self.virtio_pci_isr_checkpoints.is_some()
            || self.virtio_pci_common_checkpoints.is_some()
            || self.virtio_pci_notify_checkpoints.is_some()
            || self.virtio_pci_device_config_checkpoints.is_some()
    }

    pub(super) fn track_borrowed_scheduler_checkpoint_component(
        &mut self,
        component: CheckpointComponentId,
    ) {
        if self
            .scheduler_checkpoints
            .as_ref()
            .is_some_and(|bank| bank.has_component(&component))
        {
            self.borrowed_scheduler_checkpoint_components
                .remove(&component);
        } else {
            self.borrowed_scheduler_checkpoint_components
                .insert(component);
        }
    }
}

fn execution_mode_switch_state_transfer_label(target: &ExecutionModeTarget, tick: Tick) -> String {
    let sanitized_target = target
        .as_str()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("{EXECUTION_MODE_SWITCH_STATE_TRANSFER_LABEL_PREFIX}{sanitized_target}-{tick}")
}

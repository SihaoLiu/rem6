use rem6_checkpoint::CheckpointComponentId;
use rem6_kernel::SchedulerCheckpointAccess;

use crate::riscv_data_access_stats::{
    PreparedRiscvDataAccessProbeRestore, RiscvDataAccessProbeCheckpoint,
};
use crate::riscv_instruction_stats::PreparedRiscvRetiredInstructionProbeRestore;
use crate::scheduler_checkpoint::{SchedulerCheckpointBankGuard, SchedulerCheckpointContext};

use super::*;

impl SystemActionExecutor {
    pub fn apply(&mut self, record: &HostActionRecord) -> Result<SystemActionOutcome, SystemError> {
        self.ensure_checkpoint_action_supported(record.action())?;
        let scheduler_checkpoints = self.scheduler_checkpoints.clone();
        let mut scheduler_checkpoint_bank = if action_uses_scheduler_checkpoint(record.action()) {
            scheduler_checkpoints
                .as_ref()
                .map(|bank| bank.try_lock_except(None))
                .transpose()
                .map_err(SystemError::SchedulerCheckpoint)?
        } else {
            None
        };
        if action_uses_scheduler_checkpoint(record.action()) {
            self.retain_attached_scheduler_checkpoint_control_events(
                scheduler_checkpoint_bank.as_ref(),
            );
        }
        self.apply_with_scheduler_context(record, None, scheduler_checkpoint_bank.as_mut())
    }

    pub(crate) fn apply_with_scheduler_checkpoint(
        &mut self,
        record: &HostActionRecord,
        component: CheckpointComponentId,
        scheduler: SchedulerCheckpointAccess<'_>,
    ) -> Result<SystemActionOutcome, SystemError> {
        self.ensure_checkpoint_action_supported(record.action())?;
        let mut scheduler_checkpoint = SchedulerCheckpointContext::new(component, scheduler);
        let scheduler_checkpoints = self.scheduler_checkpoints.clone();
        let mut scheduler_checkpoint_bank = None;
        if action_uses_scheduler_checkpoint(record.action()) {
            if let Some(bank) = &scheduler_checkpoints {
                bank.validate_borrowed_scheduler(&scheduler_checkpoint)
                    .map_err(SystemError::SchedulerCheckpoint)?;
                scheduler_checkpoint_bank = Some(
                    bank.try_lock_except(Some(scheduler_checkpoint.component()))
                        .map_err(SystemError::SchedulerCheckpoint)?,
                );
            }
            self.retain_attached_scheduler_checkpoint_control_events(
                scheduler_checkpoint_bank.as_ref(),
            );
            let scheduler_instance = scheduler_checkpoint.scheduler_instance();
            let scheduler_snapshot = scheduler_checkpoint.scheduler_snapshot();
            self.retain_scheduler_checkpoint_control_events(
                scheduler_instance,
                &scheduler_snapshot,
            );
        }
        self.apply_with_scheduler_context(
            record,
            Some(&mut scheduler_checkpoint),
            scheduler_checkpoint_bank.as_mut(),
        )
    }

    fn ensure_checkpoint_action_supported(&self, action: &HostAction) -> Result<(), SystemError> {
        if action_uses_scheduler_checkpoint(action) {
            if let Some(reason) = self.checkpoint_action_rejections.iter().next() {
                return Err(SystemError::CheckpointActionsUnsupported {
                    reason: reason.clone(),
                });
            }
        }
        Ok(())
    }

    fn apply_with_scheduler_context(
        &mut self,
        record: &HostActionRecord,
        scheduler_checkpoint: Option<&mut SchedulerCheckpointContext<'_>>,
        scheduler_checkpoint_bank: Option<&mut SchedulerCheckpointBankGuard<'_>>,
    ) -> Result<SystemActionOutcome, SystemError> {
        match record.action() {
            HostAction::InjectCommand { command } => Ok(SystemActionOutcome::InjectedCommand {
                tick: record.tick(),
                event: record.event(),
                source: record.source(),
                command: command.clone(),
            }),
            HostAction::RecordGuestHostCall {
                selector,
                arguments,
                payload,
            } => Ok(SystemActionOutcome::GuestHostCall {
                tick: record.tick(),
                event: record.event(),
                source: record.source(),
                selector: *selector,
                arguments: arguments.clone(),
                payload: payload.clone(),
                response: self.resolve_guest_host_call_response(*selector),
            }),
            HostAction::RecordRoiBegin { work_id, thread_id } => {
                Ok(SystemActionOutcome::RoiBegin {
                    tick: record.tick(),
                    event: record.event(),
                    source: record.source(),
                    work_id: *work_id,
                    thread_id: *thread_id,
                })
            }
            HostAction::RecordRoiEnd { work_id, thread_id } => Ok(SystemActionOutcome::RoiEnd {
                tick: record.tick(),
                event: record.event(),
                source: record.source(),
                work_id: *work_id,
                thread_id: *thread_id,
            }),
            HostAction::ResetStats => {
                if let Some(hook) = &self.pre_stats_sync {
                    hook.sync(&mut self.stats, StatsSyncPhase::BeforeReset)?;
                }
                let outcome = self
                    .stats
                    .try_reset(record.tick())
                    .map(SystemActionOutcome::StatsReset)
                    .map_err(SystemError::Stats)?;
                if let Some(hook) = &self.pre_stats_sync {
                    hook.sync(&mut self.stats, StatsSyncPhase::AfterReset)?;
                }
                Ok(outcome)
            }
            HostAction::DumpStats => {
                if let Some(hook) = &self.pre_stats_sync {
                    hook.sync(&mut self.stats, StatsSyncPhase::BeforeDump)?;
                }
                let active_o3_cpus = self
                    .riscv_o3_runtime_stats
                    .as_ref()
                    .map(RiscvO3RuntimeStats::active_cpu_indices)
                    .unwrap_or_default();
                self.stats
                    .try_dump(record.tick())
                    .map(|record| SystemActionOutcome::StatsDump {
                        record,
                        active_o3_cpus,
                    })
                    .map_err(SystemError::Stats)
            }
            HostAction::SwitchExecutionMode { target, mode } => {
                let state_transfer = self
                    .capture_execution_mode_switch_state_transfer_with_scheduler(
                        record,
                        target,
                        *mode,
                        scheduler_checkpoint,
                        scheduler_checkpoint_bank.as_deref(),
                    )?;
                let previous_mode = self.execution_modes.insert(target.clone(), *mode);
                Ok(SystemActionOutcome::ExecutionModeSwitched {
                    tick: record.tick(),
                    event: record.event(),
                    source: record.source(),
                    target: target.clone(),
                    previous_mode,
                    mode: *mode,
                    stats_epoch: self.stats.epoch(),
                    stats_reset_tick: self.stats.reset_tick(),
                    state_transfer,
                })
            }
            HostAction::Checkpoint { label } => {
                if is_execution_mode_switch_state_transfer_label(label) {
                    return Err(SystemError::ReservedCheckpointManifestLabel {
                        label: label.clone(),
                        prefix: EXECUTION_MODE_SWITCH_STATE_TRANSFER_LABEL_PREFIX.to_string(),
                    });
                }
                let instruction_probe_snapshot = self
                    .riscv_instruction_stats
                    .as_ref()
                    .map(|stats| stats.retired_instruction_probe_snapshot());
                let data_access_probe_checkpoint =
                    self.prepare_riscv_data_access_probes_for_capture()?;
                let memory_trace_checkpoint =
                    self.memory_traces
                        .as_ref()
                        .map(|(fetch, data)| MemoryTraceCheckpoint {
                            fetch: fetch.snapshot(),
                            data: data.snapshot(),
                        });
                let mut staged_checkpoints = self.checkpoints.clone();
                let capture = self.capture_attached_checkpoint_banks_into_with_scheduler(
                    &mut staged_checkpoints,
                    record.tick(),
                    scheduler_checkpoint,
                    scheduler_checkpoint_bank.as_deref(),
                    None,
                    false,
                )?;
                self.capture_execution_modes_into(&mut staged_checkpoints)?;
                let manifest = staged_checkpoints
                    .capture(label.clone(), record.tick())
                    .map_err(SystemError::Checkpoint)?;
                self.commit_attached_checkpoint_capture(&capture);
                self.checkpoints = staged_checkpoints;
                self.captured_manifests
                    .insert(manifest.label().to_string(), manifest.clone());
                if let Some(snapshot) = instruction_probe_snapshot {
                    let checkpoint_index = self.riscv_instruction_probe_checkpoints.len();
                    self.riscv_instruction_probe_checkpoints
                        .push((manifest.clone(), snapshot));
                    if capture.pending_data_address_live_checkpoint {
                        self.pending_riscv_instruction_probe_checkpoint_indices
                            .insert(checkpoint_index);
                    }
                }
                if let Some(checkpoint) = data_access_probe_checkpoint {
                    self.riscv_data_access_probe_checkpoints
                        .push((manifest.clone(), checkpoint));
                }
                if let Some(checkpoint) = memory_trace_checkpoint {
                    self.memory_trace_checkpoints
                        .push((manifest.clone(), checkpoint));
                }
                Ok(SystemActionOutcome::Checkpoint {
                    tick: record.tick(),
                    event: record.event(),
                    source: record.source(),
                    manifest,
                })
            }
            HostAction::RestoreCheckpointByLabel { label } => {
                let manifest = self.captured_manifests.get(label).cloned().ok_or_else(|| {
                    SystemError::MissingCheckpointManifest {
                        label: label.clone(),
                    }
                })?;
                let instruction_probe_restore =
                    self.prepare_riscv_instruction_probes_for_manifest(&manifest)?;
                let data_access_probe_restore =
                    self.prepare_riscv_data_access_probes_for_manifest(&manifest);
                let memory_trace_restore = self.prepare_memory_traces_for_manifest(&manifest);
                let rebound_o3_wake_components = self.restore_checkpoint_manifest_with_scheduler(
                    &manifest,
                    scheduler_checkpoint,
                    scheduler_checkpoint_bank,
                )?;
                self.install_prepared_riscv_instruction_probe_restore(instruction_probe_restore);
                self.install_prepared_riscv_data_access_probe_restore(data_access_probe_restore);
                self.install_prepared_memory_trace_restore(memory_trace_restore);
                Ok(SystemActionOutcome::CheckpointRestored {
                    tick: record.tick(),
                    event: record.event(),
                    source: record.source(),
                    manifest,
                    rebound_o3_wake_components,
                })
            }
            HostAction::RestoreCheckpoint { manifest } => {
                let instruction_probe_restore =
                    self.prepare_riscv_instruction_probes_for_manifest(manifest)?;
                let data_access_probe_restore =
                    self.prepare_riscv_data_access_probes_for_manifest(manifest);
                let memory_trace_restore = self.prepare_memory_traces_for_manifest(manifest);
                let rebound_o3_wake_components = self.restore_checkpoint_manifest_with_scheduler(
                    manifest,
                    scheduler_checkpoint,
                    scheduler_checkpoint_bank,
                )?;
                self.install_prepared_riscv_instruction_probe_restore(instruction_probe_restore);
                self.install_prepared_riscv_data_access_probe_restore(data_access_probe_restore);
                self.install_prepared_memory_trace_restore(memory_trace_restore);
                Ok(SystemActionOutcome::CheckpointRestored {
                    tick: record.tick(),
                    event: record.event(),
                    source: record.source(),
                    manifest: manifest.clone(),
                    rebound_o3_wake_components,
                })
            }
            HostAction::Stop { code } => Ok(SystemActionOutcome::Stop(StopRequest::new(
                record.tick(),
                record.event(),
                record.source(),
                *code,
            ))),
        }
    }

    fn prepare_riscv_instruction_probes_for_manifest(
        &self,
        manifest: &rem6_checkpoint::CheckpointManifest,
    ) -> Result<Option<PreparedRiscvRetiredInstructionProbeRestore>, SystemError> {
        let Some(instruction_stats) = self.riscv_instruction_stats.as_ref() else {
            return Ok(None);
        };
        let Some((index, (_captured, snapshot))) = self
            .riscv_instruction_probe_checkpoints
            .iter()
            .enumerate()
            .rev()
            .find(|(_index, (captured, _snapshot))| captured == manifest)
        else {
            return Ok(None);
        };
        if self
            .pending_riscv_instruction_probe_checkpoint_indices
            .contains(&index)
        {
            return Err(SystemError::PendingInstructionProbeCheckpoint {
                label: manifest.label().to_string(),
                tick: manifest.tick(),
            });
        }
        instruction_stats
            .prepare_retired_instruction_probe_restore(snapshot)
            .map(Some)
            .map_err(SystemError::Stats)
    }

    pub(crate) fn finalize_pending_riscv_instruction_probe_checkpoints(&mut self, tick: Tick) {
        let Some(instruction_stats) = self.riscv_instruction_stats.as_ref() else {
            return;
        };
        let indices = self
            .pending_riscv_instruction_probe_checkpoint_indices
            .iter()
            .copied()
            .filter(|index| {
                self.riscv_instruction_probe_checkpoints
                    .get(*index)
                    .is_some_and(|(manifest, _)| manifest.tick() == tick)
            })
            .collect::<Vec<_>>();
        if indices.is_empty() {
            return;
        }
        let snapshot = instruction_stats.retired_instruction_probe_snapshot();
        for index in &indices {
            self.riscv_instruction_probe_checkpoints[*index].1 = snapshot.clone();
        }
        for index in indices {
            self.pending_riscv_instruction_probe_checkpoint_indices
                .remove(&index);
        }
    }

    fn prepare_riscv_data_access_probes_for_capture(
        &self,
    ) -> Result<Option<RiscvDataAccessProbeCheckpoint>, SystemError> {
        let Some(data_access_stats) = self.riscv_data_access_stats.as_ref() else {
            return Ok(None);
        };
        let Some(riscv_checkpoints) = self.riscv_checkpoints.as_ref() else {
            return Ok(Some(data_access_stats.data_access_probe_checkpoint()));
        };
        let mut checkpoint = data_access_stats.data_access_probe_checkpoint();
        let events = riscv_checkpoints
            .data_access_event_snapshots_from_cursors(checkpoint.cursors())
            .map_err(SystemError::RiscvCheckpoint)?;
        checkpoint
            .record_data_access_events(events)
            .map_err(SystemError::Stats)?;
        Ok(Some(checkpoint))
    }

    fn install_prepared_riscv_instruction_probe_restore(
        &self,
        prepared: Option<PreparedRiscvRetiredInstructionProbeRestore>,
    ) {
        if let (Some(instruction_stats), Some(prepared)) =
            (self.riscv_instruction_stats.as_ref(), prepared)
        {
            instruction_stats.install_prepared_retired_instruction_probe_restore(prepared);
        }
    }

    fn prepare_riscv_data_access_probes_for_manifest(
        &self,
        manifest: &rem6_checkpoint::CheckpointManifest,
    ) -> Option<PreparedRiscvDataAccessProbeRestore> {
        let (Some(data_access_stats), Some(checkpoint)) = (
            self.riscv_data_access_stats.as_ref(),
            self.riscv_data_access_probe_checkpoints
                .iter()
                .rev()
                .find(|(captured, _checkpoint)| captured == manifest)
                .map(|(_captured, checkpoint)| checkpoint),
        ) else {
            return None;
        };
        Some(data_access_stats.prepare_data_access_probe_restore(checkpoint))
    }

    fn install_prepared_riscv_data_access_probe_restore(
        &self,
        mut prepared: Option<PreparedRiscvDataAccessProbeRestore>,
    ) {
        if let (Some(prepared), Some(riscv_checkpoints)) =
            (prepared.as_mut(), self.riscv_checkpoints.as_ref())
        {
            prepared.rebase_cursors(riscv_checkpoints.data_access_event_cursors());
        }
        if let (Some(data_access_stats), Some(prepared)) =
            (self.riscv_data_access_stats.as_ref(), prepared)
        {
            data_access_stats.install_prepared_data_access_probe_restore(prepared);
        }
    }

    fn prepare_memory_traces_for_manifest(
        &self,
        manifest: &rem6_checkpoint::CheckpointManifest,
    ) -> Option<PreparedMemoryTraceRestore> {
        let checkpoint = self
            .memory_trace_checkpoints
            .iter()
            .rev()
            .find(|(captured, _checkpoint)| captured == manifest)
            .map(|(_captured, checkpoint)| checkpoint)?;
        self.memory_traces
            .as_ref()
            .map(|_| PreparedMemoryTraceRestore {
                fetch: checkpoint.fetch.clone(),
                data: checkpoint.data.clone(),
            })
    }

    fn install_prepared_memory_trace_restore(&self, prepared: Option<PreparedMemoryTraceRestore>) {
        if let (Some((fetch, data)), Some(prepared)) = (&self.memory_traces, prepared) {
            fetch.restore_checkpoint_events(prepared.fetch);
            data.restore_checkpoint_events(prepared.data);
        }
    }
}

fn action_uses_scheduler_checkpoint(action: &HostAction) -> bool {
    matches!(
        action,
        HostAction::SwitchExecutionMode { .. }
            | HostAction::Checkpoint { .. }
            | HostAction::RestoreCheckpointByLabel { .. }
            | HostAction::RestoreCheckpoint { .. }
    )
}

#[cfg(test)]
mod tests;

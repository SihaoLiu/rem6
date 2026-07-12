use rem6_checkpoint::CheckpointComponentId;
use rem6_kernel::SchedulerCheckpointAccess;

use crate::scheduler_checkpoint::{SchedulerCheckpointBankGuard, SchedulerCheckpointContext};

use super::*;

impl SystemActionExecutor {
    pub fn apply(&mut self, record: &HostActionRecord) -> Result<SystemActionOutcome, SystemError> {
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
                let mut staged_checkpoints = self.checkpoints.clone();
                self.capture_attached_checkpoint_banks_into_with_scheduler(
                    &mut staged_checkpoints,
                    record.tick(),
                    scheduler_checkpoint,
                    scheduler_checkpoint_bank.as_deref(),
                    None,
                )?;
                self.capture_execution_modes_into(&mut staged_checkpoints)?;
                let manifest = staged_checkpoints
                    .capture(label.clone(), record.tick())
                    .map_err(SystemError::Checkpoint)?;
                self.checkpoints = staged_checkpoints;
                self.captured_manifests
                    .insert(manifest.label().to_string(), manifest.clone());
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
                self.restore_checkpoint_manifest_with_scheduler(
                    &manifest,
                    scheduler_checkpoint,
                    scheduler_checkpoint_bank,
                )?;
                Ok(SystemActionOutcome::CheckpointRestored {
                    tick: record.tick(),
                    event: record.event(),
                    source: record.source(),
                    manifest,
                })
            }
            HostAction::RestoreCheckpoint { manifest } => {
                self.restore_checkpoint_manifest_with_scheduler(
                    manifest,
                    scheduler_checkpoint,
                    scheduler_checkpoint_bank,
                )?;
                Ok(SystemActionOutcome::CheckpointRestored {
                    tick: record.tick(),
                    event: record.event(),
                    source: record.source(),
                    manifest: manifest.clone(),
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
mod tests {
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    use rem6_checkpoint::CheckpointState;
    use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};
    use rem6_memory::PartitionedMemoryStore;
    use rem6_stats::StatsRegistry;

    use crate::scheduler_checkpoint::{
        SchedulerCheckpointBank, SchedulerCheckpointOwnedEvent, SchedulerCheckpointPort,
    };
    use crate::{
        GuestEventId, GuestSourceId, HostAction, MemoryStoreCheckpointBank,
        MemoryStoreCheckpointPort, SchedulerCheckpointError,
    };

    use super::*;

    fn scheduler_component(name: &str) -> CheckpointComponentId {
        CheckpointComponentId::new(name).unwrap()
    }

    fn checkpoint_record(label: &str) -> HostActionRecord {
        HostActionRecord::new(
            0,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(1),
            GuestSourceId::new(1),
            HostAction::Checkpoint {
                label: label.to_string(),
            },
        )
    }

    fn assert_action_holds_scheduler_while_waiting_on_memory(
        mut executor: SystemActionExecutor,
        record: HostActionRecord,
        scheduler: Arc<Mutex<PartitionedScheduler>>,
        memory: Arc<Mutex<PartitionedMemoryStore>>,
    ) -> (
        SystemActionExecutor,
        Result<SystemActionOutcome, SystemError>,
    ) {
        let memory_guard = memory.lock().unwrap();
        let (started_sender, started_receiver) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            started_sender.send(()).unwrap();
            let result = executor.apply(&record);
            (executor, result)
        });
        started_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        std::thread::sleep(Duration::from_millis(50));
        assert!(!worker.is_finished());
        let (acquired_sender, acquired_receiver) = mpsc::channel();
        let peer = std::thread::spawn(move || {
            let _scheduler = scheduler.lock().unwrap();
            acquired_sender.send(()).unwrap();
        });

        assert!(acquired_receiver
            .recv_timeout(Duration::from_millis(100))
            .is_err());
        drop(memory_guard);
        let result = worker.join().unwrap();
        acquired_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        peer.join().unwrap();
        result
    }

    fn executor_with_scheduler_and_memory(
        scheduler: Arc<Mutex<PartitionedScheduler>>,
        memory: Arc<Mutex<PartitionedMemoryStore>>,
    ) -> SystemActionExecutor {
        let scheduler_component = scheduler_component("scheduler0");
        let memory_component = CheckpointComponentId::new("memory0").unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        executor
            .attach_scheduler_checkpoint_bank(
                SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
                    scheduler_component,
                    scheduler,
                )])
                .unwrap(),
            )
            .unwrap();
        executor
            .attach_memory_checkpoint_bank(
                MemoryStoreCheckpointBank::new([MemoryStoreCheckpointPort::new(
                    memory_component,
                    memory,
                )])
                .unwrap(),
            )
            .unwrap();
        executor
    }

    #[test]
    fn non_checkpoint_action_does_not_lock_attached_scheduler() {
        let component = scheduler_component("scheduler0");
        let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component,
            Arc::clone(&scheduler),
        )])
        .unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        executor.attach_scheduler_checkpoint_bank(bank).unwrap();
        let record = HostActionRecord::new(
            0,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(1),
            GuestSourceId::new(1),
            HostAction::Stop { code: 0 },
        );
        let guard = scheduler.lock().unwrap();
        let (sender, receiver) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            sender.send(executor.apply(&record)).unwrap();
        });

        let outcome = receiver.recv_timeout(Duration::from_millis(100));
        drop(guard);
        worker.join().unwrap();

        assert!(matches!(outcome, Ok(Ok(SystemActionOutcome::Stop(_)))));
    }

    #[test]
    fn borrowed_scheduler_rejects_attached_component_instance_mismatch() {
        let component = scheduler_component("scheduler0");
        let attached = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let attached_scheduler = attached.lock().unwrap().instance_id();
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component.clone(),
            attached,
        )])
        .unwrap();
        let mut borrowed = PartitionedScheduler::new(1).unwrap();
        let borrowed_scheduler = borrowed.instance_id();
        let event_id = borrowed
            .schedule_at(PartitionId::new(0), 5, |_| {})
            .unwrap();
        let event = borrowed.pending_event_snapshot(event_id).unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        executor.attach_scheduler_checkpoint_bank(bank).unwrap();
        executor.scheduler_checkpoint_control_events.push(
            SchedulerCheckpointOwnedEvent::discard_on_restore(borrowed.instance_id(), event),
        );

        let result = executor.apply_with_scheduler_checkpoint(
            &checkpoint_record("mismatch"),
            component.clone(),
            borrowed.checkpoint_access(),
        );

        assert_eq!(
            result.unwrap_err(),
            SystemError::SchedulerCheckpoint(
                SchedulerCheckpointError::BorrowedSchedulerBindingMismatch {
                    borrowed_component: component.clone(),
                    borrowed_scheduler,
                    attached_component: component,
                    attached_scheduler,
                }
            )
        );
    }

    #[test]
    fn borrowed_scheduler_rejects_attached_alias_component_without_relocking() {
        let attached_component = scheduler_component("scheduler0");
        let borrowed_component = scheduler_component("scheduler-alias");
        let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let scheduler_instance = scheduler.lock().unwrap().instance_id();
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            attached_component.clone(),
            Arc::clone(&scheduler),
        )])
        .unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        executor.attach_scheduler_checkpoint_bank(bank).unwrap();
        let mut scheduler = scheduler.lock().unwrap();

        let result = executor.apply_with_scheduler_checkpoint(
            &checkpoint_record("alias"),
            borrowed_component.clone(),
            scheduler.checkpoint_access(),
        );

        assert_eq!(
            result.unwrap_err(),
            SystemError::SchedulerCheckpoint(
                SchedulerCheckpointError::BorrowedSchedulerBindingMismatch {
                    borrowed_component,
                    borrowed_scheduler: scheduler_instance,
                    attached_component,
                    attached_scheduler: scheduler_instance,
                }
            )
        );
    }

    #[test]
    fn borrowed_scheduler_checkpoint_rejects_locked_attached_peer() {
        let component0 = scheduler_component("scheduler0");
        let component1 = scheduler_component("scheduler1");
        let scheduler0 = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let scheduler1 = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let bank = SchedulerCheckpointBank::new([
            SchedulerCheckpointPort::new(component0.clone(), Arc::clone(&scheduler0)),
            SchedulerCheckpointPort::new(component1.clone(), Arc::clone(&scheduler1)),
        ])
        .unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        executor.attach_scheduler_checkpoint_bank(bank).unwrap();
        let (locked_sender, locked_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let peer = std::thread::spawn(move || {
            let scheduler1 = scheduler1.lock().unwrap();
            locked_sender.send(()).unwrap();
            release_receiver
                .recv_timeout(Duration::from_secs(2))
                .unwrap();
            drop(scheduler1);
        });
        locked_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        let mut scheduler0 = scheduler0.lock().unwrap();

        let result = executor.apply_with_scheduler_checkpoint(
            &checkpoint_record("busy-peer"),
            component0,
            scheduler0.checkpoint_access(),
        );
        release_sender.send(()).unwrap();
        peer.join().unwrap();

        assert_eq!(
            result.unwrap_err(),
            SystemError::SchedulerCheckpoint(SchedulerCheckpointError::SchedulerBusy {
                component: component1,
            })
        );
    }

    #[test]
    fn projected_restore_rejects_past_preserved_event_before_mode_commit() {
        let component = scheduler_component("scheduler0");
        let target = ExecutionModeTarget::new("cpu0");
        let source_scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        {
            let mut scheduler = source_scheduler.lock().unwrap();
            scheduler
                .schedule_at(PartitionId::new(0), 10, |_| {})
                .unwrap();
            scheduler.run_until_idle();
        }
        let mut source =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        source
            .attach_scheduler_checkpoint_bank(
                SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
                    component.clone(),
                    source_scheduler,
                )])
                .unwrap(),
            )
            .unwrap();
        source.set_execution_mode(target.clone(), ExecutionMode::Functional);
        let source_checkpoint = HostActionRecord::new(
            10,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::Checkpoint {
                label: "forward".to_string(),
            },
        );
        let SystemActionOutcome::Checkpoint { manifest, .. } =
            source.apply(&source_checkpoint).unwrap()
        else {
            panic!("expected checkpoint outcome");
        };

        let target_scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let (scheduler_instance, preserved) = {
            let mut scheduler = target_scheduler.lock().unwrap();
            let id = scheduler
                .schedule_at(PartitionId::new(0), 5, |_| {})
                .unwrap();
            (
                scheduler.instance_id(),
                scheduler.pending_event_snapshot(id).unwrap(),
            )
        };
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        executor
            .attach_scheduler_checkpoint_bank(
                SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
                    component.clone(),
                    target_scheduler,
                )])
                .unwrap(),
            )
            .unwrap();
        executor.set_execution_mode(target.clone(), ExecutionMode::Detailed);
        executor.register_scheduler_checkpoint_control_event(scheduler_instance, preserved);
        let restore = HostActionRecord::new(
            20,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(3),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpoint { manifest },
        );

        let error = executor.apply(&restore).unwrap_err();

        assert_eq!(
            error,
            SystemError::SchedulerCheckpoint(SchedulerCheckpointError::Scheduler {
                component,
                error: SchedulerError::InThePast {
                    partition: PartitionId::new(0),
                    now: 10,
                    requested: 5,
                },
            })
        );
        assert_eq!(
            executor.execution_mode(&target),
            Some(ExecutionMode::Detailed)
        );
    }

    #[test]
    fn attached_scheduler_replacement_rejects_colliding_owned_event() {
        let component = scheduler_component("scheduler0");
        let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let scheduler_instance = scheduler.lock().unwrap().instance_id();
        let event_id = scheduler
            .lock()
            .unwrap()
            .schedule_at(PartitionId::new(0), 5, |_| {})
            .unwrap();
        let event = scheduler
            .lock()
            .unwrap()
            .pending_event_snapshot(event_id)
            .unwrap();
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component.clone(),
            Arc::clone(&scheduler),
        )])
        .unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        executor.attach_scheduler_checkpoint_bank(bank).unwrap();
        executor.register_scheduler_checkpoint_control_event(scheduler_instance, event);
        let replacement = PartitionedScheduler::new(1).unwrap();
        let replacement_instance = replacement.instance_id();
        *scheduler.lock().unwrap() = replacement;
        let replacement_id = scheduler
            .lock()
            .unwrap()
            .schedule_at(PartitionId::new(0), 5, |_| {})
            .unwrap();
        let replacement_event = scheduler
            .lock()
            .unwrap()
            .pending_event_snapshot(replacement_id)
            .unwrap();
        assert_eq!(replacement_event, event);

        let error = executor
            .apply(&checkpoint_record("replacement"))
            .unwrap_err();

        assert_eq!(
            error,
            SystemError::SchedulerCheckpoint(
                SchedulerCheckpointError::AttachedSchedulerBindingMismatch {
                    component,
                    bound_scheduler: scheduler_instance,
                    live_scheduler: replacement_instance,
                }
            )
        );
    }

    #[test]
    fn borrowed_scheduler_rejects_detached_instance_after_attached_storage_rebind() {
        let component = scheduler_component("scheduler0");
        let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let bound_scheduler = scheduler.lock().unwrap().instance_id();
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component.clone(),
            Arc::clone(&scheduler),
        )])
        .unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        executor.attach_scheduler_checkpoint_bank(bank).unwrap();
        let mut detached = {
            let mut attached = scheduler.lock().unwrap();
            std::mem::replace(&mut *attached, PartitionedScheduler::new(1).unwrap())
        };
        let foreign_id = scheduler
            .lock()
            .unwrap()
            .schedule_at(PartitionId::new(0), 5, |_| {})
            .unwrap();
        let foreign = scheduler
            .lock()
            .unwrap()
            .pending_event_snapshot(foreign_id)
            .unwrap();

        let error = executor
            .apply_with_scheduler_checkpoint(
                &checkpoint_record("detached"),
                component.clone(),
                detached.checkpoint_access(),
            )
            .unwrap_err();

        assert_eq!(
            error,
            SystemError::SchedulerCheckpoint(
                SchedulerCheckpointError::BorrowedSchedulerStorageMismatch {
                    borrowed_component: component.clone(),
                    borrowed_scheduler: bound_scheduler,
                    attached_component: component,
                    attached_scheduler: bound_scheduler,
                }
            )
        );
        assert_eq!(
            scheduler.lock().unwrap().pending_event_snapshot(foreign_id),
            Some(foreign)
        );
    }

    #[test]
    fn checkpoint_holds_attached_scheduler_through_later_bank_capture() {
        let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let memory = Arc::new(Mutex::new(PartitionedMemoryStore::new()));
        let executor =
            executor_with_scheduler_and_memory(Arc::clone(&scheduler), Arc::clone(&memory));

        let (_executor, result) = assert_action_holds_scheduler_while_waiting_on_memory(
            executor,
            checkpoint_record("capture-lock"),
            scheduler,
            memory,
        );

        assert!(matches!(result, Ok(SystemActionOutcome::Checkpoint { .. })));
    }

    #[test]
    fn restore_holds_attached_scheduler_through_later_bank_restore() {
        let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let memory = Arc::new(Mutex::new(PartitionedMemoryStore::new()));
        let mut executor =
            executor_with_scheduler_and_memory(Arc::clone(&scheduler), Arc::clone(&memory));
        executor.apply(&checkpoint_record("restore-lock")).unwrap();
        let restore = HostActionRecord::new(
            1,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpointByLabel {
                label: "restore-lock".to_string(),
            },
        );

        let (_executor, result) = assert_action_holds_scheduler_while_waiting_on_memory(
            executor, restore, scheduler, memory,
        );

        assert!(matches!(
            result,
            Ok(SystemActionOutcome::CheckpointRestored { .. })
        ));
    }

    #[test]
    fn borrowed_scheduler_capture_keeps_other_attached_scheduler_ports() {
        let component0 = scheduler_component("scheduler0");
        let component1 = scheduler_component("scheduler1");
        let scheduler0 = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let scheduler1 = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let bank = SchedulerCheckpointBank::new([
            SchedulerCheckpointPort::new(component0.clone(), Arc::clone(&scheduler0)),
            SchedulerCheckpointPort::new(component1.clone(), Arc::clone(&scheduler1)),
        ])
        .unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        executor.attach_scheduler_checkpoint_bank(bank).unwrap();
        let mut scheduler0 = scheduler0.lock().unwrap();

        executor
            .apply_with_scheduler_checkpoint(
                &checkpoint_record("all-schedulers"),
                component0.clone(),
                scheduler0.checkpoint_access(),
            )
            .unwrap();

        assert!(executor
            .checkpoints()
            .chunk(&component0, "scheduler")
            .is_some());
        assert!(executor
            .checkpoints()
            .chunk(&component1, "scheduler")
            .is_some());
    }

    #[test]
    fn borrowed_scheduler_capture_removes_stale_chunk_from_later_direct_manifest() {
        let component = scheduler_component("scheduler0");
        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        let event_id = scheduler
            .schedule_at(PartitionId::new(0), 5, |_| {})
            .unwrap();
        let event = scheduler.pending_event_snapshot(event_id).unwrap();
        executor.scheduler_checkpoint_control_events.push(
            SchedulerCheckpointOwnedEvent::discard_on_restore(scheduler.instance_id(), event),
        );

        executor
            .apply_with_scheduler_checkpoint(
                &checkpoint_record("with-scheduler"),
                component.clone(),
                scheduler.checkpoint_access(),
            )
            .unwrap();
        assert!(executor
            .checkpoints()
            .chunk(&component, "scheduler")
            .is_some());
        scheduler.cancel_event(event_id).unwrap();

        let manifest = match executor
            .apply(&checkpoint_record("without-scheduler"))
            .unwrap()
        {
            SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
            other => panic!("unexpected outcome: {other:?}"),
        };

        assert!(executor
            .checkpoints()
            .chunk(&component, "scheduler")
            .is_none());
        assert!(!executor.checkpoints().contains_component(&component));
        assert!(manifest
            .states()
            .iter()
            .all(|state| state.component() != &component));

        let restore_borrowed = HostActionRecord::new(
            1,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpointByLabel {
                label: "with-scheduler".to_string(),
            },
        );
        executor
            .apply_with_scheduler_checkpoint(
                &restore_borrowed,
                component.clone(),
                scheduler.checkpoint_access(),
            )
            .unwrap();
        assert!(executor.checkpoints().contains_component(&component));

        let mut restorer =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        let restore = HostActionRecord::new(
            1,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpoint { manifest },
        );
        restorer.apply(&restore).unwrap();
    }

    #[test]
    fn borrowed_scheduler_restore_tracks_chunk_for_later_direct_manifest() {
        let component = scheduler_component("scheduler0");
        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        let mut source =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        let event_id = scheduler
            .schedule_at(PartitionId::new(0), 5, |_| {})
            .unwrap();
        let event = scheduler.pending_event_snapshot(event_id).unwrap();
        source.scheduler_checkpoint_control_events.push(
            SchedulerCheckpointOwnedEvent::discard_on_restore(scheduler.instance_id(), event),
        );
        let manifest = match source
            .apply_with_scheduler_checkpoint(
                &checkpoint_record("with-scheduler"),
                component.clone(),
                scheduler.checkpoint_access(),
            )
            .unwrap()
        {
            SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
            other => panic!("unexpected outcome: {other:?}"),
        };
        scheduler.cancel_event(event_id).unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        let restore = HostActionRecord::new(
            1,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpoint {
                manifest: manifest.clone(),
            },
        );
        executor
            .apply_with_scheduler_checkpoint(
                &restore,
                component.clone(),
                scheduler.checkpoint_access(),
            )
            .unwrap();

        let direct = match executor
            .apply(&checkpoint_record("without-scheduler"))
            .unwrap()
        {
            SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
            other => panic!("unexpected outcome: {other:?}"),
        };

        assert!(!executor.checkpoints().contains_component(&component));
        assert!(direct
            .states()
            .iter()
            .all(|state| state.component() != &component));
    }

    #[test]
    fn attached_scheduler_ownership_supersedes_stale_borrowed_tracking() {
        let component = scheduler_component("scheduler0");
        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        let event_id = scheduler
            .schedule_at(PartitionId::new(0), 5, |_| {})
            .unwrap();
        let event = scheduler.pending_event_snapshot(event_id).unwrap();
        executor.scheduler_checkpoint_control_events.push(
            SchedulerCheckpointOwnedEvent::discard_on_restore(scheduler.instance_id(), event),
        );
        executor
            .apply_with_scheduler_checkpoint(
                &checkpoint_record("borrowed"),
                component.clone(),
                scheduler.checkpoint_access(),
            )
            .unwrap();
        scheduler.cancel_event(event_id).unwrap();
        executor.apply(&checkpoint_record("prune")).unwrap();
        assert!(!executor.checkpoints().contains_component(&component));

        let scheduler = Arc::new(Mutex::new(scheduler));
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component.clone(),
            Arc::clone(&scheduler),
        )])
        .unwrap();
        executor.attach_scheduler_checkpoint_bank(bank).unwrap();

        let manifest = match executor.apply(&checkpoint_record("attached")).unwrap() {
            SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
            other => panic!("unexpected outcome: {other:?}"),
        };

        assert!(executor
            .checkpoints()
            .chunk(&component, "scheduler")
            .is_some());
        assert!(manifest.states().iter().any(|state| {
            state.component() == &component
                && state
                    .chunks()
                    .iter()
                    .any(|chunk| chunk.name() == "scheduler")
        }));
    }

    #[test]
    fn attached_scheduler_ownership_replaces_live_borrowed_component() {
        let component = scheduler_component("scheduler0");
        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        let event_id = scheduler
            .schedule_at(PartitionId::new(0), 5, |_| {})
            .unwrap();
        let event = scheduler.pending_event_snapshot(event_id).unwrap();
        executor.scheduler_checkpoint_control_events.push(
            SchedulerCheckpointOwnedEvent::discard_on_restore(scheduler.instance_id(), event),
        );
        executor
            .apply_with_scheduler_checkpoint(
                &checkpoint_record("borrowed"),
                component.clone(),
                scheduler.checkpoint_access(),
            )
            .unwrap();
        assert!(executor
            .checkpoints()
            .chunk(&component, "scheduler")
            .is_some());

        let scheduler = Arc::new(Mutex::new(scheduler));
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component.clone(),
            scheduler,
        )])
        .unwrap();
        executor.attach_scheduler_checkpoint_bank(bank).unwrap();

        let manifest = match executor.apply(&checkpoint_record("attached")).unwrap() {
            SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
            other => panic!("unexpected outcome: {other:?}"),
        };
        assert!(manifest.states().iter().any(|state| {
            state.component() == &component
                && state
                    .chunks()
                    .iter()
                    .any(|chunk| chunk.name() == "scheduler")
        }));
    }

    #[test]
    fn borrowed_scheduler_restore_keeps_other_attached_scheduler_ports() {
        let component0 = scheduler_component("scheduler0");
        let component1 = scheduler_component("scheduler1");
        let scheduler0 = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let scheduler1 = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let bank = SchedulerCheckpointBank::new([
            SchedulerCheckpointPort::new(component0.clone(), Arc::clone(&scheduler0)),
            SchedulerCheckpointPort::new(component1, Arc::clone(&scheduler1)),
        ])
        .unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        executor.attach_scheduler_checkpoint_bank(bank).unwrap();
        {
            let mut scheduler0 = scheduler0.lock().unwrap();
            executor
                .apply_with_scheduler_checkpoint(
                    &checkpoint_record("all-schedulers"),
                    component0.clone(),
                    scheduler0.checkpoint_access(),
                )
                .unwrap();
        }
        scheduler1
            .lock()
            .unwrap()
            .schedule_at(PartitionId::new(0), 7, |_| {})
            .unwrap();
        scheduler1.lock().unwrap().run_until_idle();
        assert_eq!(scheduler1.lock().unwrap().now(), 7);
        let restore = HostActionRecord::new(
            8,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpointByLabel {
                label: "all-schedulers".to_string(),
            },
        );
        let mut scheduler0 = scheduler0.lock().unwrap();

        executor
            .apply_with_scheduler_checkpoint(&restore, component0, scheduler0.checkpoint_access())
            .unwrap();

        assert_eq!(scheduler1.lock().unwrap().now(), 0);
    }

    #[test]
    fn borrowed_scheduler_restore_without_chunk_discards_only_pending_owned_wakes() {
        let component = scheduler_component("scheduler0");
        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        let manifest = match executor.apply(&checkpoint_record("legacy")).unwrap() {
            SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
            other => panic!("unexpected outcome: {other:?}"),
        };
        let event_id = scheduler
            .schedule_at(PartitionId::new(0), 5, |_| {})
            .unwrap();
        let event = scheduler.pending_event_snapshot(event_id).unwrap();
        let control_id = scheduler
            .schedule_at(PartitionId::new(0), 7, |_| {})
            .unwrap();
        let control = scheduler.pending_event_snapshot(control_id).unwrap();
        executor.register_scheduler_checkpoint_control_event(scheduler.instance_id(), control);
        let foreign_id = scheduler
            .schedule_at(PartitionId::new(0), 9, |_| {})
            .unwrap();
        let foreign = scheduler.pending_event_snapshot(foreign_id).unwrap();
        executor.scheduler_checkpoint_control_events.push(
            SchedulerCheckpointOwnedEvent::discard_on_restore(scheduler.instance_id(), event),
        );
        let restore = HostActionRecord::new(
            1,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpoint { manifest },
        );

        let outcome = executor
            .apply_with_scheduler_checkpoint(&restore, component, scheduler.checkpoint_access())
            .unwrap();

        assert!(matches!(
            outcome,
            SystemActionOutcome::CheckpointRestored { .. }
        ));
        assert!(scheduler.pending_event_snapshot(event_id).is_none());
        assert_eq!(scheduler.pending_event_snapshot(control_id), Some(control));
        assert_eq!(scheduler.pending_event_snapshot(foreign_id), Some(foreign));
    }

    #[test]
    fn borrowed_scheduler_restore_accepts_empty_legacy_component() {
        let component = scheduler_component("scheduler0");
        let manifest = CheckpointManifest::new(
            "legacy-empty",
            0,
            vec![CheckpointState::new(component.clone(), Vec::new())],
        );
        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        let event_id = scheduler
            .schedule_at(PartitionId::new(0), 5, |_| {})
            .unwrap();
        let event = scheduler.pending_event_snapshot(event_id).unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        executor.scheduler_checkpoint_control_events.push(
            SchedulerCheckpointOwnedEvent::discard_on_restore(scheduler.instance_id(), event),
        );
        let restore = HostActionRecord::new(
            1,
            PartitionId::new(0),
            PartitionId::new(0),
            GuestEventId::new(2),
            GuestSourceId::new(1),
            HostAction::RestoreCheckpoint { manifest },
        );

        executor
            .apply_with_scheduler_checkpoint(
                &restore,
                component.clone(),
                scheduler.checkpoint_access(),
            )
            .unwrap();

        assert!(scheduler.pending_event_snapshot(event_id).is_none());
        assert!(!executor.checkpoints().contains_component(&component));
    }

    #[test]
    fn direct_apply_prunes_stale_control_claim_before_event_identity_reuse() {
        let component = scheduler_component("scheduler0");
        let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component,
            Arc::clone(&scheduler),
        )])
        .unwrap();
        let mut executor =
            SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
        executor.attach_scheduler_checkpoint_bank(bank).unwrap();
        let baseline = scheduler.lock().unwrap().quiescent_snapshot().unwrap();
        let control_id = scheduler
            .lock()
            .unwrap()
            .schedule_at(PartitionId::new(0), 5, |_| {})
            .unwrap();
        let control = scheduler
            .lock()
            .unwrap()
            .pending_event_snapshot(control_id)
            .unwrap();
        executor.register_scheduler_checkpoint_control_event(
            scheduler.lock().unwrap().instance_id(),
            control,
        );
        scheduler.lock().unwrap().cancel_event(control_id).unwrap();
        scheduler
            .lock()
            .unwrap()
            .restore_quiescent(&baseline)
            .unwrap();
        let foreign_id = scheduler
            .lock()
            .unwrap()
            .schedule_at(PartitionId::new(0), 5, |_| {})
            .unwrap();
        let foreign = scheduler
            .lock()
            .unwrap()
            .pending_event_snapshot(foreign_id)
            .unwrap();
        assert_eq!(foreign.id(), control.id());
        assert_eq!(foreign.tick(), control.tick());
        assert_eq!(foreign.order(), control.order());
        assert_eq!(foreign.kind(), control.kind());
        assert_ne!(foreign, control);

        let error = executor.apply(&checkpoint_record("foreign")).unwrap_err();

        let SystemError::SchedulerCheckpoint(SchedulerCheckpointError::NonQuiescent { report }) =
            error
        else {
            panic!("unexpected error: {error:?}");
        };
        assert_eq!(report.pending_event_count(), 1);
    }
}

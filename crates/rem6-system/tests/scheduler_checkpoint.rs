use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_kernel::{
    LivelockTransitionKind, PartitionEventId, PartitionId, PartitionedScheduler,
    ScheduledEventKind, SchedulerError, WaitForNode,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, SchedulerCheckpointBank,
    SchedulerCheckpointError, SchedulerCheckpointPort, SystemActionExecutor, SystemActionOutcome,
    SystemError,
};

const SCHEDULER_CHUNK: &str = "scheduler";

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

#[test]
fn host_checkpoint_refreshes_and_restores_scheduler_state() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(22);
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let component = CheckpointComponentId::new("scheduler0").unwrap();
    let mut live_scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 5, 1).unwrap();
    let first_id = live_scheduler
        .schedule_parallel_at(core, 3, move |context| {
            context.schedule_local_after(1, |_| {}).unwrap();
        })
        .unwrap();
    assert_eq!(first_id, PartitionEventId::new(core, 0));
    assert_eq!(
        live_scheduler
            .run_until_idle_parallel()
            .unwrap()
            .final_tick(),
        5
    );
    let scheduler = Arc::new(Mutex::new(live_scheduler));
    let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
        component.clone(),
        Arc::clone(&scheduler),
    )])
    .unwrap();
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_scheduler_checkpoint_bank(bank).unwrap();

    let checkpoint = HostActionRecord::new(
        140,
        host,
        host,
        GuestEventId::new(40),
        source,
        HostAction::Checkpoint {
            label: "scheduler-ready".to_string(),
        },
    );

    let checkpoint_outcome = executor.apply(&checkpoint).unwrap();
    let manifest = match checkpoint_outcome {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    assert!(
        executor
            .checkpoints()
            .chunk(&component, "scheduler")
            .unwrap()
            .len()
            > 64
    );

    {
        let mut scheduler = scheduler.lock().unwrap();
        scheduler.schedule_parallel_at(memory, 9, |_| {}).unwrap();
        assert_eq!(
            scheduler.run_until_idle_parallel().unwrap().final_tick(),
            10
        );
        assert_eq!(scheduler.now(), 10);
    }

    let restore = HostActionRecord::new(
        144,
        host,
        host,
        GuestEventId::new(41),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    );

    let restore_outcome = executor.apply(&restore).unwrap();

    assert_eq!(
        restore_outcome,
        SystemActionOutcome::CheckpointRestored {
            tick: 144,
            event: GuestEventId::new(41),
            source,
            manifest,
        }
    );
    let mut scheduler = scheduler.lock().unwrap();
    assert_eq!(scheduler.now(), 5);
    assert_eq!(scheduler.max_parallel_workers(), 1);
    assert_eq!(scheduler.partition_now(core).unwrap(), 5);
    assert_eq!(scheduler.partition_now(memory).unwrap(), 5);
    let restored_id = scheduler.schedule_parallel_at(core, 6, |_| {}).unwrap();
    assert_eq!(restored_id, PartitionEventId::new(core, 2));
}

#[test]
fn host_checkpoint_preserves_scheduler_remote_send_order_identity() {
    let source = PartitionId::new(0);
    let target = PartitionId::new(1);
    let component = CheckpointComponentId::new("scheduler0").unwrap();
    let scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 4, 2).unwrap(),
    ));
    scheduler
        .lock()
        .unwrap()
        .schedule_parallel_at(source, 0, move |context| {
            context.schedule_remote_after(target, 4, |_| {}).unwrap();
        })
        .unwrap();
    scheduler
        .lock()
        .unwrap()
        .run_until_idle_parallel_recorded()
        .unwrap();

    let port = SchedulerCheckpointPort::new(component.clone(), Arc::clone(&scheduler));
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    port.capture_into(&mut registry).unwrap();

    let restored_scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 4, 2).unwrap(),
    ));
    SchedulerCheckpointPort::new(component, Arc::clone(&restored_scheduler))
        .restore_from(&registry)
        .unwrap();

    let mut restored = restored_scheduler.lock().unwrap();
    let restored_tick = restored.now();
    restored
        .schedule_parallel_at(source, restored_tick, move |context| {
            context.schedule_remote_after(target, 4, |_| {}).unwrap();
        })
        .unwrap();
    let run = restored.run_until_idle_parallel_recorded().unwrap();

    assert_eq!(run.remote_sends().len(), 1);
    assert_eq!(run.remote_sends()[0].source(), source);
    assert_eq!(run.remote_sends()[0].target(), target);
    assert_eq!(run.remote_sends()[0].order(), 1);
}

#[test]
fn host_checkpoint_preserves_scheduler_progress_transition_order_identity() {
    let source = PartitionId::new(0);
    let component = CheckpointComponentId::new("scheduler0").unwrap();
    let first_subject = WaitForNode::component("core0-retry").unwrap();
    let second_subject = WaitForNode::component("core0-arbitration").unwrap();
    let scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(1, 4, 1).unwrap(),
    ));
    scheduler
        .lock()
        .unwrap()
        .schedule_parallel_at(source, 0, move |context| {
            context
                .record_progress_transition(first_subject, LivelockTransitionKind::ProtocolRetry);
        })
        .unwrap();
    scheduler
        .lock()
        .unwrap()
        .run_until_idle_parallel_recorded()
        .unwrap();

    let port = SchedulerCheckpointPort::new(component.clone(), Arc::clone(&scheduler));
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    port.capture_into(&mut registry).unwrap();

    let restored_scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(1, 4, 1).unwrap(),
    ));
    SchedulerCheckpointPort::new(component, Arc::clone(&restored_scheduler))
        .restore_from(&registry)
        .unwrap();

    let mut restored = restored_scheduler.lock().unwrap();
    let restored_tick = restored.now();
    restored
        .schedule_parallel_at(source, restored_tick, move |context| {
            context.record_progress_transition(
                second_subject,
                LivelockTransitionKind::ResourceArbitration,
            );
        })
        .unwrap();
    let run = restored.run_until_idle_parallel_recorded().unwrap();

    assert_eq!(run.progress_transitions().len(), 1);
    assert_eq!(run.progress_transitions()[0].partition(), source);
    assert_eq!(run.progress_transitions()[0].order(), 1);
}

#[test]
fn host_checkpoint_rejects_scheduler_bank_truncated_payload_without_partial_restore() {
    let component0 = CheckpointComponentId::new("scheduler0").unwrap();
    let component1 = CheckpointComponentId::new("scheduler1").unwrap();
    let scheduler0 = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap(),
    ));
    let scheduler1 = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap(),
    ));
    let bank = SchedulerCheckpointBank::new([
        SchedulerCheckpointPort::new(component0.clone(), Arc::clone(&scheduler0)),
        SchedulerCheckpointPort::new(component1.clone(), Arc::clone(&scheduler1)),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();

    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();
    registry
        .write_chunk(&component1, "scheduler", vec![1, 0, 0])
        .unwrap();
    {
        let mut scheduler = scheduler0.lock().unwrap();
        scheduler
            .schedule_parallel_at(PartitionId::new(0), 7, |_| {})
            .unwrap();
        scheduler.run_until_idle_parallel().unwrap();
    }
    {
        let mut scheduler = scheduler1.lock().unwrap();
        scheduler
            .schedule_parallel_at(PartitionId::new(1), 9, |_| {})
            .unwrap();
        scheduler.run_until_idle_parallel().unwrap();
    }
    let before0 = scheduler0.lock().unwrap().snapshot();
    let before1 = scheduler1.lock().unwrap().snapshot();

    let error = bank.restore_all_from(&registry).unwrap_err();

    assert_eq!(error.component(), Some(&component1));
    assert_eq!(scheduler0.lock().unwrap().snapshot(), before0);
    assert_eq!(scheduler1.lock().unwrap().snapshot(), before1);
}

#[test]
fn scheduler_checkpoint_port_rejects_impossible_partition_count_without_partial_restore() {
    let component = CheckpointComponentId::new("scheduler_partition_count").unwrap();
    let scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap(),
    ));
    let before = scheduler.lock().unwrap().snapshot();
    let mut payload = Vec::new();
    write_u64(&mut payload, 4);
    write_u64(&mut payload, 0);
    write_u64(&mut payload, 4);
    write_u64(&mut payload, 1);
    write_u64(&mut payload, u64::MAX);
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    registry
        .write_chunk(&component, SCHEDULER_CHUNK, payload)
        .unwrap();

    let error = SchedulerCheckpointPort::new(component.clone(), Arc::clone(&scheduler))
        .restore_from(&registry)
        .unwrap_err();

    assert_eq!(
        error,
        SchedulerCheckpointError::InvalidChunk {
            component,
            reason:
                "scheduler partition count 18446744073709551615 exceeds remaining payload capacity 0 records"
                    .to_string(),
        }
    );
    assert_eq!(scheduler.lock().unwrap().snapshot(), before);
}

#[test]
fn host_checkpoint_rejects_scheduler_worker_limit_mismatch_on_restore() {
    let source_component = CheckpointComponentId::new("scheduler0").unwrap();
    let source_scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 5, 1).unwrap(),
    ));
    let source_port =
        SchedulerCheckpointPort::new(source_component.clone(), Arc::clone(&source_scheduler));
    let mut registry = CheckpointRegistry::new();
    registry.register(source_component.clone()).unwrap();
    let captured = source_port.capture_into(&mut registry).unwrap();
    assert_eq!(captured.snapshot().max_parallel_workers(), 1);

    let target_scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 5, 2).unwrap(),
    ));
    let target_port =
        SchedulerCheckpointPort::new(source_component.clone(), Arc::clone(&target_scheduler));

    assert_eq!(
        target_port.restore_from(&registry).unwrap_err(),
        SchedulerCheckpointError::Scheduler {
            component: source_component,
            error: SchedulerError::SnapshotParallelWorkerLimitMismatch {
                snapshot_max_parallel_workers: 1,
                scheduler_max_parallel_workers: 2,
            },
        }
    );
}

#[test]
fn host_checkpoint_rejects_scheduler_global_tick_before_partition_clock_without_partial_restore() {
    let component = CheckpointComponentId::new("scheduler0").unwrap();
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let source_scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 5, 1).unwrap(),
    ));
    {
        let mut scheduler = source_scheduler.lock().unwrap();
        scheduler.schedule_at(memory, 8, |_| {}).unwrap();
        scheduler.run_until_idle();
    }
    let source_port =
        SchedulerCheckpointPort::new(component.clone(), Arc::clone(&source_scheduler));
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    source_port.capture_into(&mut registry).unwrap();
    let mut payload = registry.chunk(&component, "scheduler").unwrap().to_vec();
    payload[8..16].copy_from_slice(&4u64.to_le_bytes());
    registry
        .write_chunk(&component, "scheduler", payload)
        .unwrap();

    let target_scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 5, 1).unwrap(),
    ));
    {
        let mut scheduler = target_scheduler.lock().unwrap();
        scheduler
            .schedule_parallel_at(PartitionId::new(0), 12, |_| {})
            .unwrap();
        scheduler.run_until_idle_parallel().unwrap();
    }
    let before = target_scheduler.lock().unwrap().snapshot();
    let target_port =
        SchedulerCheckpointPort::new(component.clone(), Arc::clone(&target_scheduler));

    assert_eq!(
        target_port.restore_from(&registry).unwrap_err(),
        SchedulerCheckpointError::Scheduler {
            component,
            error: SchedulerError::SnapshotGlobalTickBeforePartitionClock {
                snapshot_now: 4,
                partition: core,
                partition_now: 8,
            },
        }
    );
    assert_eq!(target_scheduler.lock().unwrap().snapshot(), before);
}

#[test]
fn host_checkpoint_rejects_scheduler_partition_clock_before_global_tick_without_partial_restore() {
    let component = CheckpointComponentId::new("scheduler0").unwrap();
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let source_scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 5, 1).unwrap(),
    ));
    {
        let mut scheduler = source_scheduler.lock().unwrap();
        scheduler.schedule_at(core, 12, |_| {}).unwrap();
        scheduler.run_until_idle();
    }
    let source_port =
        SchedulerCheckpointPort::new(component.clone(), Arc::clone(&source_scheduler));
    let mut registry = CheckpointRegistry::new();
    registry.register(component.clone()).unwrap();
    source_port.capture_into(&mut registry).unwrap();
    let mut payload = registry.chunk(&component, "scheduler").unwrap().to_vec();
    let scheduler_header_bytes = 8 + 8 + 8 + 8 + 8;
    let partition_record_bytes = 4 + 8 + 8 + 8 + 8 + 8 + 8;
    let second_partition_now_offset = scheduler_header_bytes + partition_record_bytes + 4;
    payload[second_partition_now_offset..second_partition_now_offset + 8]
        .copy_from_slice(&8u64.to_le_bytes());
    registry
        .write_chunk(&component, "scheduler", payload)
        .unwrap();

    let target_scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 5, 1).unwrap(),
    ));
    {
        let mut scheduler = target_scheduler.lock().unwrap();
        scheduler.schedule_parallel_at(memory, 20, |_| {}).unwrap();
        scheduler.run_until_idle_parallel().unwrap();
    }
    let before = target_scheduler.lock().unwrap().snapshot();
    let target_port =
        SchedulerCheckpointPort::new(component.clone(), Arc::clone(&target_scheduler));

    assert_eq!(
        target_port.restore_from(&registry).unwrap_err(),
        SchedulerCheckpointError::Scheduler {
            component,
            error: SchedulerError::SnapshotPartitionClockBeforeGlobalTick {
                snapshot_now: 12,
                partition: memory,
                partition_now: 8,
            },
        }
    );
    assert_eq!(target_scheduler.lock().unwrap().snapshot(), before);
}

#[test]
fn host_checkpoint_rejects_scheduler_with_pending_events() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(23);
    let core = PartitionId::new(0);
    let component = CheckpointComponentId::new("scheduler0").unwrap();
    let scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_min_remote_delay(2, 5).unwrap(),
    ));
    scheduler
        .lock()
        .unwrap()
        .schedule_parallel_at(core, 3, |_| {})
        .unwrap();
    let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
        component.clone(),
        Arc::clone(&scheduler),
    )])
    .unwrap();
    let mut executor =
        SystemActionExecutor::with_checkpoint(StatsRegistry::new(), CheckpointRegistry::new());
    executor.attach_scheduler_checkpoint_bank(bank).unwrap();
    let checkpoint = HostActionRecord::new(
        148,
        host,
        host,
        GuestEventId::new(42),
        source,
        HostAction::Checkpoint {
            label: "scheduler-busy".to_string(),
        },
    );

    let error = executor.apply(&checkpoint).unwrap_err();

    let SystemError::SchedulerCheckpoint(SchedulerCheckpointError::NonQuiescent { report }) = error
    else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(report.component(), &component);
    assert_eq!(report.pending_event_count(), 1);
    assert_eq!(report.pending_partition_count(), 1);
    assert_eq!(report.first_pending_tick(), Some(3));
    assert_eq!(report.last_pending_tick(), Some(3));
    assert_eq!(report.parallel_pending_event_count(), 1);
    assert_eq!(report.serial_pending_event_count(), 0);
}

#[test]
fn host_checkpoint_reports_scheduler_quiescence_blockers() {
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let io = PartitionId::new(2);
    let component = CheckpointComponentId::new("scheduler0").unwrap();
    let scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(3, 5, 2).unwrap(),
    ));
    scheduler
        .lock()
        .unwrap()
        .schedule_parallel_at(core, 3, move |context| {
            context.schedule_remote_after(memory, 5, |_| {}).unwrap();
        })
        .unwrap();
    scheduler
        .lock()
        .unwrap()
        .schedule_at(io, 9, |_| {})
        .unwrap();
    let port = SchedulerCheckpointPort::new(component.clone(), Arc::clone(&scheduler));

    let report = port.quiescence_report();

    assert_eq!(report.component(), &component);
    assert!(!report.is_quiescent());
    assert_eq!(report.partition_count(), 3);
    assert_eq!(report.pending_event_count(), 2);
    assert_eq!(report.pending_partition_count(), 2);
    assert_eq!(report.first_pending_tick(), Some(3));
    assert_eq!(report.last_pending_tick(), Some(9));
    assert_eq!(report.parallel_pending_event_count(), 1);
    assert_eq!(report.serial_pending_event_count(), 1);
    assert_eq!(report.pending_partitions().len(), 2);

    let core_report = report.pending_partition(core).unwrap();
    assert_eq!(core_report.partition(), core);
    assert_eq!(core_report.pending_event_count(), 1);
    assert_eq!(core_report.first_pending_tick(), Some(3));
    assert_eq!(core_report.last_pending_tick(), Some(3));
    assert_eq!(
        core_report.event_count_by_kind(ScheduledEventKind::Parallel),
        1
    );
    assert_eq!(
        core_report.event_count_by_kind(ScheduledEventKind::Serial),
        0
    );

    let io_report = report.pending_partition(io).unwrap();
    assert_eq!(io_report.partition(), io);
    assert_eq!(io_report.pending_event_count(), 1);
    assert_eq!(io_report.first_pending_tick(), Some(9));
    assert_eq!(io_report.last_pending_tick(), Some(9));
    assert_eq!(io_report.event_count_by_kind(ScheduledEventKind::Serial), 1);
    assert_eq!(
        io_report.event_count_by_kind(ScheduledEventKind::Parallel),
        0
    );
    assert!(report.pending_partition(memory).is_none());
}

#[test]
fn host_checkpoint_bank_reports_quiescence_by_component() {
    let component0 = CheckpointComponentId::new("scheduler0").unwrap();
    let component1 = CheckpointComponentId::new("scheduler1").unwrap();
    let scheduler0 = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap(),
    ));
    let scheduler1 = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap(),
    ));
    scheduler1
        .lock()
        .unwrap()
        .schedule_parallel_at(PartitionId::new(1), 11, |_| {})
        .unwrap();
    let bank = SchedulerCheckpointBank::new([
        SchedulerCheckpointPort::new(component0.clone(), Arc::clone(&scheduler0)),
        SchedulerCheckpointPort::new(component1.clone(), Arc::clone(&scheduler1)),
    ])
    .unwrap();

    let reports = bank.quiescence_reports();

    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].component(), &component0);
    assert!(reports[0].is_quiescent());
    assert_eq!(reports[0].pending_event_count(), 0);
    assert_eq!(reports[0].first_pending_tick(), None);
    assert_eq!(reports[0].last_pending_tick(), None);
    assert_eq!(reports[1].component(), &component1);
    assert!(!reports[1].is_quiescent());
    assert_eq!(reports[1].pending_event_count(), 1);
    assert_eq!(reports[1].first_pending_tick(), Some(11));
    assert_eq!(reports[1].last_pending_tick(), Some(11));
}

#[test]
fn host_checkpoint_bank_rejects_non_quiescent_scheduler_before_chunk_writes() {
    let component0 = CheckpointComponentId::new("scheduler0").unwrap();
    let component1 = CheckpointComponentId::new("scheduler1").unwrap();
    let scheduler0 = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap(),
    ));
    let scheduler1 = Arc::new(Mutex::new(
        PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap(),
    ));
    scheduler1
        .lock()
        .unwrap()
        .schedule_parallel_at(PartitionId::new(1), 11, |_| {})
        .unwrap();
    let bank = SchedulerCheckpointBank::new([
        SchedulerCheckpointPort::new(component0.clone(), Arc::clone(&scheduler0)),
        SchedulerCheckpointPort::new(component1.clone(), Arc::clone(&scheduler1)),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();

    let error = bank.capture_all_into(&mut registry).unwrap_err();

    let SchedulerCheckpointError::NonQuiescent { report } = error else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(report.component(), &component1);
    assert_eq!(report.pending_event_count(), 1);
    assert_eq!(report.first_pending_tick(), Some(11));
    assert_eq!(registry.chunk(&component0, "scheduler"), None);
    assert_eq!(registry.chunk(&component1, "scheduler"), None);
}

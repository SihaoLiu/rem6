use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_kernel::{PartitionEventId, PartitionId, PartitionedScheduler, SchedulerError};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, SchedulerCheckpointBank,
    SchedulerCheckpointError, SchedulerCheckpointPort, SystemActionExecutor, SystemActionOutcome,
    SystemError,
};

#[test]
fn host_checkpoint_refreshes_and_restores_scheduler_state() {
    let host = PartitionId::new(1);
    let source = GuestSourceId::new(22);
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let component = CheckpointComponentId::new("scheduler0").unwrap();
    let mut live_scheduler = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();
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
    assert_eq!(scheduler.partition_now(core).unwrap(), 5);
    assert_eq!(scheduler.partition_now(memory).unwrap(), 5);
    let restored_id = scheduler.schedule_parallel_at(core, 6, |_| {}).unwrap();
    assert_eq!(restored_id, PartitionEventId::new(core, 2));
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

    assert_eq!(
        executor.apply(&checkpoint).unwrap_err(),
        SystemError::SchedulerCheckpoint(SchedulerCheckpointError::Scheduler {
            component,
            error: SchedulerError::SnapshotContainsPendingEvents { pending_events: 1 },
        })
    );
}

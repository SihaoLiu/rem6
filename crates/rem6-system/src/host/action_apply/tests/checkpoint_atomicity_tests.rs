use super::*;

#[test]
fn live_o3_stats_restore_failure_preflights_before_cpu_and_scheduler_install() {
    let (scheduler, seeded, mut executor, manifest) = live_o3_action_fixture();
    let cpu = seeded.core.id();
    let o3_stats =
        RiscvO3RuntimeStats::register_for_cpus(executor.stats_mut(), [cpu], false).unwrap();
    executor.attach_riscv_o3_runtime_stats(o3_stats);
    executor.stats = StatsRegistry::new();
    seeded
        .core
        .write_register(Register::new(7).unwrap(), 0xcafe);
    let scheduler_before = scheduler.lock().unwrap().snapshot();
    let restore = HostActionRecord::new(
        live_o3_support::LIVE_TICK + 1,
        PartitionId::new(0),
        PartitionId::new(0),
        GuestEventId::new(2),
        GuestSourceId::new(1),
        HostAction::RestoreCheckpoint { manifest },
    );

    assert!(matches!(
        executor.apply(&restore),
        Err(SystemError::Stats(
            rem6_stats::StatsError::UnknownStat { .. }
        ))
    ));
    assert_eq!(
        seeded.core.read_register(Register::new(7).unwrap()),
        0xcafe,
        "stats preflight failure must preserve CPU state"
    );
    assert_eq!(
        scheduler.lock().unwrap().snapshot(),
        scheduler_before,
        "stats preflight failure must preserve scheduler state"
    );
}

#[test]
fn borrowed_scheduler_capture_failure_does_not_commit_component_tracking() {
    let scheduler_component = scheduler_component("borrowed-scheduler");
    let memory_component = CheckpointComponentId::new("late-memory").unwrap();
    let memory = Arc::new(Mutex::new(PartitionedMemoryStore::new()));
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
    executor
        .attach_memory_checkpoint_bank(
            MemoryStoreCheckpointBank::new([MemoryStoreCheckpointPort::new(
                memory_component.clone(),
                memory,
            )])
            .unwrap(),
        )
        .unwrap();
    executor.checkpoints.remove_component(&memory_component);

    let result = executor.apply_with_scheduler_checkpoint(
        &checkpoint_record("late-capture-failure"),
        scheduler_component.clone(),
        scheduler.checkpoint_access(),
    );

    assert!(matches!(
        result,
        Err(SystemError::Checkpoint(
            rem6_checkpoint::CheckpointError::UnknownComponent { .. }
        ))
    ));
    assert!(!executor
        .borrowed_scheduler_checkpoint_components
        .contains(&scheduler_component));
}

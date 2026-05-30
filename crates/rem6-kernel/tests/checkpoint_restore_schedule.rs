use rem6_kernel::{
    CheckpointRestoreEventPlan, CheckpointRestoreScheduleError, PartitionId,
    RestoreReplayEventKind, ScheduledEventKind,
};

#[test]
fn checkpoint_restore_plan_rejects_live_events_before_restored_tick_without_mutation() {
    let core = PartitionId::new(0);
    let mut plan = CheckpointRestoreEventPlan::new(120);

    assert_eq!(
        plan.stage_live_event(core, 0, ScheduledEventKind::Serial)
            .unwrap_err(),
        CheckpointRestoreScheduleError::LiveEventBeforeRestoredTick {
            partition: core,
            restored_tick: 120,
            requested_tick: 0,
        }
    );
    assert_eq!(plan.restored_tick(), 120);
    assert!(plan.live_events().is_empty());
    assert!(plan.warmup_events().is_empty());
}

#[test]
fn checkpoint_restore_plan_keeps_warmup_clock_isolated_from_live_scheduler_ticks() {
    let mut plan = CheckpointRestoreEventPlan::new(120);

    let event = plan
        .record_warmup_event("ruby", 0, 0, RestoreReplayEventKind::GlobalExit)
        .unwrap();
    assert_eq!(event.source(), "ruby");
    assert_eq!(event.replay_now(), 0);
    assert_eq!(event.scheduled_tick(), 0);
    assert_eq!(event.kind(), RestoreReplayEventKind::GlobalExit);
    assert!(plan.live_events().is_empty());

    assert_eq!(
        plan.record_warmup_event("ruby", 8, 6, RestoreReplayEventKind::SubsystemWake)
            .unwrap_err(),
        CheckpointRestoreScheduleError::WarmupEventBeforeReplayClock {
            source: "ruby".to_string(),
            replay_now: 8,
            requested_tick: 6,
        }
    );
    assert_eq!(
        plan.record_warmup_event("ruby", 80, 121, RestoreReplayEventKind::SubsystemWake)
            .unwrap_err(),
        CheckpointRestoreScheduleError::WarmupEventAfterRestoredTick {
            source: "ruby".to_string(),
            restored_tick: 120,
            requested_tick: 121,
        }
    );

    let summary = plan.finish_warmup(80).unwrap();
    assert_eq!(summary.restored_tick(), 120);
    assert_eq!(summary.warmup_final_tick(), 80);
    assert_eq!(summary.warmup_slack_ticks(), 40);
    assert_eq!(summary.warmup_event_count(), 1);
    assert_eq!(summary.live_event_count(), 0);
}

#[test]
fn checkpoint_restore_plan_records_live_events_at_or_after_restored_tick_in_order() {
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let mut plan = CheckpointRestoreEventPlan::new(120);

    let first = plan
        .stage_live_event(memory, 125, ScheduledEventKind::Parallel)
        .unwrap();
    let second = plan
        .stage_live_event(core, 120, ScheduledEventKind::Serial)
        .unwrap();

    assert_eq!(first.restore_order(), 0);
    assert_eq!(second.restore_order(), 1);
    assert_eq!(plan.live_events()[0].partition(), memory);
    assert_eq!(plan.live_events()[0].scheduled_tick(), 125);
    assert_eq!(plan.live_events()[0].kind(), ScheduledEventKind::Parallel);
    assert_eq!(plan.live_events()[1].partition(), core);
    assert_eq!(plan.live_events()[1].scheduled_tick(), 120);
    assert_eq!(plan.live_events()[1].kind(), ScheduledEventKind::Serial);
}

#[test]
fn checkpoint_restore_plan_exports_live_events_in_scheduler_order() {
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let io = PartitionId::new(2);
    let mut plan = CheckpointRestoreEventPlan::new(120);

    let memory_event = plan
        .stage_live_event(memory, 125, ScheduledEventKind::Parallel)
        .unwrap();
    let io_event = plan
        .stage_live_event(io, 120, ScheduledEventKind::Parallel)
        .unwrap();
    let core_event = plan
        .stage_live_event(core, 120, ScheduledEventKind::Serial)
        .unwrap();
    let memory_followup = plan
        .stage_live_event(memory, 125, ScheduledEventKind::Serial)
        .unwrap();

    assert_eq!(
        plan.live_events()
            .iter()
            .map(|event| event.restore_order())
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );

    let scheduler_order = plan.live_events_for_scheduler();
    assert_eq!(
        scheduler_order,
        vec![core_event, io_event, memory_event, memory_followup]
    );
}

#[test]
fn checkpoint_restore_plan_exports_warmup_events_in_replay_order() {
    let mut plan = CheckpointRestoreEventPlan::new(120);

    let late_wake = plan
        .record_warmup_event("ruby-cache", 30, 35, RestoreReplayEventKind::SubsystemWake)
        .unwrap()
        .clone();
    let initial_exit = plan
        .record_warmup_event("ruby", 0, 0, RestoreReplayEventKind::GlobalExit)
        .unwrap()
        .clone();
    let same_clock_wake = plan
        .record_warmup_event("ruby-dir", 30, 30, RestoreReplayEventKind::SubsystemWake)
        .unwrap()
        .clone();

    assert_eq!(
        plan.warmup_events()
            .iter()
            .map(|event| event.restore_order())
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );

    assert_eq!(
        plan.warmup_events_for_replay(),
        vec![initial_exit, same_clock_wake, late_wake]
    );
}

#[test]
fn checkpoint_restore_plan_validates_warmup_source_and_finish_boundary() {
    let mut plan = CheckpointRestoreEventPlan::new(120);

    assert_eq!(
        plan.record_warmup_event("", 0, 0, RestoreReplayEventKind::SubsystemWake)
            .unwrap_err(),
        CheckpointRestoreScheduleError::EmptyWarmupEventSource
    );
    assert_eq!(
        plan.finish_warmup(121).unwrap_err(),
        CheckpointRestoreScheduleError::WarmupFinishedAfterRestoredTick {
            restored_tick: 120,
            final_tick: 121,
        }
    );
    assert!(plan.warmup_events().is_empty());
    assert!(plan.live_events().is_empty());
}

#[test]
fn checkpoint_restore_plan_seals_warmup_before_live_handoff() {
    let mut plan = CheckpointRestoreEventPlan::new(120);
    let core = PartitionId::new(0);
    plan.record_warmup_event("ruby", 0, 0, RestoreReplayEventKind::GlobalExit)
        .unwrap();

    let summary = plan.finish_warmup(80).unwrap();

    assert_eq!(summary.warmup_final_tick(), 80);
    assert_eq!(plan.warmup_final_tick(), Some(80));
    assert_eq!(plan.warmup_event_count(), 1);
    assert_eq!(
        plan.record_warmup_event("ruby-late", 80, 80, RestoreReplayEventKind::SubsystemWake)
            .unwrap_err(),
        CheckpointRestoreScheduleError::WarmupAlreadyFinished {
            final_tick: 80,
            source: "ruby-late".to_string(),
            requested_tick: 80,
        },
    );
    assert_eq!(plan.warmup_event_count(), 1);
    let live = plan
        .stage_live_event(core, 120, ScheduledEventKind::Parallel)
        .unwrap();
    assert_eq!(live.partition(), core);
    assert_eq!(plan.live_event_count(), 1);
    assert_eq!(
        plan.finish_warmup(80).unwrap_err(),
        CheckpointRestoreScheduleError::WarmupAlreadyFinished {
            final_tick: 80,
            source: "finish".to_string(),
            requested_tick: 80,
        },
    );
}

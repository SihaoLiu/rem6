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

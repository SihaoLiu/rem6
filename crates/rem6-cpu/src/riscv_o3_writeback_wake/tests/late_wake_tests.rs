use super::*;

#[test]
fn restored_live_retire_gate_polled_late_requests_current_tick() {
    let core = core();
    let request = memory_request(31);
    core.state
        .lock()
        .expect("riscv core lock")
        .live_retire_gate
        .restore_checkpoint(Some(O3LiveRetireGateCheckpointPayload::new(request, 31)));

    assert_eq!(core.requested_o3_writeback_wake_tick(32), Some(32));
}

#[test]
fn unpublished_memory_result_polled_late_requests_current_tick() {
    let core = core_with_completed_scalar_loads();

    assert_eq!(core.requested_o3_writeback_wake_tick(21), Some(21));
}

#[test]
fn detailed_policy_disable_preserves_iq_only_scheduled_wake_until_it_fires() {
    let core = core_with_live_issue_request(24);
    core.set_detailed_live_retire_gate_enabled(true);
    assert_eq!(core.requested_o3_writeback_wake_tick(10), Some(24));
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let fired = core.clone();
    let event = scheduler
        .schedule_at(PartitionId::new(0), 24, move |context| {
            fired.mark_o3_writeback_wake_fired(context.now());
        })
        .unwrap();
    core.mark_o3_writeback_wake_scheduled(
        scheduler.instance_id(),
        scheduler.pending_event_snapshot(event).unwrap(),
    );
    assert_eq!(scheduler.snapshot().total_pending_events(), 1);

    core.set_detailed_live_retire_gate_enabled(false);

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.o3_runtime.has_live_retirement_authority());
    assert_eq!(state.o3_writeback_wake.owned_wakes().len(), 1);
    assert!(state.o3_writeback_wake.has_pending_checkpoint_authority());
    drop(state);
    scheduler.run_until_idle();
    assert_eq!(scheduler.snapshot().total_pending_events(), 0);
    assert!(!core
        .state
        .lock()
        .unwrap()
        .o3_writeback_wake
        .has_scheduled_wake_authority());
}

#[test]
fn detailed_policy_disable_rebases_remaining_memory_result_demand() {
    let core = core_with_completed_scalar_loads();
    core.set_detailed_live_retire_gate_enabled(true);
    assert_eq!(core.requested_o3_writeback_wake_tick(10), Some(20));
    let (scheduler, event) = wake(20);
    core.mark_o3_writeback_wake_scheduled(scheduler, event);
    core.set_detailed_live_retire_gate_enabled(false);
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state
            .o3_writeback_wake
            .owned_wakes()
            .into_iter()
            .map(RiscvO3WritebackWake::tick)
            .collect::<Vec<_>>(),
        vec![20],
    );
    assert_eq!(state.o3_writeback_wake.desired_tick, Some(20));
    assert!(state.o3_writeback_wake.has_pending_checkpoint_authority());
    drop(state);
    assert_eq!(core.requested_o3_writeback_wake_tick(10), None);
    core.set_detailed_live_retire_gate_enabled(false);
    assert_eq!(core.owned_o3_writeback_wakes().len(), 1);
    assert_eq!(core.owned_o3_writeback_wakes()[0].1.tick(), 20);
    assert_eq!(core.requested_o3_writeback_wake_tick(10), None);
}

#[test]
fn detailed_policy_disable_keeps_earlier_wake_until_later_memory_demand_is_rescheduled() {
    let core = core_with_completed_scalar_loads();
    assert!(
        core.state
            .lock()
            .unwrap()
            .o3_runtime
            .enqueue_live_issue_for_test(
                99,
                Address::new(0x8200),
                O3LiveIssueTraceClass::Control,
                15,
            )
    );
    core.set_detailed_live_retire_gate_enabled(true);
    assert_eq!(core.requested_o3_writeback_wake_tick(10), Some(15));
    let (scheduler, event) = wake(15);
    core.mark_o3_writeback_wake_scheduled(scheduler, event);

    core.set_detailed_live_retire_gate_enabled(false);

    assert_eq!(core.owned_o3_writeback_wakes().len(), 1);
    assert_eq!(core.owned_o3_writeback_wakes()[0].1.tick(), 15);
    assert_eq!(core.requested_o3_writeback_wake_tick(10), None);
    core.mark_o3_writeback_wake_fired(15);
    assert!(core.owned_o3_writeback_wakes().is_empty());
    assert_eq!(core.requested_o3_writeback_wake_tick(15), Some(20));
    let (scheduler, event) = wake(20);
    core.mark_o3_writeback_wake_scheduled(scheduler, event);
    assert_eq!(core.owned_o3_writeback_wakes().len(), 1);
    assert_eq!(core.owned_o3_writeback_wakes()[0].1.tick(), 20);
}

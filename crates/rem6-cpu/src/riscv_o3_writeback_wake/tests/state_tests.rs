use super::*;

#[test]
fn o3_writeback_wake_identical_request_deduplicates() {
    let mut state = RiscvO3WritebackWakeState::default();
    state.set_desired_tick(Some(20), 10);
    let (scheduler, event) = wake(20);
    state.mark_scheduled(scheduler, event);

    state.set_desired_tick(Some(20), 11);

    assert_eq!(state.requested_tick(11), None);
    assert_eq!(state.owned_wakes().len(), 1);
}

#[test]
fn o3_writeback_wake_earlier_request_detaches_later_schedule() {
    let mut state = RiscvO3WritebackWakeState::default();
    state.set_desired_tick(Some(20), 10);
    let (scheduler, event) = wake(20);
    state.mark_scheduled(scheduler, event);

    state.set_desired_tick(Some(15), 11);

    assert_eq!(state.requested_tick(11), Some(15));
    assert_eq!(state.owned_wakes().len(), 1);
}

#[test]
fn current_due_wake_blocks_fetch_while_stale_detached_wake_remains() {
    let mut state = RiscvO3WritebackWakeState::default();
    state.set_desired_tick(Some(20), 10);
    let (scheduler, event) = wake(20);
    state.mark_scheduled(scheduler, event);
    state.set_desired_tick(Some(15), 11);
    let (scheduler, event) = wake(15);
    state.mark_scheduled(scheduler, event);

    assert!(state.scheduled_wake_blocks_fetch(15));
    assert_eq!(state.owned_wakes().len(), 2);
}

#[test]
fn o3_writeback_wake_fired_schedule_clears_ownership() {
    let mut state = RiscvO3WritebackWakeState::default();
    state.set_desired_tick(Some(20), 10);
    let (scheduler, event) = wake(20);
    state.mark_scheduled(scheduler, event);

    state.mark_fired(20);

    assert!(state.owned_wakes().is_empty());
    assert!(!state.has_desired_tick());
}

#[test]
fn o3_writeback_wake_detached_schedule_prunes_after_later_tick() {
    let mut state = RiscvO3WritebackWakeState::default();
    state.set_desired_tick(Some(20), 10);
    let (scheduler, event) = wake(20);
    state.mark_scheduled(scheduler, event);
    state.set_desired_tick(Some(15), 11);

    assert_eq!(state.owned_wakes().len(), 1);
    assert_eq!(state.requested_tick(20), None);
    assert_eq!(state.owned_wakes().len(), 1);
    assert_eq!(state.requested_tick(21), None);
    assert!(state.owned_wakes().is_empty());
}

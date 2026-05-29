use std::sync::{Arc, Mutex};

use rem6_kernel::{
    PartitionEventId, PartitionId, PartitionedScheduler, ScheduledEventKind, SchedulerError,
};

#[test]
fn scheduler_cancel_event_removes_only_the_matching_pending_event() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);

    let first_log = Arc::clone(&observed);
    let first = scheduler
        .schedule_at(core, 1, move |context| {
            first_log
                .lock()
                .unwrap()
                .push((context.partition(), context.now(), "first"));
        })
        .unwrap();
    let canceled_log = Arc::clone(&observed);
    let canceled = scheduler
        .schedule_at(core, 2, move |context| {
            canceled_log
                .lock()
                .unwrap()
                .push((context.partition(), context.now(), "canceled"));
        })
        .unwrap();
    let memory_log = Arc::clone(&observed);
    let memory_event = scheduler
        .schedule_at(memory, 2, move |context| {
            memory_log
                .lock()
                .unwrap()
                .push((context.partition(), context.now(), "memory"));
        })
        .unwrap();

    scheduler.cancel_event(canceled).unwrap();

    assert_eq!(
        scheduler.cancel_event(canceled).unwrap_err(),
        SchedulerError::EventNotPending { id: canceled }
    );
    let snapshot = scheduler.snapshot();
    assert_eq!(snapshot.total_pending_events(), 2);
    assert_eq!(
        snapshot.partitions()[core.index() as usize].pending_events()[0].id(),
        first
    );
    assert_eq!(
        snapshot.partitions()[memory.index() as usize].pending_events()[0].id(),
        memory_event
    );

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 2);
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[(core, 1, "first"), (memory, 2, "memory")]
    );
}

#[test]
fn scheduler_reschedule_event_preserves_the_event_identity_and_callback() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let core = PartitionId::new(0);

    let rescheduled_log = Arc::clone(&observed);
    let rescheduled = scheduler
        .schedule_at(core, 10, move |context| {
            rescheduled_log.lock().unwrap().push((
                context.partition(),
                context.now(),
                "rescheduled",
            ));
        })
        .unwrap();
    let peer_log = Arc::clone(&observed);
    scheduler
        .schedule_at(core, 6, move |context| {
            peer_log
                .lock()
                .unwrap()
                .push((context.partition(), context.now(), "peer"));
        })
        .unwrap();

    scheduler.reschedule_event(rescheduled, 5).unwrap();

    let pending = snapshot_pending_for(&scheduler, core);
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].id(), rescheduled);
    assert_eq!(pending[0].tick(), 5);
    assert_eq!(pending[0].kind(), ScheduledEventKind::Serial);

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 2);
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[(core, 5, "rescheduled"), (core, 6, "peer")]
    );
}

#[test]
fn scheduler_reschedule_event_to_the_past_leaves_the_event_pending() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(1, 5).unwrap();
    let core = PartitionId::new(0);
    let future = scheduler.schedule_at(core, 10, |_| {}).unwrap();
    scheduler.schedule_at(core, 4, |_| {}).unwrap();

    let first_epoch = scheduler.run_next_epoch();
    assert_eq!(first_epoch.final_tick(), 5);

    assert_eq!(
        scheduler.reschedule_event(future, 3).unwrap_err(),
        SchedulerError::InThePast {
            partition: core,
            now: 5,
            requested: 3,
        }
    );
    let pending = snapshot_pending_for(&scheduler, core);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id(), future);
    assert_eq!(pending[0].tick(), 10);
}

#[test]
fn scheduler_reschedule_event_rejects_non_pending_ids() {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let core = PartitionId::new(0);
    let stale = PartitionEventId::new(core, 99);

    assert_eq!(
        scheduler.reschedule_event(stale, 5).unwrap_err(),
        SchedulerError::EventNotPending { id: stale }
    );
}

fn snapshot_pending_for(
    scheduler: &PartitionedScheduler,
    partition: PartitionId,
) -> Vec<rem6_kernel::PendingEventSnapshot> {
    scheduler.snapshot().partitions()[partition.index() as usize]
        .pending_events()
        .to_vec()
}

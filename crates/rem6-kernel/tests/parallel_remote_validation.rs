use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};

#[test]
fn parallel_scheduler_rejects_source_event_before_serial_final_tick() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();

    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);

    scheduler.schedule_at(memory, 100, |_| {}).unwrap();
    assert_eq!(scheduler.run_until_idle().final_tick(), 100);
    assert_eq!(scheduler.partition_now(core).unwrap(), 100);
    assert_eq!(scheduler.partition_now(memory).unwrap(), 100);
    let before = scheduler.snapshot();

    assert_eq!(
        scheduler
            .schedule_parallel_at(core, 99, |_| {})
            .unwrap_err(),
        SchedulerError::InThePast {
            partition: core,
            now: 100,
            requested: 99,
        }
    );
    assert_eq!(scheduler.snapshot(), before);
}

#[test]
fn parallel_context_reports_remote_delivery_before_lookahead_boundary() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);

    scheduler
        .schedule_parallel_at(core, 7, move |context| {
            let error = context
                .schedule_remote_after(memory, 4, |_| {})
                .unwrap_err();
            assert_eq!(
                error,
                SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
                    source: core,
                    target: memory,
                    source_tick: 7,
                    delivery_tick: 11,
                    minimum_delivery_tick: 12,
                }
            );
        })
        .unwrap();

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();

    assert_eq!(run.summary().executed_events(), 1);
    assert_eq!(run.remote_send_count(), 0);
    assert_eq!(scheduler.next_pending_tick(memory).unwrap(), None);
}

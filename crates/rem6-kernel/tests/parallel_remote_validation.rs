use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};

#[test]
fn parallel_context_rejects_remote_delivery_before_target_partition_clock() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();

    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);

    scheduler.schedule_at(memory, 100, |_| {}).unwrap();
    assert_eq!(scheduler.run_until_idle().final_tick(), 100);
    assert_eq!(scheduler.partition_now(core).unwrap(), 0);
    assert_eq!(scheduler.partition_now(memory).unwrap(), 100);

    let rejected_log = Arc::clone(&observed);
    scheduler
        .schedule_parallel_at(core, 0, move |context| {
            let error = context
                .schedule_remote_after(memory, 5, |_| {})
                .unwrap_err();
            assert_eq!(
                error,
                SchedulerError::InThePast {
                    partition: memory,
                    now: 100,
                    requested: 5,
                }
            );
            rejected_log.lock().unwrap().push("rejected");
        })
        .unwrap();

    let epoch = scheduler.run_next_epoch_parallel_recorded().unwrap();

    assert_eq!(epoch.summary().executed_events(), 1);
    assert_eq!(epoch.remote_send_count(), 0);
    assert_eq!(observed.lock().unwrap().as_slice(), &["rejected"]);
    assert_eq!(scheduler.next_pending_tick(memory).unwrap(), None);
}

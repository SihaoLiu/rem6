use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};

#[test]
fn scheduler_dispatches_partitions_in_deterministic_tick_order() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(3).unwrap();

    let cpu1 = PartitionId::new(1);
    let cpu0 = PartitionId::new(0);
    let network = PartitionId::new(2);

    let cpu1_log = Arc::clone(&observed);
    scheduler
        .schedule_at(cpu1, 4, move |context| {
            cpu1_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "cpu1"));
        })
        .unwrap();

    let network_log = Arc::clone(&observed);
    scheduler
        .schedule_at(network, 1, move |context| {
            network_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "network"));
        })
        .unwrap();

    let cpu0_first_log = Arc::clone(&observed);
    scheduler
        .schedule_at(cpu0, 4, move |context| {
            cpu0_first_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "cpu0-first"));
        })
        .unwrap();

    let cpu0_second_log = Arc::clone(&observed);
    scheduler
        .schedule_at(cpu0, 4, move |context| {
            cpu0_second_log.lock().unwrap().push((
                context.now(),
                context.partition(),
                "cpu0-second",
            ));
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 4);
    assert_eq!(summary.final_tick(), 4);
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[
            (1, network, "network"),
            (4, cpu0, "cpu0-first"),
            (4, cpu0, "cpu0-second"),
            (4, cpu1, "cpu1"),
        ]
    );
}

#[test]
fn scheduler_delivers_cross_partition_messages_after_explicit_delay() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    let core = PartitionId::new(0);
    let interconnect = PartitionId::new(1);

    let send_log = Arc::clone(&observed);
    let receive_log = Arc::clone(&observed);
    scheduler
        .schedule_at(core, 2, move |context| {
            send_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "send"));

            context
                .schedule_remote_after(interconnect, 3, move |context| {
                    receive_log.lock().unwrap().push((
                        context.now(),
                        context.partition(),
                        "receive",
                    ));
                })
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 2);
    assert_eq!(summary.final_tick(), 5);
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[(2, core, "send"), (5, interconnect, "receive")]
    );
}

#[test]
fn scheduler_rejects_unknown_partitions_and_zero_delay_remote_messages() {
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let core = PartitionId::new(0);
    let missing = PartitionId::new(1);

    let unknown = scheduler.schedule_at(missing, 1, |_| {}).unwrap_err();
    assert_eq!(
        unknown,
        SchedulerError::UnknownPartition {
            partition: missing,
            partitions: 1
        }
    );

    scheduler
        .schedule_at(core, 1, move |context| {
            let zero_delay = context
                .schedule_remote_after(missing, 0, |_| {})
                .unwrap_err();
            assert_eq!(
                zero_delay,
                SchedulerError::ZeroDelayRemoteMessage {
                    source: core,
                    target: missing
                }
            );
        })
        .unwrap();

    scheduler.run_until_idle();
}

#[test]
fn scheduler_rejects_empty_partition_sets() {
    assert_eq!(
        PartitionedScheduler::new(0).unwrap_err(),
        SchedulerError::NoPartitions
    );
}

use std::sync::{Arc, Barrier, Mutex};

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

#[test]
fn scheduler_epoch_stops_at_deterministic_lookahead_barrier() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();

    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);

    let core_log = Arc::clone(&observed);
    let response_log = Arc::clone(&observed);
    scheduler
        .schedule_at(core, 4, move |context| {
            core_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "core-request"));

            context
                .schedule_remote_after(memory, 5, move |context| {
                    response_log.lock().unwrap().push((
                        context.now(),
                        context.partition(),
                        "memory-receive",
                    ));
                })
                .unwrap();
        })
        .unwrap();

    let memory_log = Arc::clone(&observed);
    scheduler
        .schedule_at(memory, 6, move |context| {
            memory_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "memory-local"));
        })
        .unwrap();

    let first_epoch = scheduler.run_next_epoch();

    assert_eq!(first_epoch.executed_events(), 1);
    assert_eq!(first_epoch.final_tick(), 5);
    assert_eq!(scheduler.now(), 5);
    assert_eq!(scheduler.partition_now(core).unwrap(), 5);
    assert_eq!(scheduler.partition_now(memory).unwrap(), 5);
    assert_eq!(scheduler.next_pending_tick(memory).unwrap(), Some(6));
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[(4, core, "core-request")]
    );

    let second_epoch = scheduler.run_next_epoch();

    assert_eq!(second_epoch.executed_events(), 2);
    assert_eq!(second_epoch.final_tick(), 10);
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[
            (4, core, "core-request"),
            (6, memory, "memory-local"),
            (9, memory, "memory-receive"),
        ]
    );
}

#[test]
fn scheduler_rejects_remote_messages_below_configured_lookahead() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);

    scheduler
        .schedule_at(core, 1, move |context| {
            let error = context
                .schedule_remote_after(memory, 4, |_| {})
                .unwrap_err();
            assert_eq!(
                error,
                SchedulerError::RemoteDelayBelowLookahead {
                    source: core,
                    target: memory,
                    delay: 4,
                    minimum: 5
                }
            );
        })
        .unwrap();

    scheduler.run_until_idle();
}

#[test]
fn scheduler_allows_local_events_below_remote_lookahead() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();

    let core = PartitionId::new(0);

    let start_log = Arc::clone(&observed);
    let immediate_log = Arc::clone(&observed);
    let short_delay_log = Arc::clone(&observed);
    scheduler
        .schedule_at(core, 3, move |context| {
            start_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "start"));

            context
                .schedule_local_after(0, move |context| {
                    immediate_log.lock().unwrap().push((
                        context.now(),
                        context.partition(),
                        "local-immediate",
                    ));
                })
                .unwrap();

            context
                .schedule_local_after(1, move |context| {
                    short_delay_log.lock().unwrap().push((
                        context.now(),
                        context.partition(),
                        "local-short",
                    ));
                })
                .unwrap();
        })
        .unwrap();

    let epoch = scheduler.run_next_epoch();

    assert_eq!(epoch.executed_events(), 3);
    assert_eq!(epoch.final_tick(), 5);
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[
            (3, core, "start"),
            (3, core, "local-immediate"),
            (4, core, "local-short"),
        ]
    );
}

#[test]
fn scheduler_delivers_remote_message_at_lookahead_boundary() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();

    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);

    let send_log = Arc::clone(&observed);
    let receive_log = Arc::clone(&observed);
    scheduler
        .schedule_at(core, 0, move |context| {
            send_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "send"));

            context
                .schedule_remote_after(memory, 5, move |context| {
                    receive_log.lock().unwrap().push((
                        context.now(),
                        context.partition(),
                        "receive",
                    ));
                })
                .unwrap();
        })
        .unwrap();

    let epoch = scheduler.run_next_epoch();

    assert_eq!(epoch.executed_events(), 2);
    assert_eq!(epoch.final_tick(), 5);
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[(0, core, "send"), (5, memory, "receive")]
    );
}

#[test]
fn scheduler_rejects_zero_lookahead_configuration() {
    assert_eq!(
        PartitionedScheduler::with_min_remote_delay(2, 0).unwrap_err(),
        SchedulerError::ZeroLookahead
    );
}

#[test]
fn scheduler_plans_next_epoch_without_dispatching_events() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 5).unwrap();

    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let device = PartitionId::new(2);

    let core_log = Arc::clone(&observed);
    scheduler
        .schedule_at(core, 4, move |context| {
            core_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "core"));
        })
        .unwrap();
    scheduler.schedule_at(memory, 6, |_| {}).unwrap();
    scheduler.schedule_at(device, 5, |_| {}).unwrap();

    let plan = scheduler.plan_next_epoch().unwrap();

    assert_eq!(plan.horizon(), 5);
    assert_eq!(
        plan.ready_partitions(),
        &[
            rem6_kernel::ReadyPartition {
                partition: core,
                next_tick: 4
            },
            rem6_kernel::ReadyPartition {
                partition: device,
                next_tick: 5
            },
        ]
    );
    assert!(observed.lock().unwrap().is_empty());
    assert_eq!(scheduler.now(), 0);
}

#[test]
fn scheduler_conservative_runner_executes_epochs_until_idle() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();

    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);

    let core_log = Arc::clone(&observed);
    let response_log = Arc::clone(&observed);
    scheduler
        .schedule_at(core, 4, move |context| {
            core_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "core-request"));

            context
                .schedule_remote_after(memory, 5, move |context| {
                    response_log.lock().unwrap().push((
                        context.now(),
                        context.partition(),
                        "memory-receive",
                    ));
                })
                .unwrap();
        })
        .unwrap();

    let memory_log = Arc::clone(&observed);
    scheduler
        .schedule_at(memory, 6, move |context| {
            memory_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "memory-local"));
        })
        .unwrap();

    let summary = scheduler.run_until_idle_conservative();

    assert_eq!(summary.epochs(), 2);
    assert_eq!(summary.executed_events(), 3);
    assert_eq!(summary.final_tick(), 10);
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[
            (4, core, "core-request"),
            (6, memory, "memory-local"),
            (9, memory, "memory-receive"),
        ]
    );
    assert!(scheduler.is_idle());
}

#[test]
fn scheduler_idle_epoch_does_not_advance_time() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();

    assert!(scheduler.plan_next_epoch().is_none());

    let epoch = scheduler.run_next_epoch();
    assert_eq!(epoch.executed_events(), 0);
    assert_eq!(epoch.final_tick(), 0);
    assert_eq!(scheduler.now(), 0);

    let all_epochs = scheduler.run_until_idle_conservative();
    assert_eq!(all_epochs.epochs(), 0);
    assert_eq!(all_epochs.executed_events(), 0);
    assert_eq!(all_epochs.final_tick(), 0);
}

#[test]
fn scheduler_parallel_epoch_runs_ready_partitions_before_barrier_merge() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let barrier = Arc::new(Barrier::new(2));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 5).unwrap();

    let core0 = PartitionId::new(0);
    let core1 = PartitionId::new(1);
    let memory = PartitionId::new(2);

    let core0_log = Arc::clone(&observed);
    let core0_barrier = Arc::clone(&barrier);
    scheduler
        .schedule_parallel_at(core0, 2, move |context| {
            core0_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "core0-enter"));
            core0_barrier.wait();
            context
                .schedule_remote_after(memory, 5, move |context| {
                    let core0_local_log = Arc::clone(&core0_log);
                    context
                        .schedule_local_after(0, move |context| {
                            core0_local_log.lock().unwrap().push((
                                context.now(),
                                context.partition(),
                                "core0-memory-local",
                            ));
                        })
                        .unwrap();
                    core0_log.lock().unwrap().push((
                        context.now(),
                        context.partition(),
                        "core0-memory",
                    ));
                })
                .unwrap();
        })
        .unwrap();

    let core1_log = Arc::clone(&observed);
    let core1_barrier = Arc::clone(&barrier);
    scheduler
        .schedule_parallel_at(core1, 2, move |context| {
            core1_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "core1-enter"));
            core1_barrier.wait();
            context
                .schedule_remote_after(memory, 5, move |context| {
                    core1_log.lock().unwrap().push((
                        context.now(),
                        context.partition(),
                        "core1-memory",
                    ));
                })
                .unwrap();
        })
        .unwrap();

    let first_epoch = scheduler.run_next_epoch_parallel().unwrap();

    assert_eq!(first_epoch.executed_events(), 2);
    assert_eq!(first_epoch.final_tick(), 5);
    assert_eq!(scheduler.next_pending_tick(memory).unwrap(), Some(7));

    let second_epoch = scheduler.run_next_epoch_parallel().unwrap();

    assert_eq!(second_epoch.executed_events(), 3);
    assert_eq!(second_epoch.final_tick(), 10);
    let observed = observed.lock().unwrap();
    let mut worker_entries = observed[..2].to_vec();
    worker_entries.sort_by_key(|entry| entry.1);
    assert_eq!(
        worker_entries.as_slice(),
        &[(2, core0, "core0-enter"), (2, core1, "core1-enter")]
    );
    assert_eq!(
        &observed[2..],
        &[
            (7, memory, "core0-memory"),
            (7, memory, "core1-memory"),
            (7, memory, "core0-memory-local"),
        ]
    );
}

#[test]
fn scheduler_parallel_runner_executes_epochs_until_idle() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);

    let send_log = Arc::clone(&observed);
    let receive_log = Arc::clone(&observed);
    scheduler
        .schedule_parallel_at(core, 0, move |context| {
            send_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "send"));
            context
                .schedule_remote_after(memory, 2, move |context| {
                    receive_log.lock().unwrap().push((
                        context.now(),
                        context.partition(),
                        "receive",
                    ));
                })
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.epochs(), 1);
    assert_eq!(summary.executed_events(), 2);
    assert_eq!(summary.final_tick(), 2);
    assert!(scheduler.is_idle());
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[(0, core, "send"), (2, memory, "receive")]
    );
}

#[test]
fn scheduler_parallel_epoch_executes_boundary_remote_events_before_returning() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 4).unwrap();

    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);

    let send_log = Arc::clone(&observed);
    let receive_log = Arc::clone(&observed);
    scheduler
        .schedule_parallel_at(core, 0, move |context| {
            send_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "send"));
            context
                .schedule_remote_after(memory, 4, move |context| {
                    receive_log.lock().unwrap().push((
                        context.now(),
                        context.partition(),
                        "receive",
                    ));
                })
                .unwrap();
        })
        .unwrap();

    let epoch = scheduler.run_next_epoch_parallel().unwrap();

    assert_eq!(epoch.executed_events(), 2);
    assert_eq!(epoch.final_tick(), 4);
    assert_eq!(scheduler.now(), 4);
    assert!(scheduler.is_idle());
    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[(0, core, "send"), (4, memory, "receive")]
    );
}

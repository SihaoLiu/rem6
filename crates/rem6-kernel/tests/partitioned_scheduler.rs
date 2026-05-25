use std::sync::{Arc, Barrier, Mutex};

use rem6_kernel::{
    ParallelRunProfile, PartitionEventId, PartitionId, PartitionSnapshot, PartitionedScheduler,
    ScheduledEventKind, SchedulerDispatchRecord, SchedulerError, SchedulerSnapshot,
};

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

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();
    let summary = run.summary();

    assert_eq!(summary.epochs(), 1);
    assert_eq!(summary.executed_events(), 2);
    assert_eq!(summary.final_tick(), 2);
    assert_eq!(run.remote_send_count(), 1);
    assert_eq!(run.epochs()[0].remote_send_count(), 1);
    assert_eq!(run.batches()[0].remote_send_count(), 1);
    let core_activity = run.partition_activity(core).unwrap();
    assert_eq!(core_activity.remote_send_count(), 1);
    let memory_activity = run.partition_activity(memory).unwrap();
    assert_eq!(memory_activity.remote_send_count(), 0);
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

#[test]
fn scheduler_snapshot_reports_ordered_pending_events_without_dispatching() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 5).unwrap();

    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let device = PartitionId::new(2);

    let serial_log = Arc::clone(&observed);
    let serial_id = scheduler
        .schedule_at(core, 4, move |context| {
            serial_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "serial"));
        })
        .unwrap();
    let parallel_log = Arc::clone(&observed);
    let parallel_id = scheduler
        .schedule_parallel_at(core, 3, move |context| {
            parallel_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "parallel"));
        })
        .unwrap();
    let memory_id = scheduler.schedule_parallel_at(memory, 2, |_| {}).unwrap();
    let device_id = scheduler.schedule_parallel_at(device, 8, |_| {}).unwrap();

    let snapshot = scheduler.snapshot();

    assert_eq!(snapshot.now(), 0);
    assert_eq!(snapshot.min_remote_delay(), 5);
    assert_eq!(snapshot.total_pending_events(), 4);
    assert_eq!(snapshot.partitions().len(), 3);

    let core_snapshot = &snapshot.partitions()[0];
    assert_eq!(core_snapshot.partition(), core);
    assert_eq!(core_snapshot.now(), 0);
    assert_eq!(core_snapshot.next_event_local(), 2);
    assert_eq!(core_snapshot.pending_events().len(), 2);
    assert_eq!(core_snapshot.pending_events()[0].id(), parallel_id);
    assert_eq!(core_snapshot.pending_events()[0].tick(), 3);
    assert_eq!(
        core_snapshot.pending_events()[0].kind(),
        ScheduledEventKind::Parallel
    );
    assert_eq!(core_snapshot.pending_events()[1].id(), serial_id);
    assert_eq!(core_snapshot.pending_events()[1].tick(), 4);
    assert_eq!(
        core_snapshot.pending_events()[1].kind(),
        ScheduledEventKind::Serial
    );

    let memory_snapshot = &snapshot.partitions()[1];
    assert_eq!(memory_snapshot.partition(), memory);
    assert_eq!(memory_snapshot.pending_events()[0].id(), memory_id);
    assert_eq!(memory_snapshot.pending_events()[0].tick(), 2);

    let device_snapshot = &snapshot.partitions()[2];
    assert_eq!(device_snapshot.partition(), device);
    assert_eq!(device_snapshot.pending_events()[0].id(), device_id);
    assert_eq!(device_snapshot.pending_events()[0].tick(), 8);

    assert!(observed.lock().unwrap().is_empty());
    assert_eq!(scheduler.now(), 0);
}

#[test]
fn scheduler_quiescent_snapshot_restores_partition_clocks_and_event_ids() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);

    let first_id = scheduler
        .schedule_parallel_at(core, 3, move |context| {
            context.schedule_local_after(1, |_| {}).unwrap();
        })
        .unwrap();
    assert_eq!(first_id.local(), 0);

    let run = scheduler.run_until_idle_parallel().unwrap();
    assert_eq!(run.final_tick(), 5);
    assert!(scheduler.is_idle());

    let snapshot = scheduler.quiescent_snapshot().unwrap();
    assert_eq!(snapshot.total_pending_events(), 0);
    assert_eq!(snapshot.partitions()[0].now(), 5);
    assert_eq!(snapshot.partitions()[0].next_event_local(), 2);

    let mut mutated = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();
    mutated.schedule_parallel_at(memory, 9, |_| {}).unwrap();
    let mutated_run = mutated.run_until_idle_parallel().unwrap();
    assert_eq!(mutated_run.final_tick(), 10);

    mutated.restore_quiescent(&snapshot).unwrap();

    assert_eq!(mutated.now(), 5);
    assert_eq!(mutated.partition_now(core).unwrap(), 5);
    assert_eq!(mutated.partition_now(memory).unwrap(), 5);
    let restored_id = mutated.schedule_parallel_at(core, 6, |_| {}).unwrap();
    assert_eq!(restored_id.local(), 2);
}

#[test]
fn scheduler_quiescent_snapshot_rejects_pending_events() {
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);

    let mut busy_source = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();
    busy_source.schedule_parallel_at(core, 1, |_| {}).unwrap();
    assert_eq!(
        busy_source.quiescent_snapshot().unwrap_err(),
        SchedulerError::SnapshotContainsPendingEvents { pending_events: 1 }
    );

    let busy_snapshot = busy_source.snapshot();
    let mut clean_target = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();
    assert_eq!(
        clean_target.restore_quiescent(&busy_snapshot).unwrap_err(),
        SchedulerError::SnapshotContainsPendingEvents { pending_events: 1 }
    );

    let clean_snapshot = PartitionedScheduler::with_min_remote_delay(2, 5)
        .unwrap()
        .quiescent_snapshot()
        .unwrap();
    let mut busy_target = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();
    busy_target.schedule_parallel_at(memory, 2, |_| {}).unwrap();
    assert_eq!(
        busy_target.restore_quiescent(&clean_snapshot).unwrap_err(),
        SchedulerError::RestoreWouldDiscardPendingEvents { pending_events: 1 }
    );
}

#[test]
fn scheduler_recorded_parallel_epoch_reports_canonical_dispatch_order() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 5).unwrap();

    let core0 = PartitionId::new(0);
    let core1 = PartitionId::new(1);
    let memory = PartitionId::new(2);

    scheduler
        .schedule_parallel_at(core1, 2, move |context| {
            context.schedule_remote_after(memory, 5, |_| {}).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(core0, 2, move |context| {
            context
                .schedule_remote_after(memory, 5, move |context| {
                    context.schedule_local_after(0, |_| {}).unwrap();
                })
                .unwrap();
        })
        .unwrap();
    scheduler.schedule_parallel_at(memory, 4, |_| {}).unwrap();

    let first_epoch = scheduler.run_next_epoch_parallel_recorded().unwrap();

    assert_eq!(first_epoch.summary().executed_events(), 3);
    assert_eq!(first_epoch.summary().final_tick(), 5);
    assert_eq!(
        first_epoch.dispatches(),
        &[
            SchedulerDispatchRecord::new(
                PartitionEventId::new(core0, 0),
                2,
                ScheduledEventKind::Parallel,
            ),
            SchedulerDispatchRecord::new(
                PartitionEventId::new(core1, 0),
                2,
                ScheduledEventKind::Parallel,
            ),
            SchedulerDispatchRecord::new(
                PartitionEventId::new(memory, 0),
                4,
                ScheduledEventKind::Parallel,
            ),
        ]
    );

    let second_epoch = scheduler.run_next_epoch_parallel_recorded().unwrap();

    assert_eq!(second_epoch.summary().executed_events(), 3);
    assert_eq!(second_epoch.summary().final_tick(), 10);
    assert_eq!(
        second_epoch.dispatches(),
        &[
            SchedulerDispatchRecord::new(
                PartitionEventId::new(memory, 1),
                7,
                ScheduledEventKind::Parallel,
            ),
            SchedulerDispatchRecord::new(
                PartitionEventId::new(memory, 2),
                7,
                ScheduledEventKind::Parallel,
            ),
            SchedulerDispatchRecord::new(
                PartitionEventId::new(memory, 3),
                7,
                ScheduledEventKind::Parallel,
            ),
        ]
    );
}

#[test]
fn scheduler_recorded_parallel_epoch_reports_worker_batches() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 4).unwrap();
    let barrier = Arc::new(Barrier::new(2));

    let core0 = PartitionId::new(0);
    let core1 = PartitionId::new(1);
    let memory = PartitionId::new(2);

    let core0_barrier = Arc::clone(&barrier);
    scheduler
        .schedule_parallel_at(core0, 0, move |context| {
            core0_barrier.wait();
            context.schedule_remote_after(memory, 4, |_| {}).unwrap();
        })
        .unwrap();
    let core1_barrier = Arc::clone(&barrier);
    scheduler
        .schedule_parallel_at(core1, 0, move |context| {
            core1_barrier.wait();
            context.schedule_remote_after(memory, 4, |_| {}).unwrap();
        })
        .unwrap();

    let epoch = scheduler.run_next_epoch_parallel_recorded().unwrap();

    assert_eq!(epoch.summary().executed_events(), 4);
    assert_eq!(epoch.summary().final_tick(), 4);
    assert_eq!(epoch.dispatch_count(), 4);
    assert_eq!(epoch.batch_count(), 2);
    assert_eq!(epoch.remote_send_count(), 2);
    assert_eq!(epoch.empty_epoch_count(), 0);
    assert!(!epoch.is_empty_epoch());
    assert_eq!(epoch.max_parallel_workers(), 2);
    assert_eq!(epoch.total_parallel_workers(), 3);
    assert_eq!(epoch.profile(), ParallelRunProfile::new(1, 0, 2, 4, 3, 2));
    assert!(epoch.has_parallel_work());
    assert_eq!(
        epoch.parallel_worker_partitions(),
        vec![core0, core1, memory]
    );

    let batches = epoch.batches();
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].horizon(), 4);
    assert_eq!(batches[0].worker_count(), 2);
    assert_eq!(batches[0].remote_send_count(), 2);
    assert!(batches[0].contains_worker(core0));
    assert!(batches[0].contains_worker(core1));
    assert!(!batches[0].contains_worker(memory));
    assert_eq!(batches[0].worker_partitions(), vec![core0, core1]);
    assert_eq!(
        batches[0]
            .workers()
            .iter()
            .map(|worker| worker.partition())
            .collect::<Vec<_>>(),
        vec![core0, core1]
    );
    assert!(batches[0]
        .workers()
        .iter()
        .all(|worker| worker.start_tick() == 0
            && worker.safe_until() == 4
            && worker.next_tick() == Some(0)
            && worker.pending_events() == 1));
    assert_eq!(
        batches[0].dispatches(),
        &[
            SchedulerDispatchRecord::new(
                PartitionEventId::new(core0, 0),
                0,
                ScheduledEventKind::Parallel,
            ),
            SchedulerDispatchRecord::new(
                PartitionEventId::new(core1, 0),
                0,
                ScheduledEventKind::Parallel,
            ),
        ]
    );
    assert_eq!(batches[0].dispatch_count_for_partition(core0), 1);
    assert_eq!(batches[0].dispatch_count_for_partition(memory), 0);
    let core0_activity = batches[0].partition_activity(core0).unwrap();
    assert_eq!(core0_activity.worker_count(), 1);
    assert_eq!(core0_activity.dispatch_count(), 1);
    assert_eq!(core0_activity.remote_send_count(), 1);
    assert_eq!(core0_activity.max_pending_events(), 1);
    assert!(core0_activity.has_activity());
    let core1_activity = batches[0].partition_activity(core1).unwrap();
    assert_eq!(core1_activity.remote_send_count(), 1);
    assert_eq!(batches[0].active_partition_count(), 2);
    assert!(batches[0].has_partition_activity(core0));
    assert!(!batches[0].has_partition_activity(memory));

    assert_eq!(batches[1].horizon(), 4);
    assert_eq!(batches[1].worker_count(), 1);
    assert_eq!(batches[1].remote_send_count(), 0);
    assert_eq!(batches[1].worker_partitions(), vec![memory]);
    assert_eq!(batches[1].workers()[0].partition(), memory);
    assert_eq!(batches[1].workers()[0].start_tick(), 0);
    assert_eq!(batches[1].workers()[0].safe_until(), 4);
    assert_eq!(batches[1].workers()[0].next_tick(), Some(4));
    assert_eq!(batches[1].workers()[0].pending_events(), 2);
    assert_eq!(
        batches[1].dispatches(),
        &[
            SchedulerDispatchRecord::new(
                PartitionEventId::new(memory, 0),
                4,
                ScheduledEventKind::Parallel,
            ),
            SchedulerDispatchRecord::new(
                PartitionEventId::new(memory, 1),
                4,
                ScheduledEventKind::Parallel,
            ),
        ]
    );
    assert_eq!(batches[1].dispatch_count(), 2);
    assert_eq!(batches[1].dispatch_count_for_partition(memory), 2);
    let memory_batch_activity = batches[1].partition_activity(memory).unwrap();
    assert_eq!(memory_batch_activity.worker_count(), 1);
    assert_eq!(memory_batch_activity.dispatch_count(), 2);
    assert_eq!(memory_batch_activity.remote_send_count(), 0);
    assert_eq!(memory_batch_activity.max_pending_events(), 2);
    assert_eq!(
        batches[1].dispatches_for_partition(memory),
        batches[1].dispatches().to_vec()
    );
    let memory_epoch_activity = epoch.partition_activity(memory).unwrap();
    assert_eq!(memory_epoch_activity.worker_count(), 1);
    assert_eq!(memory_epoch_activity.dispatch_count(), 2);
    assert_eq!(memory_epoch_activity.remote_send_count(), 0);
    assert_eq!(memory_epoch_activity.max_pending_events(), 2);
    let core0_epoch_activity = epoch.partition_activity(core0).unwrap();
    assert_eq!(core0_epoch_activity.remote_send_count(), 1);
    assert_eq!(epoch.active_partition_count(), 3);
    assert!(epoch.has_partition_activity(core1));
    assert_eq!(scheduler.now(), 4);
    assert!(scheduler.is_idle());
}

#[test]
fn scheduler_recorded_parallel_epoch_reports_empty_batches_without_ready_workers() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 4).unwrap();

    let core = PartitionId::new(0);

    scheduler.schedule_parallel_at(core, 7, |_| {}).unwrap();

    let epoch = scheduler.run_next_epoch_parallel_recorded().unwrap();

    assert_eq!(epoch.summary().executed_events(), 0);
    assert_eq!(epoch.summary().final_tick(), 4);
    assert!(epoch.dispatches().is_empty());
    assert!(epoch.batches().is_empty());
    assert_eq!(epoch.dispatch_count(), 0);
    assert_eq!(epoch.batch_count(), 0);
    assert_eq!(epoch.empty_epoch_count(), 1);
    assert!(epoch.is_empty_epoch());
    assert_eq!(epoch.max_parallel_workers(), 0);
    assert_eq!(epoch.total_parallel_workers(), 0);
    assert_eq!(epoch.profile(), ParallelRunProfile::new(1, 1, 0, 0, 0, 0));
    assert!(!epoch.has_parallel_work());
    assert!(epoch.parallel_worker_partitions().is_empty());
    assert_eq!(scheduler.next_pending_tick(core).unwrap(), Some(7));
}

#[test]
fn scheduler_recorded_parallel_runner_accumulates_profile() {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 4, 2).unwrap();

    for index in 0..4 {
        scheduler
            .schedule_parallel_at(PartitionId::new(index), 0, |_| {})
            .unwrap();
    }

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();

    assert_eq!(run.summary().epochs(), 1);
    assert_eq!(run.summary().executed_events(), 4);
    assert_eq!(run.summary().final_tick(), 4);
    assert_eq!(run.epoch_count(), 1);
    assert_eq!(run.empty_epoch_count(), 0);
    assert_eq!(run.dispatch_count(), 4);
    assert_eq!(run.batch_count(), 2);
    assert_eq!(run.remote_send_count(), 0);
    assert_eq!(run.max_parallel_workers(), 2);
    assert_eq!(run.total_parallel_workers(), 4);
    assert_eq!(run.profile(), ParallelRunProfile::new(1, 0, 2, 4, 4, 2));
    assert!(run.has_parallel_work());
    assert_eq!(run.dispatches().len(), 4);
    assert_eq!(run.batches().len(), 2);
    assert_eq!(
        run.parallel_worker_partitions(),
        vec![
            PartitionId::new(0),
            PartitionId::new(1),
            PartitionId::new(2),
            PartitionId::new(3),
        ]
    );
    assert_eq!(run.active_partition_count(), 4);
    let partition0_activity = run.partition_activity(PartitionId::new(0)).unwrap();
    assert_eq!(partition0_activity.worker_count(), 1);
    assert_eq!(partition0_activity.dispatch_count(), 1);
    assert_eq!(partition0_activity.max_pending_events(), 1);
    assert!(run.has_partition_activity(PartitionId::new(3)));
    assert!(!run.has_partition_activity(PartitionId::new(4)));
    assert_eq!(
        run.epochs()[0].profile(),
        ParallelRunProfile::new(1, 0, 2, 4, 4, 2)
    );
    assert!(scheduler.is_idle());
}

#[test]
fn scheduler_recorded_parallel_runner_preserves_empty_epochs() {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 4, 2).unwrap();
    let core = PartitionId::new(0);

    scheduler.schedule_parallel_at(core, 7, |_| {}).unwrap();

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();

    assert_eq!(run.summary().epochs(), 2);
    assert_eq!(run.summary().executed_events(), 1);
    assert_eq!(run.summary().final_tick(), 8);
    assert_eq!(run.epoch_count(), 2);
    assert_eq!(run.empty_epoch_count(), 1);
    assert_eq!(run.dispatch_count(), 1);
    assert_eq!(run.batch_count(), 1);
    assert_eq!(run.max_parallel_workers(), 1);
    assert_eq!(run.total_parallel_workers(), 1);
    assert_eq!(run.profile(), ParallelRunProfile::new(2, 1, 1, 1, 1, 1));
    assert!(run.has_parallel_work());
    assert!(run.epochs()[0].is_empty_epoch());
    assert_eq!(
        run.epochs()[0].profile(),
        ParallelRunProfile::new(1, 1, 0, 0, 0, 0)
    );
    assert!(!run.epochs()[1].is_empty_epoch());
    assert_eq!(
        run.epochs()[1].profile(),
        ParallelRunProfile::new(1, 0, 1, 1, 1, 1)
    );
    assert_eq!(run.parallel_worker_partitions(), vec![core]);
    assert!(scheduler.is_idle());
}

#[test]
fn scheduler_recorded_parallel_runner_reports_idle_profile() {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap();

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();

    assert_eq!(run.summary().epochs(), 0);
    assert_eq!(run.summary().executed_events(), 0);
    assert_eq!(run.summary().final_tick(), 0);
    assert_eq!(run.epoch_count(), 0);
    assert_eq!(run.empty_epoch_count(), 0);
    assert_eq!(run.dispatch_count(), 0);
    assert_eq!(run.batch_count(), 0);
    assert_eq!(run.max_parallel_workers(), 0);
    assert_eq!(run.total_parallel_workers(), 0);
    assert_eq!(run.profile(), ParallelRunProfile::default());
    assert!(!run.has_parallel_work());
    assert!(run.epochs().is_empty());
    assert!(run.dispatches().is_empty());
    assert!(run.batches().is_empty());
    assert!(run.parallel_worker_partitions().is_empty());
    assert!(scheduler.is_idle());
}

#[test]
fn scheduler_parallel_epoch_respects_configured_worker_limit() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 4, 2).unwrap();

    for index in 0..4 {
        let partition = PartitionId::new(index);
        let log = Arc::clone(&observed);
        scheduler
            .schedule_parallel_at(partition, 0, move |context| {
                log.lock()
                    .unwrap()
                    .push((context.now(), context.partition()));
            })
            .unwrap();
    }

    let epoch = scheduler.run_next_epoch_parallel_recorded().unwrap();

    assert_eq!(epoch.summary().executed_events(), 4);
    assert_eq!(epoch.summary().final_tick(), 4);
    assert_eq!(epoch.batch_count(), 2);
    assert_eq!(epoch.max_parallel_workers(), 2);
    assert_eq!(epoch.total_parallel_workers(), 4);
    assert_eq!(
        epoch.batches()[0].worker_partitions(),
        vec![PartitionId::new(0), PartitionId::new(1)]
    );
    assert_eq!(
        epoch.batches()[1].worker_partitions(),
        vec![PartitionId::new(2), PartitionId::new(3)]
    );
    assert_eq!(scheduler.max_parallel_workers(), 2);
    assert!(scheduler.is_idle());
    let mut observed = observed.lock().unwrap().clone();
    observed.sort_by_key(|entry| entry.1);
    assert_eq!(
        observed,
        vec![
            (0, PartitionId::new(0)),
            (0, PartitionId::new(1)),
            (0, PartitionId::new(2)),
            (0, PartitionId::new(3)),
        ]
    );
}

#[test]
fn scheduler_parallel_batches_earlier_ticks_before_horizon_receivers() {
    let sent_sources = Arc::new(Mutex::new(Vec::new()));
    let receive_observations = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(5, 4, 1).unwrap();

    let target = PartitionId::new(0);
    for source_index in 1..=4 {
        let source = PartitionId::new(source_index);
        let source_sent = Arc::clone(&sent_sources);
        let receive_sent = Arc::clone(&sent_sources);
        let receive_log = Arc::clone(&receive_observations);
        scheduler
            .schedule_parallel_at(source, 0, move |context| {
                source_sent.lock().unwrap().push(source_index);
                context
                    .schedule_remote_after(target, 4, move |_| {
                        let sent_count = receive_sent.lock().unwrap().len();
                        receive_log.lock().unwrap().push((source_index, sent_count));
                    })
                    .unwrap();
            })
            .unwrap();
    }

    let epoch = scheduler.run_next_epoch_parallel_recorded().unwrap();

    assert_eq!(epoch.summary().executed_events(), 8);
    assert_eq!(epoch.summary().final_tick(), 4);
    assert_eq!(sent_sources.lock().unwrap().as_slice(), &[1, 2, 3, 4]);
    assert_eq!(
        receive_observations.lock().unwrap().as_slice(),
        &[(1, 4), (2, 4), (3, 4), (4, 4)]
    );
}

#[test]
fn scheduler_rejects_zero_parallel_worker_limit() {
    assert_eq!(
        PartitionedScheduler::with_parallel_worker_limit(2, 4, 0).unwrap_err(),
        SchedulerError::ZeroParallelWorkers
    );
}

#[test]
fn scheduler_parallel_worker_panic_returns_typed_error() {
    let core = PartitionId::new(0);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap();

    scheduler
        .schedule_parallel_at(core, 0, |_| panic!("worker panic sentinel"))
        .unwrap();

    assert_eq!(
        scheduler.run_next_epoch_parallel_recorded().unwrap_err(),
        SchedulerError::ParallelWorkerPanicked { partition: core }
    );
}

#[test]
fn scheduler_parallel_worker_panic_preserves_future_partition_events() {
    let core = PartitionId::new(0);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap();

    scheduler
        .schedule_parallel_at(core, 0, |_| panic!("worker panic sentinel"))
        .unwrap();
    let future = scheduler.schedule_parallel_at(core, 3, |_| {}).unwrap();

    assert_eq!(
        scheduler.run_next_epoch_parallel_recorded().unwrap_err(),
        SchedulerError::ParallelWorkerPanicked { partition: core }
    );
    assert_eq!(scheduler.next_pending_tick(core).unwrap(), Some(3));
    let snapshot = scheduler.snapshot();
    let pending = snapshot.partitions()[core.index() as usize].pending_events();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id(), future);
    assert_eq!(pending[0].tick(), 3);
    assert_eq!(pending[0].order(), 1);
    assert_eq!(pending[0].kind(), ScheduledEventKind::Parallel);
}

#[test]
fn scheduler_parallel_worker_panic_updates_global_time_to_executed_tick() {
    let core = PartitionId::new(0);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap();

    scheduler
        .schedule_parallel_at(core, 2, |_| panic!("worker panic sentinel"))
        .unwrap();
    scheduler.schedule_parallel_at(core, 5, |_| {}).unwrap();

    assert_eq!(
        scheduler.run_next_epoch_parallel_recorded().unwrap_err(),
        SchedulerError::ParallelWorkerPanicked { partition: core }
    );
    assert_eq!(scheduler.partition_now(core).unwrap(), 2);
    assert_eq!(scheduler.now(), 2);
    assert_eq!(scheduler.next_pending_tick(core).unwrap(), Some(5));
}

#[test]
fn scheduler_parallel_worker_panic_discards_local_events_from_panicked_callback() {
    let core = PartitionId::new(0);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap();

    scheduler
        .schedule_parallel_at(core, 1, |context| {
            context.schedule_local_after(1, |_| {}).unwrap();
            panic!("worker panic sentinel");
        })
        .unwrap();
    let future = scheduler.schedule_parallel_at(core, 5, |_| {}).unwrap();

    assert_eq!(
        scheduler.run_next_epoch_parallel_recorded().unwrap_err(),
        SchedulerError::ParallelWorkerPanicked { partition: core }
    );
    let snapshot = scheduler.snapshot();
    let pending = snapshot.partitions()[core.index() as usize].pending_events();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id(), future);
    assert_eq!(pending[0].tick(), 5);
    assert_eq!(pending[0].order(), 1);
    assert_eq!(pending[0].kind(), ScheduledEventKind::Parallel);
}

#[test]
fn scheduler_parallel_worker_panic_preserves_successful_remote_events() {
    let source = PartitionId::new(0);
    let panicker = PartitionId::new(1);
    let target = PartitionId::new(2);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(3, 4, 2).unwrap();

    scheduler
        .schedule_parallel_at(source, 0, move |context| {
            context.schedule_remote_after(target, 4, |_| {}).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(panicker, 0, move |context| {
            context.schedule_remote_after(target, 4, |_| {}).unwrap();
            panic!("worker panic sentinel");
        })
        .unwrap();

    assert_eq!(
        scheduler.run_next_epoch_parallel_recorded().unwrap_err(),
        SchedulerError::ParallelWorkerPanicked {
            partition: panicker
        }
    );
    let snapshot = scheduler.snapshot();
    let pending = snapshot.partitions()[target.index() as usize].pending_events();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].tick(), 4);
    assert_eq!(pending[0].kind(), ScheduledEventKind::Parallel);
}

#[test]
fn scheduler_parallel_worker_panic_preserves_prior_callback_remote_events() {
    let source = PartitionId::new(0);
    let target = PartitionId::new(1);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap();

    scheduler
        .schedule_parallel_at(source, 0, move |context| {
            context.schedule_remote_after(target, 4, |_| {}).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(source, 1, move |context| {
            context.schedule_remote_after(target, 4, |_| {}).unwrap();
            panic!("worker panic sentinel");
        })
        .unwrap();

    assert_eq!(
        scheduler.run_next_epoch_parallel_recorded().unwrap_err(),
        SchedulerError::ParallelWorkerPanicked { partition: source }
    );
    assert_eq!(scheduler.now(), 1);
    let snapshot = scheduler.snapshot();
    let pending = snapshot.partitions()[target.index() as usize].pending_events();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].tick(), 4);
    assert_eq!(pending[0].kind(), ScheduledEventKind::Parallel);
}

#[test]
fn scheduler_min_delay_constructor_keeps_unbounded_worker_limit() {
    let scheduler = PartitionedScheduler::with_min_remote_delay(2, 4).unwrap();

    assert_eq!(scheduler.max_parallel_workers(), usize::MAX);
    assert_eq!(
        scheduler.snapshot().max_parallel_workers(),
        scheduler.max_parallel_workers()
    );
}

#[test]
fn scheduler_snapshot_preserves_parallel_worker_limit() {
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 5, 1).unwrap();

    let snapshot = scheduler.quiescent_snapshot().unwrap();

    assert_eq!(snapshot.max_parallel_workers(), 1);

    let mut matching = PartitionedScheduler::with_parallel_worker_limit(2, 5, 1).unwrap();
    matching.restore_quiescent(&snapshot).unwrap();
    assert_eq!(matching.max_parallel_workers(), 1);

    let mut mismatched = PartitionedScheduler::with_parallel_worker_limit(2, 5, 2).unwrap();
    assert_eq!(
        mismatched.restore_quiescent(&snapshot).unwrap_err(),
        SchedulerError::SnapshotParallelWorkerLimitMismatch {
            snapshot_max_parallel_workers: 1,
            scheduler_max_parallel_workers: 2,
        }
    );

    let legacy_snapshot = SchedulerSnapshot::new(
        3,
        5,
        vec![
            PartitionSnapshot::quiescent(core, 3, 0, 0),
            PartitionSnapshot::quiescent(memory, 3, 0, 0),
        ],
    );
    assert_eq!(legacy_snapshot.max_parallel_workers(), usize::MAX);
}

#[test]
fn scheduler_parallel_plan_exposes_frontiers_and_serial_blockers_without_dispatching() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 5).unwrap();

    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let device = PartitionId::new(2);

    let serial_log = Arc::clone(&observed);
    let serial_id = scheduler
        .schedule_at(core, 4, move |context| {
            serial_log
                .lock()
                .unwrap()
                .push((context.now(), context.partition(), "serial"));
        })
        .unwrap();
    scheduler.schedule_parallel_at(memory, 8, |_| {}).unwrap();
    scheduler.schedule_parallel_at(device, 3, |_| {}).unwrap();

    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();

    assert_eq!(plan.horizon(), 5);
    assert!(!plan.is_parallel_safe());
    assert_eq!(
        plan.ready_partitions(),
        &[
            rem6_kernel::ReadyPartition {
                partition: device,
                next_tick: 3,
            },
            rem6_kernel::ReadyPartition {
                partition: core,
                next_tick: 4,
            },
        ]
    );
    assert_eq!(plan.frontiers().len(), 3);
    assert_eq!(plan.frontiers()[0].partition(), core);
    assert_eq!(plan.frontiers()[0].now(), 0);
    assert_eq!(plan.frontiers()[0].safe_until(), 5);
    assert_eq!(plan.frontiers()[0].next_tick(), Some(4));
    assert_eq!(plan.frontiers()[0].pending_events(), 1);
    assert_eq!(plan.frontiers()[1].partition(), memory);
    assert_eq!(plan.frontiers()[1].next_tick(), Some(8));
    assert_eq!(plan.frontiers()[2].partition(), device);
    assert_eq!(plan.frontiers()[2].next_tick(), Some(3));
    assert_eq!(plan.frontiers()[2].pending_events(), 1);
    assert_eq!(
        plan.frontier(memory).map(|frontier| frontier.next_tick()),
        Some(Some(8))
    );
    assert_eq!(plan.frontier(PartitionId::new(9)), None);
    assert_eq!(
        plan.serial_blockers(),
        &[SchedulerDispatchRecord::new(
            serial_id,
            4,
            ScheduledEventKind::Serial,
        )]
    );
    assert_eq!(plan.serial_blocker_count(), 1);
    assert_eq!(
        plan.first_serial_blocker(),
        Some(SchedulerDispatchRecord::new(
            serial_id,
            4,
            ScheduledEventKind::Serial,
        ))
    );

    assert_eq!(scheduler.now(), 0);
    assert_eq!(scheduler.next_pending_tick(core).unwrap(), Some(4));
    assert_eq!(scheduler.next_pending_tick(memory).unwrap(), Some(8));
    assert_eq!(scheduler.next_pending_tick(device).unwrap(), Some(3));
    assert!(observed.lock().unwrap().is_empty());

    assert_eq!(
        scheduler.run_next_epoch_parallel_recorded().unwrap_err(),
        SchedulerError::SerialEventInParallelEpoch {
            partition: core,
            tick: 4,
        }
    );
}

#[test]
fn scheduler_parallel_plan_marks_all_parallel_epoch_as_safe() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 5).unwrap();

    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let device = PartitionId::new(2);

    scheduler.schedule_parallel_at(core, 2, |_| {}).unwrap();
    scheduler.schedule_parallel_at(memory, 4, |_| {}).unwrap();
    scheduler.schedule_parallel_at(device, 9, |_| {}).unwrap();

    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();

    assert_eq!(plan.horizon(), 5);
    assert!(plan.is_parallel_safe());
    assert_eq!(plan.ready_partition_count(), 2);
    assert_eq!(plan.frontier_count(), 3);
    assert_eq!(plan.serial_blocker_count(), 0);
    assert_eq!(plan.first_serial_blocker(), None);
    assert_eq!(
        plan.ready_partitions(),
        &[
            rem6_kernel::ReadyPartition {
                partition: core,
                next_tick: 2,
            },
            rem6_kernel::ReadyPartition {
                partition: memory,
                next_tick: 4,
            },
        ]
    );
}

#[test]
fn scheduler_parallel_plan_leaves_near_limit_idle_scheduler_idle() {
    let near_end = u64::MAX - 1;
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();
    let snapshot = SchedulerSnapshot::new(
        near_end,
        5,
        vec![
            PartitionSnapshot::quiescent(core, near_end, 0, 0),
            PartitionSnapshot::quiescent(memory, near_end, 0, 0),
        ],
    );

    scheduler.restore_quiescent(&snapshot).unwrap();

    assert!(scheduler.plan_next_parallel_epoch().unwrap().is_none());
    let summary = scheduler.run_until_idle_parallel().unwrap();
    assert_eq!(summary.epochs(), 0);
    assert_eq!(summary.executed_events(), 0);
    assert_eq!(summary.final_tick(), near_end);
}

#[test]
fn scheduler_parallel_plan_reports_horizon_overflow_before_silently_stopping() {
    let near_end = u64::MAX - 1;
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();
    let snapshot = SchedulerSnapshot::new(
        near_end,
        5,
        vec![
            PartitionSnapshot::quiescent(core, near_end, 0, 0),
            PartitionSnapshot::quiescent(memory, near_end, 0, 0),
        ],
    );

    scheduler.restore_quiescent(&snapshot).unwrap();
    scheduler
        .schedule_parallel_at(core, near_end, |_| {})
        .unwrap();

    let error = scheduler.plan_next_parallel_epoch().unwrap_err();

    assert_eq!(
        error,
        SchedulerError::EpochHorizonOverflow {
            partition: core,
            now: near_end,
            delay: 5,
        }
    );
    assert_eq!(
        scheduler.run_next_epoch_parallel_recorded().unwrap_err(),
        SchedulerError::EpochHorizonOverflow {
            partition: core,
            now: near_end,
            delay: 5,
        }
    );
}

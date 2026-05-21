use std::sync::{Arc, Barrier, Mutex};

use rem6_kernel::{
    PartitionEventId, PartitionId, PartitionedScheduler, ScheduledEventKind,
    SchedulerDispatchRecord, SchedulerError,
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

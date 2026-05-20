use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptError, InterruptEvent, InterruptEventKind, InterruptLineChannel,
    InterruptLineId, InterruptLinePort, InterruptRoute, InterruptSourceId, InterruptTargetId,
    PendingInterrupt,
};
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};
use rem6_timer::{
    ProgrammableTimer, TimerArm, TimerError, TimerExpiry, TimerId, TimerSignalError, TimerSnapshot,
};

#[test]
fn programmed_timer_delivers_interrupt_at_deadline() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let source = InterruptSourceId::new(5);
    let route = InterruptRoute::new(InterruptLineId::new(20), InterruptTargetId::new(0), cpu);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    controller.lock().unwrap().register_route(route).unwrap();
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    let timer = ProgrammableTimer::new(TimerId::new(0), timer_partition, source, port);
    let observed_timer = timer.clone();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(cpu, 3, move |context| {
            timer.arm_at(context, 10).unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 3);
    assert_eq!(summary.final_tick(), 12);
    assert_eq!(observed_timer.snapshot().arms(), &[TimerArm::new(1, 3, 10)]);
    assert_eq!(
        observed_timer.snapshot().expiries(),
        &[TimerExpiry::new(1, 10)]
    );
    assert_eq!(observed_timer.snapshot().next_deadline(), None);
    let controller = controller.lock().unwrap();
    assert_eq!(
        controller.pending(),
        vec![PendingInterrupt::routed(
            InterruptLineId::new(20),
            InterruptTargetId::new(0),
            cpu,
            source,
            12,
        )]
    );
    assert_eq!(
        controller.history(),
        &[InterruptEvent::routed(
            12,
            InterruptLineId::new(20),
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn programmed_timer_delivers_interrupt_at_deadline_on_parallel_scheduler() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let source = InterruptSourceId::new(15);
    let route = InterruptRoute::new(InterruptLineId::new(25), InterruptTargetId::new(0), cpu);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    controller.lock().unwrap().register_route(route).unwrap();
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    let timer = ProgrammableTimer::new(TimerId::new(4), timer_partition, source, port);
    let observed_timer = timer.clone();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    scheduler
        .schedule_parallel_at(cpu, 3, move |context| {
            timer.arm_at_parallel(context, 10).unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 3);
    assert_eq!(summary.final_tick(), 12);
    assert_eq!(observed_timer.snapshot().arms(), &[TimerArm::new(1, 3, 10)]);
    assert_eq!(
        observed_timer.snapshot().expiries(),
        &[TimerExpiry::new(1, 10)]
    );
    assert_eq!(observed_timer.snapshot().next_deadline(), None);
    assert!(observed_timer.snapshot().signal_errors().is_empty());
    let controller = controller.lock().unwrap();
    assert_eq!(
        controller.pending(),
        vec![PendingInterrupt::routed(
            InterruptLineId::new(25),
            InterruptTargetId::new(0),
            cpu,
            source,
            12,
        )]
    );
    assert_eq!(
        controller.history(),
        &[InterruptEvent::routed(
            12,
            InterruptLineId::new(25),
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn programmed_timer_rearm_invalidates_stale_deadline() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let source = InterruptSourceId::new(6);
    let route = InterruptRoute::new(InterruptLineId::new(21), InterruptTargetId::new(0), cpu);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    controller.lock().unwrap().register_route(route).unwrap();
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    let timer = ProgrammableTimer::new(TimerId::new(1), timer_partition, source, port);
    let observed_timer = timer.clone();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(cpu, 1, move |context| {
            timer.arm_at(context, 10).unwrap();
            timer.arm_at(context, 6).unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 4);
    assert_eq!(summary.final_tick(), 10);
    assert_eq!(
        observed_timer.snapshot().arms(),
        &[TimerArm::new(1, 1, 10), TimerArm::new(2, 1, 6)]
    );
    assert_eq!(
        observed_timer.snapshot().expiries(),
        &[TimerExpiry::new(2, 6)]
    );
    assert_eq!(observed_timer.snapshot().next_deadline(), None);
    let controller = controller.lock().unwrap();
    assert_eq!(controller.history().len(), 1);
    assert_eq!(
        controller.history(),
        &[InterruptEvent::routed(
            8,
            InterruptLineId::new(21),
            InterruptTargetId::new(0),
            cpu,
            source,
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn programmed_timer_rejects_past_deadlines() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let source = InterruptSourceId::new(7);
    let route = InterruptRoute::new(InterruptLineId::new(22), InterruptTargetId::new(0), cpu);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    controller.lock().unwrap().register_route(route).unwrap();
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    let timer = ProgrammableTimer::new(TimerId::new(2), timer_partition, source, port);
    let observed_timer = timer.clone();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let observed_errors = Arc::clone(&errors);
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(cpu, 9, move |context| {
            let error = timer.arm_at(context, 8).unwrap_err();
            errors.lock().unwrap().push(error);
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 1);
    assert_eq!(summary.final_tick(), 9);
    assert_eq!(
        observed_errors.lock().unwrap().as_slice(),
        &[TimerError::DeadlineInPast {
            now: 9,
            deadline: 8,
        }]
    );
    assert!(observed_timer.snapshot().arms().is_empty());
    assert!(observed_timer.snapshot().expiries().is_empty());
    assert!(controller.lock().unwrap().history().is_empty());
}

#[test]
fn programmed_timer_snapshot_restore_reinstates_programmed_state() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let source = InterruptSourceId::new(18);
    let route = InterruptRoute::new(InterruptLineId::new(28), InterruptTargetId::new(0), cpu);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    controller.lock().unwrap().register_route(route).unwrap();
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    let timer = ProgrammableTimer::new(TimerId::new(8), timer_partition, source, port);
    let observed_timer = timer.clone();
    let captured = Arc::new(Mutex::new(None));
    let captured_writer = Arc::clone(&captured);
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(cpu, 2, move |context| {
            timer.arm_at(context, 12).unwrap();
            *captured_writer.lock().unwrap() = Some(timer.snapshot());
        })
        .unwrap();
    scheduler.run_until_idle();
    let captured = captured.lock().unwrap().clone().unwrap();
    assert_ne!(observed_timer.snapshot(), captured);

    observed_timer.restore(&captured).unwrap();

    assert_eq!(observed_timer.snapshot(), captured);
    assert_eq!(observed_timer.snapshot().next_deadline(), Some(12));
    assert_eq!(observed_timer.snapshot().arms(), &[TimerArm::new(1, 2, 12)]);
    assert!(observed_timer.snapshot().expiries().is_empty());

    let rearmed = Arc::new(Mutex::new(None));
    let rearmed_writer = Arc::clone(&rearmed);
    let timer = observed_timer.clone();
    scheduler
        .schedule_at(cpu, 15, move |context| {
            timer.arm_at(context, 20).unwrap();
            *rearmed_writer.lock().unwrap() = Some(timer.snapshot());
        })
        .unwrap();
    scheduler.run_until_idle();

    let rearmed = rearmed.lock().unwrap().clone().unwrap();
    assert_eq!(
        rearmed.arms(),
        &[TimerArm::new(1, 2, 12), TimerArm::new(2, 15, 20)]
    );
}

#[test]
fn programmed_timer_restore_rejects_foreign_snapshot() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let source = InterruptSourceId::new(19);
    let route = InterruptRoute::new(InterruptLineId::new(29), InterruptTargetId::new(0), cpu);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    controller.lock().unwrap().register_route(route).unwrap();
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    let timer = ProgrammableTimer::new(TimerId::new(9), timer_partition, source, port);
    let snapshot = TimerSnapshot::new(
        TimerId::new(10),
        timer_partition,
        source,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(
        timer.restore(&snapshot).unwrap_err(),
        TimerError::SnapshotIdentityMismatch {
            expected_id: TimerId::new(9),
            actual_id: TimerId::new(10),
            expected_partition: timer_partition,
            actual_partition: timer_partition,
            expected_source: source,
            actual_source: source,
        }
    );
}

#[test]
fn programmed_timer_records_interrupt_signal_errors() {
    let cpu = PartitionId::new(0);
    let timer_partition = PartitionId::new(1);
    let source = InterruptSourceId::new(8);
    let route = InterruptRoute::new(InterruptLineId::new(23), InterruptTargetId::new(0), cpu);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    controller.lock().unwrap().register_route(route).unwrap();
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    let timer = ProgrammableTimer::new(TimerId::new(3), timer_partition, source, port);
    let observed_timer = timer.clone();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 3).unwrap();

    scheduler
        .schedule_at(cpu, 1, move |context| {
            timer.arm_at(context, 4).unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 2);
    assert_eq!(summary.final_tick(), 4);
    assert_eq!(
        observed_timer.snapshot().expiries(),
        &[TimerExpiry::new(1, 4)]
    );
    assert_eq!(
        observed_timer.snapshot().signal_errors(),
        &[TimerSignalError::new(
            1,
            4,
            InterruptError::Scheduler(SchedulerError::RemoteDelayBelowLookahead {
                source: timer_partition,
                target: cpu,
                delay: 2,
                minimum: 3,
            }),
        )]
    );
    assert!(controller.lock().unwrap().history().is_empty());
}

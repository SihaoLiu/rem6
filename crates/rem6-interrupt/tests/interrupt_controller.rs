use rem6_interrupt::{
    InterruptClaim, InterruptController, InterruptDelivery, InterruptError, InterruptEvent,
    InterruptEventKind, InterruptLineChannel, InterruptLineId, InterruptLinePort,
    InterruptPriority, InterruptRoute, InterruptSourceId, InterruptTargetId, PendingInterrupt,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use std::sync::{Arc, Mutex};

#[test]
fn interrupt_controller_tracks_pending_lines_in_deterministic_order() {
    let mut controller = InterruptController::new();
    controller
        .register_line(InterruptLineId::new(3), InterruptTargetId::new(0))
        .unwrap();
    controller
        .register_line(InterruptLineId::new(1), InterruptTargetId::new(0))
        .unwrap();
    controller
        .register_line(InterruptLineId::new(2), InterruptTargetId::new(1))
        .unwrap();

    controller
        .assert(InterruptLineId::new(2), InterruptSourceId::new(9), 10)
        .unwrap();
    controller
        .assert(InterruptLineId::new(1), InterruptSourceId::new(7), 8)
        .unwrap();

    assert_eq!(
        controller.pending(),
        vec![
            PendingInterrupt::new(
                InterruptLineId::new(1),
                InterruptTargetId::new(0),
                InterruptSourceId::new(7),
                8,
            ),
            PendingInterrupt::new(
                InterruptLineId::new(2),
                InterruptTargetId::new(1),
                InterruptSourceId::new(9),
                10,
            ),
        ]
    );

    let snapshot = controller.snapshot(12);
    assert_eq!(snapshot.tick(), 12);
    assert_eq!(snapshot.pending(), controller.pending());
    assert_eq!(
        snapshot.history(),
        &[
            InterruptEvent::new(
                10,
                InterruptLineId::new(2),
                InterruptTargetId::new(1),
                InterruptSourceId::new(9),
                InterruptEventKind::Assert,
            ),
            InterruptEvent::new(
                8,
                InterruptLineId::new(1),
                InterruptTargetId::new(0),
                InterruptSourceId::new(7),
                InterruptEventKind::Assert,
            ),
        ]
    );
}

#[test]
fn interrupt_controller_deasserts_only_matching_source() {
    let mut controller = InterruptController::new();
    controller
        .register_line(InterruptLineId::new(5), InterruptTargetId::new(2))
        .unwrap();
    controller
        .assert(InterruptLineId::new(5), InterruptSourceId::new(11), 20)
        .unwrap();

    assert_eq!(
        controller.deassert(InterruptLineId::new(5), InterruptSourceId::new(12), 24),
        Err(InterruptError::SourceMismatch {
            line: InterruptLineId::new(5),
            expected: InterruptSourceId::new(11),
            actual: InterruptSourceId::new(12),
        })
    );
    assert_eq!(
        controller.pending(),
        vec![PendingInterrupt::new(
            InterruptLineId::new(5),
            InterruptTargetId::new(2),
            InterruptSourceId::new(11),
            20,
        )]
    );

    controller
        .deassert(InterruptLineId::new(5), InterruptSourceId::new(11), 25)
        .unwrap();
    assert!(controller.pending().is_empty());
    assert_eq!(
        controller.history(),
        &[
            InterruptEvent::new(
                20,
                InterruptLineId::new(5),
                InterruptTargetId::new(2),
                InterruptSourceId::new(11),
                InterruptEventKind::Assert,
            ),
            InterruptEvent::new(
                25,
                InterruptLineId::new(5),
                InterruptTargetId::new(2),
                InterruptSourceId::new(11),
                InterruptEventKind::Deassert,
            ),
        ]
    );
}

#[test]
fn interrupt_controller_rejects_duplicate_unknown_and_replayed_lines() {
    let mut controller = InterruptController::new();
    controller
        .register_line(InterruptLineId::new(4), InterruptTargetId::new(0))
        .unwrap();

    assert_eq!(
        controller.register_line(InterruptLineId::new(4), InterruptTargetId::new(1)),
        Err(InterruptError::DuplicateLine {
            line: InterruptLineId::new(4),
        })
    );
    assert_eq!(
        controller.assert(InterruptLineId::new(9), InterruptSourceId::new(1), 30),
        Err(InterruptError::UnknownLine {
            line: InterruptLineId::new(9),
        })
    );
    assert_eq!(
        controller.deassert(InterruptLineId::new(9), InterruptSourceId::new(1), 30),
        Err(InterruptError::UnknownLine {
            line: InterruptLineId::new(9),
        })
    );
    assert_eq!(
        controller.set_priority(InterruptLineId::new(9), InterruptPriority::new(2)),
        Err(InterruptError::UnknownLine {
            line: InterruptLineId::new(9),
        })
    );

    controller
        .assert(InterruptLineId::new(4), InterruptSourceId::new(2), 31)
        .unwrap();
    assert_eq!(
        controller.assert(InterruptLineId::new(4), InterruptSourceId::new(2), 32),
        Err(InterruptError::AlreadyPending {
            line: InterruptLineId::new(4),
            source: InterruptSourceId::new(2),
        })
    );
}

#[test]
fn interrupt_controller_records_explicit_target_partition_routes() {
    let mut controller = InterruptController::new();
    let route = InterruptRoute::new(
        InterruptLineId::new(8),
        InterruptTargetId::new(1),
        PartitionId::new(6),
    );

    controller.register_route(route).unwrap();
    assert_eq!(controller.route(InterruptLineId::new(8)), Some(&route));

    controller
        .assert(InterruptLineId::new(8), InterruptSourceId::new(3), 40)
        .unwrap();
    assert_eq!(
        controller.pending(),
        vec![PendingInterrupt::routed(
            InterruptLineId::new(8),
            InterruptTargetId::new(1),
            PartitionId::new(6),
            InterruptSourceId::new(3),
            40,
        )]
    );
    assert_eq!(
        controller.history(),
        &[InterruptEvent::routed(
            40,
            InterruptLineId::new(8),
            InterruptTargetId::new(1),
            PartitionId::new(6),
            InterruptSourceId::new(3),
            InterruptEventKind::Assert,
        )]
    );
}

#[test]
fn interrupt_controller_claims_highest_priority_line_and_masks_zero_priority() {
    let target = InterruptTargetId::new(0);
    let cpu = PartitionId::new(3);
    let masked_line = InterruptLineId::new(1);
    let low_line = InterruptLineId::new(3);
    let high_line = InterruptLineId::new(8);
    let masked_source = InterruptSourceId::new(31);
    let low_source = InterruptSourceId::new(32);
    let high_source = InterruptSourceId::new(33);
    let mut controller = InterruptController::new();

    for line in [masked_line, low_line, high_line] {
        controller
            .register_route(InterruptRoute::new(line, target, cpu))
            .unwrap();
    }
    assert_eq!(
        controller.priority(low_line).unwrap(),
        InterruptPriority::DEFAULT
    );

    controller
        .set_priority(masked_line, InterruptPriority::ZERO)
        .unwrap();
    controller
        .set_priority(low_line, InterruptPriority::new(2))
        .unwrap();
    controller
        .set_priority(high_line, InterruptPriority::new(7))
        .unwrap();
    controller.assert(masked_line, masked_source, 1).unwrap();
    controller.assert(low_line, low_source, 2).unwrap();
    controller.assert(high_line, high_source, 3).unwrap();

    assert_eq!(
        controller.claim(target, cpu, 10),
        Some(InterruptClaim::new(
            high_line,
            target,
            cpu,
            high_source,
            3,
            10
        ))
    );
    controller.complete(target, cpu, high_line, 11).unwrap();
    assert_eq!(
        controller.claim(target, cpu, 12),
        Some(InterruptClaim::new(
            low_line, target, cpu, low_source, 2, 12
        ))
    );
    controller.complete(target, cpu, low_line, 13).unwrap();
    assert_eq!(controller.claim(target, cpu, 14), None);

    assert_eq!(
        controller.pending(),
        vec![PendingInterrupt::routed(
            masked_line,
            target,
            cpu,
            masked_source,
            1,
        )]
    );

    controller
        .set_priority(masked_line, InterruptPriority::new(9))
        .unwrap();
    assert_eq!(
        controller.claim(target, cpu, 15),
        Some(InterruptClaim::new(
            masked_line,
            target,
            cpu,
            masked_source,
            1,
            15,
        ))
    );
}

#[test]
fn interrupt_controller_snapshot_restore_reinstates_routed_controller_state() {
    let target = InterruptTargetId::new(0);
    let cpu = PartitionId::new(3);
    let claimed_line = InterruptLineId::new(16);
    let masked_line = InterruptLineId::new(17);
    let extra_line = InterruptLineId::new(18);
    let claimed_source = InterruptSourceId::new(41);
    let masked_source = InterruptSourceId::new(42);
    let claimed_route = InterruptRoute::new(claimed_line, target, cpu);
    let masked_route = InterruptRoute::new(masked_line, target, cpu);
    let claimed = InterruptClaim::new(claimed_line, target, cpu, claimed_source, 4, 12);
    let mut controller = InterruptController::new();

    controller.register_route(masked_route).unwrap();
    controller.register_route(claimed_route).unwrap();
    controller
        .set_priority(masked_line, InterruptPriority::ZERO)
        .unwrap();
    controller
        .set_priority(claimed_line, InterruptPriority::new(9))
        .unwrap();
    controller.assert(masked_line, masked_source, 3).unwrap();
    controller.assert(claimed_line, claimed_source, 4).unwrap();
    assert_eq!(controller.claim(target, cpu, 12), Some(claimed));

    let captured = controller.snapshot(14);
    assert_eq!(captured.tick(), 14);
    assert_eq!(captured.routes(), &[claimed_route, masked_route]);
    assert_eq!(
        captured.priorities(),
        &[
            (claimed_line, InterruptPriority::new(9)),
            (masked_line, InterruptPriority::ZERO),
        ]
    );
    assert_eq!(captured.claimed(), &[claimed]);
    assert_eq!(
        captured.pending(),
        &[PendingInterrupt::routed(
            masked_line,
            target,
            cpu,
            masked_source,
            3,
        )]
    );

    controller.complete(target, cpu, claimed_line, 15).unwrap();
    controller
        .set_priority(masked_line, InterruptPriority::new(5))
        .unwrap();
    controller
        .register_route(InterruptRoute::new(extra_line, target, cpu))
        .unwrap();
    controller
        .assert(extra_line, InterruptSourceId::new(43), 16)
        .unwrap();

    controller.restore(&captured);

    assert_eq!(controller.snapshot(14), captured);
    assert_eq!(
        controller.priority(masked_line).unwrap(),
        InterruptPriority::ZERO
    );
    assert_eq!(
        controller.priority(claimed_line).unwrap(),
        InterruptPriority::new(9)
    );
    assert_eq!(
        controller.priority(extra_line),
        Err(InterruptError::UnknownLine { line: extra_line })
    );
    assert_eq!(controller.claim(target, cpu, 20), Some(claimed));
    controller.complete(target, cpu, claimed_line, 21).unwrap();
    assert_eq!(controller.claim(target, cpu, 22), None);
}

#[test]
fn interrupt_line_channel_routes_signals_to_target_partition() {
    let device = PartitionId::new(0);
    let cpu = PartitionId::new(1);
    let route = InterruptRoute::new(InterruptLineId::new(10), InterruptTargetId::new(2), cpu);
    let channel = InterruptLineChannel::new(route, 3).unwrap();
    let delivered = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    let assert_log = Arc::clone(&delivered);
    let deassert_log = Arc::clone(&delivered);
    scheduler
        .schedule_at(device, 5, move |context| {
            channel
                .assert(context, InterruptSourceId::new(4), move |delivery| {
                    assert_log.lock().unwrap().push(delivery);
                })
                .unwrap();
            channel
                .deassert(context, InterruptSourceId::new(4), move |delivery| {
                    deassert_log.lock().unwrap().push(delivery);
                })
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 3);
    assert_eq!(summary.final_tick(), 8);
    assert_eq!(
        delivered.lock().unwrap().as_slice(),
        &[
            InterruptDelivery::new(
                8,
                device,
                route,
                InterruptSourceId::new(4),
                InterruptEventKind::Assert,
            ),
            InterruptDelivery::new(
                8,
                device,
                route,
                InterruptSourceId::new(4),
                InterruptEventKind::Deassert,
            ),
        ]
    );
}

#[test]
fn interrupt_line_channel_rejects_zero_signal_latency() {
    assert_eq!(
        InterruptLineChannel::new(
            InterruptRoute::new(
                InterruptLineId::new(13),
                InterruptTargetId::new(0),
                PartitionId::new(1),
            ),
            0,
        ),
        Err(InterruptError::ZeroSignalLatency)
    );
}

#[test]
fn interrupt_line_port_delivers_signals_into_controller() {
    let device = PartitionId::new(0);
    let cpu = PartitionId::new(1);
    let route = InterruptRoute::new(InterruptLineId::new(11), InterruptTargetId::new(3), cpu);
    let mut controller = InterruptController::new();
    controller.register_route(route).unwrap();
    let controller = Arc::new(Mutex::new(controller));
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    let errors = port.delivery_errors();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(device, 9, move |context| {
            port.assert(context, InterruptSourceId::new(12)).unwrap();
            port.deassert(context, InterruptSourceId::new(12)).unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 3);
    assert_eq!(summary.final_tick(), 11);
    assert!(errors.lock().unwrap().is_empty());
    let controller = controller.lock().unwrap();
    assert!(controller.pending().is_empty());
    assert_eq!(
        controller.history(),
        &[
            InterruptEvent::routed(
                11,
                InterruptLineId::new(11),
                InterruptTargetId::new(3),
                cpu,
                InterruptSourceId::new(12),
                InterruptEventKind::Assert,
            ),
            InterruptEvent::routed(
                11,
                InterruptLineId::new(11),
                InterruptTargetId::new(3),
                cpu,
                InterruptSourceId::new(12),
                InterruptEventKind::Deassert,
            ),
        ]
    );
}

#[test]
fn interrupt_line_port_delivers_signals_into_controller_on_parallel_scheduler() {
    let device = PartitionId::new(0);
    let cpu = PartitionId::new(1);
    let route = InterruptRoute::new(InterruptLineId::new(14), InterruptTargetId::new(3), cpu);
    let mut controller = InterruptController::new();
    controller.register_route(route).unwrap();
    let controller = Arc::new(Mutex::new(controller));
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    let errors = port.delivery_errors();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    scheduler
        .schedule_parallel_at(device, 9, move |context| {
            port.assert_parallel(context, InterruptSourceId::new(12))
                .unwrap();
            port.deassert_parallel(context, InterruptSourceId::new(12))
                .unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 3);
    assert!(summary.final_tick() >= 11);
    assert!(errors.lock().unwrap().is_empty());
    let controller = controller.lock().unwrap();
    assert!(controller.pending().is_empty());
    assert_eq!(
        controller.history(),
        &[
            InterruptEvent::routed(
                11,
                InterruptLineId::new(14),
                InterruptTargetId::new(3),
                cpu,
                InterruptSourceId::new(12),
                InterruptEventKind::Assert,
            ),
            InterruptEvent::routed(
                11,
                InterruptLineId::new(14),
                InterruptTargetId::new(3),
                cpu,
                InterruptSourceId::new(12),
                InterruptEventKind::Deassert,
            ),
        ]
    );
}

#[test]
fn interrupt_line_port_records_delivery_errors() {
    let device = PartitionId::new(0);
    let cpu = PartitionId::new(1);
    let route = InterruptRoute::new(InterruptLineId::new(12), InterruptTargetId::new(4), cpu);
    let mut controller = InterruptController::new();
    controller
        .register_route(InterruptRoute::new(
            InterruptLineId::new(12),
            InterruptTargetId::new(5),
            cpu,
        ))
        .unwrap();
    let controller = Arc::new(Mutex::new(controller));
    let port = InterruptLinePort::new(
        InterruptLineChannel::new(route, 2).unwrap(),
        Arc::clone(&controller),
    );
    let errors = port.delivery_errors();
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    scheduler
        .schedule_at(device, 4, move |context| {
            port.assert(context, InterruptSourceId::new(6)).unwrap();
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 2);
    assert_eq!(summary.final_tick(), 6);
    assert_eq!(
        errors.lock().unwrap().as_slice(),
        &[InterruptError::RouteMismatch {
            line: InterruptLineId::new(12),
            expected: InterruptRoute::new(InterruptLineId::new(12), InterruptTargetId::new(5), cpu),
            actual: route,
        }]
    );
    let controller = controller.lock().unwrap();
    assert!(controller.pending().is_empty());
    assert!(controller.history().is_empty());
}

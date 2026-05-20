use rem6_interrupt::{
    InterruptController, InterruptError, InterruptEvent, InterruptEventKind, InterruptLineId,
    InterruptRoute, InterruptSourceId, InterruptTargetId, PendingInterrupt,
};
use rem6_kernel::PartitionId;

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

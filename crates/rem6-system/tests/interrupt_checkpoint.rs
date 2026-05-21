use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_interrupt::{
    InterruptClaim, InterruptController, InterruptLineId, InterruptPriority, InterruptRoute,
    InterruptSourceId, InterruptTargetId, PendingInterrupt,
};
use rem6_kernel::PartitionId;
use rem6_system::{InterruptControllerCheckpointPort, InterruptControllerCheckpointRecord};

#[test]
fn interrupt_controller_checkpoint_captures_and_restores_controller_state() {
    let target = InterruptTargetId::new(0);
    let cpu = PartitionId::new(2);
    let claimed_line = InterruptLineId::new(4);
    let pending_line = InterruptLineId::new(5);
    let extra_line = InterruptLineId::new(6);
    let claimed_source = InterruptSourceId::new(31);
    let pending_source = InterruptSourceId::new(32);
    let claimed = InterruptClaim::new(claimed_line, target, cpu, claimed_source, 7, 11);
    let mut controller = InterruptController::new();
    controller
        .register_route(InterruptRoute::new(claimed_line, target, cpu))
        .unwrap();
    controller
        .register_route(InterruptRoute::new(pending_line, target, cpu))
        .unwrap();
    controller
        .set_priority(claimed_line, InterruptPriority::new(8))
        .unwrap();
    controller
        .set_priority(pending_line, InterruptPriority::ZERO)
        .unwrap();
    controller.assert(pending_line, pending_source, 5).unwrap();
    controller.assert(claimed_line, claimed_source, 7).unwrap();
    assert_eq!(controller.claim(target, cpu, 11), Some(claimed));
    let controller = Arc::new(Mutex::new(controller));
    let component = CheckpointComponentId::new("interrupt0").unwrap();
    let port = InterruptControllerCheckpointPort::new(component.clone(), Arc::clone(&controller));
    let mut registry = CheckpointRegistry::new();

    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry, 12).unwrap();

    assert_eq!(
        captured,
        InterruptControllerCheckpointRecord::new(
            component.clone(),
            controller.lock().unwrap().snapshot(12),
        )
    );
    assert!(registry.chunk(&component, "interrupt").unwrap().len() > 96);

    {
        let mut controller = controller.lock().unwrap();
        controller.complete(target, cpu, claimed_line, 13).unwrap();
        controller
            .set_priority(pending_line, InterruptPriority::new(9))
            .unwrap();
        controller
            .register_route(InterruptRoute::new(extra_line, target, cpu))
            .unwrap();
        controller
            .assert(extra_line, InterruptSourceId::new(33), 14)
            .unwrap();
    }

    let restored = port.restore_from(&registry).unwrap();

    assert_eq!(restored, captured);
    let controller = controller.lock().unwrap();
    assert_eq!(controller.snapshot(12), captured.snapshot().clone());
    assert_eq!(
        controller.pending(),
        vec![PendingInterrupt::routed(
            pending_line,
            target,
            cpu,
            pending_source,
            5,
        )]
    );
    assert_eq!(controller.claimed(), vec![claimed]);
    assert_eq!(
        controller.priority(pending_line).unwrap(),
        InterruptPriority::ZERO
    );
    assert_eq!(
        controller.priority(extra_line),
        Err(rem6_interrupt::InterruptError::UnknownLine { line: extra_line })
    );
}

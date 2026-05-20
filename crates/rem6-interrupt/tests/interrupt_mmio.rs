use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptClaim, InterruptController, InterruptControllerMmioDevice, InterruptEvent,
    InterruptEventKind, InterruptLineId, InterruptPriority, InterruptRoute, InterruptSourceId,
    InterruptTargetId, INTERRUPT_MMIO_CLAIM_COMPLETE_OFFSET, INTERRUPT_MMIO_PENDING_OFFSET,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{MmioBus, MmioCompletion, MmioRequest, MmioRequestId, MmioResponse, MmioRoute};

fn full_mask(bytes: u64) -> ByteMask {
    ByteMask::full(AccessSize::new(bytes).unwrap()).unwrap()
}

fn le64(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

#[test]
fn interrupt_mmio_claims_and_completes_pending_lines() {
    let cpu = PartitionId::new(0);
    let controller_partition = PartitionId::new(1);
    let target = InterruptTargetId::new(0);
    let line_a = InterruptLineId::new(3);
    let line_b = InterruptLineId::new(5);
    let source_a = InterruptSourceId::new(30);
    let source_b = InterruptSourceId::new(50);
    let base = Address::new(0x1000);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    {
        let mut controller = controller.lock().unwrap();
        controller
            .register_route(InterruptRoute::new(line_a, target, cpu))
            .unwrap();
        controller
            .register_route(InterruptRoute::new(line_b, target, cpu))
            .unwrap();
        controller.assert(line_b, source_b, 0).unwrap();
        controller.assert(line_a, source_a, 0).unwrap();
    }

    let device = InterruptControllerMmioDevice::new(Arc::clone(&controller), base, target, cpu);
    let route = MmioRoute::new(cpu, controller_partition, 2, 1).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        AddressRange::new(base, AccessSize::new(0x100).unwrap()).unwrap(),
        route,
        device,
    )
    .unwrap();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 1, move |context| {
            for (id, offset, operation) in [
                (1, INTERRUPT_MMIO_PENDING_OFFSET, None),
                (2, INTERRUPT_MMIO_CLAIM_COMPLETE_OFFSET, None),
                (3, INTERRUPT_MMIO_CLAIM_COMPLETE_OFFSET, None),
                (4, INTERRUPT_MMIO_CLAIM_COMPLETE_OFFSET, Some(line_a.get())),
                (5, INTERRUPT_MMIO_CLAIM_COMPLETE_OFFSET, None),
            ] {
                let sink = Arc::clone(&completed);
                let request = match operation {
                    Some(line) => MmioRequest::write(
                        MmioRequestId::new(id),
                        Address::new(base.get() + offset),
                        le64(line),
                        full_mask(8),
                    )
                    .unwrap(),
                    None => MmioRequest::read(
                        MmioRequestId::new(id),
                        Address::new(base.get() + offset),
                        AccessSize::new(8).unwrap(),
                    )
                    .unwrap(),
                };
                bus.submit(context, request, move |completion| {
                    sink.lock().unwrap().push(completion);
                })
                .unwrap();
            }
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.executed_events(), 11);
    assert_eq!(summary.final_tick(), 4);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(1),
                    Some(le64(line_a.get())),
                )),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(2),
                    Some(le64(line_a.get())),
                )),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(3),
                    Some(le64(line_a.get())),
                )),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(4), None)),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(5),
                    Some(le64(line_b.get())),
                )),
            ),
        ]
    );

    let controller = controller.lock().unwrap();
    assert!(controller.pending().is_empty());
    assert_eq!(
        controller.claimed(),
        vec![InterruptClaim::new(line_b, target, cpu, source_b, 0, 3)]
    );
    assert_eq!(
        controller.history(),
        &[
            InterruptEvent::routed(0, line_b, target, cpu, source_b, InterruptEventKind::Assert),
            InterruptEvent::routed(0, line_a, target, cpu, source_a, InterruptEventKind::Assert),
            InterruptEvent::routed(3, line_a, target, cpu, source_a, InterruptEventKind::Claim),
            InterruptEvent::routed(
                3,
                line_a,
                target,
                cpu,
                source_a,
                InterruptEventKind::Complete,
            ),
            InterruptEvent::routed(3, line_b, target, cpu, source_b, InterruptEventKind::Claim),
        ]
    );
}

#[test]
fn interrupt_parallel_mmio_claims_and_completes_pending_lines() {
    let cpu = PartitionId::new(0);
    let controller_partition = PartitionId::new(1);
    let target = InterruptTargetId::new(0);
    let line_a = InterruptLineId::new(7);
    let line_b = InterruptLineId::new(9);
    let source_a = InterruptSourceId::new(70);
    let source_b = InterruptSourceId::new(90);
    let base = Address::new(0x2000);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    {
        let mut controller = controller.lock().unwrap();
        controller
            .register_route(InterruptRoute::new(line_a, target, cpu))
            .unwrap();
        controller
            .register_route(InterruptRoute::new(line_b, target, cpu))
            .unwrap();
        controller
            .set_priority(line_b, InterruptPriority::new(5))
            .unwrap();
        controller.assert(line_b, source_b, 0).unwrap();
        controller.assert(line_a, source_a, 0).unwrap();
    }

    let device = InterruptControllerMmioDevice::new(Arc::clone(&controller), base, target, cpu);
    let route = MmioRoute::new(cpu, controller_partition, 2, 2).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        AddressRange::new(base, AccessSize::new(0x100).unwrap()).unwrap(),
        route,
        device,
    )
    .unwrap();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 1, move |context| {
            for (id, offset, operation) in [
                (11, INTERRUPT_MMIO_PENDING_OFFSET, None),
                (12, INTERRUPT_MMIO_CLAIM_COMPLETE_OFFSET, None),
                (13, INTERRUPT_MMIO_CLAIM_COMPLETE_OFFSET, Some(line_b.get())),
                (14, INTERRUPT_MMIO_CLAIM_COMPLETE_OFFSET, None),
            ] {
                let sink = Arc::clone(&completed);
                let request = match operation {
                    Some(line) => MmioRequest::write(
                        MmioRequestId::new(id),
                        Address::new(base.get() + offset),
                        le64(line),
                        full_mask(8),
                    )
                    .unwrap(),
                    None => MmioRequest::read(
                        MmioRequestId::new(id),
                        Address::new(base.get() + offset),
                        AccessSize::new(8).unwrap(),
                    )
                    .unwrap(),
                };
                bus.submit_parallel(context, request, move |completion| {
                    sink.lock().unwrap().push(completion);
                })
                .unwrap();
            }
        })
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 9);
    assert!(summary.final_tick() >= 5);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[
            MmioCompletion::new(
                5,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(11),
                    Some(le64(line_b.get())),
                )),
            ),
            MmioCompletion::new(
                5,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(12),
                    Some(le64(line_b.get())),
                )),
            ),
            MmioCompletion::new(
                5,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(13), None)),
            ),
            MmioCompletion::new(
                5,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(14),
                    Some(le64(line_a.get())),
                )),
            ),
        ]
    );

    let controller = controller.lock().unwrap();
    assert!(controller.pending().is_empty());
    assert_eq!(
        controller.claimed(),
        vec![InterruptClaim::new(line_a, target, cpu, source_a, 0, 3)]
    );
    assert_eq!(
        controller.history(),
        &[
            InterruptEvent::routed(0, line_b, target, cpu, source_b, InterruptEventKind::Assert),
            InterruptEvent::routed(0, line_a, target, cpu, source_a, InterruptEventKind::Assert),
            InterruptEvent::routed(3, line_b, target, cpu, source_b, InterruptEventKind::Claim,),
            InterruptEvent::routed(
                3,
                line_b,
                target,
                cpu,
                source_b,
                InterruptEventKind::Complete,
            ),
            InterruptEvent::routed(3, line_a, target, cpu, source_a, InterruptEventKind::Claim),
        ]
    );
}

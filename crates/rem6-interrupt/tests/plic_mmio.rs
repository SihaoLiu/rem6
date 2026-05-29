use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptClaim, InterruptController, InterruptError, InterruptEvent, InterruptEventKind,
    InterruptLineId, InterruptRoute, InterruptSourceId, InterruptTargetId, PlicMmioDevice,
    PLIC_MMIO_CLAIM_COMPLETE_OFFSET, PLIC_MMIO_CONTEXT_BASE_OFFSET, PLIC_MMIO_ENABLE_BASE_OFFSET,
    PLIC_MMIO_PENDING_BASE_OFFSET, PLIC_MMIO_PRIORITY_STRIDE, PLIC_MMIO_REGISTER_BYTES,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{
    MmioBus, MmioCompletion, MmioError, MmioOperation, MmioRequest, MmioRequestId, MmioResponse,
    MmioRoute,
};

fn full_mask(bytes: u64) -> ByteMask {
    ByteMask::full(AccessSize::new(bytes).unwrap()).unwrap()
}

fn le32(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn read(id: u64, base: Address, offset: u64) -> MmioRequest {
    MmioRequest::read(
        MmioRequestId::new(id),
        Address::new(base.get() + offset),
        AccessSize::new(PLIC_MMIO_REGISTER_BYTES).unwrap(),
    )
    .unwrap()
}

fn write(id: u64, base: Address, offset: u64, value: u32) -> MmioRequest {
    MmioRequest::write(
        MmioRequestId::new(id),
        Address::new(base.get() + offset),
        le32(value),
        full_mask(PLIC_MMIO_REGISTER_BYTES),
    )
    .unwrap()
}

fn plic_bus(
    controller: Arc<Mutex<InterruptController>>,
    base: Address,
    target: InterruptTargetId,
    cpu: PartitionId,
    plic_partition: PartitionId,
    response_latency: u64,
) -> (MmioBus, MmioRoute) {
    let route = MmioRoute::new(cpu, plic_partition, 2, response_latency).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        AddressRange::new(base, AccessSize::new(0x210000).unwrap()).unwrap(),
        route,
        PlicMmioDevice::new(controller, base, target, cpu),
    )
    .unwrap();
    (bus, route)
}

#[test]
fn plic_mmio_requires_enable_and_threshold_before_claim() {
    let cpu = PartitionId::new(0);
    let plic_partition = PartitionId::new(1);
    let target = InterruptTargetId::new(0);
    let base = Address::new(0x0c00_0000);
    let line_low = InterruptLineId::new(1);
    let line_high = InterruptLineId::new(2);
    let source_low = InterruptSourceId::new(11);
    let source_high = InterruptSourceId::new(12);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    {
        let mut controller = controller.lock().unwrap();
        controller
            .register_route(InterruptRoute::new(line_low, target, cpu))
            .unwrap();
        controller
            .register_route(InterruptRoute::new(line_high, target, cpu))
            .unwrap();
        controller.assert(line_low, source_low, 0).unwrap();
        controller.assert(line_high, source_high, 0).unwrap();
    }
    let (bus, route) = plic_bus(
        Arc::clone(&controller),
        base,
        target,
        cpu,
        plic_partition,
        1,
    );
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 1, move |context| {
            for request in [
                write(1, base, line_low.get() * PLIC_MMIO_PRIORITY_STRIDE, 3),
                write(2, base, line_high.get() * PLIC_MMIO_PRIORITY_STRIDE, 5),
                read(3, base, PLIC_MMIO_PENDING_BASE_OFFSET),
                read(
                    4,
                    base,
                    PLIC_MMIO_CONTEXT_BASE_OFFSET + PLIC_MMIO_CLAIM_COMPLETE_OFFSET,
                ),
                write(5, base, PLIC_MMIO_CONTEXT_BASE_OFFSET, 4),
                write(6, base, PLIC_MMIO_ENABLE_BASE_OFFSET, 0b110),
                read(
                    7,
                    base,
                    PLIC_MMIO_CONTEXT_BASE_OFFSET + PLIC_MMIO_CLAIM_COMPLETE_OFFSET,
                ),
                write(
                    8,
                    base,
                    PLIC_MMIO_CONTEXT_BASE_OFFSET + PLIC_MMIO_CLAIM_COMPLETE_OFFSET,
                    line_high.get() as u32,
                ),
                write(9, base, PLIC_MMIO_CONTEXT_BASE_OFFSET, 2),
                read(
                    10,
                    base,
                    PLIC_MMIO_CONTEXT_BASE_OFFSET + PLIC_MMIO_CLAIM_COMPLETE_OFFSET,
                ),
            ] {
                let sink = Arc::clone(&completed);
                bus.submit(context, request, move |completion| {
                    sink.lock().unwrap().push(completion);
                })
                .unwrap();
            }
        })
        .unwrap();

    let summary = scheduler.run_until_idle();

    assert_eq!(summary.final_tick(), 4);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(1), None)),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(2), None)),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(3),
                    Some(le32(0b110)),
                )),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(4),
                    Some(le32(0))
                )),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(5), None)),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(6), None)),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(7),
                    Some(le32(line_high.get() as u32)),
                )),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(8), None)),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(9), None)),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(10),
                    Some(le32(line_low.get() as u32)),
                )),
            ),
        ]
    );

    let controller = controller.lock().unwrap();
    assert!(controller.pending().is_empty());
    assert_eq!(
        controller.claimed(),
        vec![InterruptClaim::new(line_low, target, cpu, source_low, 0, 3)]
    );
    assert_eq!(
        controller.history(),
        &[
            InterruptEvent::routed(
                0,
                line_low,
                target,
                cpu,
                source_low,
                InterruptEventKind::Assert
            ),
            InterruptEvent::routed(
                0,
                line_high,
                target,
                cpu,
                source_high,
                InterruptEventKind::Assert,
            ),
            InterruptEvent::routed(
                3,
                line_high,
                target,
                cpu,
                source_high,
                InterruptEventKind::Claim,
            ),
            InterruptEvent::routed(
                3,
                line_high,
                target,
                cpu,
                source_high,
                InterruptEventKind::Complete,
            ),
            InterruptEvent::routed(
                3,
                line_low,
                target,
                cpu,
                source_low,
                InterruptEventKind::Claim
            ),
        ]
    );
}

#[test]
fn plic_parallel_claim_repeats_until_matching_completion() {
    let cpu = PartitionId::new(0);
    let plic_partition = PartitionId::new(1);
    let target = InterruptTargetId::new(0);
    let base = Address::new(0x0c20_0000);
    let line = InterruptLineId::new(5);
    let wrong_line = InterruptLineId::new(6);
    let source = InterruptSourceId::new(25);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    {
        let mut controller = controller.lock().unwrap();
        controller
            .register_route(InterruptRoute::new(line, target, cpu))
            .unwrap();
        controller.assert(line, source, 0).unwrap();
    }
    let (bus, route) = plic_bus(
        Arc::clone(&controller),
        base,
        target,
        cpu,
        plic_partition,
        2,
    );
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 1, move |context| {
            for request in [
                write(21, base, line.get() * PLIC_MMIO_PRIORITY_STRIDE, 7),
                write(22, base, PLIC_MMIO_ENABLE_BASE_OFFSET, 1 << line.get()),
                read(
                    23,
                    base,
                    PLIC_MMIO_CONTEXT_BASE_OFFSET + PLIC_MMIO_CLAIM_COMPLETE_OFFSET,
                ),
                read(
                    24,
                    base,
                    PLIC_MMIO_CONTEXT_BASE_OFFSET + PLIC_MMIO_CLAIM_COMPLETE_OFFSET,
                ),
                write(
                    25,
                    base,
                    PLIC_MMIO_CONTEXT_BASE_OFFSET + PLIC_MMIO_CLAIM_COMPLETE_OFFSET,
                    wrong_line.get() as u32,
                ),
                read(
                    26,
                    base,
                    PLIC_MMIO_CONTEXT_BASE_OFFSET + PLIC_MMIO_CLAIM_COMPLETE_OFFSET,
                ),
            ] {
                let sink = Arc::clone(&completed);
                bus.submit_parallel(context, request, move |completion| {
                    sink.lock().unwrap().push(completion);
                })
                .unwrap();
            }
        })
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert!(summary.final_tick() >= 5);
    assert_eq!(
        completions.lock().unwrap().as_slice(),
        &[
            MmioCompletion::new(
                5,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(21), None)),
            ),
            MmioCompletion::new(
                5,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(22), None)),
            ),
            MmioCompletion::new(
                5,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(23),
                    Some(le32(line.get() as u32)),
                )),
            ),
            MmioCompletion::new(
                5,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(24),
                    Some(le32(line.get() as u32)),
                )),
            ),
            MmioCompletion::new(
                5,
                route,
                Err(MmioError::DeviceError {
                    request: MmioRequestId::new(25),
                    message: InterruptError::ClaimMismatch {
                        target,
                        target_partition: cpu,
                        expected: line,
                        actual: wrong_line,
                    }
                    .to_string(),
                }),
            ),
            MmioCompletion::new(
                5,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(26),
                    Some(le32(line.get() as u32)),
                )),
            ),
        ]
    );

    let controller = controller.lock().unwrap();
    assert!(controller.pending().is_empty());
    assert_eq!(
        controller.claimed(),
        vec![InterruptClaim::new(line, target, cpu, source, 0, 3)]
    );
    assert_eq!(
        controller.history(),
        &[
            InterruptEvent::routed(0, line, target, cpu, source, InterruptEventKind::Assert),
            InterruptEvent::routed(3, line, target, cpu, source, InterruptEventKind::Claim),
        ]
    );
}

#[test]
fn plic_rejects_bad_width_and_pending_writes() {
    let cpu = PartitionId::new(0);
    let target = InterruptTargetId::new(0);
    let base = Address::new(0x0c40_0000);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let device = PlicMmioDevice::new(controller, base, target, cpu);
    let mut scheduler = PartitionedScheduler::new(1).unwrap();

    scheduler
        .schedule_at(cpu, 1, move |context| {
            assert_eq!(
                device
                    .respond(
                        context,
                        &MmioRequest::read(
                            MmioRequestId::new(31),
                            base,
                            AccessSize::new(8).unwrap(),
                        )
                        .unwrap(),
                    )
                    .unwrap_err(),
                MmioError::AccessSizeMismatch {
                    request: MmioRequestId::new(31),
                    expected: PLIC_MMIO_REGISTER_BYTES,
                    actual: 8,
                },
            );
            assert_eq!(
                device
                    .respond(
                        context,
                        &MmioRequest::write(
                            MmioRequestId::new(32),
                            Address::new(base.get() + PLIC_MMIO_PENDING_BASE_OFFSET),
                            le32(1),
                            full_mask(PLIC_MMIO_REGISTER_BYTES),
                        )
                        .unwrap(),
                    )
                    .unwrap_err(),
                MmioError::AccessDenied {
                    request: MmioRequestId::new(32),
                    operation: MmioOperation::Write,
                    access: rem6_mmio::MmioAccess::ReadOnly,
                },
            );
        })
        .unwrap();

    scheduler.run_until_idle();
}

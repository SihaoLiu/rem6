use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptClaim, InterruptController, InterruptError, InterruptEvent, InterruptEventKind,
    InterruptLineId, InterruptRoute, InterruptSourceId, InterruptTargetId, PlicContextRoute,
    PlicContextSnapshot, PlicMmioDevice, PlicSnapshot, PLIC_MMIO_CLAIM_COMPLETE_OFFSET,
    PLIC_MMIO_CONTEXT_BASE_OFFSET, PLIC_MMIO_CONTEXT_STRIDE, PLIC_MMIO_ENABLE_BASE_OFFSET,
    PLIC_MMIO_ENABLE_CONTEXT_STRIDE, PLIC_MMIO_PENDING_BASE_OFFSET, PLIC_MMIO_PRIORITY_STRIDE,
    PLIC_MMIO_REGISTER_BYTES,
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

fn plic_bus_with_contexts(
    controller: Arc<Mutex<InterruptController>>,
    base: Address,
    contexts: impl IntoIterator<Item = PlicContextRoute>,
    cpu: PartitionId,
    plic_partition: PartitionId,
    response_latency: u64,
) -> (MmioBus, MmioRoute) {
    let route = MmioRoute::new(cpu, plic_partition, 2, response_latency).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        AddressRange::new(base, AccessSize::new(0x220000).unwrap()).unwrap(),
        route,
        PlicMmioDevice::with_contexts(controller, base, contexts),
    )
    .unwrap();
    (bus, route)
}

#[test]
fn plic_mmio_routes_enable_threshold_and_claim_by_context() {
    let cpu0 = PartitionId::new(0);
    let cpu1 = PartitionId::new(1);
    let plic_partition = PartitionId::new(2);
    let target0 = InterruptTargetId::new(0);
    let target1 = InterruptTargetId::new(1);
    let base = Address::new(0x0c60_0000);
    let line0 = InterruptLineId::new(3);
    let line1 = InterruptLineId::new(4);
    let source0 = InterruptSourceId::new(33);
    let source1 = InterruptSourceId::new(44);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    {
        let mut controller = controller.lock().unwrap();
        controller
            .register_route(InterruptRoute::new(line0, target0, cpu0))
            .unwrap();
        controller
            .register_route(InterruptRoute::new(line1, target1, cpu1))
            .unwrap();
        controller.assert(line0, source0, 0).unwrap();
        controller.assert(line1, source1, 0).unwrap();
    }
    let (bus, route) = plic_bus_with_contexts(
        Arc::clone(&controller),
        base,
        [
            PlicContextRoute::new(0, target0, cpu0),
            PlicContextRoute::new(1, target1, cpu1),
        ],
        cpu0,
        plic_partition,
        1,
    );
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::new(3).unwrap();

    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu0, 1, move |context| {
            let context1_enable = PLIC_MMIO_ENABLE_BASE_OFFSET + PLIC_MMIO_ENABLE_CONTEXT_STRIDE;
            let context1_base = PLIC_MMIO_CONTEXT_BASE_OFFSET + PLIC_MMIO_CONTEXT_STRIDE;
            for request in [
                write(41, base, line0.get() * PLIC_MMIO_PRIORITY_STRIDE, 6),
                write(42, base, line1.get() * PLIC_MMIO_PRIORITY_STRIDE, 7),
                write(43, base, PLIC_MMIO_ENABLE_BASE_OFFSET, 1 << line0.get()),
                write(44, base, PLIC_MMIO_CONTEXT_BASE_OFFSET, 5),
                write(45, base, context1_enable, 1 << line1.get()),
                write(46, base, context1_base, 6),
                read(
                    47,
                    base,
                    PLIC_MMIO_CONTEXT_BASE_OFFSET + PLIC_MMIO_CLAIM_COMPLETE_OFFSET,
                ),
                read(48, base, context1_base + PLIC_MMIO_CLAIM_COMPLETE_OFFSET),
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
                Ok(MmioResponse::completed(MmioRequestId::new(41), None)),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(42), None)),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(43), None)),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(44), None)),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(45), None)),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(MmioRequestId::new(46), None)),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(47),
                    Some(le32(line0.get() as u32)),
                )),
            ),
            MmioCompletion::new(
                4,
                route,
                Ok(MmioResponse::completed(
                    MmioRequestId::new(48),
                    Some(le32(line1.get() as u32)),
                )),
            ),
        ]
    );

    let controller = controller.lock().unwrap();
    assert!(controller.pending().is_empty());
    assert_eq!(
        controller.claimed(),
        vec![
            InterruptClaim::new(line0, target0, cpu0, source0, 0, 3),
            InterruptClaim::new(line1, target1, cpu1, source1, 0, 3),
        ]
    );
}

#[test]
fn plic_snapshot_restores_context_enable_and_threshold_state() {
    let cpu0 = PartitionId::new(0);
    let cpu1 = PartitionId::new(1);
    let target0 = InterruptTargetId::new(0);
    let target1 = InterruptTargetId::new(1);
    let base = Address::new(0x0c70_0000);
    let line0 = InterruptLineId::new(2);
    let line1 = InterruptLineId::new(35);
    let source0 = InterruptSourceId::new(52);
    let source1 = InterruptSourceId::new(53);
    let contexts = [
        PlicContextRoute::new(0, target0, cpu0),
        PlicContextRoute::new(1, target1, cpu1),
    ];
    let source = PlicMmioDevice::with_contexts(
        Arc::new(Mutex::new(InterruptController::new())),
        base,
        contexts,
    );
    let mut source_scheduler = PartitionedScheduler::new(2).unwrap();

    let source_program = source.clone();
    source_scheduler
        .schedule_at(cpu0, 1, move |context| {
            let context1_enable = PLIC_MMIO_ENABLE_BASE_OFFSET + PLIC_MMIO_ENABLE_CONTEXT_STRIDE;
            let context1_base = PLIC_MMIO_CONTEXT_BASE_OFFSET + PLIC_MMIO_CONTEXT_STRIDE;
            for request in [
                write(61, base, PLIC_MMIO_ENABLE_BASE_OFFSET, 1 << line0.get()),
                write(62, base, PLIC_MMIO_CONTEXT_BASE_OFFSET, 4),
                write(
                    63,
                    base,
                    context1_enable + (line1.get() / 32) * PLIC_MMIO_REGISTER_BYTES,
                    1 << (line1.get() % 32),
                ),
                write(64, base, context1_base, 6),
            ] {
                source_program.respond(context, &request).unwrap();
            }
        })
        .unwrap();
    source_scheduler.run_until_idle();

    let snapshot = source.snapshot();

    assert_eq!(
        snapshot,
        PlicSnapshot::new(
            base,
            vec![
                PlicContextSnapshot::new(
                    0,
                    target0,
                    cpu0,
                    vec![line0],
                    rem6_interrupt::InterruptPriority::new(4),
                ),
                PlicContextSnapshot::new(
                    1,
                    target1,
                    cpu1,
                    vec![line1],
                    rem6_interrupt::InterruptPriority::new(6),
                ),
            ],
        )
    );

    let restored_controller = Arc::new(Mutex::new(InterruptController::new()));
    {
        let mut controller = restored_controller.lock().unwrap();
        controller
            .register_route(InterruptRoute::new(line0, target0, cpu0))
            .unwrap();
        controller
            .register_route(InterruptRoute::new(line1, target1, cpu1))
            .unwrap();
        controller
            .set_priority(line0, rem6_interrupt::InterruptPriority::new(5))
            .unwrap();
        controller
            .set_priority(line1, rem6_interrupt::InterruptPriority::new(7))
            .unwrap();
        controller.assert(line0, source0, 0).unwrap();
        controller.assert(line1, source1, 0).unwrap();
    }
    let restored = PlicMmioDevice::with_contexts(Arc::clone(&restored_controller), base, contexts);
    restored.restore(&snapshot).unwrap();
    let mut restored_scheduler = PartitionedScheduler::new(2).unwrap();
    let restored_device = restored.clone();

    restored_scheduler
        .schedule_at(cpu0, 2, move |context| {
            let context1_base = PLIC_MMIO_CONTEXT_BASE_OFFSET + PLIC_MMIO_CONTEXT_STRIDE;
            let context0_claim = restored_device
                .respond(
                    context,
                    &read(
                        65,
                        base,
                        PLIC_MMIO_CONTEXT_BASE_OFFSET + PLIC_MMIO_CLAIM_COMPLETE_OFFSET,
                    ),
                )
                .unwrap();
            let context1_claim = restored_device
                .respond(
                    context,
                    &read(66, base, context1_base + PLIC_MMIO_CLAIM_COMPLETE_OFFSET),
                )
                .unwrap();

            assert_eq!(context0_claim.data(), Some(&le32(line0.get() as u32)[..]));
            assert_eq!(context1_claim.data(), Some(&le32(line1.get() as u32)[..]));
        })
        .unwrap();
    restored_scheduler.run_until_idle();

    assert_eq!(
        restored_controller.lock().unwrap().claimed(),
        vec![
            InterruptClaim::new(line0, target0, cpu0, source0, 0, 2),
            InterruptClaim::new(line1, target1, cpu1, source1, 0, 2),
        ]
    );
}

#[test]
fn plic_source_count_bounds_pending_enable_and_claim_visibility() {
    let cpu = PartitionId::new(0);
    let target = InterruptTargetId::new(0);
    let base = Address::new(0x0c75_0000);
    let valid_line = InterruptLineId::new(4);
    let hidden_line = InterruptLineId::new(5);
    let valid_source = InterruptSourceId::new(54);
    let hidden_source = InterruptSourceId::new(55);
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    {
        let mut controller = controller.lock().unwrap();
        controller
            .register_route(InterruptRoute::new(valid_line, target, cpu))
            .unwrap();
        controller
            .register_route(InterruptRoute::new(hidden_line, target, cpu))
            .unwrap();
        controller
            .set_priority(valid_line, rem6_interrupt::InterruptPriority::new(4))
            .unwrap();
        controller
            .set_priority(hidden_line, rem6_interrupt::InterruptPriority::new(9))
            .unwrap();
        controller.assert(valid_line, valid_source, 0).unwrap();
        controller.assert(hidden_line, hidden_source, 0).unwrap();
    }
    let device = PlicMmioDevice::with_source_count(Arc::clone(&controller), base, target, cpu, 4);
    let snapshot_source = device.clone();
    let mut scheduler = PartitionedScheduler::new(1).unwrap();

    scheduler
        .schedule_at(cpu, 1, move |context| {
            let pending = device
                .respond(context, &read(71, base, PLIC_MMIO_PENDING_BASE_OFFSET))
                .unwrap();
            assert_eq!(pending.data(), Some(&le32(1 << valid_line.get())[..]));

            device
                .respond(
                    context,
                    &write(
                        72,
                        base,
                        PLIC_MMIO_ENABLE_BASE_OFFSET,
                        (1 << valid_line.get()) | (1 << hidden_line.get()),
                    ),
                )
                .unwrap();
            let enable = device
                .respond(context, &read(73, base, PLIC_MMIO_ENABLE_BASE_OFFSET))
                .unwrap();
            assert_eq!(enable.data(), Some(&le32(1 << valid_line.get())[..]));

            let claim = device
                .respond(
                    context,
                    &read(
                        74,
                        base,
                        PLIC_MMIO_CONTEXT_BASE_OFFSET + PLIC_MMIO_CLAIM_COMPLETE_OFFSET,
                    ),
                )
                .unwrap();
            assert_eq!(claim.data(), Some(&le32(valid_line.get() as u32)[..]));

            assert_eq!(
                device
                    .respond(
                        context,
                        &write(75, base, hidden_line.get() * PLIC_MMIO_PRIORITY_STRIDE, 7,),
                    )
                    .unwrap_err(),
                MmioError::UnmappedAddress {
                    address: Address::new(
                        base.get() + hidden_line.get() * PLIC_MMIO_PRIORITY_STRIDE,
                    ),
                },
            );
            assert_eq!(
                device
                    .respond(
                        context,
                        &read(
                            76,
                            base,
                            PLIC_MMIO_PENDING_BASE_OFFSET + PLIC_MMIO_REGISTER_BYTES,
                        ),
                    )
                    .unwrap_err(),
                MmioError::UnmappedAddress {
                    address: Address::new(
                        base.get() + PLIC_MMIO_PENDING_BASE_OFFSET + PLIC_MMIO_REGISTER_BYTES,
                    ),
                },
            );
        })
        .unwrap();
    scheduler.run_until_idle();

    assert_eq!(
        snapshot_source.snapshot(),
        PlicSnapshot::new(
            base,
            vec![PlicContextSnapshot::new(
                0,
                target,
                cpu,
                vec![valid_line],
                rem6_interrupt::InterruptPriority::ZERO,
            )],
        )
    );
    assert_eq!(
        controller.lock().unwrap().claimed(),
        vec![InterruptClaim::new(
            valid_line,
            target,
            cpu,
            valid_source,
            0,
            1
        )]
    );
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

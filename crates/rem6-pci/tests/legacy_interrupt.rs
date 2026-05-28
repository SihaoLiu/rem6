use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptError, InterruptEvent, InterruptEventKind, InterruptLineId,
    InterruptRoute, InterruptSourceId, InterruptTargetId, PendingInterrupt,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_pci::{
    PciError, PciFunctionAddress, PciInterruptPin, PciLegacyInterruptMapper,
    PciLegacyInterruptPath, PciLegacyInterruptPolicy, PciLegacyInterruptPort,
};

fn function(device: u8) -> PciFunctionAddress {
    PciFunctionAddress::new(0, device, 0).unwrap()
}

fn mapper(policy: PciLegacyInterruptPolicy) -> PciLegacyInterruptMapper {
    PciLegacyInterruptMapper::new(InterruptLineId::new(32), 4, policy).unwrap()
}

fn controller_and_port(
    pci_function: PciFunctionAddress,
    pin: PciInterruptPin,
    target_partition: PartitionId,
    signal_latency: u64,
) -> (Arc<Mutex<InterruptController>>, PciLegacyInterruptPort) {
    let controller = Arc::new(Mutex::new(InterruptController::new()));
    let route = mapper(PciLegacyInterruptPolicy::DevicePinModulo)
        .route(
            pci_function,
            pin,
            InterruptTargetId::new(0),
            target_partition,
            signal_latency,
        )
        .unwrap();
    controller
        .lock()
        .unwrap()
        .register_route(route.interrupt_route())
        .unwrap();
    let port = PciLegacyInterruptPort::new(route, Arc::clone(&controller)).unwrap();

    assert_eq!(port.function(), pci_function);
    assert_eq!(port.pin(), pin);

    (controller, port)
}

#[test]
fn pci_legacy_interrupt_mapper_rejects_invalid_inputs_and_maps_policies() {
    let pci_function = function(31);

    assert_eq!(
        PciLegacyInterruptMapper::new(
            InterruptLineId::new(32),
            0,
            PciLegacyInterruptPolicy::DevicePinModulo,
        ),
        Err(PciError::ZeroLegacyInterruptLines)
    );
    assert_eq!(
        mapper(PciLegacyInterruptPolicy::DevicePinModulo).line(pci_function, PciInterruptPin::None),
        Err(PciError::MissingLegacyInterruptPin {
            function: pci_function,
        })
    );
    assert_eq!(
        mapper(PciLegacyInterruptPolicy::DeviceModulo).line(pci_function, PciInterruptPin::IntD),
        Ok(InterruptLineId::new(35))
    );
    assert_eq!(
        mapper(PciLegacyInterruptPolicy::PinModulo).line(pci_function, PciInterruptPin::IntD),
        Ok(InterruptLineId::new(35))
    );
    assert_eq!(
        mapper(PciLegacyInterruptPolicy::DevicePinModulo).line(function(2), PciInterruptPin::IntC),
        Ok(InterruptLineId::new(32))
    );
    assert_eq!(
        mapper(PciLegacyInterruptPolicy::DevicePinModulo).line(pci_function, PciInterruptPin::IntA),
        Ok(InterruptLineId::new(35))
    );
}

#[test]
fn pci_legacy_interrupt_mapper_swizzles_bridge_path_before_platform_line() {
    let endpoint = PciFunctionAddress::new(2, 5, 0).unwrap();
    let downstream_bridge = PciFunctionAddress::new(1, 1, 0).unwrap();
    let root_bridge = PciFunctionAddress::new(0, 1, 0).unwrap();
    let path = PciLegacyInterruptPath::new(endpoint, PciInterruptPin::IntA)
        .unwrap()
        .with_upstream_bridge(downstream_bridge)
        .with_upstream_bridge(root_bridge);

    assert_eq!(path.endpoint_function(), endpoint);
    assert_eq!(path.endpoint_pin(), PciInterruptPin::IntA);
    assert_eq!(path.root_function(), root_bridge);
    assert_eq!(path.root_pin(), PciInterruptPin::IntC);
    assert_eq!(path.upstream_bridges(), &[downstream_bridge, root_bridge]);
    assert_eq!(
        PciLegacyInterruptPath::new(endpoint, PciInterruptPin::None),
        Err(PciError::MissingLegacyInterruptPin { function: endpoint })
    );

    assert_eq!(
        mapper(PciLegacyInterruptPolicy::PinModulo).line_for_path(&path),
        Ok(InterruptLineId::new(34))
    );
    let route = mapper(PciLegacyInterruptPolicy::DevicePinModulo)
        .route_for_path(&path, InterruptTargetId::new(0), PartitionId::new(0), 4)
        .unwrap();
    assert_eq!(route.function(), endpoint);
    assert_eq!(route.pin(), PciInterruptPin::IntA);
    assert_eq!(route.line(), InterruptLineId::new(35));
    assert_eq!(route.signal_latency(), 4);
}

#[test]
fn pci_legacy_interrupt_port_posts_and_clears_serial_intx() {
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let source = InterruptSourceId::new(80);
    let (controller, port) = controller_and_port(function(5), PciInterruptPin::IntB, cpu, 2);
    assert_eq!(port.line(), InterruptLineId::new(34));
    let mut scheduler = PartitionedScheduler::new(2).unwrap();
    let post_port = port.clone();
    let clear_port = port.clone();

    scheduler
        .schedule_at(pci, 5, move |context| {
            post_port.post(context, source).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_at(pci, 10, move |context| {
            clear_port.clear(context, source).unwrap();
        })
        .unwrap();

    scheduler.run_until_idle();

    let controller = controller.lock().unwrap();
    assert_eq!(
        controller.history(),
        &[
            InterruptEvent::routed(
                7,
                InterruptLineId::new(34),
                InterruptTargetId::new(0),
                cpu,
                source,
                InterruptEventKind::Assert,
            ),
            InterruptEvent::routed(
                12,
                InterruptLineId::new(34),
                InterruptTargetId::new(0),
                cpu,
                source,
                InterruptEventKind::Deassert,
            ),
        ]
    );
    assert!(controller.pending().is_empty());
}

#[test]
fn pci_legacy_interrupt_port_posts_and_clears_parallel_intx() {
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let source = InterruptSourceId::new(81);
    let (controller, port) = controller_and_port(function(6), PciInterruptPin::IntD, cpu, 3);
    assert_eq!(
        port.interrupt_route(),
        InterruptRoute::new(InterruptLineId::new(33), InterruptTargetId::new(0), cpu)
    );
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let post_port = port.clone();
    let clear_port = port.clone();

    scheduler
        .schedule_parallel_at(pci, 4, move |context| {
            post_port.post_parallel(context, source).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(pci, 9, move |context| {
            clear_port.clear_parallel(context, source).unwrap();
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    let controller = controller.lock().unwrap();
    assert_eq!(
        controller.history(),
        &[
            InterruptEvent::routed(
                7,
                InterruptLineId::new(33),
                InterruptTargetId::new(0),
                cpu,
                source,
                InterruptEventKind::Assert,
            ),
            InterruptEvent::routed(
                12,
                InterruptLineId::new(33),
                InterruptTargetId::new(0),
                cpu,
                source,
                InterruptEventKind::Deassert,
            ),
        ]
    );
    assert!(controller.pending().is_empty());
}

#[test]
fn pci_legacy_interrupt_port_keeps_parallel_delivery_errors_observable() {
    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let asserted_source = InterruptSourceId::new(82);
    let wrong_source = InterruptSourceId::new(83);
    let (controller, port) = controller_and_port(function(7), PciInterruptPin::IntA, cpu, 2);
    let delivery_errors = port.delivery_errors();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let post_port = port.clone();
    let clear_port = port.clone();

    scheduler
        .schedule_parallel_at(pci, 4, move |context| {
            post_port.post_parallel(context, asserted_source).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(pci, 9, move |context| {
            clear_port.clear_parallel(context, wrong_source).unwrap();
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        delivery_errors.lock().unwrap().as_slice(),
        &[InterruptError::SourceMismatch {
            line: InterruptLineId::new(35),
            expected: asserted_source,
            actual: wrong_source,
        }]
    );
    assert_eq!(
        controller.lock().unwrap().pending(),
        vec![PendingInterrupt::routed(
            InterruptLineId::new(35),
            InterruptTargetId::new(0),
            cpu,
            asserted_source,
            6,
        )]
    );
}

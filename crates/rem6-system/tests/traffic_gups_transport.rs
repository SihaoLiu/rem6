use std::sync::{Arc, Mutex};

use rem6_fabric::{FabricLinkId, FabricPath, FabricPathHop};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequestId, MemoryResponse,
    ResponseStatus,
};
use rem6_system::traffic_gups_controller_transport_run;
use rem6_system::TrafficGupsTransportError;
use rem6_traffic::{
    GupsTrafficGenerator, TrafficController, TrafficControllerConfig, TrafficControllerState,
    TrafficGupsConfig, TrafficIdleConfig, TrafficIdleGenerator, TrafficStateGenerator,
    TrafficStateGraphConfig, TrafficStateId, TrafficStateSpec, TrafficTransition,
    TrafficTransitionProbability, TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryTrace, MemoryTraceKind, MemoryTransport, RequestDelivery,
    TargetOutcome, TransportEndpointId,
};
use rem6_workload::{WorkloadGupsRunSummary, WorkloadRouteId};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn workload_route(name: &str) -> WorkloadRouteId {
    WorkloadRouteId::new(name).unwrap()
}

fn fabric_path(name: &str) -> FabricPath {
    FabricPath::new([FabricPathHop::new(FabricLinkId::new(name).unwrap(), 2, 8).unwrap()]).unwrap()
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn state(id: u32, duration: u64) -> TrafficStateSpec {
    TrafficStateSpec::new(TrafficStateId::new(id), duration)
}

fn transition(from: u32, to: u32) -> TrafficTransition {
    TrafficTransition::new(
        TrafficStateId::new(from),
        TrafficStateId::new(to),
        TrafficTransitionProbability::from_micros(TRAFFIC_TRANSITION_PROBABILITY_SCALE).unwrap(),
    )
}

fn graph() -> TrafficStateGraphConfig {
    TrafficStateGraphConfig::new(
        vec![state(0, 1), state(1, u64::MAX)],
        TrafficStateId::new(0),
        vec![transition(0, 1), transition(1, 1)],
    )
    .unwrap()
}

fn gups_controller() -> TrafficController {
    let config = TrafficGupsConfig::new(AgentId::new(5), line_layout(), Address::new(0x1000), 64)
        .unwrap()
        .with_update_limit(1)
        .unwrap()
        .with_rng_state(0);
    let state = TrafficControllerState::new(
        TrafficStateId::new(0),
        TrafficStateGenerator::Gups(GupsTrafficGenerator::new(config)),
    );
    let idle = TrafficControllerState::new(
        TrafficStateId::new(1),
        TrafficStateGenerator::Idle(TrafficIdleGenerator::new(TrafficIdleConfig::new(u64::MAX))),
    );
    TrafficController::new(TrafficControllerConfig::new(graph(), vec![state, idle]).unwrap())
}

fn idle_controller() -> TrafficController {
    let idle_state = TrafficControllerState::new(
        TrafficStateId::new(0),
        TrafficStateGenerator::Idle(TrafficIdleGenerator::new(TrafficIdleConfig::new(u64::MAX))),
    );
    TrafficController::new(
        TrafficControllerConfig::new(
            TrafficStateGraphConfig::new(
                vec![state(0, u64::MAX)],
                TrafficStateId::new(0),
                vec![transition(0, 0)],
            )
            .unwrap(),
            vec![idle_state],
        )
        .unwrap(),
    )
}

#[test]
fn traffic_gups_transport_run_feeds_read_response_back_into_controller() {
    let mut controller = gups_controller();
    controller.start(0).unwrap();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();
    let core = endpoint("core0");
    let memory = endpoint("memory0");
    let route = transport
        .add_route(
            MemoryRoute::new(
                core.clone(),
                PartitionId::new(0),
                memory.clone(),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();
    let writes = Arc::new(Mutex::new(Vec::new()));
    let write_log = Arc::clone(&writes);
    let target = Arc::new(
        move |delivery: &RequestDelivery| match delivery.request().operation() {
            MemoryOperation::ReadShared => {
                let data = 0x0102_0304_0506_0708_u64.to_le_bytes().to_vec();
                TargetOutcome::Respond(
                    MemoryResponse::completed(delivery.request(), Some(data)).unwrap(),
                )
            }
            MemoryOperation::Write => {
                write_log.lock().unwrap().push((
                    delivery.tick(),
                    delivery.request().id(),
                    delivery.request().range().start(),
                    delivery.request().data().unwrap().to_vec(),
                ));
                TargetOutcome::Respond(MemoryResponse::completed(delivery.request(), None).unwrap())
            }
            operation => panic!("unexpected GUPS transport operation: {operation:?}"),
        },
    );

    let run = traffic_gups_controller_transport_run(
        &mut controller,
        TrafficStateId::new(0),
        &mut scheduler,
        &transport,
        route,
        trace.clone(),
        target,
    )
    .unwrap();

    assert_eq!(run.scheduled_count(), 2);
    assert_eq!(run.response_deliveries().len(), 2);
    assert_eq!(run.response_stats().response_count(), 2);
    assert_eq!(run.response_stats().completed_response_count(), 2);
    assert_eq!(run.response_stats().retry_response_count(), 0);
    assert_eq!(
        run.response_stats()
            .store_conditional_failed_response_count(),
        0
    );
    assert_eq!(run.response_stats().read_response_count(), 1);
    assert_eq!(run.response_stats().write_response_count(), 1);
    assert_eq!(run.response_stats().response_data_byte_count(), 8);
    assert_eq!(
        run.workload_gups_run_summary(workload_route("gups.transport")),
        WorkloadGupsRunSummary::new(workload_route("gups.transport"), run.final_tick())
            .with_scheduled_count(2)
            .with_response_count(2)
            .with_completed_response_count(2)
            .with_read_response_count(1)
            .with_write_response_count(1)
            .with_response_data_byte_count(8)
            .with_memory_trace_event_count(run.memory_trace_events().len()),
    );
    assert_eq!(
        run.response_deliveries()[0].response().request_id(),
        MemoryRequestId::new(AgentId::new(5), 0)
    );
    assert_eq!(
        run.response_deliveries()[0].response().status(),
        ResponseStatus::Completed
    );
    assert_eq!(
        run.response_deliveries()[1].response().request_id(),
        MemoryRequestId::new(AgentId::new(5), 1)
    );
    assert_eq!(
        run.response_deliveries()[1].response().status(),
        ResponseStatus::Completed
    );
    assert_eq!(
        *writes.lock().unwrap(),
        vec![(
            13,
            MemoryRequestId::new(AgentId::new(5), 1),
            Address::new(0x1000),
            0x0102_0304_0506_0708_u64.to_le_bytes().to_vec(),
        )]
    );
    assert_eq!(run.final_tick(), 18);

    let trace_events = trace.snapshot();
    assert_eq!(run.memory_trace_events(), trace_events.as_slice());
    assert_eq!(trace_events.len(), 6);
    assert_eq!(trace_events[0].kind(), MemoryTraceKind::RequestSent);
    assert_eq!(trace_events[1].kind(), MemoryTraceKind::RequestArrived);
    assert_eq!(trace_events[2].kind(), MemoryTraceKind::ResponseArrived);
    assert_eq!(trace_events[3].kind(), MemoryTraceKind::RequestSent);
    assert_eq!(trace_events[4].kind(), MemoryTraceKind::RequestArrived);
    assert_eq!(trace_events[5].kind(), MemoryTraceKind::ResponseArrived);
}

#[test]
fn traffic_gups_transport_run_counts_retry_write_responses() {
    let mut controller = gups_controller();
    controller.start(0).unwrap();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core0"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();
    let target = Arc::new(
        move |delivery: &RequestDelivery| match delivery.request().operation() {
            MemoryOperation::ReadShared => {
                let data = 0x0102_0304_0506_0708_u64.to_le_bytes().to_vec();
                TargetOutcome::Respond(
                    MemoryResponse::completed(delivery.request(), Some(data)).unwrap(),
                )
            }
            MemoryOperation::Write => {
                TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
            }
            operation => panic!("unexpected GUPS transport operation: {operation:?}"),
        },
    );

    let run = traffic_gups_controller_transport_run(
        &mut controller,
        TrafficStateId::new(0),
        &mut scheduler,
        &transport,
        route,
        MemoryTrace::new(),
        target,
    )
    .unwrap();

    let stats = run.response_stats();
    assert_eq!(stats.response_count(), 2);
    assert_eq!(stats.completed_response_count(), 1);
    assert_eq!(stats.retry_response_count(), 1);
    assert_eq!(stats.store_conditional_failed_response_count(), 0);
    assert_eq!(stats.read_response_count(), 1);
    assert_eq!(stats.write_response_count(), 1);
    assert_eq!(stats.response_data_byte_count(), 8);
}

#[test]
fn traffic_gups_transport_run_does_not_advance_to_unrelated_future_work() {
    let mut controller = gups_controller();
    controller.start(0).unwrap();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    scheduler
        .schedule_parallel_at(PartitionId::new(2), 100, |_| {})
        .unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core0"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();
    let writes = Arc::new(Mutex::new(Vec::new()));
    let write_log = Arc::clone(&writes);
    let target = Arc::new(
        move |delivery: &RequestDelivery| match delivery.request().operation() {
            MemoryOperation::ReadShared => {
                let data = 0x0102_0304_0506_0708_u64.to_le_bytes().to_vec();
                TargetOutcome::Respond(
                    MemoryResponse::completed(delivery.request(), Some(data)).unwrap(),
                )
            }
            MemoryOperation::Write => {
                write_log.lock().unwrap().push(delivery.tick());
                TargetOutcome::Respond(MemoryResponse::completed(delivery.request(), None).unwrap())
            }
            operation => panic!("unexpected GUPS transport operation: {operation:?}"),
        },
    );

    let run = traffic_gups_controller_transport_run(
        &mut controller,
        TrafficStateId::new(0),
        &mut scheduler,
        &transport,
        route,
        MemoryTrace::new(),
        target,
    )
    .unwrap();

    assert_eq!(*writes.lock().unwrap(), vec![13]);
    assert_eq!(run.final_tick(), 18);
    assert_eq!(
        scheduler.next_pending_tick(PartitionId::new(2)).unwrap(),
        Some(100)
    );
}

#[test]
fn traffic_gups_transport_run_does_not_overshoot_response_epoch() {
    let mut controller = gups_controller();
    controller.start(0).unwrap();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 5).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core0"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                5,
                5,
            )
            .unwrap(),
        )
        .unwrap();
    let writes = Arc::new(Mutex::new(Vec::new()));
    let write_log = Arc::clone(&writes);
    let target = Arc::new(
        move |delivery: &RequestDelivery| match delivery.request().operation() {
            MemoryOperation::ReadShared => {
                let data = 0x0102_0304_0506_0708_u64.to_le_bytes().to_vec();
                TargetOutcome::Respond(
                    MemoryResponse::completed(delivery.request(), Some(data)).unwrap(),
                )
            }
            MemoryOperation::Write => {
                write_log.lock().unwrap().push(delivery.tick());
                TargetOutcome::Respond(MemoryResponse::completed(delivery.request(), None).unwrap())
            }
            operation => panic!("unexpected GUPS transport operation: {operation:?}"),
        },
    );

    let run = traffic_gups_controller_transport_run(
        &mut controller,
        TrafficStateId::new(0),
        &mut scheduler,
        &transport,
        route,
        MemoryTrace::new(),
        target,
    )
    .unwrap();

    assert_eq!(*writes.lock().unwrap(), vec![17]);
    assert_eq!(run.final_tick(), 22);
}

#[test]
fn traffic_gups_transport_run_rejects_retry_read_response_before_write() {
    let mut controller = gups_controller();
    controller.start(0).unwrap();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core0"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();
    let writes = Arc::new(Mutex::new(Vec::new()));
    let write_log = Arc::clone(&writes);
    let target = Arc::new(
        move |delivery: &RequestDelivery| match delivery.request().operation() {
            MemoryOperation::ReadShared => {
                TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
            }
            MemoryOperation::Write => {
                write_log.lock().unwrap().push(delivery.request().id());
                TargetOutcome::Respond(MemoryResponse::completed(delivery.request(), None).unwrap())
            }
            operation => panic!("unexpected GUPS transport operation: {operation:?}"),
        },
    );

    let error = traffic_gups_controller_transport_run(
        &mut controller,
        TrafficStateId::new(0),
        &mut scheduler,
        &transport,
        route,
        MemoryTrace::new(),
        target,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        TrafficGupsTransportError::ReadResponseNotCompleted {
            request,
            status: ResponseStatus::Retry,
        } if request == MemoryRequestId::new(AgentId::new(5), 0)
    ));
    assert!(writes.lock().unwrap().is_empty());
}

#[test]
fn traffic_gups_transport_run_rejects_non_gups_request_batches() {
    let mut controller = idle_controller();
    controller.start(0).unwrap();
    let before = controller.snapshot();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core0"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();

    let error = traffic_gups_controller_transport_run(
        &mut controller,
        TrafficStateId::new(0),
        &mut scheduler,
        &transport,
        route,
        MemoryTrace::new(),
        Arc::new(|_| TargetOutcome::NoResponse),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        TrafficGupsTransportError::UnsupportedControllerBatch {
            state,
            event_count: 1,
        } if state == TrafficStateId::new(0)
    ));
    assert_eq!(controller.snapshot(), before);
}

#[test]
fn traffic_gups_transport_run_rejects_fabric_backed_routes() {
    let mut controller = gups_controller();
    controller.start(0).unwrap();
    let before = controller.snapshot();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new_path(
                endpoint("core0"),
                PartitionId::new(0),
                [
                    MemoryRouteHop::new(endpoint("memory0"), PartitionId::new(1), 3, 5)
                        .unwrap()
                        .with_request_fabric_path(fabric_path("gups-request")),
                ],
            )
            .unwrap(),
        )
        .unwrap();

    let error = traffic_gups_controller_transport_run(
        &mut controller,
        TrafficStateId::new(0),
        &mut scheduler,
        &transport,
        route,
        MemoryTrace::new(),
        Arc::new(|_| TargetOutcome::NoResponse),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        TrafficGupsTransportError::UnsupportedFabricRoute { unsupported }
            if unsupported == route
    ));
    assert_eq!(controller.snapshot(), before);
}

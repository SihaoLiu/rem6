use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryResponse,
    ResponseStatus,
};
use rem6_system::{traffic_trace_replay_target_outcome, TrafficTraceReplayTargetError};
use rem6_traffic::{TrafficTraceReplayAction, TrafficTraceReplayActionQueue};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport, ResponseDelivery,
    TargetOutcome, TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request(sequence: u64) -> MemoryRequest {
    request_from(1, sequence)
}

fn request_from(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(agent), sequence),
        Address::new(0x4000 + sequence * 0x40),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap()
}

fn completed_response(request: &MemoryRequest, data: &[u8]) -> MemoryResponse {
    MemoryResponse::completed(request, Some(data.to_vec())).unwrap()
}

#[test]
fn traffic_trace_replay_target_outcome_drives_transport_response_timing() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();
    let responses = Arc::new(Mutex::new(Vec::new()));
    let action_queue = Arc::new(Mutex::new(TrafficTraceReplayActionQueue::default()));

    let core = endpoint("core0");
    let memory = endpoint("memory0");
    let route = transport
        .add_route(
            MemoryRoute::new(
                core.clone(),
                PartitionId::new(0),
                memory,
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();
    let req = request(10);
    action_queue
        .lock()
        .unwrap()
        .record_action(TrafficTraceReplayAction::MemoryResponse {
            tick: 7,
            response: completed_response(&req, &[0xde, 0xad, 0xbe, 0xef, 0x44, 0x55, 0x66, 0x77]),
        })
        .unwrap();

    let queue = Arc::clone(&action_queue);
    let response_log = Arc::clone(&responses);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            trace.clone(),
            move |delivery, _context| {
                assert_eq!(delivery.tick(), 3);
                traffic_trace_replay_target_outcome(&mut queue.lock().unwrap(), &delivery).unwrap()
            },
            move |delivery: ResponseDelivery| {
                response_log.lock().unwrap().push((
                    delivery.tick(),
                    delivery.endpoint().clone(),
                    delivery.response().data().unwrap().to_vec(),
                ));
            },
        )
        .unwrap();

    let summary = scheduler.run_until_idle_conservative();

    assert_eq!(summary.final_tick(), 12);
    assert_eq!(
        *responses.lock().unwrap(),
        vec![(
            12,
            core.clone(),
            vec![0xde, 0xad, 0xbe, 0xef, 0x44, 0x55, 0x66, 0x77]
        )]
    );
    assert!(action_queue.lock().unwrap().is_empty());
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                route,
                core.clone(),
                MemoryTraceKind::RequestSent,
                req.id()
            ),
            MemoryTraceEvent::request(
                3,
                route,
                endpoint("memory0"),
                MemoryTraceKind::RequestArrived,
                req.id()
            ),
            MemoryTraceEvent::response(12, route, core, req.id(), ResponseStatus::Completed),
        ]
    );
}

#[test]
fn traffic_trace_replay_target_outcome_rejects_wrong_request_response() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let action_queue = Arc::new(Mutex::new(TrafficTraceReplayActionQueue::default()));
    let errors = Arc::new(Mutex::new(Vec::new()));

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
    let req = request(20);
    let wrong_req = request_from(2, 99);
    action_queue
        .lock()
        .unwrap()
        .record_action(TrafficTraceReplayAction::MemoryResponse {
            tick: 7,
            response: completed_response(&wrong_req, &[0xaa; 8]),
        })
        .unwrap();

    let queue = Arc::clone(&action_queue);
    let error_log = Arc::clone(&errors);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            MemoryTrace::new(),
            move |delivery, _context| match traffic_trace_replay_target_outcome(
                &mut queue.lock().unwrap(),
                &delivery,
            ) {
                Ok(outcome) => outcome,
                Err(error) => {
                    error_log.lock().unwrap().push(error);
                    TargetOutcome::NoResponse
                }
            },
            |_| panic!("mismatched trace response must not reach the requester"),
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        *errors.lock().unwrap(),
        vec![TrafficTraceReplayTargetError::RequestMismatch {
            request: req.id(),
            response: wrong_req.id(),
        }]
    );
    assert!(!action_queue.lock().unwrap().is_empty());
}

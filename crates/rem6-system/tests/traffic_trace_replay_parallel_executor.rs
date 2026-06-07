use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_system::TrafficTraceReplayControllerParallelExecutor;
use rem6_transport::{MemoryRoute, MemoryTrace, MemoryTransport};

mod support;

use support::traffic_trace::{
    controller_for_packets, endpoint, PacketFields, GEM5_READ_REQ, GEM5_READ_RESP_WITH_INVALIDATE,
};

#[test]
fn traffic_trace_replay_controller_parallel_executor_keeps_delivery_sink_api() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0xc500),
            size: Some(8),
            packet_id: Some(116),
        },
        PacketFields {
            tick: 4,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0xc500),
            size: Some(8),
            packet_id: Some(116),
        },
    ]);
    assert!(controller.start(0).unwrap().is_empty());
    let request_batch = controller.next_event(0, 0).unwrap().unwrap();
    let request_id = request_batch.request().unwrap().request().id();
    let response_batch = controller.next_event(4, 0).unwrap().unwrap();
    let sink_log = Arc::new(Mutex::new(Vec::new()));
    let sink_record = Arc::clone(&sink_log);
    let executor = TrafficTraceReplayControllerParallelExecutor::new(controller).with_target_sink(
        move |order, delivery| {
            sink_record.lock().unwrap().push((
                order.tick(),
                order.sequence(),
                delivery.request().id(),
            ));
        },
    );
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

    assert!(executor
        .submit_batch_request_parallel(
            &request_batch,
            &mut scheduler,
            &transport,
            route,
            MemoryTrace::new(),
            |_| {},
        )
        .unwrap());
    assert_eq!(
        executor
            .record_batch_parallel(&response_batch, &mut scheduler, PartitionId::new(1), 4)
            .unwrap(),
        0
    );
    scheduler.run_until_idle_parallel().unwrap();

    assert!(executor.errors().is_empty());
    assert_eq!(&*sink_log.lock().unwrap(), &[(0, 0, request_id)]);
}

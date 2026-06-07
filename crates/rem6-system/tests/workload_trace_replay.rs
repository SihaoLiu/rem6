use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AddressRange, ResponseStatus};
use rem6_system::{
    RiscvWorkloadReplay, RiscvWorkloadReplayError, RiscvWorkloadTrafficTraceReplay,
    TrafficTraceReplayControlError, TrafficTraceReplayControllerControlError,
    TrafficTraceReplayControllerTargetError, TrafficTraceReplayTargetError,
};
use rem6_traffic::TrafficTraceErrorKind;
use rem6_transport::MemoryTraceKind;
use rem6_workload::{
    HostEventIntent, WorkloadHostEvent, WorkloadHostPlacement, WorkloadManifest,
    WorkloadMemoryRoute, WorkloadMemoryTarget, WorkloadReplayPlan, WorkloadResource,
    WorkloadResourceId, WorkloadResourceKind, WorkloadRiscvCore, WorkloadRouteId, WorkloadTopology,
};

mod support;

use support::traffic_trace::{
    controller_for_packets, endpoint, PacketFields, GEM5_FLUSH_REQ, GEM5_MEM_FENCE_REQ,
    GEM5_MEM_FENCE_RESP, GEM5_READ_REQ, GEM5_READ_RESP_WITH_INVALIDATE, GEM5_WRITE_ERROR,
    GEM5_WRITE_REQ,
};

fn workload_id(value: &str) -> rem6_workload::WorkloadId {
    rem6_workload::WorkloadId::new(value).unwrap()
}

fn resource_id(value: &str) -> WorkloadResourceId {
    WorkloadResourceId::new(value).unwrap()
}

fn route_id(value: &str) -> WorkloadRouteId {
    WorkloadRouteId::new(value).unwrap()
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
}

fn kernel_resource() -> WorkloadResource {
    WorkloadResource::new(
        resource_id("kernel"),
        WorkloadResourceKind::Kernel,
        "sha256:kernel",
        "resources/kernel.elf",
    )
    .unwrap()
}

fn replay_topology() -> WorkloadTopology {
    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 51).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                16,
                AddressRange::new(Address::new(0x8000), AccessSize::new(0x1000).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 2, 3)
                .unwrap(),
        )
        .unwrap()
        .add_riscv_core(
            WorkloadRiscvCore::new(
                0,
                0,
                7,
                Address::new(0x8000),
                "cpu0.ifetch",
                route_id("cpu0.fetch"),
            )
            .unwrap(),
        )
        .unwrap()
}

fn replay_manifest(id: &str) -> WorkloadManifest {
    WorkloadManifest::builder(workload_id(id), boot_image())
        .with_topology(replay_topology())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            0,
            HostEventIntent::Stop {
                reason: "host-stop".to_string(),
            },
        ))
        .build()
        .unwrap()
}

fn replay_with_controller(
    id: &str,
    packets: &[PacketFields],
) -> Result<rem6_system::RiscvWorkloadReplayOutcome, RiscvWorkloadReplayError> {
    let manifest = replay_manifest(id);
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(packets);
    RiscvWorkloadReplay::new(plan)
        .with_max_turns(64)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            controller,
            route_id("cpu0.fetch"),
            PartitionId::new(2),
        ))
        .run_parallel()
}

#[test]
fn workload_replay_runs_bound_traffic_trace_controller() {
    let outcome = replay_with_controller(
        "riscv-replay-traffic-trace",
        &[
            PacketFields {
                tick: 0,
                command: GEM5_READ_REQ,
                address: Some(0xa000),
                size: Some(8),
                packet_id: Some(900),
            },
            PacketFields {
                tick: 3,
                command: GEM5_READ_RESP_WITH_INVALIDATE,
                address: Some(0xa000),
                size: Some(8),
                packet_id: Some(900),
            },
        ],
    )
    .unwrap();

    let traffic_replays = outcome.traffic_trace_replays();
    assert_eq!(traffic_replays.len(), 1);
    let traffic_replay = &traffic_replays[0];
    assert_eq!(traffic_replay.route(), &route_id("cpu0.fetch"));
    assert_eq!(traffic_replay.scheduled_count(), 1);
    assert!(traffic_replay.errors().is_empty());
    assert!(traffic_replay.runtime().is_empty());
    assert_eq!(traffic_replay.response_deliveries().len(), 1);
    let delivery = &traffic_replay.response_deliveries()[0];
    assert_eq!(delivery.tick(), 6);
    assert_eq!(delivery.endpoint(), &endpoint("cpu0.ifetch"));
    assert_eq!(delivery.response().status(), ResponseStatus::Completed);

    let trace_events = traffic_replay.memory_trace_events();
    assert_eq!(trace_events.len(), 3);
    assert_eq!(trace_events[0].kind(), MemoryTraceKind::RequestSent);
    assert_eq!(trace_events[1].kind(), MemoryTraceKind::RequestArrived);
    assert_eq!(trace_events[2].kind(), MemoryTraceKind::ResponseArrived);
    assert_eq!(
        trace_events[2].response_status(),
        Some(ResponseStatus::Completed)
    );
}

#[test]
fn workload_replay_records_bound_traffic_trace_failures_and_sidebands() {
    let outcome = replay_with_controller(
        "riscv-replay-traffic-trace-errors",
        &[
            PacketFields {
                tick: 0,
                command: GEM5_WRITE_REQ,
                address: Some(0xa040),
                size: Some(8),
                packet_id: Some(901),
            },
            PacketFields {
                tick: 2,
                command: GEM5_FLUSH_REQ,
                address: Some(0xa040),
                size: Some(64),
                packet_id: Some(902),
            },
            PacketFields {
                tick: 4,
                command: GEM5_WRITE_ERROR,
                address: Some(0xa040),
                size: Some(8),
                packet_id: Some(901),
            },
        ],
    )
    .unwrap();

    let traffic_replays = outcome.traffic_trace_replays();
    assert_eq!(traffic_replays.len(), 1);
    let traffic_replay = &traffic_replays[0];
    assert_eq!(traffic_replay.scheduled_count(), 2);
    assert!(traffic_replay.errors().is_empty());
    assert!(traffic_replay.response_deliveries().is_empty());
    assert_eq!(traffic_replay.memory_trace_events().len(), 2);
    assert_eq!(
        traffic_replay.memory_trace_events()[0].kind(),
        MemoryTraceKind::RequestSent,
    );
    assert_eq!(
        traffic_replay.memory_trace_events()[1].kind(),
        MemoryTraceKind::RequestArrived,
    );

    let runtime = traffic_replay.runtime();
    assert_eq!(runtime.memory_failures().len(), 1);
    let failure = runtime.memory_failures()[0];
    assert_eq!(failure.tick(), 4);
    assert_eq!(
        failure.record().failure().error(),
        TrafficTraceErrorKind::Write
    );
    assert_eq!(runtime.sideband_events().len(), 1);
    let sideband = runtime.sideband_events()[0];
    assert_eq!(sideband.tick(), 2);
    assert_eq!(sideband.event().tick(), 2);
    assert!(runtime.control_acks().is_empty());
    assert!(runtime.control_failures().is_empty());
    assert!(runtime.is_empty());
}

#[test]
fn workload_replay_records_bound_traffic_trace_control_acks() {
    let outcome = replay_with_controller(
        "riscv-replay-traffic-trace-control",
        &[
            PacketFields {
                tick: 1,
                command: GEM5_MEM_FENCE_REQ,
                address: None,
                size: None,
                packet_id: Some(903),
            },
            PacketFields {
                tick: 5,
                command: GEM5_MEM_FENCE_RESP,
                address: None,
                size: None,
                packet_id: Some(903),
            },
        ],
    )
    .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert_eq!(traffic_replay.scheduled_count(), 1);
    assert!(traffic_replay.errors().is_empty());
    assert!(traffic_replay.response_deliveries().is_empty());
    assert!(traffic_replay.memory_trace_events().is_empty());
    let runtime = traffic_replay.runtime();
    assert_eq!(runtime.control_acks().len(), 1);
    let ack = runtime.control_acks()[0];
    assert_eq!(ack.tick(), 5);
    assert_eq!(ack.trace_tick(), 5);
    assert!(runtime.control_failures().is_empty());
    assert!(runtime.memory_failures().is_empty());
    assert!(runtime.sideband_events().is_empty());
    assert!(runtime.is_empty());
}

#[test]
fn workload_replay_fails_on_bound_traffic_trace_callback_errors() {
    let error = replay_with_controller(
        "riscv-replay-traffic-trace-callback-error",
        &[PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0xa080),
            size: Some(8),
            packet_id: Some(904),
        }],
    )
    .unwrap_err();

    let RiscvWorkloadReplayError::TrafficTraceReplayCallback { route, errors } = error else {
        panic!("expected workload traffic trace replay callback error");
    };
    assert_eq!(route, route_id("cpu0.fetch"));
    assert_eq!(errors.control(), &[]);
    assert_eq!(errors.target().len(), 1);
    assert!(matches!(
        errors.target()[0],
        TrafficTraceReplayControllerTargetError::Target(
            TrafficTraceReplayTargetError::ActionQueueEmpty { .. }
        )
    ));
}

#[test]
fn workload_replay_fails_on_bound_traffic_trace_control_callback_errors() {
    let error = replay_with_controller(
        "riscv-replay-traffic-trace-control-callback-error",
        &[PacketFields {
            tick: 4,
            command: GEM5_MEM_FENCE_REQ,
            address: None,
            size: None,
            packet_id: Some(905),
        }],
    )
    .unwrap_err();

    let RiscvWorkloadReplayError::TrafficTraceReplayCallback { route, errors } = error else {
        panic!("expected workload traffic trace replay callback error");
    };
    assert_eq!(route, route_id("cpu0.fetch"));
    assert_eq!(errors.target(), &[]);
    assert_eq!(errors.control().len(), 1);
    assert_eq!(
        errors.control()[0],
        TrafficTraceReplayControllerControlError::Control(
            TrafficTraceReplayControlError::ActionQueueEmpty { delivery_tick: 4 },
        ),
    );
}

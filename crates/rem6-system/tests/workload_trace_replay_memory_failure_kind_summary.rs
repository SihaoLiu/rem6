use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AddressRange};
use rem6_system::{RiscvWorkloadReplay, RiscvWorkloadTrafficTraceReplay};
use rem6_traffic::TrafficTraceErrorKind;
use rem6_workload::{
    HostEventIntent, WorkloadExpectedTrafficTraceReplaySummary, WorkloadHostEvent,
    WorkloadHostPlacement, WorkloadManifest, WorkloadMemoryRoute, WorkloadMemoryTarget,
    WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadRiscvCore, WorkloadRouteId,
    WorkloadTopology,
};

mod support;

use support::traffic_trace::{
    controller_for_packets, PacketFields, GEM5_FUNCTIONAL_WRITE_ERROR, GEM5_READ_ERROR,
    GEM5_READ_REQ, GEM5_WRITE_REQ,
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

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), 0x0000_0073_u32.to_le_bytes().to_vec())
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
    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 31).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                64,
                AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 2, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu0.data"), "cpu0.dmem", 0, "memory", 2, 2, 3)
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
            .unwrap()
            .with_data("cpu0.dmem", route_id("cpu0.data"))
            .unwrap(),
        )
        .unwrap()
}

fn replay_manifest() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-memory-failure-kind-summary"),
        boot_image(),
    )
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
    .add_expected_traffic_trace_replay_summary(
        WorkloadExpectedTrafficTraceReplaySummary::new(route_id("cpu0.data"))
            .with_minimum_memory_failure_count(2)
            .with_minimum_memory_failure_read_count(1)
            .with_minimum_memory_failure_functional_write_count(1),
    )
    .unwrap()
    .build()
    .unwrap()
}

#[test]
fn workload_replay_summarizes_memory_failure_error_kinds() {
    let plan = rem6_workload::WorkloadReplayPlan::from_manifest(&replay_manifest()).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(970),
        },
        PacketFields {
            tick: 3,
            command: GEM5_READ_ERROR,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(970),
        },
        PacketFields {
            tick: 4,
            command: GEM5_WRITE_REQ,
            address: Some(0x9010),
            size: Some(8),
            packet_id: Some(971),
        },
        PacketFields {
            tick: 6,
            command: GEM5_FUNCTIONAL_WRITE_ERROR,
            address: None,
            size: None,
            packet_id: Some(971),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(96)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            controller,
            route_id("cpu0.data"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert!(traffic_replay.response_deliveries().is_empty());
    let failure_kinds = traffic_replay
        .memory_failure_records()
        .iter()
        .map(|record| record.error())
        .collect::<Vec<_>>();
    assert_eq!(
        failure_kinds,
        vec![
            TrafficTraceErrorKind::Read,
            TrafficTraceErrorKind::FunctionalWrite,
        ],
    );

    let summary = &outcome.result().traffic_trace_replay_summaries()[0];
    assert_eq!(summary.memory_failure_count(), 2);
    assert_eq!(summary.memory_failure_invalid_destination_count(), 0);
    assert_eq!(summary.memory_failure_bad_address_count(), 0);
    assert_eq!(summary.memory_failure_read_count(), 1);
    assert_eq!(summary.memory_failure_write_count(), 0);
    assert_eq!(summary.memory_failure_functional_read_count(), 0);
    assert_eq!(summary.memory_failure_functional_write_count(), 1);
    plan.verify_result(outcome.result()).unwrap();
}

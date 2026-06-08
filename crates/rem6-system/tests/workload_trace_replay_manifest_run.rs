use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AddressRange};
use rem6_system::{RiscvWorkloadReplay, RiscvWorkloadReplayError, RiscvWorkloadTrafficTraceReplay};
use rem6_workload::{
    HostEventIntent, WorkloadExpectedTrafficTraceReplaySummary, WorkloadHostEvent,
    WorkloadHostPlacement, WorkloadManifest, WorkloadMemoryRoute, WorkloadMemoryTarget,
    WorkloadReplayPlan, WorkloadResolvedResources, WorkloadResource, WorkloadResourceId,
    WorkloadResourceKind, WorkloadResourcePayload, WorkloadRiscvCore, WorkloadRouteId,
    WorkloadTopology, WorkloadTrafficTraceReplayRun,
};

mod support;

use support::traffic_trace::{
    controller_for_packets, packet_trace_bytes, PacketFields, GEM5_MEM_FENCE_REQ,
    GEM5_MEM_FENCE_RESP, GEM5_READ_REQ, GEM5_READ_RESP,
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

fn resource(id: &str, kind: WorkloadResourceKind) -> WorkloadResource {
    WorkloadResource::new(
        resource_id(id),
        kind,
        format!("sha256:{id}"),
        format!("resources/{id}"),
    )
    .unwrap()
}

fn replay_topology() -> WorkloadTopology {
    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 51).unwrap())
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

fn manifest() -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("manifest-trace-run-exec"), boot_image())
        .with_topology(replay_topology())
        .add_resource(resource("kernel", WorkloadResourceKind::Kernel))
        .unwrap()
        .add_resource(resource("trace", WorkloadResourceKind::Input))
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            0,
            HostEventIntent::Stop {
                reason: "host-stop".to_string(),
            },
        ))
        .add_traffic_trace_replay(WorkloadTrafficTraceReplayRun::new(
            route_id("cpu0.fetch"),
            resource_id("trace"),
            1_000,
            7,
            64,
            99,
            2,
        ))
        .unwrap()
        .add_expected_traffic_trace_replay_summary(
            WorkloadExpectedTrafficTraceReplaySummary::new(route_id("cpu0.fetch"))
                .with_minimum_response_delivery_count(1)
                .with_minimum_trace_completed_response_count(1)
                .with_minimum_trace_read_response_count(1)
                .with_minimum_trace_response_data_byte_count(8)
                .with_minimum_trace_response_fill_data_byte_count(8)
                .with_minimum_control_ack_count(1)
                .with_minimum_sync_control_ack_count(1),
        )
        .unwrap()
        .build()
        .unwrap()
}

fn resolved_resources(manifest: &WorkloadManifest) -> WorkloadResolvedResources {
    WorkloadResolvedResources::from_manifest(
        manifest,
        [
            WorkloadResourcePayload::new(resource_id("kernel"), "sha256:kernel", Vec::new())
                .unwrap(),
            WorkloadResourcePayload::new(
                resource_id("trace"),
                "sha256:trace",
                packet_trace_bytes(&[
                    PacketFields {
                        tick: 0,
                        command: GEM5_READ_REQ,
                        address: Some(0x9008),
                        size: Some(8),
                        packet_id: Some(990),
                    },
                    PacketFields {
                        tick: 3,
                        command: GEM5_READ_RESP,
                        address: Some(0x9008),
                        size: Some(8),
                        packet_id: Some(990),
                    },
                    PacketFields {
                        tick: 4,
                        command: GEM5_MEM_FENCE_REQ,
                        address: None,
                        size: None,
                        packet_id: Some(991),
                    },
                    PacketFields {
                        tick: 7,
                        command: GEM5_MEM_FENCE_RESP,
                        address: None,
                        size: None,
                        packet_id: Some(991),
                    },
                ]),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn workload_replay_executes_manifest_declared_trace_replay() {
    let manifest = manifest();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_resolved_resources(resolved_resources(&manifest))
        .with_max_turns(96)
        .run_parallel()
        .unwrap();

    assert_eq!(outcome.traffic_trace_replays().len(), 1);
    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert_eq!(traffic_replay.response_deliveries().len(), 1);
    assert_eq!(traffic_replay.memory_response_records().len(), 1);
    assert_eq!(traffic_replay.runtime().control_acks().len(), 1);

    let summary = &outcome.result().traffic_trace_replay_summaries()[0];
    assert_eq!(summary.response_delivery_count(), 1);
    assert_eq!(summary.trace_completed_response_count(), 1);
    assert_eq!(summary.trace_read_response_count(), 1);
    assert_eq!(summary.trace_response_data_byte_count(), 8);
    assert_eq!(summary.trace_response_fill_data_byte_count(), 8);
    assert_eq!(summary.control_ack_count(), 1);
    assert_eq!(summary.sync_control_ack_count(), 1);
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_rejects_duplicate_manifest_and_explicit_trace_routes() {
    let manifest = manifest();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let error = RiscvWorkloadReplay::new(plan)
        .with_resolved_resources(resolved_resources(&manifest))
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            controller_for_packets(&[]),
            route_id("cpu0.fetch"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap_err();

    assert!(matches!(
        error,
        RiscvWorkloadReplayError::Workload(rem6_workload::WorkloadError::DuplicateRoute { route })
            if route == route_id("cpu0.fetch")
    ));
}

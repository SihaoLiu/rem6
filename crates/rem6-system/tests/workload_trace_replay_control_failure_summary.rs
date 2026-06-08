use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AddressRange};
use rem6_system::{RiscvWorkloadReplay, RiscvWorkloadReplayError, RiscvWorkloadTrafficTraceReplay};
use rem6_workload::{
    HostEventIntent, WorkloadExpectedTrafficTraceReplaySummary, WorkloadHostEvent,
    WorkloadHostPlacement, WorkloadManifest, WorkloadMemoryRoute, WorkloadMemoryTarget,
    WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadRiscvCore, WorkloadRouteId,
    WorkloadTopology,
};

mod support;

use rem6_traffic::{
    TrafficTraceCacheKind, TrafficTraceControlFailureSource, TrafficTraceDiagnosticKind,
    TrafficTraceErrorKind, TrafficTraceHtmKind, TrafficTraceTlbKind,
};
use support::traffic_trace::{
    controller_for_packets, PacketFields, GEM5_FLUSH_REQ, GEM5_HTM_ABORT, GEM5_HTM_REQ,
    GEM5_HTM_REQ_RESP, GEM5_INVALID_DEST_ERROR, GEM5_MEM_FENCE_REQ, GEM5_MEM_FENCE_RESP,
    GEM5_PRINT_REQ, GEM5_TLBI_EXT_SYNC, GEM5_WRITE_ERROR,
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

fn replay_manifest_with_expected_summary(
    id: &str,
    expected: WorkloadExpectedTrafficTraceReplaySummary,
) -> WorkloadManifest {
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
        .add_expected_traffic_trace_replay_summary(expected)
        .unwrap()
        .build()
        .unwrap()
}

fn replay_with_controller(
    id: &str,
    packets: &[PacketFields],
) -> Result<rem6_system::RiscvWorkloadReplayOutcome, RiscvWorkloadReplayError> {
    let manifest = replay_manifest(id);
    let plan = rem6_workload::WorkloadReplayPlan::from_manifest(&manifest).unwrap();
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

fn replay_with_manifest_and_controller(
    manifest: WorkloadManifest,
    packets: &[PacketFields],
) -> Result<rem6_system::RiscvWorkloadReplayOutcome, RiscvWorkloadReplayError> {
    let plan = rem6_workload::WorkloadReplayPlan::from_manifest(&manifest).unwrap();
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
fn workload_replay_summarizes_control_failure_sources() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("cpu0.fetch"))
        .with_minimum_trace_sideband_failure_count(4);
    let manifest =
        replay_manifest_with_expected_summary("riscv-replay-control-failure-sources", expected);
    let plan = rem6_workload::WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let outcome = replay_with_manifest_and_controller(
        manifest,
        &[
            PacketFields {
                tick: 1,
                command: GEM5_MEM_FENCE_REQ,
                address: None,
                size: None,
                packet_id: Some(970),
            },
            PacketFields {
                tick: 2,
                command: GEM5_INVALID_DEST_ERROR,
                address: None,
                size: None,
                packet_id: Some(970),
            },
            PacketFields {
                tick: 3,
                command: GEM5_TLBI_EXT_SYNC,
                address: None,
                size: None,
                packet_id: Some(971),
            },
            PacketFields {
                tick: 4,
                command: GEM5_INVALID_DEST_ERROR,
                address: None,
                size: None,
                packet_id: Some(971),
            },
            PacketFields {
                tick: 5,
                command: GEM5_FLUSH_REQ,
                address: Some(0xa000),
                size: Some(64),
                packet_id: Some(972),
            },
            PacketFields {
                tick: 6,
                command: GEM5_WRITE_ERROR,
                address: Some(0xa000),
                size: Some(64),
                packet_id: Some(972),
            },
            PacketFields {
                tick: 7,
                command: GEM5_PRINT_REQ,
                address: Some(0xa100),
                size: Some(1),
                packet_id: Some(973),
            },
            PacketFields {
                tick: 8,
                command: GEM5_INVALID_DEST_ERROR,
                address: Some(0xa100),
                size: Some(1),
                packet_id: Some(973),
            },
            PacketFields {
                tick: 9,
                command: GEM5_HTM_ABORT,
                address: Some(0xa200),
                size: Some(16),
                packet_id: Some(974),
            },
            PacketFields {
                tick: 10,
                command: GEM5_INVALID_DEST_ERROR,
                address: Some(0xa200),
                size: Some(16),
                packet_id: Some(974),
            },
        ],
    )
    .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert_eq!(traffic_replay.runtime().control_failures().len(), 5);
    let sideband_failures = traffic_replay.trace_sideband_failure_records();
    assert_eq!(sideband_failures.len(), 4);
    assert_eq!(sideband_failures[0].tick(), 4);
    assert_eq!(sideband_failures[0].trace_tick(), 3);
    assert_eq!(
        sideband_failures[0].error(),
        TrafficTraceErrorKind::InvalidDestination
    );
    assert_eq!(sideband_failures[0].trace_packet_id(), Some(971));
    match sideband_failures[0].source() {
        TrafficTraceControlFailureSource::Tlb(source) => {
            assert_eq!(source.kind(), TrafficTraceTlbKind::ExternalSync);
        }
        source => panic!("unexpected sideband failure source: {source:?}"),
    }
    assert_eq!(sideband_failures[1].tick(), 6);
    assert_eq!(sideband_failures[1].trace_tick(), 5);
    assert_eq!(sideband_failures[1].error(), TrafficTraceErrorKind::Write);
    assert_eq!(sideband_failures[1].address(), Some(Address::new(0xa000)));
    match sideband_failures[1].source() {
        TrafficTraceControlFailureSource::Cache(source) => {
            assert_eq!(source.kind(), TrafficTraceCacheKind::Flush);
        }
        source => panic!("unexpected sideband failure source: {source:?}"),
    }
    assert_eq!(sideband_failures[2].tick(), 8);
    assert_eq!(sideband_failures[2].trace_tick(), 7);
    assert_eq!(
        sideband_failures[2].error(),
        TrafficTraceErrorKind::InvalidDestination
    );
    match sideband_failures[2].source() {
        TrafficTraceControlFailureSource::Diagnostic(source) => {
            assert_eq!(source.kind(), TrafficTraceDiagnosticKind::Print);
        }
        source => panic!("unexpected sideband failure source: {source:?}"),
    }
    assert_eq!(sideband_failures[3].tick(), 10);
    assert_eq!(sideband_failures[3].trace_tick(), 9);
    assert_eq!(
        sideband_failures[3].error(),
        TrafficTraceErrorKind::InvalidDestination
    );
    assert_eq!(sideband_failures[3].size_bytes(), Some(16));
    match sideband_failures[3].source() {
        TrafficTraceControlFailureSource::Htm(source) => {
            assert_eq!(source.kind(), TrafficTraceHtmKind::Abort);
        }
        source => panic!("unexpected sideband failure source: {source:?}"),
    }

    let summary = &outcome.result().traffic_trace_replay_summaries()[0];
    assert_eq!(summary.control_failure_count(), 5);
    assert_eq!(summary.sync_control_failure_count(), 1);
    assert_eq!(summary.tlb_control_failure_count(), 1);
    assert_eq!(summary.cache_control_failure_count(), 1);
    assert_eq!(summary.htm_control_failure_count(), 1);
    assert_eq!(summary.diagnostic_control_failure_count(), 1);
    assert_eq!(summary.trace_sideband_failure_count(), 4);
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_summarizes_control_failure_error_kinds() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("cpu0.fetch"))
        .with_minimum_control_failure_count(5)
        .with_minimum_control_failure_invalid_destination_count(4)
        .with_minimum_control_failure_write_count(1);
    let manifest =
        replay_manifest_with_expected_summary("riscv-replay-control-failure-error-kinds", expected);
    let plan = rem6_workload::WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let outcome = replay_with_manifest_and_controller(
        manifest,
        &[
            PacketFields {
                tick: 1,
                command: GEM5_MEM_FENCE_REQ,
                address: None,
                size: None,
                packet_id: Some(990),
            },
            PacketFields {
                tick: 2,
                command: GEM5_INVALID_DEST_ERROR,
                address: None,
                size: None,
                packet_id: Some(990),
            },
            PacketFields {
                tick: 3,
                command: GEM5_TLBI_EXT_SYNC,
                address: None,
                size: None,
                packet_id: Some(991),
            },
            PacketFields {
                tick: 4,
                command: GEM5_INVALID_DEST_ERROR,
                address: None,
                size: None,
                packet_id: Some(991),
            },
            PacketFields {
                tick: 5,
                command: GEM5_FLUSH_REQ,
                address: Some(0xa000),
                size: Some(64),
                packet_id: Some(992),
            },
            PacketFields {
                tick: 6,
                command: GEM5_WRITE_ERROR,
                address: Some(0xa000),
                size: Some(64),
                packet_id: Some(992),
            },
            PacketFields {
                tick: 7,
                command: GEM5_PRINT_REQ,
                address: Some(0xa100),
                size: Some(1),
                packet_id: Some(993),
            },
            PacketFields {
                tick: 8,
                command: GEM5_INVALID_DEST_ERROR,
                address: Some(0xa100),
                size: Some(1),
                packet_id: Some(993),
            },
            PacketFields {
                tick: 9,
                command: GEM5_HTM_ABORT,
                address: Some(0xa200),
                size: Some(16),
                packet_id: Some(994),
            },
            PacketFields {
                tick: 10,
                command: GEM5_INVALID_DEST_ERROR,
                address: Some(0xa200),
                size: Some(16),
                packet_id: Some(994),
            },
        ],
    )
    .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    let failure_kinds = traffic_replay
        .runtime()
        .control_failures()
        .iter()
        .map(|failure| failure.record().failure().error())
        .collect::<Vec<_>>();
    assert_eq!(
        failure_kinds
            .iter()
            .filter(|kind| **kind == TrafficTraceErrorKind::InvalidDestination)
            .count(),
        4,
    );
    assert_eq!(
        failure_kinds
            .iter()
            .filter(|kind| **kind == TrafficTraceErrorKind::Write)
            .count(),
        1,
    );

    let summary = &outcome.result().traffic_trace_replay_summaries()[0];
    assert_eq!(summary.control_failure_count(), 5);
    assert_eq!(summary.control_failure_invalid_destination_count(), 4);
    assert_eq!(summary.control_failure_bad_address_count(), 0);
    assert_eq!(summary.control_failure_read_count(), 0);
    assert_eq!(summary.control_failure_write_count(), 1);
    assert_eq!(summary.control_failure_functional_read_count(), 0);
    assert_eq!(summary.control_failure_functional_write_count(), 0);
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_keeps_response_required_failures_out_of_sideband_records() {
    let outcome = replay_with_controller(
        "riscv-replay-response-required-failure-records",
        &[
            PacketFields {
                tick: 1,
                command: GEM5_MEM_FENCE_REQ,
                address: None,
                size: None,
                packet_id: Some(975),
            },
            PacketFields {
                tick: 2,
                command: GEM5_INVALID_DEST_ERROR,
                address: None,
                size: None,
                packet_id: Some(975),
            },
            PacketFields {
                tick: 3,
                command: GEM5_HTM_REQ,
                address: Some(0xb100),
                size: Some(16),
                packet_id: Some(976),
            },
            PacketFields {
                tick: 4,
                command: GEM5_INVALID_DEST_ERROR,
                address: Some(0xb100),
                size: Some(16),
                packet_id: Some(976),
            },
        ],
    )
    .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert_eq!(traffic_replay.runtime().control_failures().len(), 2);
    assert!(traffic_replay.trace_sideband_failure_records().is_empty());
    assert_eq!(traffic_replay.sync_records().len(), 1);
    assert_eq!(traffic_replay.htm_begin_records().len(), 1);

    let summary = &outcome.result().traffic_trace_replay_summaries()[0];
    assert_eq!(summary.control_failure_count(), 2);
    assert_eq!(summary.sync_control_failure_count(), 1);
    assert_eq!(summary.htm_control_failure_count(), 1);
    assert_eq!(summary.trace_htm_begin_count(), 1);
    assert_eq!(summary.tlb_control_failure_count(), 0);
    assert_eq!(summary.cache_control_failure_count(), 0);
    assert_eq!(summary.diagnostic_control_failure_count(), 0);
}

#[test]
fn workload_replay_summarizes_control_ack_sources() {
    let outcome = replay_with_controller(
        "riscv-replay-control-ack-sources",
        &[
            PacketFields {
                tick: 1,
                command: GEM5_MEM_FENCE_REQ,
                address: None,
                size: None,
                packet_id: Some(980),
            },
            PacketFields {
                tick: 3,
                command: GEM5_MEM_FENCE_RESP,
                address: None,
                size: None,
                packet_id: Some(980),
            },
            PacketFields {
                tick: 5,
                command: GEM5_HTM_REQ,
                address: Some(0xb000),
                size: Some(16),
                packet_id: Some(981),
            },
            PacketFields {
                tick: 7,
                command: GEM5_HTM_REQ_RESP,
                address: Some(0xb000),
                size: Some(16),
                packet_id: Some(981),
            },
        ],
    )
    .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert_eq!(traffic_replay.runtime().control_acks().len(), 2);
    assert_eq!(traffic_replay.sync_records().len(), 1);
    assert_eq!(traffic_replay.htm_begin_records().len(), 1);

    let summary = &outcome.result().traffic_trace_replay_summaries()[0];
    assert_eq!(summary.control_ack_count(), 2);
    assert_eq!(summary.sync_control_ack_count(), 1);
    assert_eq!(summary.htm_control_ack_count(), 1);
    assert_eq!(summary.trace_htm_begin_count(), 1);
}

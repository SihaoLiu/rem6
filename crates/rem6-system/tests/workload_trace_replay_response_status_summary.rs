use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AddressRange, ResponseStatus};
use rem6_system::{RiscvWorkloadReplay, RiscvWorkloadTrafficTraceReplay};
use rem6_workload::{
    HostEventIntent, WorkloadDataCacheProtocol, WorkloadExpectedTrafficTraceReplaySummary,
    WorkloadHostEvent, WorkloadHostPlacement, WorkloadManifest, WorkloadMemoryRoute,
    WorkloadMemoryTarget, WorkloadResource, WorkloadResourceId, WorkloadResourceKind,
    WorkloadRiscvCore, WorkloadRiscvDataCache, WorkloadRouteId, WorkloadTopology,
};

mod support;

use support::traffic_trace::{
    controller_for_packets, PacketFields, GEM5_READ_REQ, GEM5_READ_RESP_WITH_INVALIDATE,
    GEM5_STORE_COND_FAIL_REQ, GEM5_STORE_COND_RESP,
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
        .add_segment(
            Address::new(0x9008),
            0xfedc_ba98_7654_3210_u64.to_le_bytes().to_vec(),
        )
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
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("dcache.backing"),
                "dcache.dir",
                2,
                "memory",
                2,
                2,
                3,
            )
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
        .with_riscv_data_cache(
            WorkloadRiscvDataCache::new(
                WorkloadDataCacheProtocol::Msi,
                0,
                Address::new(0x9000),
                2,
                "dcache.dir",
                route_id("dcache.backing"),
            )
            .unwrap(),
        )
        .unwrap()
}

fn replay_manifest() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-response-status-summary"),
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
            .with_minimum_response_delivery_count(1)
            .with_minimum_trace_response_data_byte_count(8)
            .with_minimum_trace_response_fill_data_byte_count(8)
            .with_minimum_trace_completed_response_count(1)
            .with_minimum_trace_store_conditional_failed_response_count(1)
            .with_minimum_trace_read_response_count(1)
            .with_minimum_trace_write_response_count(1)
            .with_minimum_trace_invalidate_response_count(1)
            .with_minimum_trace_llsc_response_count(1)
            .with_minimum_trace_writable_intent_response_count(1),
    )
    .unwrap()
    .build()
    .unwrap()
}

#[test]
fn workload_replay_summarizes_executable_response_statuses() {
    let plan = rem6_workload::WorkloadReplayPlan::from_manifest(&replay_manifest()).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(990),
        },
        PacketFields {
            tick: 3,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(990),
        },
        PacketFields {
            tick: 4,
            command: GEM5_STORE_COND_FAIL_REQ,
            address: Some(0x9010),
            size: Some(8),
            packet_id: Some(991),
        },
        PacketFields {
            tick: 7,
            command: GEM5_STORE_COND_RESP,
            address: Some(0x9010),
            size: Some(8),
            packet_id: Some(991),
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
    let delivered_statuses = traffic_replay
        .response_deliveries()
        .iter()
        .map(|delivery| delivery.response().status())
        .collect::<Vec<_>>();
    assert_eq!(
        delivered_statuses,
        vec![
            ResponseStatus::Completed,
            ResponseStatus::StoreConditionalFailed,
        ],
    );
    let recorded_statuses = traffic_replay
        .memory_response_records()
        .iter()
        .map(|record| record.status())
        .collect::<Vec<_>>();
    assert_eq!(
        recorded_statuses,
        vec![
            ResponseStatus::Completed,
            ResponseStatus::StoreConditionalFailed,
        ],
    );

    let summary = &outcome.result().traffic_trace_replay_summaries()[0];
    assert_eq!(summary.response_delivery_count(), 2);
    assert_eq!(summary.trace_response_data_byte_count(), 8);
    assert_eq!(summary.trace_response_fill_data_byte_count(), 8);
    assert_eq!(summary.trace_completed_response_count(), 1);
    assert_eq!(summary.trace_retry_response_count(), 0);
    assert_eq!(summary.trace_store_conditional_failed_response_count(), 1);
    assert_eq!(summary.trace_read_response_count(), 1);
    assert_eq!(summary.trace_write_response_count(), 1);
    assert_eq!(summary.trace_prefetch_response_count(), 0);
    assert_eq!(summary.trace_invalidate_response_count(), 1);
    assert_eq!(summary.trace_upgrade_response_count(), 0);
    assert_eq!(summary.trace_llsc_response_count(), 1);
    assert_eq!(summary.trace_locked_rmw_response_count(), 0);
    assert_eq!(summary.trace_writable_intent_response_count(), 1);
    plan.verify_result(outcome.result()).unwrap();
}

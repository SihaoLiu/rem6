use rem6_boot::BootImage;
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AddressRange};
use rem6_system::{RiscvTraceHtmAccessKind, RiscvWorkloadReplay, RiscvWorkloadTrafficTraceReplay};
use rem6_workload::{
    HostEventIntent, WorkloadDataCacheProtocol, WorkloadExpectedTrafficTraceReplaySummary,
    WorkloadHostEvent, WorkloadHostPlacement, WorkloadManifest, WorkloadMemoryRoute,
    WorkloadMemoryTarget, WorkloadResource, WorkloadResourceId, WorkloadResourceKind,
    WorkloadRiscvCore, WorkloadRiscvDataCache, WorkloadRouteId, WorkloadTopology,
};

mod support;

use support::traffic_trace::{
    controller_for_packets, PacketFields, GEM5_HTM_REQ, GEM5_HTM_REQ_RESP,
    GEM5_LOCKED_RMW_READ_REQ, GEM5_LOCKED_RMW_READ_RESP,
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
        workload_id("riscv-replay-htm-locked-rmw-access"),
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
            .with_minimum_trace_htm_access_count(2),
    )
    .unwrap()
    .build()
    .unwrap()
}

#[test]
fn workload_replay_records_locked_rmw_read_as_htm_writable_access() {
    let manifest = replay_manifest();
    let plan = rem6_workload::WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 1,
            command: GEM5_HTM_REQ,
            address: None,
            size: None,
            packet_id: Some(991),
        },
        PacketFields {
            tick: 2,
            command: GEM5_HTM_REQ_RESP,
            address: None,
            size: None,
            packet_id: Some(991),
        },
        PacketFields {
            tick: 3,
            command: GEM5_LOCKED_RMW_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(992),
        },
        PacketFields {
            tick: 5,
            command: GEM5_LOCKED_RMW_READ_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(992),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(160)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            controller,
            route_id("cpu0.data"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert!(traffic_replay.runtime().is_empty(), "{traffic_replay:#?}");

    let records = outcome.run().trace_htm_access_records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].kind(), RiscvTraceHtmAccessKind::ReadSet);
    assert_eq!(records[0].tick(), 5);
    assert_eq!(records[0].trace_packet_id(), Some(992));
    assert_eq!(records[0].address(), Address::new(0x9008));
    assert_eq!(records[0].line(), Address::new(0x9000));
    assert_eq!(records[1].kind(), RiscvTraceHtmAccessKind::WriteSet);
    assert_eq!(records[1].tick(), 5);
    assert_eq!(records[1].trace_packet_id(), Some(992));
    assert_eq!(records[1].address(), Address::new(0x9008));
    assert_eq!(records[1].line(), Address::new(0x9000));
    assert_eq!(traffic_replay.trace_htm_access_records(), records);

    let summary = &outcome.result().traffic_trace_replay_summaries()[0];
    assert_eq!(summary.trace_htm_access_count(), 2);
    plan.verify_result(outcome.result()).unwrap();
}

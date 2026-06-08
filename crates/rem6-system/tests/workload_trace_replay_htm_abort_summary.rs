use rem6_boot::BootImage;
use rem6_cpu::{CpuId, RiscvClusterHtmAbortOutcome};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AddressRange};
use rem6_system::{RiscvWorkloadReplay, RiscvWorkloadTrafficTraceReplay};
use rem6_workload::{
    HostEventIntent, WorkloadDataCacheProtocol, WorkloadExpectedTrafficTraceReplaySummary,
    WorkloadHostEvent, WorkloadHostPlacement, WorkloadManifest, WorkloadMemoryRoute,
    WorkloadMemoryTarget, WorkloadReplayPlan, WorkloadResource, WorkloadResourceId,
    WorkloadResourceKind, WorkloadRiscvCore, WorkloadRiscvDataCache, WorkloadRouteId,
    WorkloadTopology,
};

mod support;

use support::traffic_trace::{controller_for_packets, PacketFields, GEM5_HTM_ABORT};

fn workload_id(value: &str) -> rem6_workload::WorkloadId {
    rem6_workload::WorkloadId::new(value).unwrap()
}

fn resource_id(value: &str) -> WorkloadResourceId {
    WorkloadResourceId::new(value).unwrap()
}

fn route_id(value: &str) -> WorkloadRouteId {
    WorkloadRouteId::new(value).unwrap()
}

fn boot_image_with_data_cache_line() -> BootImage {
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

fn replay_topology_with_data_cache() -> WorkloadTopology {
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

fn replay_manifest_with_htm_abort_expectation(id: &str) -> WorkloadManifest {
    WorkloadManifest::builder(workload_id(id), boot_image_with_data_cache_line())
        .with_topology(replay_topology_with_data_cache())
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
                .with_minimum_sideband_event_count(1)
                .with_minimum_htm_abort_event_count(1)
                .with_minimum_trace_htm_abort_count(1),
        )
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn workload_replay_summarizes_executable_htm_abort_sidebands() {
    let manifest =
        replay_manifest_with_htm_abort_expectation("riscv-replay-trace-htm-abort-summary");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[PacketFields {
        tick: 4,
        command: GEM5_HTM_ABORT,
        address: None,
        size: None,
        packet_id: Some(960),
    }]);

    let outcome = RiscvWorkloadReplay::new(plan)
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
    assert_eq!(traffic_replay.runtime().sideband_events().len(), 1);
    let htm_aborts = traffic_replay.htm_abort_records();
    assert_eq!(htm_aborts.len(), 1);
    assert!(matches!(
        htm_aborts[0].cluster_outcome(),
        RiscvClusterHtmAbortOutcome::NoActiveTransaction { cpu, .. }
            if *cpu == CpuId::new(0)
    ));
    assert_eq!(htm_aborts[0].trace_packet_id(), Some(960));

    let summary = &outcome.result().traffic_trace_replay_summaries()[0];
    assert_eq!(summary.htm_abort_event_count(), 1);
    assert_eq!(summary.trace_htm_abort_count(), 1);
    WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .verify_result(outcome.result())
        .unwrap();
}

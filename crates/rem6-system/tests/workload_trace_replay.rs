use rem6_boot::BootImage;
use rem6_cpu::{CpuId, RiscvClusterHtmAbortOutcome, RiscvClusterHtmBeginOutcome};
use rem6_kernel::PartitionId;
use rem6_memory::{
    AccessSize, Address, AddressRange, MemoryTargetId, ResponseStatus, TranslationQueueConfig,
    TranslationTlbConfig, TranslationTlbStats,
};
use rem6_system::{
    RiscvDataCacheProtocol, RiscvTraceDiagnosticKind, RiscvTraceHtmAccessKind, RiscvWorkloadReplay,
    RiscvWorkloadReplayError, RiscvWorkloadTraceSyncOutcome, RiscvWorkloadTrafficTraceReplay,
    TrafficTraceReplayControlError, TrafficTraceReplayControllerControlError,
    TrafficTraceReplayControllerTargetError, TrafficTraceReplayTargetError,
};
use rem6_traffic::{
    TrafficTraceCacheKind, TrafficTraceErrorKind, TrafficTraceResponseKind, TrafficTraceSyncKind,
};
use rem6_transport::MemoryTraceKind;
use rem6_workload::{
    HostEventIntent, WorkloadDataCacheProtocol, WorkloadExpectedTrafficTraceReplaySummary,
    WorkloadHostEvent, WorkloadHostPlacement, WorkloadManifest, WorkloadMemoryRoute,
    WorkloadMemoryTarget, WorkloadReplayPlan, WorkloadResource, WorkloadResourceId,
    WorkloadResourceKind, WorkloadRiscvCore, WorkloadRiscvDataCache, WorkloadRiscvDataTranslation,
    WorkloadRouteId, WorkloadTopology, WorkloadTrafficTraceReplaySummaryExpectationError,
    WorkloadTranslationPageMapping,
};

mod support;

use support::traffic_trace::{
    controller_for_packet_records, controller_for_packets, endpoint, PacketFields, PacketRecord,
    GEM5_CLEAN_INVALID_REQ, GEM5_CLEAN_INVALID_RESP, GEM5_CLEAN_SHARED_REQ, GEM5_CLEAN_SHARED_RESP,
    GEM5_FLAG_KERNEL, GEM5_FLUSH_REQ, GEM5_FUNCTIONAL_READ_ERROR, GEM5_FUNCTIONAL_WRITE_ERROR,
    GEM5_HTM_ABORT, GEM5_HTM_REQ, GEM5_HTM_REQ_RESP, GEM5_INVALID_DEST_ERROR, GEM5_MEM_FENCE_REQ,
    GEM5_MEM_FENCE_RESP, GEM5_MEM_SYNC_REQ, GEM5_MEM_SYNC_RESP, GEM5_PRINT_REQ, GEM5_READ_ERROR,
    GEM5_READ_REQ, GEM5_READ_RESP, GEM5_READ_RESP_WITH_INVALIDATE, GEM5_SOFT_PF_REQ,
    GEM5_SOFT_PF_RESP, GEM5_STORE_COND_FAIL_REQ, GEM5_STORE_COND_REQ, GEM5_STORE_COND_RESP,
    GEM5_SYNC_INV_L1, GEM5_TLBI_EXT_SYNC, GEM5_WRITEBACK_DIRTY, GEM5_WRITE_COMPLETE_RESP,
    GEM5_WRITE_ERROR, GEM5_WRITE_REQ, GEM5_WRITE_RESP,
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

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn u_type(imm: u32, rd: u8, opcode: u32) -> u32 {
    (imm & 0xffff_f000) | (u32::from(rd) << 7) | opcode
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
}

fn boot_image_with_data_cache_line() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
        .add_segment(
            Address::new(0x9008),
            0xfedc_ba98_7654_3210_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
}

fn boot_image_with_delayed_data_cache_line() -> BootImage {
    let mut image = BootImage::new(Address::new(0x8000));
    for instruction in 0..16_u64 {
        image = image
            .add_segment(Address::new(0x8000 + instruction * 4), word(0x0000_0013))
            .unwrap();
    }
    image
        .add_segment(Address::new(0x8040), word(0x0000_0073))
        .unwrap()
        .add_segment(
            Address::new(0x9008),
            0xfedc_ba98_7654_3210_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
}

fn boot_image_with_two_data_route_cache_line() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
        .add_segment(Address::new(0x8040), word(0x0000_0073))
        .unwrap()
        .add_segment(
            Address::new(0x9008),
            0xfedc_ba98_7654_3210_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
}

fn boot_image_with_two_data_cache_lines() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
        .add_segment(
            Address::new(0x9008),
            0xfedc_ba98_7654_3210_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
        .add_segment(
            Address::new(0x9048),
            0x0123_4567_89ab_cdef_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
}

fn boot_image_with_translated_load() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(u_type(0x0000_4000, 2, 0x37)))
        .unwrap()
        .add_segment(Address::new(0x8004), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x8008), word(0x0000_0073))
        .unwrap()
        .add_segment(
            Address::new(0x9008),
            0x0123_4567_89ab_cdef_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
}

fn snapshot_line_data(
    snapshot: &rem6_memory::PartitionedMemorySnapshot,
    target: MemoryTargetId,
    line: Address,
) -> Vec<u8> {
    snapshot
        .partitions()
        .iter()
        .find(|partition| partition.target() == target)
        .and_then(|partition| {
            partition
                .lines()
                .iter()
                .find(|candidate| candidate.line() == line)
        })
        .unwrap()
        .data()
        .to_vec()
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

fn replay_topology_with_data_cache_protocol(
    protocol: WorkloadDataCacheProtocol,
) -> WorkloadTopology {
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
                protocol,
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

fn replay_topology_with_two_data_cache_lines(
    protocol: WorkloadDataCacheProtocol,
) -> WorkloadTopology {
    replay_topology_with_data_cache_protocol(protocol)
        .with_riscv_data_cache(
            WorkloadRiscvDataCache::new(
                protocol,
                0,
                Address::new(0x9000),
                2,
                "dcache.dir",
                route_id("dcache.backing"),
            )
            .unwrap()
            .with_line_address(Address::new(0x9040)),
        )
        .unwrap()
}

fn replay_topology_with_two_data_routes(protocol: WorkloadDataCacheProtocol) -> WorkloadTopology {
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
            WorkloadMemoryRoute::new(route_id("cpu1.fetch"), "cpu1.ifetch", 1, "memory", 2, 2, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu1.data"), "cpu1.dmem", 1, "memory", 2, 2, 3)
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
        .add_riscv_core(
            WorkloadRiscvCore::new(
                1,
                1,
                8,
                Address::new(0x8040),
                "cpu1.ifetch",
                route_id("cpu1.fetch"),
            )
            .unwrap()
            .with_data("cpu1.dmem", route_id("cpu1.data"))
            .unwrap(),
        )
        .unwrap()
        .with_riscv_data_cache(
            WorkloadRiscvDataCache::new(
                protocol,
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

fn replay_topology_with_data_translation() -> WorkloadTopology {
    let translation = WorkloadRiscvDataTranslation::with_tlb(
        TranslationQueueConfig::new(4, 0).unwrap(),
        TranslationTlbConfig::new(4).unwrap(),
    )
    .with_page_mapping(WorkloadTranslationPageMapping::new(
        Address::new(0x4000),
        Address::new(0x9000),
        1,
    ));

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
            .with_data_translation("cpu0.dmem", route_id("cpu0.data"), translation)
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

fn replay_manifest_with_data_cache(id: &str) -> WorkloadManifest {
    replay_manifest_with_data_cache_protocol(id, WorkloadDataCacheProtocol::Msi)
}

fn replay_manifest_with_data_cache_protocol(
    id: &str,
    protocol: WorkloadDataCacheProtocol,
) -> WorkloadManifest {
    replay_manifest_with_data_cache_protocol_image_and_stop(
        id,
        protocol,
        boot_image_with_data_cache_line(),
        0,
    )
}

fn replay_manifest_with_delayed_data_cache_stop(id: &str, stop_tick: u64) -> WorkloadManifest {
    replay_manifest_with_data_cache_protocol_image_and_stop(
        id,
        WorkloadDataCacheProtocol::Msi,
        boot_image_with_delayed_data_cache_line(),
        stop_tick,
    )
}

fn replay_manifest_with_data_cache_protocol_image_and_stop(
    id: &str,
    protocol: WorkloadDataCacheProtocol,
    boot_image: BootImage,
    stop_tick: u64,
) -> WorkloadManifest {
    WorkloadManifest::builder(workload_id(id), boot_image)
        .with_topology(replay_topology_with_data_cache_protocol(protocol))
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            stop_tick,
            HostEventIntent::Stop {
                reason: "host-stop".to_string(),
            },
        ))
        .build()
        .unwrap()
}

fn replay_manifest_with_two_data_cache_lines(id: &str) -> WorkloadManifest {
    WorkloadManifest::builder(workload_id(id), boot_image_with_two_data_cache_lines())
        .with_topology(replay_topology_with_two_data_cache_lines(
            WorkloadDataCacheProtocol::Msi,
        ))
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

fn replay_manifest_with_two_data_routes(id: &str) -> WorkloadManifest {
    WorkloadManifest::builder(workload_id(id), boot_image_with_two_data_route_cache_line())
        .with_topology(replay_topology_with_two_data_routes(
            WorkloadDataCacheProtocol::Msi,
        ))
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

fn replay_manifest_with_data_translation(id: &str) -> WorkloadManifest {
    WorkloadManifest::builder(workload_id(id), boot_image_with_translated_load())
        .with_topology(replay_topology_with_data_translation())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            26,
            HostEventIntent::Stop {
                reason: "host-stop".to_string(),
            },
        ))
        .build()
        .unwrap()
}

fn replay_manifest_with_trace_summary_expectation(id: &str) -> WorkloadManifest {
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
        .add_expected_traffic_trace_replay_summary(
            WorkloadExpectedTrafficTraceReplaySummary::new(route_id("cpu0.fetch"))
                .with_minimum_scheduled_count(1)
                .with_minimum_response_delivery_count(1)
                .with_minimum_memory_trace_event_count(3),
        )
        .unwrap()
        .build()
        .unwrap()
}

fn replay_manifest_with_strict_trace_summary_expectation(id: &str) -> WorkloadManifest {
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
        .add_expected_traffic_trace_replay_summary(
            WorkloadExpectedTrafficTraceReplaySummary::new(route_id("cpu0.fetch"))
                .with_minimum_scheduled_count(1)
                .with_minimum_response_delivery_count(2)
                .with_minimum_memory_trace_event_count(3),
        )
        .unwrap()
        .build()
        .unwrap()
}

fn replay_with_controller(
    id: &str,
    packets: &[PacketFields],
) -> Result<rem6_system::RiscvWorkloadReplayOutcome, RiscvWorkloadReplayError> {
    let manifest = replay_manifest(id);
    replay_manifest_with_controller(manifest, packets)
}

fn replay_with_packet_records(
    id: &str,
    packets: &[PacketRecord],
) -> Result<rem6_system::RiscvWorkloadReplayOutcome, RiscvWorkloadReplayError> {
    let manifest = replay_manifest(id);
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packet_records(packets);
    RiscvWorkloadReplay::new(plan)
        .with_max_turns(64)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            controller,
            route_id("cpu0.fetch"),
            PartitionId::new(2),
        ))
        .run_parallel()
}

fn replay_manifest_with_controller(
    manifest: WorkloadManifest,
    packets: &[PacketFields],
) -> Result<rem6_system::RiscvWorkloadReplayOutcome, RiscvWorkloadReplayError> {
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

    let response_records = traffic_replay.memory_response_records();
    assert_eq!(response_records.len(), 1);
    let response_record = &response_records[0];
    assert_eq!(response_record.tick(), 3);
    assert_eq!(response_record.trace_tick(), 3);
    assert_eq!(response_record.sequence(), 1);
    assert_eq!(
        response_record.request_id(),
        delivery.response().request_id()
    );
    assert_eq!(
        response_record.kind(),
        TrafficTraceResponseKind::ReadWithInvalidate
    );
    assert_eq!(response_record.status(), ResponseStatus::Completed);
    assert_eq!(response_record.address(), Some(Address::new(0xa000)));
    assert_eq!(response_record.line(), Address::new(0xa000));
    assert_eq!(response_record.size_bytes(), Some(8));
    assert_eq!(response_record.trace_packet_id(), Some(900));
    assert_eq!(response_record.trace_data_bytes(), Some(8));
    assert_eq!(response_record.response_data_bytes(), Some(8));
    assert!(!response_record.data_cache_response_applied());
}

#[test]
fn workload_replay_records_prefetch_trace_fill_without_response_payload() {
    let outcome = replay_with_controller(
        "riscv-replay-trace-prefetch-response-record",
        &[
            PacketFields {
                tick: 0,
                command: GEM5_SOFT_PF_REQ,
                address: Some(0xa040),
                size: Some(16),
                packet_id: Some(914),
            },
            PacketFields {
                tick: 4,
                command: GEM5_SOFT_PF_RESP,
                address: Some(0xa040),
                size: Some(16),
                packet_id: Some(914),
            },
        ],
    )
    .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert_eq!(traffic_replay.response_deliveries().len(), 1);
    assert_eq!(
        traffic_replay.response_deliveries()[0].response().status(),
        ResponseStatus::Completed
    );
    assert_eq!(
        traffic_replay.response_deliveries()[0].response().data(),
        None
    );

    let response_records = traffic_replay.memory_response_records();
    assert_eq!(response_records.len(), 1);
    let response_record = &response_records[0];
    assert_eq!(response_record.tick(), 4);
    assert_eq!(response_record.trace_tick(), 4);
    assert_eq!(response_record.sequence(), 1);
    assert_eq!(
        response_record.kind(),
        TrafficTraceResponseKind::SoftPrefetch
    );
    assert_eq!(response_record.status(), ResponseStatus::Completed);
    assert_eq!(response_record.address(), Some(Address::new(0xa040)));
    assert_eq!(response_record.line(), Address::new(0xa040));
    assert_eq!(response_record.size_bytes(), Some(16));
    assert_eq!(response_record.trace_packet_id(), Some(914));
    assert_eq!(response_record.trace_data_bytes(), Some(16));
    assert_eq!(response_record.response_data_bytes(), None);
    assert!(!response_record.data_cache_response_applied());
}

#[test]
fn workload_replay_records_trace_write_completion_metadata() {
    let manifest = replay_manifest_with_data_cache("riscv-replay-trace-write-completion-record");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_WRITE_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(918),
        },
        PacketFields {
            tick: 3,
            command: GEM5_WRITE_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(918),
        },
        PacketFields {
            tick: 5,
            command: GEM5_WRITE_COMPLETE_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(918),
        },
    ]);

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
    assert_eq!(traffic_replay.response_deliveries().len(), 1);

    let response_records = traffic_replay.memory_response_records();
    assert_eq!(response_records.len(), 1);
    let response_record = &response_records[0];
    assert_eq!(response_record.kind(), TrafficTraceResponseKind::Write);
    assert_eq!(response_record.trace_packet_id(), Some(918));

    let completion_records = traffic_replay.memory_write_completion_records();
    assert_eq!(completion_records.len(), 1);
    let completion = &completion_records[0];
    assert_eq!(completion.tick(), 5);
    assert_eq!(completion.trace_tick(), 5);
    assert_eq!(completion.sequence(), 2);
    assert_eq!(completion.request_id(), response_record.request_id());
    assert_eq!(completion.kind(), TrafficTraceResponseKind::WriteComplete);
    assert_eq!(completion.address(), Some(Address::new(0x9008)));
    assert_eq!(completion.line(), Address::new(0x9000));
    assert_eq!(completion.size_bytes(), Some(8));
    assert_eq!(completion.trace_packet_id(), Some(918));
    assert_eq!(completion.trace_pc(), None);

    let summaries = outcome.result().traffic_trace_replay_summaries();
    assert_eq!(summaries.len(), 1);
    let summary = &summaries[0];
    assert_eq!(summary.route(), &route_id("cpu0.data"));
    assert_eq!(summary.memory_write_completion_count(), 1);
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
fn workload_replay_records_fetch_trace_read_error_metadata() {
    let outcome = replay_with_controller(
        "riscv-replay-fetch-trace-read-error",
        &[
            PacketFields {
                tick: 0,
                command: GEM5_READ_REQ,
                address: Some(0xa000),
                size: Some(8),
                packet_id: Some(912),
            },
            PacketFields {
                tick: 3,
                command: GEM5_READ_ERROR,
                address: Some(0xa000),
                size: Some(8),
                packet_id: Some(912),
            },
        ],
    )
    .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert_eq!(traffic_replay.runtime().memory_failures().len(), 1);
    assert!(traffic_replay.response_deliveries().is_empty());

    let failures = traffic_replay.memory_failure_records();
    assert_eq!(failures.len(), 1);
    let failure = &failures[0];
    assert_eq!(failure.tick(), 3);
    assert_eq!(failure.trace_tick(), 3);
    assert_eq!(failure.sequence(), 1);
    assert_eq!(failure.error(), TrafficTraceErrorKind::Read);
    assert_eq!(failure.address(), Some(Address::new(0xa000)));
    assert_eq!(failure.line(), Address::new(0xa000));
    assert_eq!(failure.size_bytes(), Some(8));
    assert_eq!(failure.trace_packet_id(), Some(912));
    assert_eq!(
        failure.request_id(),
        traffic_replay.runtime().memory_failures()[0]
            .record()
            .failure()
            .request_id()
    );
}

#[test]
fn workload_replay_records_addressless_fetch_functional_read_error_metadata() {
    let outcome = replay_with_controller(
        "riscv-replay-fetch-trace-functional-read-error",
        &[
            PacketFields {
                tick: 0,
                command: GEM5_READ_REQ,
                address: Some(0xa040),
                size: Some(8),
                packet_id: Some(913),
            },
            PacketFields {
                tick: 3,
                command: GEM5_FUNCTIONAL_READ_ERROR,
                address: None,
                size: None,
                packet_id: Some(913),
            },
        ],
    )
    .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert_eq!(traffic_replay.runtime().memory_failures().len(), 1);
    assert!(traffic_replay.response_deliveries().is_empty());

    let failures = traffic_replay.memory_failure_records();
    assert_eq!(failures.len(), 1);
    let failure = &failures[0];
    assert_eq!(failure.tick(), 3);
    assert_eq!(failure.trace_tick(), 3);
    assert_eq!(failure.sequence(), 1);
    assert_eq!(failure.error(), TrafficTraceErrorKind::FunctionalRead);
    assert_eq!(failure.address(), Some(Address::new(0xa040)));
    assert_eq!(failure.line(), Address::new(0xa040));
    assert_eq!(failure.size_bytes(), None);
    assert_eq!(failure.trace_packet_id(), Some(913));
    assert_eq!(
        failure.request_id(),
        traffic_replay.runtime().memory_failures()[0]
            .record()
            .failure()
            .request_id()
    );
}

#[test]
fn workload_replay_records_typed_trace_sideband_summary_counts() {
    let outcome = replay_with_controller(
        "riscv-replay-typed-trace-sideband-summary",
        &[
            PacketFields {
                tick: 1,
                command: GEM5_TLBI_EXT_SYNC,
                address: Some(0),
                size: Some(64),
                packet_id: Some(920),
            },
            PacketFields {
                tick: 2,
                command: GEM5_PRINT_REQ,
                address: Some(0xa000),
                size: Some(1),
                packet_id: Some(921),
            },
            PacketFields {
                tick: 3,
                command: GEM5_HTM_ABORT,
                address: None,
                size: None,
                packet_id: Some(922),
            },
            PacketFields {
                tick: 4,
                command: GEM5_FLUSH_REQ,
                address: Some(0xa040),
                size: Some(64),
                packet_id: Some(923),
            },
        ],
    )
    .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert_eq!(traffic_replay.scheduled_count(), 4);
    assert!(traffic_replay.errors().is_empty());
    assert!(traffic_replay.response_deliveries().is_empty());
    assert!(traffic_replay.memory_trace_events().is_empty());
    let runtime = traffic_replay.runtime();
    assert_eq!(runtime.sideband_events().len(), 4);
    assert!(runtime.is_empty());
    let htm_aborts = traffic_replay.htm_abort_records();
    assert_eq!(htm_aborts.len(), 1);
    assert_eq!(htm_aborts[0].tick(), 3);
    assert_eq!(htm_aborts[0].trace_tick(), 3);
    assert_eq!(htm_aborts[0].sequence(), 2);
    assert_eq!(htm_aborts[0].address(), None);
    assert_eq!(htm_aborts[0].size_bytes(), None);
    assert_eq!(htm_aborts[0].trace_packet_id(), Some(922));
    assert_eq!(htm_aborts[0].trace_pc(), None);

    let summaries = outcome.result().traffic_trace_replay_summaries();
    assert_eq!(summaries.len(), 1);
    let summary = &summaries[0];
    assert_eq!(summary.route(), &route_id("cpu0.fetch"));
    assert_eq!(summary.scheduled_count(), 4);
    assert_eq!(summary.response_delivery_count(), 0);
    assert_eq!(summary.memory_trace_event_count(), 0);
    assert_eq!(summary.memory_failure_count(), 0);
    assert_eq!(summary.control_ack_count(), 0);
    assert_eq!(summary.control_failure_count(), 0);
    assert_eq!(summary.sideband_event_count(), 4);
    assert_eq!(summary.tlb_sync_event_count(), 1);
    assert_eq!(summary.cache_flush_event_count(), 1);
    assert_eq!(summary.trace_cache_flush_count(), 0);
    assert_eq!(summary.diagnostic_print_event_count(), 1);
    assert_eq!(summary.trace_diagnostic_count(), 0);
    assert_eq!(summary.htm_abort_event_count(), 1);
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

    let sync_records = traffic_replay.sync_records();
    assert_eq!(sync_records.len(), 1);
    let sync = &sync_records[0];
    assert_eq!(sync.completion_tick(), 5);
    assert_eq!(sync.trace_tick(), 5);
    assert_eq!(sync.trace_sequence(), 1);
    assert_eq!(sync.source_tick(), 1);
    assert_eq!(sync.source_sequence(), 0);
    assert_eq!(sync.kind(), TrafficTraceSyncKind::MemFence);
    assert!(!sync.kernel_sync());
    assert_eq!(sync.trace_packet_id(), Some(903));
    assert_eq!(sync.trace_pc(), None);
    assert_eq!(sync.outcome(), &RiscvWorkloadTraceSyncOutcome::Ack);
}

#[test]
fn workload_replay_records_bound_kernel_mem_sync_control_acks() {
    let outcome = replay_with_packet_records(
        "riscv-replay-traffic-trace-kernel-sync-control",
        &[
            PacketRecord {
                tick: 1,
                command: GEM5_MEM_SYNC_REQ,
                address: None,
                size: None,
                flags: Some(GEM5_FLAG_KERNEL),
                packet_id: Some(905),
                pc: Some(0x1008),
            },
            PacketRecord {
                tick: 6,
                command: GEM5_MEM_SYNC_RESP,
                address: None,
                size: None,
                flags: None,
                packet_id: Some(905),
                pc: Some(0x1010),
            },
        ],
    )
    .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert!(traffic_replay.response_deliveries().is_empty());
    assert!(traffic_replay.memory_trace_events().is_empty());
    let runtime = traffic_replay.runtime();
    assert_eq!(runtime.control_acks().len(), 1);
    assert_eq!(runtime.control_acks()[0].tick(), 6);
    assert_eq!(runtime.control_acks()[0].trace_tick(), 6);
    assert!(runtime.control_failures().is_empty());
    assert!(runtime.is_empty());

    let sync_records = traffic_replay.sync_records();
    assert_eq!(sync_records.len(), 1);
    let sync = &sync_records[0];
    assert_eq!(sync.completion_tick(), 6);
    assert_eq!(sync.trace_tick(), 6);
    assert_eq!(sync.trace_sequence(), 1);
    assert_eq!(sync.source_tick(), 1);
    assert_eq!(sync.source_sequence(), 0);
    assert_eq!(sync.kind(), TrafficTraceSyncKind::MemSync);
    assert!(sync.kernel_sync());
    assert_eq!(sync.trace_packet_id(), Some(905));
    assert_eq!(sync.trace_pc(), Some(Address::new(0x1008)));
    assert_eq!(sync.outcome(), &RiscvWorkloadTraceSyncOutcome::Ack);
}

#[test]
fn workload_replay_records_bound_traffic_trace_control_failures() {
    let outcome = replay_with_controller(
        "riscv-replay-traffic-trace-control-failure",
        &[
            PacketFields {
                tick: 1,
                command: GEM5_MEM_FENCE_REQ,
                address: None,
                size: None,
                packet_id: Some(904),
            },
            PacketFields {
                tick: 5,
                command: GEM5_INVALID_DEST_ERROR,
                address: None,
                size: None,
                packet_id: Some(904),
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
    assert!(runtime.control_acks().is_empty());
    assert_eq!(runtime.control_failures().len(), 1);
    let failure = runtime.control_failures()[0];
    assert_eq!(failure.tick(), 5);
    assert_eq!(failure.record().tick(), 5);
    assert_eq!(
        failure.record().failure().error(),
        TrafficTraceErrorKind::InvalidDestination
    );
    assert!(runtime.memory_failures().is_empty());
    assert!(runtime.sideband_events().is_empty());
    assert!(runtime.is_empty());

    let sync_records = traffic_replay.sync_records();
    assert_eq!(sync_records.len(), 1);
    let sync = &sync_records[0];
    assert_eq!(sync.completion_tick(), 5);
    assert_eq!(sync.trace_tick(), 5);
    assert_eq!(sync.trace_sequence(), 1);
    assert_eq!(sync.source_tick(), 1);
    assert_eq!(sync.source_sequence(), 0);
    assert_eq!(sync.kind(), TrafficTraceSyncKind::MemFence);
    assert!(!sync.kernel_sync());
    assert_eq!(sync.trace_packet_id(), Some(904));
    assert_eq!(sync.trace_pc(), None);
    assert_eq!(
        sync.outcome(),
        &RiscvWorkloadTraceSyncOutcome::Failure {
            error: TrafficTraceErrorKind::InvalidDestination,
        }
    );
}

#[test]
fn workload_replay_applies_bound_trace_flush_to_data_cache_line() {
    let manifest = replay_manifest_with_data_cache("riscv-replay-trace-flush-data-cache");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(940),
        },
        PacketFields {
            tick: 3,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(940),
        },
        PacketFields {
            tick: 4,
            command: GEM5_FLUSH_REQ,
            address: Some(0x9000),
            size: Some(64),
            packet_id: Some(941),
        },
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(942),
        },
        PacketFields {
            tick: 8,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(942),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(64)
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
    let cache_flushes = traffic_replay.trace_cache_flush_records();
    assert_eq!(cache_flushes.len(), 1);
    assert_eq!(cache_flushes[0].tick(), 4);
    assert_eq!(cache_flushes[0].trace_tick(), 4);
    assert_eq!(cache_flushes[0].sequence(), 2);
    assert_eq!(cache_flushes[0].kind(), TrafficTraceCacheKind::Flush);
    assert_eq!(cache_flushes[0].protocol(), RiscvDataCacheProtocol::Msi);
    assert_eq!(cache_flushes[0].target(), MemoryTargetId::new(0));
    assert_eq!(cache_flushes[0].address(), Address::new(0x9000));
    assert_eq!(cache_flushes[0].line(), Address::new(0x9000));
    assert_eq!(cache_flushes[0].size_bytes(), 64);
    assert_eq!(cache_flushes[0].trace_packet_id(), Some(941));
    assert_eq!(cache_flushes[0].trace_pc(), None);
    let data_cache_runs = outcome.run().data_cache_runs();
    assert_eq!(data_cache_runs.len(), 2);
    assert!(data_cache_runs[0].has_directory_activity());
    assert!(data_cache_runs[1].has_directory_activity());
    let summary = &outcome.result().traffic_trace_replay_summaries()[0];
    assert_eq!(summary.cache_flush_event_count(), 1);
    assert_eq!(summary.trace_cache_flush_count(), 1);
}

#[test]
fn workload_replay_applies_mem_sync_inv_l1_to_data_cache_line() {
    let manifest =
        replay_manifest_with_delayed_data_cache_stop("riscv-replay-trace-mem-sync-inv-l1", 24);
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packet_records(&[
        PacketRecord {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            flags: None,
            packet_id: Some(943),
            pc: None,
        },
        PacketRecord {
            tick: 3,
            command: GEM5_READ_RESP,
            address: Some(0x9008),
            size: Some(8),
            flags: None,
            packet_id: Some(943),
            pc: None,
        },
        PacketRecord {
            tick: 4,
            command: GEM5_MEM_SYNC_REQ,
            address: None,
            size: None,
            flags: Some(GEM5_FLAG_KERNEL | GEM5_SYNC_INV_L1),
            packet_id: Some(944),
            pc: Some(0x2000),
        },
        PacketRecord {
            tick: 6,
            command: GEM5_MEM_SYNC_RESP,
            address: None,
            size: None,
            flags: None,
            packet_id: Some(944),
            pc: None,
        },
        PacketRecord {
            tick: 7,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            flags: None,
            packet_id: Some(945),
            pc: None,
        },
        PacketRecord {
            tick: 10,
            command: GEM5_READ_RESP,
            address: Some(0x9008),
            size: Some(8),
            flags: None,
            packet_id: Some(945),
            pc: None,
        },
    ]);

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
    assert_eq!(traffic_replay.scheduled_count(), 3);
    assert_eq!(traffic_replay.response_deliveries().len(), 2);
    assert_eq!(traffic_replay.runtime().control_acks().len(), 1);
    assert!(traffic_replay.runtime().control_failures().is_empty());
    let sync_records = traffic_replay.sync_records();
    assert_eq!(sync_records.len(), 1);
    assert_eq!(sync_records[0].kind(), TrafficTraceSyncKind::MemSync);
    assert!(sync_records[0].kernel_sync());
    assert!(sync_records[0].invalidates_l1());

    let data_cache_runs = outcome.run().data_cache_runs();
    assert_eq!(data_cache_runs.len(), 2);
    assert!(data_cache_runs[0].has_directory_activity());
    assert!(data_cache_runs[1].has_directory_activity());
}

#[test]
fn workload_replay_does_not_mutate_data_cache_for_trace_write_error() {
    let manifest = replay_manifest_with_data_cache("riscv-replay-trace-write-error-data-cache");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_WRITE_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(950),
        },
        PacketFields {
            tick: 3,
            command: GEM5_WRITE_ERROR,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(950),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(64)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            controller,
            route_id("cpu0.data"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert_eq!(traffic_replay.runtime().memory_failures().len(), 1);
    let failure = traffic_replay.runtime().memory_failures()[0];
    assert_eq!(failure.tick(), 3);
    assert_eq!(
        failure.record().failure().error(),
        TrafficTraceErrorKind::Write
    );
    let trace_errors = outcome.run().trace_error_records();
    assert_eq!(trace_errors.len(), 1);
    let trace_error = trace_errors[0];
    assert_eq!(trace_error.tick(), 3);
    assert_eq!(trace_error.trace_tick(), 3);
    assert_eq!(trace_error.sequence(), 1);
    assert_eq!(
        trace_error.request_id(),
        failure.record().failure().request_id()
    );
    assert_eq!(trace_error.error(), TrafficTraceErrorKind::Write);
    assert_eq!(trace_error.protocol(), RiscvDataCacheProtocol::Msi);
    assert_eq!(trace_error.target(), MemoryTargetId::new(0));
    assert_eq!(trace_error.address(), Address::new(0x9008));
    assert_eq!(trace_error.line(), Address::new(0x9000));
    assert_eq!(trace_error.size_bytes(), Some(8));
    assert_eq!(trace_error.trace_packet_id(), Some(950));
    let memory_failures = traffic_replay.memory_failure_records();
    assert_eq!(memory_failures.len(), 1);
    assert_eq!(memory_failures[0].tick(), 3);
    assert_eq!(memory_failures[0].error(), TrafficTraceErrorKind::Write);
    assert_eq!(memory_failures[0].address(), Some(Address::new(0x9008)));
    assert_eq!(memory_failures[0].line(), Address::new(0x9000));
    assert_eq!(memory_failures[0].size_bytes(), Some(8));
    assert_eq!(memory_failures[0].trace_packet_id(), Some(950));
    assert!(outcome.run().data_cache_runs().is_empty());
}

#[test]
fn workload_replay_records_addressless_functional_write_error_from_request_context() {
    let manifest =
        replay_manifest_with_data_cache("riscv-replay-trace-functional-write-error-data-cache");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_WRITE_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(951),
        },
        PacketFields {
            tick: 3,
            command: GEM5_FUNCTIONAL_WRITE_ERROR,
            address: None,
            size: None,
            packet_id: Some(951),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(64)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            controller,
            route_id("cpu0.data"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert_eq!(traffic_replay.runtime().memory_failures().len(), 1);
    let failure = traffic_replay.runtime().memory_failures()[0];
    assert_eq!(failure.tick(), 3);
    assert_eq!(
        failure.record().failure().error(),
        TrafficTraceErrorKind::FunctionalWrite
    );
    let trace_errors = outcome.run().trace_error_records();
    assert_eq!(trace_errors.len(), 1);
    let trace_error = trace_errors[0];
    assert_eq!(trace_error.tick(), 3);
    assert_eq!(trace_error.trace_tick(), 3);
    assert_eq!(trace_error.sequence(), 1);
    assert_eq!(
        trace_error.request_id(),
        failure.record().failure().request_id()
    );
    assert_eq!(trace_error.error(), TrafficTraceErrorKind::FunctionalWrite);
    assert_eq!(trace_error.protocol(), RiscvDataCacheProtocol::Msi);
    assert_eq!(trace_error.target(), MemoryTargetId::new(0));
    assert_eq!(trace_error.address(), Address::new(0x9000));
    assert_eq!(trace_error.line(), Address::new(0x9000));
    assert_eq!(trace_error.size_bytes(), None);
    assert_eq!(trace_error.trace_packet_id(), Some(951));
    let memory_failures = traffic_replay.memory_failure_records();
    assert_eq!(memory_failures.len(), 1);
    assert_eq!(memory_failures[0].tick(), 3);
    assert_eq!(
        memory_failures[0].error(),
        TrafficTraceErrorKind::FunctionalWrite
    );
    assert_eq!(memory_failures[0].address(), Some(Address::new(0x9000)));
    assert_eq!(memory_failures[0].line(), Address::new(0x9000));
    assert_eq!(memory_failures[0].size_bytes(), None);
    assert_eq!(memory_failures[0].trace_packet_id(), Some(951));
    assert!(outcome.run().data_cache_runs().is_empty());
}

#[test]
fn workload_replay_delivers_trace_store_conditional_failed_response() {
    let manifest =
        replay_manifest_with_data_cache("riscv-replay-trace-store-conditional-failed-response");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_STORE_COND_FAIL_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(951),
        },
        PacketFields {
            tick: 3,
            command: GEM5_STORE_COND_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(951),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(64)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            controller,
            route_id("cpu0.data"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert!(traffic_replay.runtime().memory_failures().is_empty());
    assert_eq!(traffic_replay.response_deliveries().len(), 1);
    assert_eq!(
        traffic_replay.response_deliveries()[0].response().status(),
        ResponseStatus::StoreConditionalFailed
    );
    assert!(outcome.run().data_cache_runs().is_empty());
}

#[test]
fn workload_replay_applies_trace_flush_after_no_response_writeback() {
    let manifest =
        replay_manifest_with_data_cache("riscv-replay-trace-flush-after-no-response-writeback");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(952),
        },
        PacketFields {
            tick: 3,
            command: GEM5_READ_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(952),
        },
        PacketFields {
            tick: 4,
            command: GEM5_WRITEBACK_DIRTY,
            address: Some(0x9000),
            size: Some(64),
            packet_id: Some(953),
        },
        PacketFields {
            tick: 5,
            command: GEM5_FLUSH_REQ,
            address: Some(0x9000),
            size: Some(64),
            packet_id: Some(954),
        },
        PacketFields {
            tick: 6,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(955),
        },
        PacketFields {
            tick: 8,
            command: GEM5_READ_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(955),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(64)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            controller,
            route_id("cpu0.data"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert_eq!(traffic_replay.scheduled_count(), 4);
    assert_eq!(
        traffic_replay
            .memory_trace_events()
            .iter()
            .filter(|event| event.kind() == MemoryTraceKind::RequestSent)
            .count(),
        3
    );
    assert_eq!(traffic_replay.runtime().sideband_events().len(), 1);
    let data_cache_runs = outcome.run().data_cache_runs();
    assert_eq!(data_cache_runs.len(), 2);
    assert!(data_cache_runs[0].has_directory_activity());
    assert!(data_cache_runs[1].has_directory_activity());
}

#[test]
fn workload_replay_invalidates_data_cache_line_after_trace_read_with_invalidate() {
    let manifest = replay_manifest_with_data_cache("riscv-replay-trace-read-invalidate-data-cache");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(951),
        },
        PacketFields {
            tick: 3,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(951),
        },
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(952),
        },
        PacketFields {
            tick: 8,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(952),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(64)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            controller,
            route_id("cpu0.data"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert!(traffic_replay.runtime().memory_failures().is_empty());
    let response_records = traffic_replay.memory_response_records();
    assert_eq!(response_records.len(), 2);
    assert_eq!(response_records[0].tick(), 3);
    assert_eq!(response_records[0].trace_tick(), 3);
    assert_eq!(
        response_records[0].kind(),
        TrafficTraceResponseKind::ReadWithInvalidate
    );
    assert_eq!(response_records[0].status(), ResponseStatus::Completed);
    assert!(response_records[0].data_cache_response_applied());
    assert_eq!(response_records[1].tick(), 8);
    assert_eq!(response_records[1].trace_tick(), 8);
    assert_eq!(
        response_records[1].kind(),
        TrafficTraceResponseKind::ReadWithInvalidate
    );
    assert_eq!(response_records[1].status(), ResponseStatus::Completed);
    assert!(response_records[1].data_cache_response_applied());
    let data_cache_runs = outcome.run().data_cache_runs();
    assert_eq!(data_cache_runs.len(), 2);
    assert!(data_cache_runs[0].has_directory_activity());
    assert!(data_cache_runs[1].has_directory_activity());
}

#[test]
fn workload_replay_invalidates_data_cache_line_after_trace_clean_invalid_response() {
    let manifest = replay_manifest_with_data_cache("riscv-replay-trace-clean-invalid-data-cache");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(956),
        },
        PacketFields {
            tick: 3,
            command: GEM5_READ_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(956),
        },
        PacketFields {
            tick: 4,
            command: GEM5_CLEAN_INVALID_REQ,
            address: Some(0x9000),
            size: Some(64),
            packet_id: Some(957),
        },
        PacketFields {
            tick: 6,
            command: GEM5_CLEAN_INVALID_RESP,
            address: Some(0x9000),
            size: Some(64),
            packet_id: Some(957),
        },
        PacketFields {
            tick: 6,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(958),
        },
        PacketFields {
            tick: 8,
            command: GEM5_READ_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(958),
        },
    ]);

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
    assert!(traffic_replay.runtime().memory_failures().is_empty());
    let data_cache_runs = outcome.run().data_cache_runs();
    assert_eq!(data_cache_runs.len(), 3);
    assert!(data_cache_runs[0].has_directory_activity());
    assert!(data_cache_runs[1].has_directory_activity());
    assert!(data_cache_runs[2].has_directory_activity());
}

#[test]
fn workload_replay_cleans_data_cache_line_after_trace_clean_shared_response() {
    let manifest = replay_manifest_with_data_cache("riscv-replay-trace-clean-shared-data-cache");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_WRITE_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(961),
        },
        PacketFields {
            tick: 3,
            command: GEM5_WRITE_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(961),
        },
        PacketFields {
            tick: 4,
            command: GEM5_CLEAN_SHARED_REQ,
            address: Some(0x9000),
            size: Some(64),
            packet_id: Some(962),
        },
        PacketFields {
            tick: 7,
            command: GEM5_CLEAN_SHARED_RESP,
            address: Some(0x9000),
            size: Some(64),
            packet_id: Some(962),
        },
    ]);

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
    assert!(traffic_replay.runtime().memory_failures().is_empty());
    let line = outcome.memory_snapshot();
    let line = snapshot_line_data(line, MemoryTargetId::new(0), Address::new(0x9000));
    assert_eq!(&line[8..16], &[7; 8]);
}

#[test]
fn workload_replay_orders_trace_flush_before_delayed_read_response_cache_mutation() {
    let manifest =
        replay_manifest_with_data_cache("riscv-replay-trace-flush-before-delayed-response");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(953),
        },
        PacketFields {
            tick: 3,
            command: GEM5_FLUSH_REQ,
            address: Some(0x9000),
            size: Some(64),
            packet_id: Some(954),
        },
        PacketFields {
            tick: 4,
            command: GEM5_READ_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(953),
        },
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(955),
        },
        PacketFields {
            tick: 8,
            command: GEM5_READ_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(955),
        },
    ]);

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
    assert!(traffic_replay.runtime().is_empty(), "{traffic_replay:#?}");
    assert_eq!(traffic_replay.runtime().sideband_events().len(), 1);
    let data_cache_runs = outcome.run().data_cache_runs();
    assert_eq!(data_cache_runs.len(), 2);
    assert!(data_cache_runs[0].has_directory_activity());
    assert!(!data_cache_runs[1].has_directory_activity());
}

#[test]
fn workload_replay_records_trace_printreq_data_cache_diagnostic() {
    let manifest = replay_manifest_with_data_cache("riscv-replay-trace-print-data-cache");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(956),
        },
        PacketFields {
            tick: 3,
            command: GEM5_READ_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(956),
        },
        PacketFields {
            tick: 4,
            command: GEM5_PRINT_REQ,
            address: Some(0x9008),
            size: Some(1),
            packet_id: Some(957),
        },
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(958),
        },
        PacketFields {
            tick: 8,
            command: GEM5_READ_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(958),
        },
    ]);

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
    assert!(traffic_replay.runtime().is_empty(), "{traffic_replay:#?}");
    assert_eq!(traffic_replay.runtime().sideband_events().len(), 1);
    let replay_diagnostics = traffic_replay.trace_diagnostic_records();
    assert_eq!(replay_diagnostics.len(), 1);
    assert_eq!(
        replay_diagnostics[0].kind(),
        RiscvTraceDiagnosticKind::DataCacheLine
    );
    assert_eq!(replay_diagnostics[0].tick(), 4);
    assert_eq!(
        replay_diagnostics[0].protocol(),
        RiscvDataCacheProtocol::Msi
    );
    assert_eq!(replay_diagnostics[0].target(), MemoryTargetId::new(0));
    assert_eq!(replay_diagnostics[0].address(), Address::new(0x9008));
    assert_eq!(replay_diagnostics[0].line(), Address::new(0x9000));
    assert_eq!(replay_diagnostics[0].cached_copy_count(), 1);
    assert!(replay_diagnostics[0].has_cached_copy());
    assert!(replay_diagnostics[0].has_backing_line());
    let data_cache_runs = outcome.run().data_cache_runs();
    assert_eq!(data_cache_runs.len(), 2);
    assert!(data_cache_runs[0].has_directory_activity());
    assert!(!data_cache_runs[1].has_directory_activity());

    let diagnostics = outcome.run().trace_diagnostic_records();
    assert_eq!(diagnostics, replay_diagnostics);
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.kind(), RiscvTraceDiagnosticKind::DataCacheLine);
    assert_eq!(diagnostic.tick(), 4);
    assert_eq!(diagnostic.protocol(), RiscvDataCacheProtocol::Msi);
    assert_eq!(diagnostic.target(), MemoryTargetId::new(0));
    assert_eq!(diagnostic.address(), Address::new(0x9008));
    assert_eq!(diagnostic.line(), Address::new(0x9000));
    assert_eq!(diagnostic.cached_copy_count(), 1);
    assert!(diagnostic.has_cached_copy());
    assert!(diagnostic.has_backing_line());
    let summary = &outcome.result().traffic_trace_replay_summaries()[0];
    assert_eq!(summary.diagnostic_print_event_count(), 1);
    assert_eq!(summary.trace_diagnostic_count(), 1);
}

#[test]
fn workload_replay_applies_tlbi_ext_sync_to_data_translation_tlb() {
    let manifest =
        replay_manifest_with_data_translation("riscv-replay-trace-tlbi-data-translation");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[PacketFields {
        tick: 24,
        command: GEM5_TLBI_EXT_SYNC,
        address: Some(0),
        size: Some(64),
        packet_id: Some(959),
    }]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(128)
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
    assert_eq!(traffic_replay.runtime().sideband_events().len(), 1);
    let core = outcome.cluster().core(CpuId::new(0)).unwrap();
    assert_eq!(
        core.data_translation_tlb_stats(),
        Some(TranslationTlbStats::new(0, 1, 0, 1, 0))
    );
    assert_eq!(core.data_translation_tlb_entry_count(), Some(0));
    let summary = &outcome.result().traffic_trace_replay_summaries()[0];
    assert_eq!(summary.tlb_sync_event_count(), 1);
}

#[test]
fn workload_replay_binds_htm_abort_sideband_to_data_route_core() {
    let manifest = replay_manifest_with_data_cache("riscv-replay-trace-htm-abort-data-core");
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
    let htm_aborts = traffic_replay.htm_abort_records();
    assert_eq!(htm_aborts.len(), 1);
    assert!(matches!(
        htm_aborts[0].cluster_outcome(),
        RiscvClusterHtmAbortOutcome::NoActiveTransaction { cpu, .. }
            if *cpu == CpuId::new(0)
    ));
    assert_eq!(htm_aborts[0].trace_packet_id(), Some(960));
}

#[test]
fn workload_replay_binds_htm_request_response_to_data_route_transaction() {
    let manifest = replay_manifest_with_data_cache("riscv-replay-trace-htm-begin-data-core");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 1,
            command: GEM5_HTM_REQ,
            address: None,
            size: None,
            packet_id: Some(961),
        },
        PacketFields {
            tick: 2,
            command: GEM5_HTM_REQ_RESP,
            address: None,
            size: None,
            packet_id: Some(961),
        },
        PacketFields {
            tick: 3,
            command: GEM5_HTM_ABORT,
            address: None,
            size: None,
            packet_id: Some(963),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(128)
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
    assert_eq!(traffic_replay.runtime().control_acks().len(), 1);
    let htm_begins = traffic_replay.htm_begin_records();
    assert_eq!(htm_begins.len(), 1);
    assert!(matches!(
        htm_begins[0].cluster_outcome(),
        RiscvClusterHtmBeginOutcome::Begun { cpu, begin, .. }
            if *cpu == CpuId::new(0) && begin.depth() == 1
    ));
    assert_eq!(htm_begins[0].trace_packet_id(), Some(961));
    assert_eq!(htm_begins[0].tick(), 2);

    let htm_aborts = traffic_replay.htm_abort_records();
    assert_eq!(htm_aborts.len(), 1);
    assert!(matches!(
        htm_aborts[0].cluster_outcome(),
        RiscvClusterHtmAbortOutcome::Aborted { cpu, abort, .. }
        if *cpu == CpuId::new(0) && abort.uid() == htm_begins[0].begin_uid().unwrap()
    ));
}

#[test]
fn workload_replay_orders_same_tick_htm_response_before_abort_sideband() {
    let manifest =
        replay_manifest_with_data_cache("riscv-replay-trace-htm-same-tick-response-abort");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 1,
            command: GEM5_HTM_REQ,
            address: None,
            size: None,
            packet_id: Some(966),
        },
        PacketFields {
            tick: 2,
            command: GEM5_HTM_REQ_RESP,
            address: None,
            size: None,
            packet_id: Some(966),
        },
        PacketFields {
            tick: 2,
            command: GEM5_HTM_ABORT,
            address: None,
            size: None,
            packet_id: Some(967),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(128)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            controller,
            route_id("cpu0.data"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty(), "{traffic_replay:#?}");
    let htm_begins = traffic_replay.htm_begin_records();
    assert_eq!(htm_begins.len(), 1);
    assert_eq!(htm_begins[0].tick(), 2);
    assert_eq!(htm_begins[0].trace_packet_id(), Some(966));
    let begin_uid = htm_begins[0].begin_uid().unwrap();

    let htm_aborts = traffic_replay.htm_abort_records();
    assert_eq!(htm_aborts.len(), 1);
    assert_eq!(htm_aborts[0].tick(), 2);
    assert_eq!(htm_aborts[0].trace_packet_id(), Some(967));
    assert!(matches!(
        htm_aborts[0].cluster_outcome(),
        RiscvClusterHtmAbortOutcome::Aborted { cpu, abort, .. }
            if *cpu == CpuId::new(0) && abort.uid() == begin_uid
    ));
}

#[test]
fn workload_replay_does_not_begin_htm_transaction_for_trace_control_failure() {
    let manifest = replay_manifest_with_data_cache("riscv-replay-trace-htm-begin-error");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 1,
            command: GEM5_HTM_REQ,
            address: None,
            size: None,
            packet_id: Some(964),
        },
        PacketFields {
            tick: 2,
            command: GEM5_INVALID_DEST_ERROR,
            address: None,
            size: None,
            packet_id: Some(964),
        },
        PacketFields {
            tick: 3,
            command: GEM5_HTM_ABORT,
            address: None,
            size: None,
            packet_id: Some(965),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(128)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            controller,
            route_id("cpu0.data"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert!(traffic_replay.runtime().control_acks().is_empty());
    assert_eq!(traffic_replay.runtime().control_failures().len(), 1);
    assert_eq!(traffic_replay.runtime().control_failures()[0].tick(), 2);
    assert_eq!(
        traffic_replay.runtime().control_failures()[0]
            .record()
            .failure()
            .error(),
        TrafficTraceErrorKind::InvalidDestination
    );
    assert!(traffic_replay.htm_begin_records().is_empty());
    let htm_aborts = traffic_replay.htm_abort_records();
    assert_eq!(htm_aborts.len(), 1);
    assert!(matches!(
        htm_aborts[0].cluster_outcome(),
        RiscvClusterHtmAbortOutcome::NoActiveTransaction { cpu, .. }
            if *cpu == CpuId::new(0)
    ));
}

#[test]
fn workload_replay_records_htm_transaction_data_cache_access_sets() {
    let manifest = replay_manifest_with_data_cache("riscv-replay-trace-htm-access-sets");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 1,
            command: GEM5_HTM_REQ,
            address: None,
            size: None,
            packet_id: Some(964),
        },
        PacketFields {
            tick: 2,
            command: GEM5_HTM_REQ_RESP,
            address: None,
            size: None,
            packet_id: Some(964),
        },
        PacketFields {
            tick: 3,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(965),
        },
        PacketFields {
            tick: 5,
            command: GEM5_READ_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(965),
        },
        PacketFields {
            tick: 6,
            command: GEM5_WRITE_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(966),
        },
        PacketFields {
            tick: 8,
            command: GEM5_WRITE_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(966),
        },
        PacketFields {
            tick: 9,
            command: GEM5_HTM_ABORT,
            address: None,
            size: None,
            packet_id: Some(967),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
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
    let uid = traffic_replay.htm_begin_records()[0].begin_uid().unwrap();

    let records = outcome.run().trace_htm_access_records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].kind(), RiscvTraceHtmAccessKind::ReadSet);
    assert_eq!(records[0].transaction_uid(), uid);
    assert_eq!(records[0].tick(), 5);
    assert_eq!(records[0].trace_packet_id(), Some(965));
    assert_eq!(records[0].address(), Address::new(0x9008));
    assert_eq!(records[0].line(), Address::new(0x9000));
    assert_eq!(records[0].size_bytes(), 8);
    assert_eq!(records[0].protocol(), RiscvDataCacheProtocol::Msi);
    assert_eq!(records[1].kind(), RiscvTraceHtmAccessKind::WriteSet);
    assert_eq!(records[1].transaction_uid(), uid);
    assert_eq!(records[1].tick(), 8);
    assert_eq!(records[1].trace_packet_id(), Some(966));
    assert_eq!(records[1].address(), Address::new(0x9008));
    assert_eq!(records[1].line(), Address::new(0x9000));
    assert_eq!(records[1].size_bytes(), 8);
    assert_eq!(records[1].protocol(), RiscvDataCacheProtocol::Msi);
}

#[test]
fn workload_replay_rolls_back_htm_data_cache_writes_on_abort() {
    for (name, protocol) in [
        ("msi", WorkloadDataCacheProtocol::Msi),
        ("mesi", WorkloadDataCacheProtocol::Mesi),
        ("moesi", WorkloadDataCacheProtocol::Moesi),
        ("chi", WorkloadDataCacheProtocol::Chi),
    ] {
        let manifest = replay_manifest_with_data_cache_protocol(
            &format!("riscv-replay-trace-htm-abort-rollback-{name}"),
            protocol,
        );
        let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
        let controller = controller_for_packets(&[
            PacketFields {
                tick: 1,
                command: GEM5_HTM_REQ,
                address: None,
                size: None,
                packet_id: Some(971),
            },
            PacketFields {
                tick: 2,
                command: GEM5_HTM_REQ_RESP,
                address: None,
                size: None,
                packet_id: Some(971),
            },
            PacketFields {
                tick: 3,
                command: GEM5_WRITE_REQ,
                address: Some(0x9008),
                size: Some(8),
                packet_id: Some(972),
            },
            PacketFields {
                tick: 5,
                command: GEM5_WRITE_RESP,
                address: Some(0x9008),
                size: Some(8),
                packet_id: Some(972),
            },
            PacketFields {
                tick: 6,
                command: GEM5_HTM_ABORT,
                address: None,
                size: None,
                packet_id: Some(973),
            },
        ]);

        let outcome = RiscvWorkloadReplay::new(plan)
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
        assert_eq!(outcome.run().trace_htm_access_records().len(), 1);

        let line = outcome.memory_snapshot();
        let line = snapshot_line_data(line, MemoryTargetId::new(0), Address::new(0x9000));
        assert_eq!(&line[8..16], &0xfedc_ba98_7654_3210_u64.to_le_bytes());
    }
}

#[test]
fn workload_replay_keeps_htm_rollback_snapshots_route_scoped() {
    let manifest = replay_manifest_with_two_data_routes("riscv-replay-trace-htm-route-rollback");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let cpu0_controller = controller_for_packets(&[
        PacketFields {
            tick: 1,
            command: GEM5_HTM_REQ,
            address: None,
            size: None,
            packet_id: Some(974),
        },
        PacketFields {
            tick: 2,
            command: GEM5_HTM_REQ_RESP,
            address: None,
            size: None,
            packet_id: Some(974),
        },
        PacketFields {
            tick: 3,
            command: GEM5_HTM_ABORT,
            address: None,
            size: None,
            packet_id: Some(975),
        },
    ]);
    let cpu1_controller = controller_for_packets(&[
        PacketFields {
            tick: 1,
            command: GEM5_HTM_REQ,
            address: None,
            size: None,
            packet_id: Some(976),
        },
        PacketFields {
            tick: 2,
            command: GEM5_HTM_REQ_RESP,
            address: None,
            size: None,
            packet_id: Some(976),
        },
        PacketFields {
            tick: 4,
            command: GEM5_WRITE_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(977),
        },
        PacketFields {
            tick: 6,
            command: GEM5_WRITE_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(977),
        },
        PacketFields {
            tick: 7,
            command: GEM5_HTM_ABORT,
            address: None,
            size: None,
            packet_id: Some(978),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(192)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            cpu0_controller,
            route_id("cpu0.data"),
            PartitionId::new(2),
        ))
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            cpu1_controller,
            route_id("cpu1.data"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap();

    for traffic_replay in outcome.traffic_trace_replays() {
        assert!(traffic_replay.errors().is_empty());
        assert!(traffic_replay.runtime().is_empty(), "{traffic_replay:#?}");
    }

    let line = outcome.memory_snapshot();
    let line = snapshot_line_data(line, MemoryTargetId::new(0), Address::new(0x9000));
    assert_eq!(&line[8..16], &0xfedc_ba98_7654_3210_u64.to_le_bytes());
}

#[test]
fn workload_replay_preserves_other_route_cache_writes_across_htm_abort() {
    let manifest = replay_manifest_with_two_data_routes("riscv-replay-trace-htm-other-route-write");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let cpu0_controller = controller_for_packets(&[
        PacketFields {
            tick: 1,
            command: GEM5_HTM_REQ,
            address: None,
            size: None,
            packet_id: Some(979),
        },
        PacketFields {
            tick: 2,
            command: GEM5_HTM_REQ_RESP,
            address: None,
            size: None,
            packet_id: Some(979),
        },
        PacketFields {
            tick: 6,
            command: GEM5_HTM_ABORT,
            address: None,
            size: None,
            packet_id: Some(980),
        },
    ]);
    let cpu1_controller = controller_for_packets(&[
        PacketFields {
            tick: 3,
            command: GEM5_WRITE_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(981),
        },
        PacketFields {
            tick: 5,
            command: GEM5_WRITE_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(981),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(192)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            cpu0_controller,
            route_id("cpu0.data"),
            PartitionId::new(2),
        ))
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            cpu1_controller,
            route_id("cpu1.data"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap();

    for traffic_replay in outcome.traffic_trace_replays() {
        assert!(traffic_replay.errors().is_empty());
        assert!(traffic_replay.runtime().is_empty(), "{traffic_replay:#?}");
    }

    let line = outcome.memory_snapshot();
    let line = snapshot_line_data(line, MemoryTargetId::new(0), Address::new(0x9000));
    assert_eq!(&line[8..16], &[7; 8]);
}

#[test]
fn workload_replay_ignores_failed_store_conditional_for_htm_rollback_write_set() {
    let manifest = replay_manifest_with_two_data_routes("riscv-replay-trace-htm-failed-sc");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let cpu0_controller = controller_for_packets(&[
        PacketFields {
            tick: 1,
            command: GEM5_HTM_REQ,
            address: None,
            size: None,
            packet_id: Some(982),
        },
        PacketFields {
            tick: 2,
            command: GEM5_HTM_REQ_RESP,
            address: None,
            size: None,
            packet_id: Some(982),
        },
        PacketFields {
            tick: 3,
            command: GEM5_STORE_COND_FAIL_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(983),
        },
        PacketFields {
            tick: 5,
            command: GEM5_STORE_COND_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(983),
        },
        PacketFields {
            tick: 7,
            command: GEM5_HTM_ABORT,
            address: None,
            size: None,
            packet_id: Some(984),
        },
    ]);
    let cpu1_controller = controller_for_packets(&[
        PacketFields {
            tick: 4,
            command: GEM5_WRITE_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(985),
        },
        PacketFields {
            tick: 6,
            command: GEM5_WRITE_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(985),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(192)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            cpu0_controller,
            route_id("cpu0.data"),
            PartitionId::new(2),
        ))
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            cpu1_controller,
            route_id("cpu1.data"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap();

    for traffic_replay in outcome.traffic_trace_replays() {
        assert!(traffic_replay.errors().is_empty());
        assert!(traffic_replay.runtime().is_empty(), "{traffic_replay:#?}");
    }

    let line = outcome.memory_snapshot();
    let line = snapshot_line_data(line, MemoryTargetId::new(0), Address::new(0x9000));
    assert_eq!(&line[8..16], &[7; 8]);
}

#[test]
fn workload_replay_uses_executable_store_conditional_status_for_htm_rollback_write_set() {
    let manifest = replay_manifest_with_two_data_routes("riscv-replay-trace-htm-sc-status");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let cpu0_controller = controller_for_packets(&[
        PacketFields {
            tick: 1,
            command: GEM5_HTM_REQ,
            address: None,
            size: None,
            packet_id: Some(986),
        },
        PacketFields {
            tick: 2,
            command: GEM5_HTM_REQ_RESP,
            address: None,
            size: None,
            packet_id: Some(986),
        },
        PacketFields {
            tick: 3,
            command: GEM5_STORE_COND_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(987),
        },
        PacketFields {
            tick: 5,
            command: GEM5_STORE_COND_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(987),
        },
        PacketFields {
            tick: 7,
            command: GEM5_HTM_ABORT,
            address: None,
            size: None,
            packet_id: Some(988),
        },
    ]);
    let cpu1_controller = controller_for_packets(&[
        PacketFields {
            tick: 4,
            command: GEM5_WRITE_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(989),
        },
        PacketFields {
            tick: 6,
            command: GEM5_WRITE_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(989),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(192)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            cpu0_controller,
            route_id("cpu0.data"),
            PartitionId::new(2),
        ))
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            cpu1_controller,
            route_id("cpu1.data"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap();

    for traffic_replay in outcome.traffic_trace_replays() {
        assert!(traffic_replay.errors().is_empty());
        assert!(traffic_replay.runtime().is_empty(), "{traffic_replay:#?}");
    }

    let line = outcome.memory_snapshot();
    let line = snapshot_line_data(line, MemoryTargetId::new(0), Address::new(0x9000));
    assert_eq!(&line[8..16], &[7; 8]);
}

#[test]
fn workload_replay_orders_htm_begin_after_earlier_same_tick_cache_response() {
    let manifest = replay_manifest_with_data_cache("riscv-replay-trace-htm-begin-after-write");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 1,
            command: GEM5_WRITE_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(990),
        },
        PacketFields {
            tick: 2,
            command: GEM5_HTM_REQ,
            address: None,
            size: None,
            packet_id: Some(991),
        },
        PacketFields {
            tick: 4,
            command: GEM5_WRITE_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(990),
        },
        PacketFields {
            tick: 4,
            command: GEM5_HTM_REQ_RESP,
            address: None,
            size: None,
            packet_id: Some(991),
        },
        PacketFields {
            tick: 5,
            command: GEM5_WRITE_REQ,
            address: Some(0x9010),
            size: Some(8),
            packet_id: Some(992),
        },
        PacketFields {
            tick: 7,
            command: GEM5_WRITE_RESP,
            address: Some(0x9010),
            size: Some(8),
            packet_id: Some(992),
        },
        PacketFields {
            tick: 8,
            command: GEM5_HTM_ABORT,
            address: None,
            size: None,
            packet_id: Some(993),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(192)
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

    let line = outcome.memory_snapshot();
    let line = snapshot_line_data(line, MemoryTargetId::new(0), Address::new(0x9000));
    assert_eq!(&line[8..16], &[7; 8]);
    assert_eq!(&line[16..24], &[0; 8]);
}

#[test]
fn workload_replay_orders_htm_access_sets_across_data_cache_lines() {
    let manifest = replay_manifest_with_two_data_cache_lines("riscv-replay-trace-htm-line-order");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 1,
            command: GEM5_HTM_REQ,
            address: None,
            size: None,
            packet_id: Some(968),
        },
        PacketFields {
            tick: 2,
            command: GEM5_HTM_REQ_RESP,
            address: None,
            size: None,
            packet_id: Some(968),
        },
        PacketFields {
            tick: 3,
            command: GEM5_READ_REQ,
            address: Some(0x9048),
            size: Some(8),
            packet_id: Some(969),
        },
        PacketFields {
            tick: 5,
            command: GEM5_READ_RESP,
            address: Some(0x9048),
            size: Some(8),
            packet_id: Some(969),
        },
        PacketFields {
            tick: 6,
            command: GEM5_WRITE_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(970),
        },
        PacketFields {
            tick: 8,
            command: GEM5_WRITE_RESP,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(970),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(160)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            controller,
            route_id("cpu0.data"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap();

    let records = outcome.run().trace_htm_access_records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].trace_packet_id(), Some(969));
    assert_eq!(records[0].tick(), 5);
    assert_eq!(records[0].line(), Address::new(0x9040));
    assert_eq!(records[1].trace_packet_id(), Some(970));
    assert_eq!(records[1].tick(), 8);
    assert_eq!(records[1].line(), Address::new(0x9000));
}

#[test]
fn workload_replay_does_not_apply_fetch_trace_to_data_cache_line() {
    let manifest = replay_manifest_with_data_cache("riscv-replay-fetch-trace-data-cache-scope");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(943),
        },
        PacketFields {
            tick: 3,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(943),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(64)
        .with_traffic_trace_replay(RiscvWorkloadTrafficTraceReplay::new(
            controller,
            route_id("cpu0.fetch"),
            PartitionId::new(2),
        ))
        .run_parallel()
        .unwrap();

    let traffic_replay = &outcome.traffic_trace_replays()[0];
    assert!(traffic_replay.errors().is_empty());
    assert!(traffic_replay.runtime().sideband_events().is_empty());
    assert!(outcome.run().data_cache_runs().is_empty());
}

#[test]
fn workload_replay_orders_trace_flush_after_earlier_request_delivery_tick() {
    let manifest = replay_manifest_with_data_cache("riscv-replay-trace-flush-delivery-order");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 2,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(944),
        },
        PacketFields {
            tick: 4,
            command: GEM5_FLUSH_REQ,
            address: Some(0x9000),
            size: Some(64),
            packet_id: Some(945),
        },
        PacketFields {
            tick: 5,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(944),
        },
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(946),
        },
        PacketFields {
            tick: 8,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(946),
        },
    ]);

    let outcome = RiscvWorkloadReplay::new(plan)
        .with_max_turns(64)
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
    let data_cache_runs = outcome.run().data_cache_runs();
    assert_eq!(data_cache_runs.len(), 2);
    assert!(data_cache_runs[0].has_directory_activity());
    assert!(data_cache_runs[1].has_directory_activity());
}

#[test]
fn workload_replay_applies_bound_trace_flush_to_mesi_data_cache_line() {
    let manifest = replay_manifest_with_data_cache_protocol(
        "riscv-replay-trace-flush-mesi-data-cache",
        WorkloadDataCacheProtocol::Mesi,
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(947),
        },
        PacketFields {
            tick: 3,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(947),
        },
        PacketFields {
            tick: 4,
            command: GEM5_FLUSH_REQ,
            address: Some(0x9000),
            size: Some(64),
            packet_id: Some(948),
        },
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(949),
        },
        PacketFields {
            tick: 8,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x9008),
            size: Some(8),
            packet_id: Some(949),
        },
    ]);

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
    let data_cache_runs = outcome.run().data_cache_runs();
    assert_eq!(data_cache_runs.len(), 2);
    assert!(data_cache_runs[0].has_directory_activity());
    assert!(data_cache_runs[1].has_directory_activity());
}

#[test]
fn workload_replay_result_satisfies_bound_traffic_trace_summary_expectation() {
    let manifest =
        replay_manifest_with_trace_summary_expectation("riscv-replay-traffic-trace-summary");
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let outcome = replay_manifest_with_controller(
        manifest,
        &[
            PacketFields {
                tick: 0,
                command: GEM5_READ_REQ,
                address: Some(0xa000),
                size: Some(8),
                packet_id: Some(910),
            },
            PacketFields {
                tick: 3,
                command: GEM5_READ_RESP_WITH_INVALIDATE,
                address: Some(0xa000),
                size: Some(8),
                packet_id: Some(910),
            },
        ],
    )
    .unwrap();

    let summaries = outcome.result().traffic_trace_replay_summaries();
    assert_eq!(summaries.len(), 1);
    let summary = &summaries[0];
    assert_eq!(summary.route(), &route_id("cpu0.fetch"));
    assert_eq!(summary.scheduled_count(), 1);
    assert_eq!(summary.response_delivery_count(), 1);
    assert_eq!(summary.memory_trace_event_count(), 3);
    assert_eq!(summary.memory_failure_count(), 0);
    assert_eq!(summary.control_ack_count(), 0);
    assert_eq!(summary.control_failure_count(), 0);
    assert_eq!(summary.sideband_event_count(), 0);
    assert_eq!(summary.tlb_sync_event_count(), 0);
    assert_eq!(summary.cache_flush_event_count(), 0);
    assert_eq!(summary.trace_cache_flush_count(), 0);
    assert_eq!(summary.diagnostic_print_event_count(), 0);
    assert_eq!(summary.trace_diagnostic_count(), 0);
    assert_eq!(summary.htm_abort_event_count(), 0);
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_fails_when_bound_traffic_trace_summary_expectation_is_underreported() {
    let manifest = replay_manifest_with_strict_trace_summary_expectation(
        "riscv-replay-traffic-trace-summary-mismatch",
    );
    let error = replay_manifest_with_controller(
        manifest,
        &[
            PacketFields {
                tick: 0,
                command: GEM5_READ_REQ,
                address: Some(0xa000),
                size: Some(8),
                packet_id: Some(911),
            },
            PacketFields {
                tick: 3,
                command: GEM5_READ_RESP_WITH_INVALIDATE,
                address: Some(0xa000),
                size: Some(8),
                packet_id: Some(911),
            },
        ],
    )
    .unwrap_err();

    let RiscvWorkloadReplayError::Workload(
        rem6_workload::WorkloadError::TrafficTraceReplaySummaryExpectation(error),
    ) = error
    else {
        panic!("expected workload traffic trace replay summary expectation error");
    };
    let WorkloadTrafficTraceReplaySummaryExpectationError::BelowMinimum { expected, actual } =
        error.as_ref()
    else {
        panic!("expected traffic trace replay summary below-minimum error");
    };
    assert_eq!(expected.route(), &route_id("cpu0.fetch"));
    assert_eq!(expected.minimum_response_delivery_count(), 2);
    assert_eq!(actual.route(), &route_id("cpu0.fetch"));
    assert_eq!(actual.scheduled_count(), 1);
    assert_eq!(actual.response_delivery_count(), 1);
    assert_eq!(actual.memory_trace_event_count(), 3);
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

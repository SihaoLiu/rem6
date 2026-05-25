use rem6_boot::BootImage;
use rem6_cpu::CpuId;
use rem6_dram::{DramGeometry, DramMemoryTechnology, DramTiming, ExternalMemoryProfile};
use rem6_isa_riscv::Register;
use rem6_memory::{AccessSize, Address, AddressRange, CacheLineLayout, MemoryTargetId};
use rem6_system::{
    ExecutionMode, ExecutionModeTarget, GuestHostCallResponse, RiscvSystemRunStopReason,
    RiscvWorkloadReplay, SystemActionOutcome,
};
use rem6_workload::{
    HostEventIntent, WorkloadDataCacheProtocol, WorkloadExecutionMode, WorkloadExecutionModeSwitch,
    WorkloadExpectedDataCacheProtocolRunCount, WorkloadExpectedDataCacheRunAttribution,
    WorkloadGuestHostCallResponse, WorkloadHostActionSummary, WorkloadHostEvent,
    WorkloadHostPlacement, WorkloadManifest, WorkloadMemoryRoute, WorkloadMemoryTarget,
    WorkloadReplayPlan, WorkloadResource, WorkloadResourceId, WorkloadResourceKind,
    WorkloadRiscvCore, WorkloadRiscvDataCache, WorkloadRouteFabric, WorkloadRouteHop,
    WorkloadRouteId, WorkloadStatsScope, WorkloadTopology,
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

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn stat_value(stats: &rem6_stats::StatSnapshot, path: &str) -> Option<u64> {
    stats
        .samples()
        .iter()
        .find(|sample| sample.path() == path)
        .map(rem6_stats::StatSample::value)
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

fn u_type(imm: i32, rd: u8, opcode: u32) -> u32 {
    ((imm as u32) & 0xffff_f000) | (u32::from(rd) << 7) | opcode
}

fn j_type(imm: i32, rd: u8) -> u32 {
    let imm = imm as u32;
    (((imm >> 20) & 0x1) << 31)
        | (((imm >> 1) & 0x3ff) << 21)
        | (((imm >> 11) & 0x1) << 20)
        | (((imm >> 12) & 0xff) << 12)
        | (u32::from(rd) << 7)
        | 0x6f
}

fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32, opcode: u32) -> u32 {
    let imm = (imm as u32) & 0x0fff;
    ((imm >> 5) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | opcode
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
        .add_segment(Address::new(0x9000), word(0x0010_0073))
        .unwrap()
}

fn boot_image_with_second_target_entry() -> BootImage {
    BootImage::new(Address::new(0x9000))
        .add_segment(Address::new(0x9000), word(0x0000_0073))
        .unwrap()
}

fn boot_image_with_second_target_jump() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(j_type(0x1000, 0)))
        .unwrap()
        .add_segment(Address::new(0x9000), word(0x0000_0073))
        .unwrap()
}

fn boot_image_with_data_load() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(u_type(0x9000, 2, 0x37)))
        .unwrap()
        .add_segment(Address::new(0x8004), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x8008), word(0x0000_0073))
        .unwrap()
        .add_segment(
            Address::new(0x9008),
            0xfedc_ba98_7654_3210_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
}

fn boot_image_with_two_data_loads() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(u_type(0x9000, 2, 0x37)))
        .unwrap()
        .add_segment(Address::new(0x8004), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x8008), word(i_type(24, 2, 0x3, 6, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x800c), word(0x0000_0073))
        .unwrap()
        .add_segment(
            Address::new(0x9008),
            0xfedc_ba98_7654_3210_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
        .add_segment(
            Address::new(0x9018),
            0x0123_4567_89ab_cdef_u64.to_le_bytes().to_vec(),
        )
        .unwrap()
}

fn boot_image_with_data_store() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(u_type(0x9000, 2, 0x37)))
        .unwrap()
        .add_segment(Address::new(0x8004), word(i_type(0x7b, 0, 0x0, 3, 0x13)))
        .unwrap()
        .add_segment(Address::new(0x8008), word(s_type(8, 3, 2, 0x3, 0x23)))
        .unwrap()
        .add_segment(Address::new(0x800c), word(0x0000_0073))
        .unwrap()
        .add_segment(Address::new(0x9008), vec![0; 8])
        .unwrap()
}

fn dram_geometry() -> DramGeometry {
    DramGeometry::new(4, 64, 16).unwrap()
}

fn dram_timing() -> DramTiming {
    DramTiming::new(4, 8, 10, 3, 5).unwrap()
}

fn hbm_profile(target: u32) -> ExternalMemoryProfile {
    ExternalMemoryProfile::hbm(
        MemoryTargetId::new(target),
        layout(),
        2,
        2,
        dram_geometry(),
        dram_timing(),
    )
    .unwrap()
}

fn single_channel_ddr_profile(target: u32) -> ExternalMemoryProfile {
    ExternalMemoryProfile::ddr(
        MemoryTargetId::new(target),
        layout(),
        1,
        1,
        DramGeometry::new(2, 64, 16).unwrap(),
        dram_timing(),
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
                16,
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
            WorkloadMemoryRoute::new(route_id("cpu1.fetch"), "cpu1.ifetch", 1, "memory", 2, 2, 3)
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
        .add_riscv_core(
            WorkloadRiscvCore::new(
                1,
                1,
                8,
                Address::new(0x9000),
                "cpu1.ifetch",
                route_id("cpu1.fetch"),
            )
            .unwrap(),
        )
        .unwrap()
}

fn replay_topology_with_second_target_entry() -> WorkloadTopology {
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
        .add_memory_target(
            WorkloadMemoryTarget::new(
                1,
                32,
                AddressRange::new(Address::new(0x9000), AccessSize::new(0x1000).unwrap()).unwrap(),
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
                Address::new(0x9000),
                "cpu0.ifetch",
                route_id("cpu0.fetch"),
            )
            .unwrap(),
        )
        .unwrap()
}

fn replay_topology_with_second_target_jump() -> WorkloadTopology {
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
        .add_memory_target(
            WorkloadMemoryTarget::new(
                1,
                32,
                AddressRange::new(Address::new(0x9000), AccessSize::new(0x1000).unwrap()).unwrap(),
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

fn replay_topology_with_second_target_data() -> WorkloadTopology {
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
        .add_memory_target(
            WorkloadMemoryTarget::new(
                1,
                32,
                AddressRange::new(Address::new(0x9000), AccessSize::new(0x1000).unwrap()).unwrap(),
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

fn replay_topology_with_profiled_contended_fetches() -> WorkloadTopology {
    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 51).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                16,
                AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap()).unwrap(),
            )
            .unwrap()
            .with_external_memory_profile(single_channel_ddr_profile(0))
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 2, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu1.fetch"), "cpu1.ifetch", 1, "memory", 2, 2, 3)
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
        .add_riscv_core(
            WorkloadRiscvCore::new(
                1,
                1,
                8,
                Address::new(0x8000),
                "cpu1.ifetch",
                route_id("cpu1.fetch"),
            )
            .unwrap(),
        )
        .unwrap()
}

fn replay_topology_with_fabric_fetch() -> WorkloadTopology {
    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 51).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                16,
                AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 2, 3)
                .unwrap()
                .with_fabric(
                    WorkloadRouteFabric::new("cpu_mem", 4)
                        .unwrap()
                        .with_virtual_networks(1, 2)
                        .with_credit_depth(2)
                        .unwrap(),
                ),
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

fn replay_topology_with_multihop_fabric_fetch() -> WorkloadTopology {
    let cpu_to_router = WorkloadRouteFabric::new("cpu_router", 4)
        .unwrap()
        .with_virtual_networks(1, 2)
        .with_credit_depth(2)
        .unwrap();
    let router_to_memory = WorkloadRouteFabric::new("router_memory", 8)
        .unwrap()
        .with_virtual_networks(3, 4)
        .with_credit_depth(2)
        .unwrap();

    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 51).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                16,
                AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new_path(
                route_id("cpu0.fetch"),
                "cpu0.ifetch",
                0,
                [
                    WorkloadRouteHop::new("router0.cpu", 1, 2, 2)
                        .unwrap()
                        .with_fabric(cpu_to_router),
                    WorkloadRouteHop::new("memory", 2, 2, 3)
                        .unwrap()
                        .with_fabric(router_to_memory),
                ],
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
            .unwrap(),
        )
        .unwrap()
}

fn replay_topology_with_data_route() -> WorkloadTopology {
    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 51).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                16,
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

fn add_data_cache_backing_route(topology: WorkloadTopology) -> WorkloadTopology {
    topology
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
}

fn replay_topology_with_data_cache(protocol: WorkloadDataCacheProtocol) -> WorkloadTopology {
    add_data_cache_backing_route(replay_topology_with_data_route())
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

fn replay_topology_with_msi_data_cache() -> WorkloadTopology {
    replay_topology_with_data_cache(WorkloadDataCacheProtocol::Msi)
}

fn replay_topology_with_msi_data_cache_lines() -> WorkloadTopology {
    add_data_cache_backing_route(replay_topology_with_data_route())
        .with_riscv_data_cache(
            WorkloadRiscvDataCache::new(
                WorkloadDataCacheProtocol::Msi,
                0,
                Address::new(0x9000),
                2,
                "dcache.dir",
                route_id("dcache.backing"),
            )
            .unwrap()
            .with_line_address(Address::new(0x9010)),
        )
        .unwrap()
}

fn replay_topology_with_profiled_data_route() -> WorkloadTopology {
    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 51).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                16,
                AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap()).unwrap(),
            )
            .unwrap()
            .with_external_memory_profile(hbm_profile(0))
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

fn replay_topology_with_profiled_msi_data_cache() -> WorkloadTopology {
    add_data_cache_backing_route(replay_topology_with_profiled_data_route())
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
    WorkloadManifest::builder(workload_id("riscv-replay"), boot_image())
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

fn replay_manifest_with_second_target_entry() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-second-target-entry"),
        boot_image_with_second_target_entry(),
    )
    .with_topology(replay_topology_with_second_target_entry())
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

fn replay_manifest_with_second_target_jump() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-second-target-jump"),
        boot_image_with_second_target_jump(),
    )
    .with_topology(replay_topology_with_second_target_jump())
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

fn replay_manifest_with_second_target_data_load() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-second-target-data-load"),
        boot_image_with_data_load(),
    )
    .with_topology(replay_topology_with_second_target_data())
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

fn replay_manifest_with_profiled_contended_fetches() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-profiled-contended-fetches"),
        boot_image(),
    )
    .with_topology(replay_topology_with_profiled_contended_fetches())
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

fn replay_manifest_with_fabric_fetch() -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("riscv-replay-fabric-fetch"), boot_image())
        .with_topology(replay_topology_with_fabric_fetch())
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

fn replay_manifest_with_multihop_fabric_fetch() -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("multihop-fabric-fetch"), boot_image())
        .with_topology(replay_topology_with_multihop_fabric_fetch())
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

fn replay_manifest_with_profiled_data_load() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-profiled-data-load"),
        boot_image_with_data_load(),
    )
    .with_topology(replay_topology_with_profiled_data_route())
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

fn replay_manifest_with_profiled_msi_data_cache_load() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-profiled-msi-data-cache-load"),
        boot_image_with_data_load(),
    )
    .with_topology(replay_topology_with_profiled_msi_data_cache())
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

fn replay_manifest_with_data_load() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-data-load"),
        boot_image_with_data_load(),
    )
    .with_topology(replay_topology_with_data_route())
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

fn replay_manifest_with_msi_data_cache_load() -> WorkloadManifest {
    replay_manifest_with_data_cache_load(
        WorkloadDataCacheProtocol::Msi,
        "riscv-replay-msi-data-cache-load",
    )
}

fn replay_manifest_with_data_cache_load(
    protocol: WorkloadDataCacheProtocol,
    workload: &str,
) -> WorkloadManifest {
    WorkloadManifest::builder(workload_id(workload), boot_image_with_data_load())
        .with_topology(replay_topology_with_data_cache(protocol))
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_expected_data_cache_protocol_run_count(
            WorkloadExpectedDataCacheProtocolRunCount::new(protocol, 1).unwrap(),
        )
        .unwrap()
        .add_expected_data_cache_run_attribution(WorkloadExpectedDataCacheRunAttribution::new(1, 0))
        .unwrap()
        .add_host_event(WorkloadHostEvent::new(
            0,
            HostEventIntent::Stop {
                reason: "host-stop".to_string(),
            },
        ))
        .build()
        .unwrap()
}

fn replay_manifest_with_msi_data_cache_loads() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-msi-data-cache-loads"),
        boot_image_with_two_data_loads(),
    )
    .with_topology(replay_topology_with_msi_data_cache_lines())
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

fn replay_manifest_with_msi_data_cache_store() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-msi-data-cache-store"),
        boot_image_with_data_store(),
    )
    .with_topology(replay_topology_with_msi_data_cache())
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

fn replay_manifest_with_data_store() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-data-store"),
        boot_image_with_data_store(),
    )
    .with_topology(replay_topology_with_data_route())
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

fn replay_manifest_with_planned_host_actions() -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("riscv-replay-host-actions"), boot_image())
        .with_topology(replay_topology())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            1,
            HostEventIntent::RoiBegin {
                label: "roi".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            1,
            HostEventIntent::Checkpoint {
                label: "after-boot".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            1,
            HostEventIntent::RestoreCheckpoint {
                label: "after-boot".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            2,
            HostEventIntent::SwitchExecutionMode {
                target: "cpu0".to_string(),
                mode: WorkloadExecutionMode::Functional,
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            2,
            HostEventIntent::RoiEnd {
                label: "roi".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            0,
            HostEventIntent::Stop {
                reason: "host-stop".to_string(),
            },
        ))
        .build()
        .unwrap()
}

fn replay_manifest_with_planned_guest_host_call() -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("riscv-replay-guest-host-call"), boot_image())
        .with_topology(replay_topology())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            1,
            HostEventIntent::GuestHostCall {
                selector: 0x900,
                arguments: vec![11, 13],
                payload: vec![2, 4, 6],
                response: Some(WorkloadGuestHostCallResponse::ok(
                    vec![21, 34],
                    vec![0xde, 0xad],
                )),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            0,
            HostEventIntent::Stop {
                reason: "host-stop".to_string(),
            },
        ))
        .build()
        .unwrap()
}

#[test]
fn workload_replay_plan_reconstructs_parallel_riscv_system_run() {
    let manifest = replay_manifest();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(20)
        .run_parallel()
        .unwrap();

    assert_eq!(outcome.result().manifest_identity(), manifest.identity());
    assert_eq!(
        outcome.result().final_tick(),
        outcome.run().final_tick().unwrap()
    );
    assert_eq!(outcome.result().stop_reason(), Some("host-stop"));
    assert_eq!(outcome.result().checkpoint_labels(), &[] as &[String]);
    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert_eq!(
        summary.scheduler_epoch_count(),
        outcome.run().parallel_scheduler_profile().epoch_count(),
    );
    assert_eq!(
        summary.scheduler_dispatch_count(),
        outcome.run().parallel_scheduler_profile().dispatch_count(),
    );
    assert_eq!(
        summary.scheduler_batch_count(),
        outcome.run().parallel_scheduler_profile().batch_count(),
    );
    assert_eq!(
        summary.active_scheduler_partition_count(),
        outcome.run().active_parallel_scheduler_partition_count(),
    );
    assert_eq!(
        summary.max_parallel_scheduler_workers(),
        outcome.run().max_parallel_scheduler_workers(),
    );
    assert_eq!(summary.riscv_core_count(), 2);
    assert_eq!(
        summary.active_riscv_core_count(),
        outcome.run().active_cpu_count(),
    );
    assert_eq!(summary.active_riscv_core_count(), 2);
    let run_cpu_activity = outcome.run().cpu_activities();
    assert_eq!(
        summary.riscv_fetch_issue_count(),
        run_cpu_activity
            .values()
            .map(|activity| activity.fetch_issue_count())
            .sum::<usize>(),
    );
    assert_eq!(
        summary.riscv_committed_instruction_count(),
        run_cpu_activity
            .values()
            .map(|activity| activity.instruction_execution_count())
            .sum::<usize>(),
    );
    assert_eq!(summary.riscv_committed_instruction_count(), 2);
    assert_eq!(
        summary.riscv_data_access_issue_count(),
        run_cpu_activity
            .values()
            .map(|activity| activity.data_access_issue_count())
            .sum::<usize>(),
    );
    assert_eq!(
        summary.riscv_scheduled_trap_count(),
        run_cpu_activity
            .values()
            .map(|activity| activity.scheduled_trap_count())
            .sum::<usize>(),
    );
    assert_eq!(summary.riscv_scheduled_trap_count(), 2);
    assert!(summary.has_riscv_core_activity());
    assert_eq!(summary.data_cache_parallel_run_count(), 0);
    assert_eq!(summary.attributed_data_cache_parallel_run_count(), 0);
    assert_eq!(summary.unattributed_data_cache_parallel_run_count(), 0);
    assert!(summary.data_cache_protocol_counts().is_empty());
    assert!(summary.data_cache_protocols().is_empty());
    assert_eq!(summary.attributed_data_cache_protocol_run_count(), 0);
    assert_eq!(
        summary.data_cache_parallel_run_count_for_protocol(WorkloadDataCacheProtocol::Msi),
        0,
    );
    assert!(!summary.has_data_cache_protocol(WorkloadDataCacheProtocol::Msi));
    assert_eq!(summary.data_cache_wait_for_edge_count(), 0);
    assert!(!summary.has_unattributed_data_cache_parallel_runs());
    assert!(!summary.has_data_cache_diagnostics());
    assert_eq!(
        summary.fabric_wait_for_edge_count(),
        outcome.run().fabric_wait_for_edge_count(),
    );
    assert_eq!(
        summary.dram_wait_for_edge_count(),
        outcome.run().dram_wait_for_edge_count(),
    );
    assert_eq!(
        summary.resource_wait_for_edge_count(),
        outcome.run().fabric_wait_for_edge_count() + outcome.run().dram_wait_for_edge_count(),
    );
    assert_eq!(
        summary.full_system_wait_for_edge_count(),
        summary.resource_wait_for_edge_count()
            + summary.data_cache_wait_for_edge_count()
            + summary.compute_wait_for_edge_count()
            + summary.dma_wait_for_edge_count(),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_dispatch_count(),
        outcome
            .run()
            .full_system_parallel_scheduler_dispatch_count(),
    );
    assert!(summary.has_full_system_parallel_scheduler_work());
    assert!(summary.has_parallel_scheduler_work());
    assert!(!summary.has_data_cache_parallel_work());
    plan.verify_result(outcome.result()).unwrap();

    assert!(matches!(
        outcome.run().stop_reason(),
        RiscvSystemRunStopReason::HostStop(_)
    ));
    let stats = outcome.result().stats_snapshot().unwrap();
    assert_eq!(stat_value(stats, "cpu0.committed_insts"), Some(1));
    assert_eq!(stat_value(stats, "cpu1.committed_insts"), Some(1));
    assert_eq!(outcome.run().scheduled_traps().len(), 2);
    assert!(outcome.run().active_partition_count() >= 2);
    assert!(outcome.run().max_parallel_scheduler_workers() >= 1);
}

#[test]
fn workload_replay_uses_entry_target_layout_for_fetches() {
    let manifest = replay_manifest_with_second_target_entry();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(20)
        .run_parallel()
        .unwrap();

    assert_eq!(outcome.result().manifest_identity(), manifest.identity());
    assert_eq!(outcome.result().stop_reason(), Some("host-stop"));
    assert_eq!(outcome.run().active_cpu_count(), 1);
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_uses_fetch_target_layout_after_pc_redirect() {
    let manifest = replay_manifest_with_second_target_jump();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    let core = outcome.cluster().core(CpuId::new(0)).unwrap();
    assert_eq!(core.pc(), Address::new(0x9000));
    assert_eq!(outcome.run().active_cpu_count(), 1);
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_uses_data_target_layout_for_data_accesses() {
    let manifest = replay_manifest_with_second_target_data_load();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    assert_eq!(
        outcome
            .cluster()
            .core(CpuId::new(0))
            .unwrap()
            .read_register(Register::new(5).unwrap()),
        0xfedc_ba98_7654_3210
    );
    assert_eq!(outcome.run().active_cpu_count(), 1);
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_summary_reports_resource_wait_diagnostics() {
    let manifest = replay_manifest_with_profiled_contended_fetches();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert!(outcome.run().has_dram_wait_for_edges());
    assert_eq!(
        summary.fabric_wait_for_edge_count(),
        outcome.run().fabric_wait_for_edge_count(),
    );
    assert_eq!(
        summary.fabric_deadlock_diagnostic_count(),
        outcome.run().fabric_deadlock_diagnostic_count(),
    );
    assert_eq!(
        summary.dram_wait_for_edge_count(),
        outcome.run().dram_wait_for_edge_count(),
    );
    assert_eq!(
        summary.dram_deadlock_diagnostic_count(),
        outcome.run().dram_deadlock_diagnostic_count(),
    );
    assert_eq!(
        summary.resource_wait_for_edge_count(),
        outcome.run().fabric_wait_for_edge_count() + outcome.run().dram_wait_for_edge_count(),
    );
    assert_eq!(
        summary.resource_deadlock_diagnostic_count(),
        outcome.run().fabric_deadlock_diagnostic_count()
            + outcome.run().dram_deadlock_diagnostic_count(),
    );
    assert_eq!(
        summary.full_system_wait_for_edge_count(),
        summary.resource_wait_for_edge_count()
            + summary.data_cache_wait_for_edge_count()
            + summary.compute_wait_for_edge_count()
            + summary.dma_wait_for_edge_count(),
    );
    assert_eq!(
        summary.full_system_deadlock_diagnostic_count(),
        summary.resource_deadlock_diagnostic_count()
            + summary.data_cache_deadlock_diagnostic_count()
            + summary.compute_deadlock_diagnostic_count()
            + summary.dma_deadlock_diagnostic_count(),
    );
    assert!(summary.has_dram_diagnostics());
    assert!(summary.has_resource_diagnostics());
    assert!(summary.has_full_system_diagnostics());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_routes_declared_fabric_path_and_reports_activity() {
    let manifest = replay_manifest_with_fabric_fetch();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert!(outcome.run().has_fabric_activity());
    assert_eq!(
        summary.fabric_transfer_count(),
        outcome.run().fabric_transfer_count(),
    );
    assert_eq!(
        summary.active_fabric_lane_count(),
        outcome.run().active_fabric_lane_count(),
    );
    assert_eq!(
        summary.fabric_byte_count(),
        outcome.run().fabric_profile().byte_count(),
    );
    assert!(summary.has_fabric_activity());
    assert_eq!(
        summary.resource_activity_count(),
        outcome.run().resource_activity_count(),
    );
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_routes_declared_multihop_fabric_path_and_reports_activity() {
    let manifest = replay_manifest_with_multihop_fabric_fetch();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    let summary = outcome.result().parallel_execution_summary().unwrap();
    let links = outcome
        .run()
        .fabric_activities()
        .keys()
        .map(|(link, _virtual_network)| link.as_str().to_string())
        .collect::<Vec<_>>();
    let lanes = outcome
        .run()
        .fabric_activities()
        .keys()
        .map(|(link, virtual_network)| (link.as_str().to_string(), virtual_network.get()))
        .collect::<Vec<_>>();

    assert!(links.iter().any(|link| link == "cpu_router"));
    assert!(links.iter().any(|link| link == "router_memory"));
    assert!(lanes
        .iter()
        .any(|(link, virtual_network)| link == "cpu_router" && *virtual_network == 1));
    assert!(lanes
        .iter()
        .any(|(link, virtual_network)| link == "cpu_router" && *virtual_network == 2));
    assert!(lanes
        .iter()
        .any(|(link, virtual_network)| link == "router_memory" && *virtual_network == 3));
    assert!(lanes
        .iter()
        .any(|(link, virtual_network)| link == "router_memory" && *virtual_network == 4));
    assert!(summary.fabric_transfer_count() >= 4);
    assert_eq!(
        summary.fabric_transfer_count(),
        outcome.run().fabric_transfer_count(),
    );
    assert_eq!(
        summary.active_fabric_lane_count(),
        outcome.run().active_fabric_lane_count(),
    );
    assert!(summary.has_fabric_activity());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_executes_planned_host_actions() {
    let manifest = replay_manifest_with_planned_host_actions();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(20)
        .run_parallel()
        .unwrap();

    assert_eq!(
        outcome.result().checkpoint_labels(),
        &["after-boot".to_string()]
    );
    assert_eq!(
        outcome.result().restored_checkpoint_labels(),
        &["after-boot".to_string()]
    );
    assert_eq!(
        outcome.result().execution_mode_switches(),
        &[
            WorkloadExecutionModeSwitch::new(2, "cpu0", WorkloadExecutionMode::Functional,)
                .with_stats_scope(1, 1)
        ]
    );
    assert_eq!(
        outcome.result().execution_mode_switches()[0].stats_scope(),
        Some(&WorkloadStatsScope::new(1, 1))
    );
    let mut host_summary = WorkloadHostActionSummary::default();
    host_summary.record_stop();
    host_summary.record_stats_reset();
    host_summary.record_checkpoint();
    host_summary.record_checkpoint_restore();
    host_summary.record_stats_dump();
    host_summary.record_execution_mode_switch();
    host_summary.record_stop();
    assert_eq!(outcome.result().host_action_summary(), Some(&host_summary));

    let stats = outcome.result().stats_snapshot().unwrap();
    assert_eq!(stats.reset_tick(), 1);
    assert_eq!(stats.epoch(), 1);

    assert!(outcome.host_action_outcomes().iter().any(|event| matches!(
        event,
        SystemActionOutcome::StatsReset(record)
            if record.tick() == 1 && record.epoch() == 1
    )));
    assert!(outcome.host_action_outcomes().iter().any(|event| matches!(
        event,
        SystemActionOutcome::StatsDump(record)
            if record.tick() == 2 && record.reset_tick() == 1
    )));
    assert!(outcome.host_action_outcomes().iter().any(|event| matches!(
        event,
        SystemActionOutcome::Checkpoint { tick, event, source, manifest }
            if *tick == 1
                && event.get() == 10_002
                && source.get() == 51
                && manifest.label() == "after-boot"
    )));
    assert!(outcome.host_action_outcomes().iter().any(|event| matches!(
        event,
        SystemActionOutcome::CheckpointRestored { tick, event, source, manifest }
            if *tick == 1
                && event.get() == 10_003
                && source.get() == 51
                && manifest.label() == "after-boot"
    )));
    assert!(outcome.host_action_outcomes().iter().any(|event| matches!(
        event,
        SystemActionOutcome::ExecutionModeSwitched {
            tick,
            event,
            source,
            target,
            previous_mode,
            mode,
            stats_epoch,
            stats_reset_tick,
        } if *tick == 2
            && event.get() == 10_005
            && source.get() == 51
            && target == &ExecutionModeTarget::new("cpu0")
            && previous_mode.is_none()
            && *mode == ExecutionMode::Functional
            && *stats_epoch == 1
            && *stats_reset_tick == 1
    )));
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_records_planned_guest_host_calls() {
    let manifest = replay_manifest_with_planned_guest_host_call();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(20)
        .run_parallel()
        .unwrap();

    let mut host_summary = WorkloadHostActionSummary::default();
    host_summary.record_guest_host_call();
    host_summary.record_stop();
    host_summary.record_stop();
    assert_eq!(outcome.result().host_action_summary(), Some(&host_summary));
    assert!(outcome.host_action_outcomes().iter().any(|event| matches!(
        event,
        SystemActionOutcome::GuestHostCall {
            tick,
            event,
            source,
            selector,
            arguments,
            payload,
            response,
        } if *tick == 1
            && event.get() == 10_001
            && source.get() == 51
            && *selector == 0x900
            && arguments == &vec![11, 13]
            && payload == &vec![2, 4, 6]
            && response == &GuestHostCallResponse::ok(vec![21, 34], vec![0xde, 0xad])
    )));
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_reconstructs_riscv_data_route() {
    let manifest = replay_manifest_with_data_load();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    let cpu0 = CpuId::new(0);
    let activity = outcome.run().cpu_activity(cpu0).unwrap();
    assert_eq!(activity.data_access_issue_count(), 1);
    assert_eq!(
        outcome
            .cluster()
            .core(cpu0)
            .unwrap()
            .read_register(Register::new(5).unwrap()),
        0xfedc_ba98_7654_3210
    );
    let data_events = outcome.cluster().core(cpu0).unwrap().data_access_events();
    assert_eq!(data_events.len(), 2);
    assert_eq!(data_events[0].endpoint().unwrap().as_str(), "cpu0.dmem");
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_routes_data_load_through_declared_msi_cache() {
    let manifest = replay_manifest_with_msi_data_cache_load();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    assert_eq!(
        outcome
            .cluster()
            .core(CpuId::new(0))
            .unwrap()
            .read_register(Register::new(5).unwrap()),
        0xfedc_ba98_7654_3210
    );
    assert_eq!(outcome.run().data_cache_run_count(), 1);
    assert_eq!(
        outcome
            .run()
            .data_cache_run_count_for_protocol(rem6_system::RiscvDataCacheProtocol::Msi),
        1,
    );
    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert_eq!(
        summary.data_cache_parallel_run_count_for_protocol(WorkloadDataCacheProtocol::Msi),
        1,
    );
    assert!(summary.has_data_cache_parallel_work());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_routes_data_load_through_declared_cache_protocols() {
    let protocols = [
        (
            WorkloadDataCacheProtocol::Msi,
            rem6_system::RiscvDataCacheProtocol::Msi,
            "riscv-replay-protocol-msi-data-cache-load",
        ),
        (
            WorkloadDataCacheProtocol::Mesi,
            rem6_system::RiscvDataCacheProtocol::Mesi,
            "riscv-replay-protocol-mesi-data-cache-load",
        ),
        (
            WorkloadDataCacheProtocol::Moesi,
            rem6_system::RiscvDataCacheProtocol::Moesi,
            "riscv-replay-protocol-moesi-data-cache-load",
        ),
    ];

    for (workload_protocol, run_protocol, workload) in protocols {
        let manifest = replay_manifest_with_data_cache_load(workload_protocol, workload);
        let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

        let outcome = RiscvWorkloadReplay::new(plan.clone())
            .with_max_turns(32)
            .run_parallel()
            .unwrap();

        assert_eq!(
            outcome
                .cluster()
                .core(CpuId::new(0))
                .unwrap()
                .read_register(Register::new(5).unwrap()),
            0xfedc_ba98_7654_3210
        );
        assert_eq!(outcome.run().data_cache_run_count(), 1);
        assert_eq!(
            outcome
                .run()
                .data_cache_run_count_for_protocol(run_protocol),
            1,
        );
        let summary = outcome.result().parallel_execution_summary().unwrap();
        assert_eq!(
            summary.data_cache_parallel_run_count_for_protocol(workload_protocol),
            1,
        );
        assert_eq!(
            summary.data_cache_parallel_scheduler_empty_epoch_count(),
            outcome
                .run()
                .data_cache_parallel_scheduler_empty_epoch_count(),
        );
        assert_eq!(
            summary.full_system_parallel_scheduler_empty_epoch_count(),
            summary.scheduler_empty_epoch_count()
                + outcome
                    .run()
                    .data_cache_parallel_scheduler_empty_epoch_count(),
        );
        assert!(summary.has_data_cache_parallel_work());
        plan.verify_result(outcome.result()).unwrap();
    }
}

#[test]
fn workload_replay_routes_data_loads_through_declared_msi_cache_lines() {
    let manifest = replay_manifest_with_msi_data_cache_loads();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(48)
        .run_parallel()
        .unwrap();

    let core = outcome.cluster().core(CpuId::new(0)).unwrap();
    assert_eq!(
        core.read_register(Register::new(5).unwrap()),
        0xfedc_ba98_7654_3210
    );
    assert_eq!(
        core.read_register(Register::new(6).unwrap()),
        0x0123_4567_89ab_cdef
    );
    assert_eq!(outcome.run().data_cache_run_count(), 2);
    assert_eq!(
        outcome
            .run()
            .data_cache_run_count_for_protocol(rem6_system::RiscvDataCacheProtocol::Msi),
        2,
    );
    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert_eq!(
        summary.data_cache_parallel_run_count_for_protocol(WorkloadDataCacheProtocol::Msi),
        2,
    );
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_preserves_riscv_data_store_in_memory_snapshot() {
    let manifest = replay_manifest_with_data_store();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    let cpu0 = CpuId::new(0);
    let activity = outcome.run().cpu_activity(cpu0).unwrap();
    assert_eq!(activity.data_access_issue_count(), 1);
    let partition = outcome
        .memory_snapshot()
        .partitions()
        .iter()
        .find(|partition| partition.target() == MemoryTargetId::new(0))
        .unwrap();
    let line = partition
        .lines()
        .iter()
        .find(|line| line.line() == Address::new(0x9000))
        .unwrap();
    assert_eq!(&line.data()[8..16], &0x7b_u64.to_le_bytes());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_preserves_cached_riscv_data_store_in_memory_snapshot() {
    let manifest = replay_manifest_with_msi_data_cache_store();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    assert_eq!(outcome.run().data_cache_run_count(), 1);
    let partition = outcome
        .memory_snapshot()
        .partitions()
        .iter()
        .find(|partition| partition.target() == MemoryTargetId::new(0))
        .unwrap();
    let line = partition
        .lines()
        .iter()
        .find(|line| line.line() == Address::new(0x9000))
        .unwrap();
    assert_eq!(&line.data()[8..16], &0x7b_u64.to_le_bytes());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_uses_profiled_external_memory() {
    let manifest = replay_manifest_with_profiled_data_load();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(48)
        .run_parallel()
        .unwrap();

    assert_eq!(
        outcome
            .cluster()
            .core(CpuId::new(0))
            .unwrap()
            .read_register(Register::new(5).unwrap()),
        0xfedc_ba98_7654_3210
    );
    assert_eq!(outcome.run().dram_profile().active_target_count(), 1);
    assert!(outcome.run().dram_profile().read_count() >= 3);
    assert!(outcome.run().dram_profile().max_ready_latency_cycles() >= 12);
    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert_eq!(
        summary.active_dram_target_count(),
        outcome.run().dram_profile().active_target_count(),
    );
    assert_eq!(
        summary.dram_access_count(),
        outcome.run().dram_profile().access_count(),
    );
    assert_eq!(
        summary.dram_read_count(),
        outcome.run().dram_profile().read_count(),
    );
    assert_eq!(
        summary.dram_max_ready_latency_cycles(),
        outcome.run().dram_profile().max_ready_latency_cycles(),
    );
    assert!(summary.has_dram_activity());
    assert_eq!(
        summary.resource_activity_count(),
        outcome.run().resource_activity_count(),
    );
    assert!(summary.has_resource_activity());
    let dram = outcome.dram_snapshot().unwrap();
    let target = dram
        .targets()
        .iter()
        .find(|target| target.target() == MemoryTargetId::new(0))
        .unwrap();
    assert_eq!(
        target.profile().unwrap().technology(),
        DramMemoryTechnology::Hbm
    );
    assert_eq!(target.profile().unwrap().parallel_port_count(), 4);
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_routes_profiled_data_cache_miss_through_dram() {
    let manifest = replay_manifest_with_profiled_msi_data_cache_load();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(48)
        .run_parallel()
        .unwrap();

    assert_eq!(
        outcome
            .cluster()
            .core(CpuId::new(0))
            .unwrap()
            .read_register(Register::new(5).unwrap()),
        0xfedc_ba98_7654_3210
    );
    assert_eq!(outcome.run().data_cache_run_count(), 1);
    let cache_run = &outcome.run().data_cache_runs()[0];
    assert_eq!(cache_run.dram_access_count(), 1);
    let dram_activity = cache_run
        .dram_target_activity(MemoryTargetId::new(0))
        .unwrap();
    assert_eq!(dram_activity.profile().read_count(), 1);
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_rejects_manifest_without_topology() {
    let manifest = WorkloadManifest::builder(workload_id("missing-topology"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let error = RiscvWorkloadReplay::new(plan).run_parallel().unwrap_err();
    assert!(matches!(
        error,
        rem6_system::RiscvWorkloadReplayError::MissingTopology
    ));
}

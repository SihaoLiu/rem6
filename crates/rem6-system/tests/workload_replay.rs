use rem6_accelerator::AcceleratorEngineId;
use rem6_boot::BootImage;
use rem6_cpu::CpuId;
use rem6_dram::{DramGeometry, DramMemoryTechnology, DramTiming, ExternalMemoryProfile};
use rem6_gpu::GpuDeviceId;
use rem6_isa_riscv::Register;
use rem6_memory::{AccessSize, Address, AddressRange, CacheLineLayout, MemoryTargetId};
use rem6_system::{
    RiscvSystemRunStopReason, RiscvWorkloadReplay, RiscvWorkloadReplayError, SystemActionOutcome,
};
use rem6_workload::{
    HostEventIntent, WorkloadAcceleratorCommand, WorkloadAcceleratorCommandKind,
    WorkloadAcceleratorDevice, WorkloadAcceleratorDmaCopy, WorkloadDataCacheProtocol,
    WorkloadGpuDevice, WorkloadGpuDmaCopy, WorkloadGpuKernelLaunch, WorkloadHostEvent,
    WorkloadHostPlacement, WorkloadManifest, WorkloadMemoryRoute, WorkloadMemoryTarget,
    WorkloadReplayPlan, WorkloadResource, WorkloadResourceId, WorkloadResourceKind,
    WorkloadRiscvCore, WorkloadRiscvDataCache, WorkloadRouteFabric, WorkloadRouteHop,
    WorkloadRouteId, WorkloadTopology,
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

fn boot_image_with_gpu_dma_data() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
        .add_segment(Address::new(0x9024), vec![0x3a, 0x4b, 0x5c, 0x6d])
        .unwrap()
        .add_segment(Address::new(0x9048), vec![0; 4])
        .unwrap()
}

fn boot_image_with_contended_dma_data() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
        .add_segment(Address::new(0x9024), vec![0x3a, 0x4b, 0x5c, 0x6d])
        .unwrap()
        .add_segment(Address::new(0x9048), vec![0; 4])
        .unwrap()
        .add_segment(Address::new(0x9064), vec![0x7e, 0x8f, 0x90, 0xa1])
        .unwrap()
        .add_segment(Address::new(0x9088), vec![0; 4])
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
        .with_virtual_networks(1, 2)
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

fn replay_topology_with_gpu_kernel() -> WorkloadTopology {
    WorkloadTopology::new(5, 2, 4, WorkloadHostPlacement::new(4, 2, 51).unwrap())
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
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 3, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("gpu0.command"),
                "cpu0.gpu",
                0,
                "gpu0.control",
                3,
                2,
                1,
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
        .add_gpu_device(WorkloadGpuDevice::new(12, 3, 2, 1, route_id("gpu0.command")).unwrap())
        .unwrap()
        .add_gpu_kernel_launch(WorkloadGpuKernelLaunch::new(12, 90, 2, 1).unwrap())
        .unwrap()
}

fn replay_topology_with_gpu_dma_copy() -> WorkloadTopology {
    WorkloadTopology::new(5, 2, 4, WorkloadHostPlacement::new(4, 2, 51).unwrap())
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
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 3, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("gpu0.command"),
                "cpu0.gpu",
                0,
                "gpu0.control",
                3,
                2,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("gpu0.dma"), "gpu0.dma", 3, "memory", 2, 3, 5)
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
        .add_gpu_device(WorkloadGpuDevice::new(12, 3, 2, 1, route_id("gpu0.command")).unwrap())
        .unwrap()
        .add_gpu_dma_copy(
            WorkloadGpuDmaCopy::new(
                12,
                200,
                route_id("gpu0.dma"),
                77,
                Address::new(0x9024),
                Address::new(0x9048),
                4,
            )
            .unwrap(),
        )
        .unwrap()
}

fn replay_topology_with_accelerator_command() -> WorkloadTopology {
    WorkloadTopology::new(5, 2, 4, WorkloadHostPlacement::new(4, 2, 51).unwrap())
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
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 3, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("accelerator0.command"),
                "cpu0.accelerator",
                0,
                "accelerator0.control",
                3,
                2,
                1,
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
        .add_accelerator_device(
            WorkloadAcceleratorDevice::new(22, 3, 2, route_id("accelerator0.command")).unwrap(),
        )
        .unwrap()
        .add_accelerator_command(
            WorkloadAcceleratorCommand::new(
                22,
                120,
                WorkloadAcceleratorCommandKind::NpuInference { tiles: 4 },
                3,
            )
            .unwrap(),
        )
        .unwrap()
}

fn replay_topology_with_contended_compute() -> WorkloadTopology {
    WorkloadTopology::new(6, 2, 4, WorkloadHostPlacement::new(5, 2, 51).unwrap())
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
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 3, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("gpu0.command"),
                "cpu0.gpu",
                0,
                "gpu0.control",
                3,
                2,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("accelerator0.command"),
                "cpu0.accelerator",
                0,
                "accelerator0.control",
                4,
                2,
                1,
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
        .add_gpu_device(WorkloadGpuDevice::new(12, 3, 1, 1, route_id("gpu0.command")).unwrap())
        .unwrap()
        .add_gpu_kernel_launch(WorkloadGpuKernelLaunch::new(12, 90, 3, 4).unwrap())
        .unwrap()
        .add_accelerator_device(
            WorkloadAcceleratorDevice::new(22, 4, 1, route_id("accelerator0.command")).unwrap(),
        )
        .unwrap()
        .add_accelerator_command(
            WorkloadAcceleratorCommand::new(
                22,
                120,
                WorkloadAcceleratorCommandKind::NpuInference { tiles: 4 },
                4,
            )
            .unwrap(),
        )
        .unwrap()
        .add_accelerator_command(
            WorkloadAcceleratorCommand::new(
                22,
                121,
                WorkloadAcceleratorCommandKind::NpuInference { tiles: 5 },
                4,
            )
            .unwrap(),
        )
        .unwrap()
}

fn replay_topology_with_accelerator_dma_copy() -> WorkloadTopology {
    WorkloadTopology::new(5, 2, 4, WorkloadHostPlacement::new(4, 2, 51).unwrap())
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
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 3, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("accelerator0.command"),
                "cpu0.accelerator",
                0,
                "accelerator0.control",
                3,
                2,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("accelerator0.dma"),
                "accelerator0.dma",
                3,
                "memory",
                2,
                3,
                5,
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
        .add_accelerator_device(
            WorkloadAcceleratorDevice::new(22, 3, 2, route_id("accelerator0.command")).unwrap(),
        )
        .unwrap()
        .add_accelerator_dma_copy(
            WorkloadAcceleratorDmaCopy::new(
                22,
                300,
                route_id("accelerator0.dma"),
                88,
                Address::new(0x9024),
                Address::new(0x9048),
                4,
            )
            .unwrap(),
        )
        .unwrap()
}

fn replay_topology_with_profiled_contended_dma_copies() -> WorkloadTopology {
    WorkloadTopology::new(6, 2, 4, WorkloadHostPlacement::new(5, 2, 51).unwrap())
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
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 3, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("gpu0.command"),
                "cpu0.gpu",
                0,
                "gpu0.control",
                3,
                2,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("gpu0.dma"), "gpu0.dma", 3, "memory", 2, 3, 5)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("accelerator0.command"),
                "cpu0.accelerator",
                0,
                "accelerator0.control",
                4,
                2,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("accelerator0.dma"),
                "accelerator0.dma",
                4,
                "memory",
                2,
                3,
                5,
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
        .add_gpu_device(WorkloadGpuDevice::new(12, 3, 2, 1, route_id("gpu0.command")).unwrap())
        .unwrap()
        .add_gpu_dma_copy(
            WorkloadGpuDmaCopy::new(
                12,
                200,
                route_id("gpu0.dma"),
                77,
                Address::new(0x9024),
                Address::new(0x9048),
                4,
            )
            .unwrap(),
        )
        .unwrap()
        .add_gpu_dma_copy(
            WorkloadGpuDmaCopy::new(
                12,
                201,
                route_id("gpu0.dma"),
                77,
                Address::new(0x9064),
                Address::new(0x9088),
                4,
            )
            .unwrap(),
        )
        .unwrap()
        .add_accelerator_device(
            WorkloadAcceleratorDevice::new(22, 4, 2, route_id("accelerator0.command")).unwrap(),
        )
        .unwrap()
        .add_accelerator_dma_copy(
            WorkloadAcceleratorDmaCopy::new(
                22,
                300,
                route_id("accelerator0.dma"),
                88,
                Address::new(0x9024),
                Address::new(0x9048),
                4,
            )
            .unwrap(),
        )
        .unwrap()
        .add_accelerator_dma_copy(
            WorkloadAcceleratorDmaCopy::new(
                22,
                301,
                route_id("accelerator0.dma"),
                88,
                Address::new(0x9064),
                Address::new(0x9088),
                4,
            )
            .unwrap(),
        )
        .unwrap()
}

fn replay_topology_with_cached_accelerator_dma_copy() -> WorkloadTopology {
    WorkloadTopology::new(5, 2, 4, WorkloadHostPlacement::new(4, 2, 51).unwrap())
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
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 3, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu0.data"), "cpu0.dmem", 0, "memory", 2, 3, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("accelerator0.command"),
                "cpu0.accelerator",
                0,
                "accelerator0.control",
                3,
                2,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("accelerator0.dma"),
                "accelerator0.dma",
                3,
                "memory",
                2,
                3,
                5,
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
                Address::new(0x9020),
                2,
                "dcache.dir",
            )
            .unwrap(),
        )
        .unwrap()
        .add_accelerator_device(
            WorkloadAcceleratorDevice::new(22, 3, 2, route_id("accelerator0.command")).unwrap(),
        )
        .unwrap()
        .add_accelerator_dma_copy(
            WorkloadAcceleratorDmaCopy::new(
                22,
                300,
                route_id("accelerator0.dma"),
                88,
                Address::new(0x9024),
                Address::new(0x9048),
                4,
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

fn replay_topology_with_msi_data_cache() -> WorkloadTopology {
    replay_topology_with_data_route()
        .with_riscv_data_cache(
            WorkloadRiscvDataCache::new(
                WorkloadDataCacheProtocol::Msi,
                0,
                Address::new(0x9000),
                2,
                "dcache.dir",
            )
            .unwrap(),
        )
        .unwrap()
}

fn replay_topology_with_msi_data_cache_lines() -> WorkloadTopology {
    replay_topology_with_data_route()
        .with_riscv_data_cache(
            WorkloadRiscvDataCache::new(
                WorkloadDataCacheProtocol::Msi,
                0,
                Address::new(0x9000),
                2,
                "dcache.dir",
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
    replay_topology_with_profiled_data_route()
        .with_riscv_data_cache(
            WorkloadRiscvDataCache::new(
                WorkloadDataCacheProtocol::Msi,
                0,
                Address::new(0x9000),
                2,
                "dcache.dir",
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

fn replay_manifest_with_gpu_kernel() -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("riscv-replay-gpu-kernel"), boot_image())
        .with_topology(replay_topology_with_gpu_kernel())
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

fn replay_manifest_with_gpu_dma_copy() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-gpu-dma-copy"),
        boot_image_with_gpu_dma_data(),
    )
    .with_topology(replay_topology_with_gpu_dma_copy())
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

fn replay_manifest_with_accelerator_command() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-accelerator-command"),
        boot_image(),
    )
    .with_topology(replay_topology_with_accelerator_command())
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

fn replay_manifest_with_contended_compute() -> WorkloadManifest {
    WorkloadManifest::builder(workload_id("riscv-replay-contended-compute"), boot_image())
        .with_topology(replay_topology_with_contended_compute())
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

fn replay_manifest_with_accelerator_dma_copy() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-accelerator-dma-copy"),
        boot_image_with_gpu_dma_data(),
    )
    .with_topology(replay_topology_with_accelerator_dma_copy())
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

fn replay_manifest_with_profiled_contended_dma_copies() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-profiled-contended-dma-copies"),
        boot_image_with_contended_dma_data(),
    )
    .with_topology(replay_topology_with_profiled_contended_dma_copies())
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

fn replay_manifest_with_cached_accelerator_dma_copy() -> WorkloadManifest {
    WorkloadManifest::builder(
        workload_id("riscv-replay-cached-accelerator-dma-copy"),
        boot_image_with_gpu_dma_data(),
    )
    .with_topology(replay_topology_with_cached_accelerator_dma_copy())
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
    WorkloadManifest::builder(
        workload_id("riscv-replay-msi-data-cache-load"),
        boot_image_with_data_load(),
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

    assert!(links.iter().any(|link| link == "cpu_router"));
    assert!(links.iter().any(|link| link == "router_memory"));
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
fn workload_replay_summary_reports_compute_wait_diagnostics() {
    let manifest = replay_manifest_with_contended_compute();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(64)
        .run_parallel()
        .unwrap();

    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert_eq!(summary.gpu_kernel_launch_count(), 1);
    assert!(summary.gpu_workgroup_completion_count() >= 1);
    assert_eq!(summary.accelerator_command_count(), 2);
    assert!(summary.accelerator_completion_count() >= 1);
    assert!(summary.gpu_compute_wait_for_edge_count() >= 1);
    assert!(summary.accelerator_compute_wait_for_edge_count() >= 1);
    assert_eq!(
        summary.compute_wait_for_edge_count(),
        summary.gpu_compute_wait_for_edge_count()
            + summary.accelerator_compute_wait_for_edge_count(),
    );
    assert_eq!(
        summary.compute_deadlock_diagnostic_count(),
        summary.gpu_compute_deadlock_diagnostic_count()
            + summary.accelerator_compute_deadlock_diagnostic_count(),
    );
    assert_eq!(
        summary.full_system_wait_for_edge_count(),
        summary.resource_wait_for_edge_count()
            + summary.data_cache_wait_for_edge_count()
            + summary.compute_wait_for_edge_count()
            + summary.dma_wait_for_edge_count(),
    );
    assert!(summary.has_gpu_compute_diagnostics());
    assert!(summary.has_accelerator_compute_diagnostics());
    assert!(summary.has_compute_diagnostics());
    assert!(summary.has_full_system_diagnostics());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_summary_reports_dma_wait_diagnostics() {
    let manifest = replay_manifest_with_profiled_contended_dma_copies();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(512)
        .run_parallel()
        .unwrap();

    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert_eq!(summary.gpu_dma_copy_count(), 2);
    assert_eq!(summary.gpu_dma_completion_count(), 2);
    assert_eq!(summary.accelerator_dma_copy_count(), 2);
    assert_eq!(summary.accelerator_dma_completion_count(), 2);
    assert!(summary.gpu_dma_wait_for_edge_count() >= 1);
    assert!(summary.accelerator_dma_wait_for_edge_count() >= 1);
    assert_eq!(
        summary.dma_wait_for_edge_count(),
        summary.gpu_dma_wait_for_edge_count() + summary.accelerator_dma_wait_for_edge_count(),
    );
    assert_eq!(
        summary.dma_deadlock_diagnostic_count(),
        summary.gpu_dma_deadlock_diagnostic_count()
            + summary.accelerator_dma_deadlock_diagnostic_count(),
    );
    assert_eq!(
        summary.full_system_wait_for_edge_count(),
        summary.resource_wait_for_edge_count()
            + summary.data_cache_wait_for_edge_count()
            + summary.compute_wait_for_edge_count()
            + summary.dma_wait_for_edge_count(),
    );
    assert!(summary.has_gpu_dma_diagnostics());
    assert!(summary.has_accelerator_dma_diagnostics());
    assert!(summary.has_dma_diagnostics());
    assert!(summary.has_full_system_diagnostics());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_runs_declared_gpu_kernel_on_parallel_scheduler() {
    let manifest = replay_manifest_with_gpu_kernel();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    let gpu = outcome.gpu_snapshot(GpuDeviceId::new(12)).unwrap();
    assert_eq!(gpu.completions().len(), 2);
    assert!(gpu.pending_dma_writes().is_empty());
    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert_eq!(summary.gpu_kernel_launch_count(), 1);
    assert_eq!(summary.gpu_trace_event_count(), 6);
    assert_eq!(summary.gpu_workgroup_completion_count(), 2);
    assert_eq!(summary.active_gpu_device_count(), 1);
    assert!(summary.has_gpu_compute_activity());
    assert!(summary.has_full_system_parallel_scheduler_work());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_runs_declared_gpu_dma_copy_on_parallel_memory_backend() {
    let manifest = replay_manifest_with_gpu_dma_copy();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    let partition = outcome
        .memory_snapshot()
        .partitions()
        .iter()
        .find(|partition| partition.target() == MemoryTargetId::new(0))
        .unwrap();
    let destination = partition
        .lines()
        .iter()
        .find(|line| line.line() == Address::new(0x9040))
        .unwrap();
    assert_eq!(&destination.data()[8..12], &[0x3a, 0x4b, 0x5c, 0x6d]);
    let gpu = outcome.gpu_snapshot(GpuDeviceId::new(12)).unwrap();
    assert_eq!(gpu.dma_completions().len(), 1);
    assert!(gpu.pending_dma_writes().is_empty());
    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert_eq!(summary.gpu_dma_copy_count(), 1);
    assert_eq!(summary.gpu_dma_completion_count(), 1);
    assert_eq!(summary.active_gpu_dma_device_count(), 1);
    assert!(summary.has_gpu_dma_activity());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_rejects_overflowing_gpu_dma_request_sequence() {
    let topology = replay_topology_with_gpu_dma_copy()
        .add_gpu_dma_copy(
            WorkloadGpuDmaCopy::new(
                12,
                u64::MAX,
                route_id("gpu0.dma"),
                78,
                Address::new(0x9024),
                Address::new(0x9048),
                4,
            )
            .unwrap(),
        )
        .unwrap();
    let manifest = WorkloadManifest::builder(
        workload_id("riscv-replay-gpu-dma-overflow"),
        boot_image_with_gpu_dma_data(),
    )
    .with_topology(topology)
    .add_resource(kernel_resource())
    .unwrap()
    .add_required_resource(resource_id("kernel"))
    .build()
    .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let error = RiscvWorkloadReplay::new(plan).run_parallel().unwrap_err();

    assert_eq!(
        error,
        RiscvWorkloadReplayError::GpuDmaRequestSequenceOverflow { transfer: u64::MAX }
    );
}

#[test]
fn workload_replay_runs_declared_accelerator_command_on_parallel_scheduler() {
    let manifest = replay_manifest_with_accelerator_command();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    let accelerator = outcome
        .accelerator_snapshot(AcceleratorEngineId::new(22))
        .unwrap();
    assert_eq!(accelerator.completed().len(), 1);
    assert!(accelerator.pending_dma_writes().is_empty());
    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert_eq!(summary.accelerator_command_count(), 1);
    assert_eq!(summary.accelerator_trace_event_count(), 3);
    assert_eq!(summary.accelerator_completion_count(), 1);
    assert_eq!(summary.active_accelerator_device_count(), 1);
    assert!(summary.has_accelerator_compute_activity());
    assert!(summary.has_full_system_parallel_scheduler_work());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_runs_declared_accelerator_dma_copy_on_parallel_memory_backend() {
    let manifest = replay_manifest_with_accelerator_dma_copy();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    let partition = outcome
        .memory_snapshot()
        .partitions()
        .iter()
        .find(|partition| partition.target() == MemoryTargetId::new(0))
        .unwrap();
    let destination = partition
        .lines()
        .iter()
        .find(|line| line.line() == Address::new(0x9040))
        .unwrap();
    assert_eq!(&destination.data()[8..12], &[0x3a, 0x4b, 0x5c, 0x6d]);
    let accelerator = outcome
        .accelerator_snapshot(AcceleratorEngineId::new(22))
        .unwrap();
    assert_eq!(accelerator.dma_completions().len(), 1);
    assert!(accelerator.pending_dma_writes().is_empty());
    let summary = outcome.result().parallel_execution_summary().unwrap();
    assert_eq!(summary.accelerator_dma_copy_count(), 1);
    assert_eq!(summary.accelerator_dma_completion_count(), 1);
    assert_eq!(summary.active_accelerator_dma_device_count(), 1);
    assert!(summary.has_accelerator_dma_activity());
    plan.verify_result(outcome.result()).unwrap();
}

#[test]
fn workload_replay_routes_accelerator_dma_read_through_declared_msi_cache() {
    let manifest = replay_manifest_with_cached_accelerator_dma_copy();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let outcome = RiscvWorkloadReplay::new(plan.clone())
        .with_max_turns(32)
        .run_parallel()
        .unwrap();

    let partition = outcome
        .memory_snapshot()
        .partitions()
        .iter()
        .find(|partition| partition.target() == MemoryTargetId::new(0))
        .unwrap();
    let destination = partition
        .lines()
        .iter()
        .find(|line| line.line() == Address::new(0x9040))
        .unwrap();
    assert_eq!(&destination.data()[8..12], &[0x3a, 0x4b, 0x5c, 0x6d]);
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
    assert_eq!(summary.accelerator_dma_copy_count(), 1);
    assert_eq!(summary.accelerator_dma_completion_count(), 1);
    assert!(summary.has_data_cache_parallel_work());
    assert!(summary.has_accelerator_dma_activity());
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
        SystemActionOutcome::StatsSnapshot(snapshot)
            if snapshot.tick() == 2 && snapshot.reset_tick() == 1
    )));
    assert!(outcome.host_action_outcomes().iter().any(|event| matches!(
        event,
        SystemActionOutcome::Checkpoint { tick, event, source, manifest }
            if *tick == 1
                && event.get() == 10_002
                && source.get() == 51
                && manifest.label() == "after-boot"
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
    assert_eq!(data_events[0].endpoint().as_str(), "cpu0.dmem");
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

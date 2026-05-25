use rem6_accelerator::AcceleratorEngineId;
use rem6_boot::BootImage;
use rem6_dram::{DramGeometry, DramTiming, ExternalMemoryProfile};
use rem6_gpu::GpuDeviceId;
use rem6_memory::{AccessSize, Address, AddressRange, CacheLineLayout, MemoryTargetId};
use rem6_system::{RiscvWorkloadReplay, RiscvWorkloadReplayError};
use rem6_workload::{
    HostEventIntent, WorkloadAcceleratorCommand, WorkloadAcceleratorCommandKind,
    WorkloadAcceleratorDevice, WorkloadAcceleratorDmaCopy, WorkloadDataCacheProtocol,
    WorkloadExpectedDataCacheProtocolRunCount, WorkloadExpectedDataCacheRunAttribution,
    WorkloadGpuDevice, WorkloadGpuDmaCopy, WorkloadGpuKernelLaunch, WorkloadHostEvent,
    WorkloadHostPlacement, WorkloadManifest, WorkloadMemoryRoute, WorkloadMemoryTarget,
    WorkloadParallelBatchScope, WorkloadReplayPlan, WorkloadResource, WorkloadResourceId,
    WorkloadResourceKind, WorkloadRiscvCore, WorkloadRiscvDataCache, WorkloadRouteId,
    WorkloadTopology,
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

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(0x0000_0073))
        .unwrap()
}

fn boot_image_with_gpu_dma_data() -> BootImage {
    boot_image()
        .add_segment(Address::new(0x9024), vec![0x3a, 0x4b, 0x5c, 0x6d])
        .unwrap()
        .add_segment(Address::new(0x9048), vec![0; 4])
        .unwrap()
}

fn boot_image_with_contended_dma_data() -> BootImage {
    boot_image_with_gpu_dma_data()
        .add_segment(Address::new(0x9064), vec![0x7e, 0x8f, 0x90, 0xa1])
        .unwrap()
        .add_segment(Address::new(0x9088), vec![0; 4])
        .unwrap()
}

fn dram_timing() -> DramTiming {
    DramTiming::new(4, 8, 10, 3, 5).unwrap()
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
        .add_gpu_device(
            WorkloadGpuDevice::new(
                12,
                3,
                2,
                1,
                "gpu0.control",
                "gpu0.dma",
                route_id("gpu0.command"),
            )
            .unwrap(),
        )
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
        .add_gpu_device(
            WorkloadGpuDevice::new(
                12,
                3,
                2,
                1,
                "gpu0.control",
                "gpu0.dma",
                route_id("gpu0.command"),
            )
            .unwrap(),
        )
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
            WorkloadAcceleratorDevice::new(
                22,
                3,
                2,
                "accelerator0.control",
                "accelerator0.dma",
                route_id("accelerator0.command"),
            )
            .unwrap(),
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
        .add_gpu_device(
            WorkloadGpuDevice::new(
                12,
                3,
                1,
                1,
                "gpu0.control",
                "gpu0.dma",
                route_id("gpu0.command"),
            )
            .unwrap(),
        )
        .unwrap()
        .add_gpu_kernel_launch(WorkloadGpuKernelLaunch::new(12, 90, 3, 4).unwrap())
        .unwrap()
        .add_accelerator_device(
            WorkloadAcceleratorDevice::new(
                22,
                4,
                1,
                "accelerator0.control",
                "accelerator0.dma",
                route_id("accelerator0.command"),
            )
            .unwrap(),
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
            WorkloadAcceleratorDevice::new(
                22,
                3,
                2,
                "accelerator0.control",
                "accelerator0.dma",
                route_id("accelerator0.command"),
            )
            .unwrap(),
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
        .add_gpu_device(
            WorkloadGpuDevice::new(
                12,
                3,
                2,
                1,
                "gpu0.control",
                "gpu0.dma",
                route_id("gpu0.command"),
            )
            .unwrap(),
        )
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
            WorkloadAcceleratorDevice::new(
                22,
                4,
                2,
                "accelerator0.control",
                "accelerator0.dma",
                route_id("accelerator0.command"),
            )
            .unwrap(),
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
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("dcache.backing"),
                "dcache.dir",
                2,
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
                route_id("dcache.backing"),
            )
            .unwrap(),
        )
        .unwrap()
        .add_accelerator_device(
            WorkloadAcceleratorDevice::new(
                22,
                3,
                2,
                "accelerator0.control",
                "accelerator0.dma",
                route_id("accelerator0.command"),
            )
            .unwrap(),
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
    .add_expected_data_cache_protocol_run_count(
        WorkloadExpectedDataCacheProtocolRunCount::new(WorkloadDataCacheProtocol::Msi, 1).unwrap(),
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
    assert!(summary.gpu_dma_scheduler_batch_count() >= summary.gpu_dma_copy_count());
    assert!(
        summary.accelerator_dma_scheduler_batch_count() >= summary.accelerator_dma_copy_count()
    );
    assert_eq!(
        summary.dma_scheduler_batch_count(),
        summary.gpu_dma_scheduler_batch_count() + summary.accelerator_dma_scheduler_batch_count(),
    );
    assert_eq!(
        summary.gpu_dma_scheduler_batch_timeline().len(),
        summary.gpu_dma_scheduler_batch_count(),
    );
    assert_eq!(
        summary.accelerator_dma_scheduler_batch_timeline().len(),
        summary.accelerator_dma_scheduler_batch_count(),
    );
    assert_eq!(
        summary.dma_scheduler_batch_timeline().len(),
        summary.dma_scheduler_batch_count(),
    );
    assert!(summary
        .full_system_parallel_scheduler_batch_timeline()
        .iter()
        .any(|record| record.scope() == WorkloadParallelBatchScope::GpuDmaScheduler));
    assert!(summary
        .full_system_parallel_scheduler_batch_timeline()
        .iter()
        .any(|record| record.scope() == WorkloadParallelBatchScope::AcceleratorDmaScheduler));
    assert_eq!(
        summary.dma_scheduler_total_workers(),
        summary.gpu_dma_scheduler_total_workers()
            + summary.accelerator_dma_scheduler_total_workers(),
    );
    assert!(
        summary.dma_scheduler_max_workers() >= summary.gpu_dma_scheduler_max_workers()
            && summary.dma_scheduler_max_workers()
                >= summary.accelerator_dma_scheduler_max_workers()
    );
    assert!(
        summary.gpu_dma_scheduler_batch_worker_ticks()
            >= summary.gpu_dma_scheduler_batch_count() as u64
    );
    assert!(
        summary.accelerator_dma_scheduler_batch_worker_ticks()
            >= summary.accelerator_dma_scheduler_batch_count() as u64
    );
    assert_eq!(
        summary.dma_scheduler_batch_worker_ticks(),
        summary.gpu_dma_scheduler_batch_worker_ticks()
            + summary.accelerator_dma_scheduler_batch_worker_ticks(),
    );
    assert!(
        summary.full_system_parallel_scheduler_batch_count()
            >= summary.scheduler_batch_count()
                + summary.data_cache_parallel_scheduler_batch_count()
                + summary.dma_scheduler_batch_count()
    );
    assert!(
        summary.full_system_parallel_scheduler_total_workers()
            >= summary.total_parallel_scheduler_workers()
                + summary.data_cache_parallel_scheduler_total_workers()
                + summary.dma_scheduler_total_workers()
    );
    assert!(
        summary.full_system_parallel_scheduler_max_workers() >= summary.dma_scheduler_max_workers()
    );
    assert!(
        summary.full_system_parallel_scheduler_batch_worker_ticks()
            >= summary.parallel_scheduler_batch_worker_ticks()
                + summary.data_cache_parallel_scheduler_batch_worker_ticks()
                + summary.dma_scheduler_batch_worker_ticks()
    );
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

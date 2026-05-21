use rem6_boot::BootImage;
use rem6_dram::{
    DramGeometry, DramMemoryTechnology, DramTiming, ExternalMemoryProfile, ExternalMemoryTopology,
};
use rem6_kernel::Tick;
use rem6_memory::{AccessSize, Address, CacheLineLayout, MemoryTargetId};
use rem6_stats::StatsRegistry;
use rem6_workload::{
    CheckpointLineage, HostEventIntent, WorkloadAcceleratorCommand, WorkloadAcceleratorCommandKind,
    WorkloadAcceleratorDevice, WorkloadDataCacheProtocol, WorkloadDataCacheProtocolCount,
    WorkloadError, WorkloadGpuDevice, WorkloadGpuKernelLaunch, WorkloadHostEvent,
    WorkloadHostPlacement, WorkloadId, WorkloadManifest, WorkloadMemoryRoute, WorkloadMemoryTarget,
    WorkloadParallelExecutionSummary, WorkloadReplayPlan, WorkloadResource, WorkloadResourceId,
    WorkloadResourceKind, WorkloadResult, WorkloadRiscvCore, WorkloadRouteId, WorkloadTopology,
};

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
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

fn memory_target(id: u32) -> MemoryTargetId {
    MemoryTargetId::new(id)
}

fn dram_geometry() -> DramGeometry {
    DramGeometry::new(4, 64, 16).unwrap()
}

fn dram_timing() -> DramTiming {
    DramTiming::new(4, 8, 10, 3, 5).unwrap()
}

fn hbm_profile(target: u32) -> ExternalMemoryProfile {
    ExternalMemoryProfile::hbm(
        memory_target(target),
        layout(),
        2,
        2,
        dram_geometry(),
        dram_timing(),
    )
    .unwrap()
}

fn hbm_profile_with_layout(target: u32, line_layout: CacheLineLayout) -> ExternalMemoryProfile {
    ExternalMemoryProfile::hbm(
        memory_target(target),
        line_layout,
        2,
        2,
        DramGeometry::new(4, 64, line_layout.bytes()).unwrap(),
        dram_timing(),
    )
    .unwrap()
}

fn hbm_profile_with_geometry(target: u32, geometry: DramGeometry) -> ExternalMemoryProfile {
    ExternalMemoryProfile::hbm(
        memory_target(target),
        layout(),
        2,
        2,
        geometry,
        dram_timing(),
    )
    .unwrap()
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), vec![0x13, 0x05, 0x00, 0x00])
        .unwrap()
        .add_segment(Address::new(0x8010), vec![0x73, 0x00, 0x00, 0x00])
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

fn disk_resource() -> WorkloadResource {
    WorkloadResource::new(
        resource_id("disk"),
        WorkloadResourceKind::DiskImage,
        "sha256:disk",
        "resources/rootfs.img",
    )
    .unwrap()
}

#[test]
fn workload_memory_target_records_external_memory_profile() {
    let profile = hbm_profile(0);
    let target = WorkloadMemoryTarget::new(
        0,
        16,
        rem6_memory::AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap())
            .unwrap(),
    )
    .unwrap()
    .with_external_memory_profile(profile)
    .unwrap();

    assert_eq!(target.external_memory_profile(), Some(&profile));
    assert_eq!(
        target.external_memory_profile().unwrap().technology(),
        DramMemoryTechnology::Hbm
    );
    assert_eq!(
        target.external_memory_profile().unwrap().topology(),
        ExternalMemoryTopology::Hbm {
            stacks: 2,
            pseudo_channels_per_stack: 2,
        }
    );
}

#[test]
fn workload_memory_target_rejects_mismatched_external_memory_profile() {
    let range =
        rem6_memory::AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap())
            .unwrap();

    let wrong_target = WorkloadMemoryTarget::new(0, 16, range)
        .unwrap()
        .with_external_memory_profile(hbm_profile(1))
        .unwrap_err();
    assert_eq!(
        wrong_target,
        WorkloadError::MemoryProfileTargetMismatch {
            target: 0,
            profile_target: 1,
        }
    );

    let wrong_line_size = WorkloadMemoryTarget::new(0, 16, range)
        .unwrap()
        .with_external_memory_profile(hbm_profile_with_layout(
            0,
            CacheLineLayout::new(32).unwrap(),
        ))
        .unwrap_err();
    assert_eq!(
        wrong_line_size,
        WorkloadError::MemoryProfileLineSizeMismatch {
            target: 0,
            line_bytes: 16,
            profile_line_bytes: 32,
        }
    );

    let wrong_geometry_line_size = WorkloadMemoryTarget::new(0, 16, range)
        .unwrap()
        .with_external_memory_profile(hbm_profile_with_geometry(
            0,
            DramGeometry::new(4, 64, 32).unwrap(),
        ))
        .unwrap_err();
    assert_eq!(
        wrong_geometry_line_size,
        WorkloadError::MemoryProfileGeometryLineSizeMismatch {
            target: 0,
            layout_line_bytes: 16,
            geometry_line_bytes: 32,
        }
    );
}

fn replay_manifest_with_planned_outputs() -> WorkloadManifest {
    WorkloadManifest::builder(id("planned-output-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            20,
            HostEventIntent::Checkpoint {
                label: "warm".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            80,
            HostEventIntent::Stop {
                reason: "done".to_string(),
            },
        ))
        .build()
        .unwrap()
}

fn riscv_topology() -> WorkloadTopology {
    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 41).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                16,
                rem6_memory::AddressRange::new(
                    Address::new(0x8000),
                    AccessSize::new(0x2000).unwrap(),
                )
                .unwrap(),
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
                Address::new(0x9000),
                "cpu1.ifetch",
                route_id("cpu1.fetch"),
            )
            .unwrap()
            .with_data("cpu1.dmem", route_id("cpu1.data"))
            .unwrap(),
        )
        .unwrap()
}

#[test]
fn workload_topology_records_gpu_devices_and_kernel_launches() {
    let topology = riscv_topology()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("gpu0.command"),
                "cpu0.gpu",
                0,
                "gpu0.control",
                3,
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_gpu_device(WorkloadGpuDevice::new(12, 3, 2, 1, route_id("gpu0.command")).unwrap())
        .unwrap()
        .add_gpu_kernel_launch(WorkloadGpuKernelLaunch::new(12, 90, 3, 5).unwrap())
        .unwrap();

    assert_eq!(topology.gpu_devices().len(), 1);
    assert_eq!(topology.gpu_devices()[0].device(), 12);
    assert_eq!(topology.gpu_devices()[0].partition(), 3);
    assert_eq!(topology.gpu_devices()[0].compute_units(), 2);
    assert_eq!(topology.gpu_devices()[0].wave_slots_per_compute_unit(), 1);
    assert_eq!(
        topology.gpu_devices()[0].command_route(),
        &route_id("gpu0.command")
    );
    assert_eq!(topology.gpu_kernel_launches().len(), 1);
    assert_eq!(topology.gpu_kernel_launches()[0].device(), 12);
    assert_eq!(topology.gpu_kernel_launches()[0].kernel(), 90);
    assert_eq!(topology.gpu_kernel_launches()[0].workgroups(), 3);
    assert_eq!(topology.gpu_kernel_launches()[0].workgroup_latency(), 5);
}

#[test]
fn workload_topology_rejects_invalid_gpu_declarations() {
    let missing_route = riscv_topology()
        .add_gpu_device(WorkloadGpuDevice::new(12, 3, 2, 1, route_id("gpu0.command")).unwrap())
        .unwrap_err();
    assert_eq!(
        missing_route,
        WorkloadError::MissingGpuCommandRoute {
            device: 12,
            route: route_id("gpu0.command"),
        }
    );

    let topology = riscv_topology()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("gpu0.command"),
                "cpu0.gpu",
                0,
                "gpu0.control",
                3,
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_gpu_device(WorkloadGpuDevice::new(12, 3, 2, 1, route_id("gpu0.command")).unwrap())
        .unwrap();
    let duplicate = topology
        .clone()
        .add_gpu_device(WorkloadGpuDevice::new(12, 3, 2, 1, route_id("gpu0.command")).unwrap())
        .unwrap_err();
    assert_eq!(duplicate, WorkloadError::DuplicateGpuDevice { device: 12 });

    let missing_device = topology
        .add_gpu_kernel_launch(WorkloadGpuKernelLaunch::new(99, 90, 3, 5).unwrap())
        .unwrap_err();
    assert_eq!(
        missing_device,
        WorkloadError::MissingGpuDevice { device: 99 }
    );
}

#[test]
fn workload_topology_records_accelerator_devices_and_commands() {
    let topology = riscv_topology()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("accelerator0.command"),
                "cpu0.accelerator",
                0,
                "accelerator0.control",
                3,
                1,
                1,
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
                80,
                WorkloadAcceleratorCommandKind::NpuInference { tiles: 4 },
                7,
            )
            .unwrap(),
        )
        .unwrap();

    assert_eq!(topology.accelerator_devices().len(), 1);
    assert_eq!(topology.accelerator_devices()[0].engine(), 22);
    assert_eq!(topology.accelerator_devices()[0].partition(), 3);
    assert_eq!(topology.accelerator_devices()[0].lanes(), 2);
    assert_eq!(
        topology.accelerator_devices()[0].command_route(),
        &route_id("accelerator0.command")
    );
    assert_eq!(topology.accelerator_commands().len(), 1);
    assert_eq!(topology.accelerator_commands()[0].engine(), 22);
    assert_eq!(topology.accelerator_commands()[0].command(), 80);
    assert_eq!(
        topology.accelerator_commands()[0].kind(),
        &WorkloadAcceleratorCommandKind::NpuInference { tiles: 4 }
    );
    assert_eq!(topology.accelerator_commands()[0].execution_latency(), 7);
}

#[test]
fn workload_topology_rejects_invalid_accelerator_declarations() {
    let missing_route = riscv_topology()
        .add_accelerator_device(
            WorkloadAcceleratorDevice::new(22, 3, 2, route_id("accelerator0.command")).unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        missing_route,
        WorkloadError::MissingAcceleratorCommandRoute {
            engine: 22,
            route: route_id("accelerator0.command"),
        }
    );

    let topology = riscv_topology()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("accelerator0.command"),
                "cpu0.accelerator",
                0,
                "accelerator0.control",
                3,
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_accelerator_device(
            WorkloadAcceleratorDevice::new(22, 3, 2, route_id("accelerator0.command")).unwrap(),
        )
        .unwrap();
    let duplicate = topology
        .clone()
        .add_accelerator_device(
            WorkloadAcceleratorDevice::new(22, 3, 2, route_id("accelerator0.command")).unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateAcceleratorDevice { engine: 22 }
    );

    let missing_device = topology
        .add_accelerator_command(
            WorkloadAcceleratorCommand::new(
                99,
                80,
                WorkloadAcceleratorCommandKind::GpuKernel { workgroups: 4 },
                7,
            )
            .unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        missing_device,
        WorkloadError::MissingAcceleratorDevice { engine: 99 }
    );
}

#[test]
fn workload_manifest_records_boot_resources_host_events_and_lineage() {
    let manifest = WorkloadManifest::builder(id("riscv-smoke"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(disk_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_required_resource(resource_id("disk"))
        .add_host_event(WorkloadHostEvent::new(
            25,
            HostEventIntent::StatsReset {
                label: "roi-begin".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            40,
            HostEventIntent::Checkpoint {
                label: "warm".to_string(),
            },
        ))
        .with_checkpoint_lineage(CheckpointLineage::RestoredFrom {
            label: "booted".to_string(),
            manifest_identity: "checkpoint-manifest-1".to_string(),
        })
        .build()
        .unwrap();

    assert_eq!(manifest.id().as_str(), "riscv-smoke");
    assert_eq!(manifest.boot().entry(), Address::new(0x8000));
    assert_eq!(manifest.boot().segments().len(), 2);
    assert_eq!(
        manifest.boot().segments()[0].range().size(),
        AccessSize::new(4).unwrap(),
    );
    assert_eq!(manifest.resources().len(), 2);
    assert_eq!(manifest.resources()[0].id().as_str(), "disk");
    assert_eq!(manifest.resources()[1].id().as_str(), "kernel");
    assert_eq!(manifest.required_resources().len(), 2);
    assert_eq!(manifest.host_events().len(), 2);
    assert_eq!(manifest.host_events()[0].tick(), 25 as Tick);
    assert_eq!(manifest.checkpoint_lineage().unwrap().label(), "booted");
    assert!(!manifest.identity().as_str().is_empty());
}

#[test]
fn workload_manifest_identity_is_stable_for_resource_insertion_order() {
    let first = WorkloadManifest::builder(id("stable-order"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(disk_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_required_resource(resource_id("disk"))
        .build()
        .unwrap();

    let second = WorkloadManifest::builder(id("stable-order"), boot_image())
        .add_resource(disk_resource())
        .unwrap()
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("disk"))
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    assert_eq!(first.resources(), second.resources());
    assert_eq!(first.required_resources(), second.required_resources());
    assert_eq!(first.identity(), second.identity());
}

#[test]
fn workload_manifest_rejects_missing_and_duplicate_resource_metadata() {
    let missing = WorkloadManifest::builder(id("missing-resource"), boot_image())
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap_err();
    assert_eq!(
        missing,
        WorkloadError::MissingRequiredResource {
            resource: resource_id("kernel"),
        }
    );

    let duplicate = WorkloadManifest::builder(id("duplicate-resource"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(kernel_resource())
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateResource {
            resource: resource_id("kernel"),
        }
    );

    let empty_locator = WorkloadResource::new(
        resource_id("bad"),
        WorkloadResourceKind::Input,
        "sha256:bad",
        "",
    )
    .unwrap_err();
    assert_eq!(
        empty_locator,
        WorkloadError::EmptyResourceLocator {
            resource: resource_id("bad"),
        }
    );
}

#[test]
fn workload_result_links_to_manifest_identity_and_stats_snapshot() {
    let manifest = WorkloadManifest::builder(id("result-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    let mut stats = StatsRegistry::new();
    let instructions = stats.register_counter("cpu0.committed", "inst").unwrap();
    stats.increment(instructions, 17).unwrap();
    let snapshot = stats.snapshot(90);

    let result = WorkloadResult::new(manifest.identity(), 96)
        .with_stop_reason("host-stop")
        .with_stats_snapshot(snapshot.clone())
        .with_checkpoint_label("after-roi");

    assert_eq!(result.manifest_identity(), manifest.identity());
    assert_eq!(result.final_tick(), 96);
    assert_eq!(result.stop_reason(), Some("host-stop"));
    assert_eq!(result.stats_snapshot(), Some(&snapshot));
    assert_eq!(result.checkpoint_labels(), &["after-roi".to_string()]);
}

#[test]
fn workload_result_records_parallel_execution_summary() {
    let manifest = WorkloadManifest::builder(id("result-parallel-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let summary = WorkloadParallelExecutionSummary::default()
        .with_scheduler_counts(3, 1, 7, 5)
        .with_scheduler_partitions(4, 2)
        .with_data_cache_parallel_counts(6, 9, 11, 13, 3)
        .with_data_cache_run_attribution(5, 1)
        .with_data_cache_protocol_counts([
            WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Moesi, 3),
            WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Msi, 2),
        ])
        .with_data_cache_diagnostics(17, 19);

    let result = WorkloadResult::new(manifest.identity(), 96)
        .with_parallel_execution_summary(summary.clone());

    assert_eq!(result.parallel_execution_summary(), Some(&summary));
    assert_eq!(summary.scheduler_epoch_count(), 3);
    assert_eq!(summary.scheduler_empty_epoch_count(), 1);
    assert_eq!(summary.scheduler_dispatch_count(), 7);
    assert_eq!(summary.scheduler_batch_count(), 5);
    assert_eq!(summary.active_scheduler_partition_count(), 4);
    assert_eq!(summary.max_parallel_scheduler_workers(), 2);
    assert_eq!(summary.data_cache_parallel_run_count(), 6);
    assert_eq!(summary.data_cache_parallel_scheduler_epoch_count(), 9);
    assert_eq!(summary.data_cache_parallel_scheduler_dispatch_count(), 11);
    assert_eq!(summary.data_cache_parallel_scheduler_batch_count(), 13);
    assert_eq!(summary.data_cache_parallel_scheduler_max_workers(), 3);
    assert_eq!(summary.attributed_data_cache_parallel_run_count(), 5);
    assert_eq!(summary.unattributed_data_cache_parallel_run_count(), 1);
    assert_eq!(
        summary.data_cache_protocol_counts(),
        &[
            WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Msi, 2),
            WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Moesi, 3),
        ]
    );
    assert_eq!(
        summary.data_cache_protocols(),
        vec![
            WorkloadDataCacheProtocol::Msi,
            WorkloadDataCacheProtocol::Moesi,
        ],
    );
    assert_eq!(WorkloadDataCacheProtocol::Msi.as_str(), "msi");
    assert_eq!(WorkloadDataCacheProtocol::Mesi.as_str(), "mesi");
    assert_eq!(WorkloadDataCacheProtocol::Moesi.as_str(), "moesi");
    assert!(!summary.data_cache_protocol_counts()[0].is_empty());
    assert!(WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Mesi, 0).is_empty());
    assert_eq!(summary.attributed_data_cache_protocol_run_count(), 5);
    assert_eq!(
        summary
            .data_cache_protocol_count_map()
            .get(&WorkloadDataCacheProtocol::Msi),
        Some(&2),
    );
    assert_eq!(
        summary.data_cache_parallel_run_count_for_protocol(WorkloadDataCacheProtocol::Mesi),
        0,
    );
    assert_eq!(
        summary.data_cache_parallel_run_count_for_protocol(WorkloadDataCacheProtocol::Moesi),
        3,
    );
    assert!(summary.has_data_cache_protocol(WorkloadDataCacheProtocol::Msi));
    assert!(!summary.has_data_cache_protocol(WorkloadDataCacheProtocol::Mesi));
    assert_eq!(summary.data_cache_wait_for_edge_count(), 17);
    assert_eq!(summary.data_cache_deadlock_diagnostic_count(), 19);
    assert!(summary.has_unattributed_data_cache_parallel_runs());
    assert!(summary.has_data_cache_diagnostics());
    assert_eq!(summary.full_system_parallel_scheduler_epoch_count(), 12);
    assert_eq!(summary.full_system_parallel_scheduler_dispatch_count(), 18);
    assert_eq!(summary.full_system_parallel_scheduler_batch_count(), 18);
    assert_eq!(summary.full_system_parallel_scheduler_max_workers(), 3);
    assert!(summary.has_full_system_parallel_scheduler_work());
    assert!(summary.has_parallel_scheduler_work());
    assert!(summary.has_data_cache_parallel_work());
}

#[test]
fn workload_manifest_reconstructs_boot_image_and_replay_plan() {
    let manifest = WorkloadManifest::builder(id("replay-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(disk_resource())
        .unwrap()
        .add_required_resource(resource_id("disk"))
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            80,
            HostEventIntent::Stop {
                reason: "done".to_string(),
            },
        ))
        .add_host_event(WorkloadHostEvent::new(
            20,
            HostEventIntent::RoiBegin {
                label: "main".to_string(),
            },
        ))
        .with_checkpoint_lineage(CheckpointLineage::CreatedByWorkload {
            label: "cold-boot".to_string(),
        })
        .build()
        .unwrap();

    assert_eq!(manifest.to_boot_image().unwrap(), boot_image());

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(plan.manifest_identity(), manifest.identity());
    assert_eq!(plan.boot(), manifest.boot());
    assert_eq!(plan.to_boot_image().unwrap(), boot_image());
    assert_eq!(plan.required_resources().len(), 2);
    assert_eq!(plan.required_resources()[0].id().as_str(), "disk");
    assert_eq!(
        plan.required_resources()[0].kind(),
        WorkloadResourceKind::DiskImage
    );
    assert_eq!(plan.required_resources()[1].id().as_str(), "kernel");
    assert_eq!(plan.host_events().len(), 2);
    assert_eq!(plan.host_events()[0].tick(), 20);
    assert_eq!(plan.host_events()[1].tick(), 80);
    assert_eq!(
        plan.checkpoint_lineage().unwrap(),
        &CheckpointLineage::CreatedByWorkload {
            label: "cold-boot".to_string(),
        },
    );
}

#[test]
fn workload_manifest_records_topology_for_full_system_replay() {
    let manifest = WorkloadManifest::builder(id("topology-run"), boot_image())
        .with_topology(riscv_topology())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let topology = plan.topology().unwrap();

    assert_eq!(topology.partition_count(), 4);
    assert_eq!(topology.min_remote_delay(), 2);
    assert_eq!(topology.parallel_worker_limit(), 2);
    assert_eq!(topology.host().partition(), 3);
    assert_eq!(topology.host().latency(), 2);
    assert_eq!(topology.host().source(), 41);
    assert_eq!(topology.memory_targets().len(), 1);
    assert_eq!(topology.memory_targets()[0].target(), 0);
    assert_eq!(topology.memory_targets()[0].line_bytes(), 16);
    assert_eq!(topology.memory_routes().len(), 4);
    assert_eq!(topology.memory_routes()[0].id().as_str(), "cpu0.data");
    assert_eq!(topology.riscv_cores().len(), 2);
    assert_eq!(
        topology.riscv_cores()[1].fetch_route().as_str(),
        "cpu1.fetch"
    );
    assert_eq!(topology.riscv_cores()[0].data_endpoint(), Some("cpu0.dmem"));
    assert_eq!(
        topology.riscv_cores()[0]
            .data_route()
            .map(WorkloadRouteId::as_str),
        Some("cpu0.data")
    );

    let different_topology = WorkloadManifest::builder(id("topology-run"), boot_image())
        .with_topology(
            riscv_topology()
                .add_memory_route(
                    WorkloadMemoryRoute::new(
                        route_id("extra.fetch"),
                        "cpu2.ifetch",
                        2,
                        "memory",
                        2,
                        2,
                        3,
                    )
                    .unwrap(),
                )
                .unwrap(),
        )
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    assert_ne!(manifest.identity(), different_topology.identity());
}

#[test]
fn workload_topology_exposes_external_memory_profile_by_target() {
    let profile = hbm_profile(0);
    let topology = WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 41).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                16,
                rem6_memory::AddressRange::new(
                    Address::new(0x8000),
                    AccessSize::new(0x2000).unwrap(),
                )
                .unwrap(),
            )
            .unwrap()
            .with_external_memory_profile(profile)
            .unwrap(),
        )
        .unwrap();

    assert_eq!(topology.external_memory_profile(0), Some(&profile));
    assert_eq!(topology.external_memory_profile(1), None);
}

#[test]
fn workload_manifest_identity_changes_with_external_memory_profile() {
    let hbm_two_by_two = WorkloadMemoryTarget::new(
        0,
        16,
        rem6_memory::AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap())
            .unwrap(),
    )
    .unwrap()
    .with_external_memory_profile(hbm_profile(0))
    .unwrap();

    let hbm_one_by_four = WorkloadMemoryTarget::new(
        0,
        16,
        rem6_memory::AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap())
            .unwrap(),
    )
    .unwrap()
    .with_external_memory_profile(
        ExternalMemoryProfile::hbm(
            memory_target(0),
            layout(),
            1,
            4,
            dram_geometry(),
            dram_timing(),
        )
        .unwrap(),
    )
    .unwrap();

    let first = WorkloadManifest::builder(id("profiled-topology"), boot_image())
        .with_topology(
            WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 41).unwrap())
                .unwrap()
                .add_memory_target(hbm_two_by_two)
                .unwrap(),
        )
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    let second = WorkloadManifest::builder(id("profiled-topology"), boot_image())
        .with_topology(
            WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 41).unwrap())
                .unwrap()
                .add_memory_target(hbm_one_by_four)
                .unwrap(),
        )
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    assert_ne!(first.identity(), second.identity());
}

#[test]
fn workload_result_validation_rejects_wrong_manifest_and_late_stats() {
    let manifest = WorkloadManifest::builder(id("result-source"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let other = WorkloadManifest::builder(id("different-source"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    let valid = WorkloadResult::new(manifest.identity(), 90);
    valid.verify_manifest(&manifest).unwrap();

    let wrong_manifest = WorkloadResult::new(other.identity(), 90)
        .verify_manifest(&manifest)
        .unwrap_err();
    assert_eq!(
        wrong_manifest,
        WorkloadError::ManifestIdentityMismatch {
            expected: manifest.identity(),
            actual: other.identity(),
        }
    );

    let stats = StatsRegistry::new().snapshot(95);
    let late_stats = WorkloadResult::new(manifest.identity(), 90)
        .with_stats_snapshot(stats)
        .verify_manifest(&manifest)
        .unwrap_err();
    assert_eq!(
        late_stats,
        WorkloadError::StatsAfterFinalTick {
            stats_tick: 95,
            final_tick: 90,
        }
    );
}

#[test]
fn workload_replay_plan_validates_matching_result_outputs() {
    let manifest = replay_manifest_with_planned_outputs();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let snapshot = StatsRegistry::new().snapshot(75);
    let result = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_stats_snapshot(snapshot)
        .with_checkpoint_label("warm");

    assert_eq!(plan.planned_checkpoint_labels(), &["warm".to_string()]);
    assert_eq!(plan.planned_stop_reason(), Some("done"));
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_truncated_runs() {
    let manifest = replay_manifest_with_planned_outputs();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let result = WorkloadResult::new(plan.manifest_identity(), 60)
        .with_stop_reason("done")
        .with_checkpoint_label("warm");

    let error = plan.verify_result(&result).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::PlannedHostEventAfterFinalTick {
            event_tick: 80,
            final_tick: 60,
        }
    );
}

#[test]
fn workload_replay_plan_rejects_unplanned_outputs() {
    let manifest = replay_manifest_with_planned_outputs();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let unexpected_checkpoint = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_label("cold");
    let error = plan.verify_result(&unexpected_checkpoint).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::UnexpectedCheckpointLabel {
            label: "cold".to_string(),
        }
    );

    let wrong_stop = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("aborted")
        .with_checkpoint_label("warm");
    let error = plan.verify_result(&wrong_stop).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::StopReasonMismatch {
            expected: "done".to_string(),
            actual: Some("aborted".to_string()),
        }
    );

    let missing_stop =
        WorkloadResult::new(plan.manifest_identity(), 80).with_checkpoint_label("warm");
    let error = plan.verify_result(&missing_stop).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::StopReasonMismatch {
            expected: "done".to_string(),
            actual: None,
        }
    );
}

#[test]
fn workload_replay_plan_rejects_missing_planned_checkpoint() {
    let manifest = replay_manifest_with_planned_outputs();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let result = WorkloadResult::new(plan.manifest_identity(), 80).with_stop_reason("done");

    let error = plan.verify_result(&result).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointLabel {
            label: "warm".to_string(),
        }
    );
}

#[test]
fn workload_replay_plan_rejects_stop_reason_without_planned_stop() {
    let manifest = WorkloadManifest::builder(id("no-stop-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            20,
            HostEventIntent::Checkpoint {
                label: "warm".to_string(),
            },
        ))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let result = WorkloadResult::new(plan.manifest_identity(), 20)
        .with_stop_reason("host-stop")
        .with_checkpoint_label("warm");

    let error = plan.verify_result(&result).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::UnexpectedStopReason {
            actual: "host-stop".to_string(),
        }
    );
}

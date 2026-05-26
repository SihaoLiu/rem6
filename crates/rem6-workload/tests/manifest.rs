use rem6_boot::BootImage;
use rem6_dram::{
    DramGeometry, DramMemoryTechnology, DramTiming, ExternalMemoryProfile, ExternalMemoryTopology,
    NvmMediaTiming,
};
use rem6_fabric::{QosPriority, QosRequestorId};
use rem6_kernel::Tick;
use rem6_memory::{AccessSize, Address, CacheLineLayout, MemoryTargetId};
use rem6_stats::StatsRegistry;
use rem6_workload::{
    CheckpointLineage, HostEventIntent, WorkloadAcceleratorCommand, WorkloadAcceleratorCommandKind,
    WorkloadAcceleratorDevice, WorkloadAcceleratorDmaCopy, WorkloadCheckpointComponentSummary,
    WorkloadCheckpointManifestSummary, WorkloadDiskImageConstruction,
    WorkloadDiskImageConstructionStep, WorkloadError, WorkloadExecutionMode,
    WorkloadExecutionModeSwitch, WorkloadExpectedCheckpointComponentSummary,
    WorkloadExpectedCheckpointManifestSummary, WorkloadGpuDevice, WorkloadGpuDmaCopy,
    WorkloadGpuKernelLaunch, WorkloadGuestHostCallResponse, WorkloadHostActionSummary,
    WorkloadHostEvent, WorkloadHostPlacement, WorkloadId, WorkloadManifest, WorkloadMemoryRoute,
    WorkloadMemoryTarget, WorkloadQosPolicy, WorkloadQosQueuePolicyKind,
    WorkloadQosRequestorPriority, WorkloadQosTurnaroundPolicyKind, WorkloadReplayPlan,
    WorkloadResource, WorkloadResourceAcquisition, WorkloadResourceAcquisitionField,
    WorkloadResourceAcquisitionKind, WorkloadResourceConstructionField, WorkloadResourceId,
    WorkloadResourceKind, WorkloadResourceKindField, WorkloadResult, WorkloadRiscvCore,
    WorkloadRouteFabric, WorkloadRouteHop, WorkloadRouteId, WorkloadStatsScope, WorkloadTopology,
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

fn nvm_profile(target: u32) -> ExternalMemoryProfile {
    ExternalMemoryProfile::nvm(
        memory_target(target),
        layout(),
        2,
        8,
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
    let profile = nvm_profile(0);
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
        DramMemoryTechnology::Nvm
    );
    assert_eq!(
        target.external_memory_profile().unwrap().topology(),
        ExternalMemoryTopology::Nvm {
            controllers: 2,
            media_banks_per_controller: 8,
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
fn workload_memory_route_records_fabric_path_metadata() {
    let fabric = WorkloadRouteFabric::new("mesh.cpu.mem", 16)
        .unwrap()
        .with_virtual_networks(1, 2)
        .with_credit_depth(3)
        .unwrap();
    let route =
        WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 1, 3, 5)
            .unwrap()
            .with_fabric(fabric.clone());

    assert_eq!(route.fabric(), Some(&fabric));
    assert_eq!(route.fabric().unwrap().link(), "mesh.cpu.mem");
    assert_eq!(route.fabric().unwrap().bandwidth_bytes_per_tick(), 16);
    assert_eq!(route.fabric().unwrap().request_virtual_network(), 1);
    assert_eq!(route.fabric().unwrap().response_virtual_network(), 2);
    assert_eq!(route.fabric().unwrap().credit_depth(), Some(3));

    assert_eq!(
        WorkloadRouteFabric::new("", 16).unwrap_err(),
        WorkloadError::EmptyFabricLink,
    );
    assert_eq!(
        WorkloadRouteFabric::new("mesh.cpu.mem", 0).unwrap_err(),
        WorkloadError::ZeroFabricBandwidth {
            link: "mesh.cpu.mem".to_string(),
        },
    );
    assert_eq!(
        WorkloadRouteFabric::new("mesh.cpu.mem", 16)
            .unwrap()
            .with_credit_depth(0)
            .unwrap_err(),
        WorkloadError::ZeroFabricCreditDepth {
            link: "mesh.cpu.mem".to_string(),
        },
    );
}

#[test]
fn workload_topology_records_qos_policy_and_requestor_intents() {
    let policy = WorkloadQosPolicy::new(4, QosPriority::new(3))
        .unwrap()
        .with_queue_policy(WorkloadQosQueuePolicyKind::LeastRecentlyGranted)
        .with_turnaround_policy(WorkloadQosTurnaroundPolicyKind::PreferCurrentDirection)
        .with_priority_escalation()
        .with_requestor_priority(QosRequestorId::new(8), QosPriority::new(1))
        .unwrap()
        .with_requestor_priority(QosRequestorId::new(7), QosPriority::new(0))
        .unwrap();
    let topology = riscv_topology().with_qos_policy(policy.clone());

    assert_eq!(topology.qos_policy(), Some(&policy));
    assert_eq!(policy.priority_levels(), 4);
    assert_eq!(policy.default_priority(), QosPriority::new(3));
    assert_eq!(
        policy.queue_policy(),
        WorkloadQosQueuePolicyKind::LeastRecentlyGranted,
    );
    assert_eq!(
        policy.turnaround_policy(),
        WorkloadQosTurnaroundPolicyKind::PreferCurrentDirection,
    );
    assert!(policy.priority_escalation_enabled());
    assert_eq!(
        policy.requestor_priorities(),
        &[
            WorkloadQosRequestorPriority::new(QosRequestorId::new(7), QosPriority::new(0)),
            WorkloadQosRequestorPriority::new(QosRequestorId::new(8), QosPriority::new(1)),
        ]
    );
    assert_eq!(
        policy.priority_for(QosRequestorId::new(7)),
        QosPriority::new(0),
    );
    assert_eq!(
        policy.priority_for(QosRequestorId::new(99)),
        QosPriority::new(3),
    );

    let plain = WorkloadManifest::builder(id("qos-policy-identity"), boot_image())
        .with_topology(riscv_topology())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let with_qos = WorkloadManifest::builder(id("qos-policy-identity"), boot_image())
        .with_topology(topology)
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&with_qos).unwrap();

    assert_ne!(plain.identity(), with_qos.identity());
    assert_eq!(plan.topology().unwrap().qos_policy(), Some(&policy));
}

#[test]
fn workload_qos_policy_rejects_invalid_declarations() {
    assert_eq!(
        WorkloadQosPolicy::new(0, QosPriority::new(0)).unwrap_err(),
        WorkloadError::ZeroQosPriorityLevels,
    );
    assert_eq!(
        WorkloadQosPolicy::new(2, QosPriority::new(2)).unwrap_err(),
        WorkloadError::QosPriorityOutOfRange {
            priority: QosPriority::new(2),
            priority_levels: 2,
        },
    );
    let invalid_requestor = WorkloadQosPolicy::new(2, QosPriority::new(1))
        .unwrap()
        .with_requestor_priority(QosRequestorId::new(7), QosPriority::new(3))
        .unwrap_err();
    assert_eq!(
        invalid_requestor,
        WorkloadError::QosPriorityOutOfRange {
            priority: QosPriority::new(3),
            priority_levels: 2,
        },
    );
    let duplicate = WorkloadQosPolicy::new(4, QosPriority::new(3))
        .unwrap()
        .with_requestor_priority(QosRequestorId::new(7), QosPriority::new(0))
        .unwrap()
        .with_requestor_priority(QosRequestorId::new(7), QosPriority::new(1))
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateQosRequestorPriority {
            requestor: QosRequestorId::new(7),
        },
    );
}

#[test]
fn workload_memory_route_records_multihop_fabric_path_metadata() {
    let cpu_to_router = WorkloadRouteFabric::new("mesh.cpu.router0", 8)
        .unwrap()
        .with_virtual_networks(1, 2)
        .with_credit_depth(2)
        .unwrap();
    let router_to_memory = WorkloadRouteFabric::new("mesh.router0.memory", 16)
        .unwrap()
        .with_virtual_networks(3, 4)
        .with_credit_depth(4)
        .unwrap();
    let router_hop = WorkloadRouteHop::new("router0.cpu", 1, 2, 3)
        .unwrap()
        .with_fabric(cpu_to_router.clone());
    let memory_hop = WorkloadRouteHop::new("memory", 2, 5, 7)
        .unwrap()
        .with_fabric(router_to_memory.clone());

    let route = WorkloadMemoryRoute::new_path(
        route_id("cpu0.fetch"),
        "cpu0.ifetch",
        0,
        [router_hop.clone(), memory_hop.clone()],
    )
    .unwrap();

    assert_eq!(route.source_endpoint(), "cpu0.ifetch");
    assert_eq!(route.source_partition(), 0);
    assert_eq!(route.target_endpoint(), "memory");
    assert_eq!(route.target_partition(), 2);
    assert_eq!(route.request_latency(), 7);
    assert_eq!(route.response_latency(), 10);
    assert_eq!(route.hops(), &[router_hop, memory_hop]);
    assert_eq!(route.hops()[0].fabric(), Some(&cpu_to_router));
    assert_eq!(route.hops()[1].fabric(), Some(&router_to_memory));

    assert_eq!(
        WorkloadMemoryRoute::new_path(
            route_id("empty.path"),
            "cpu0.ifetch",
            0,
            std::iter::empty::<WorkloadRouteHop>(),
        )
        .unwrap_err(),
        WorkloadError::EmptyMemoryRoutePath {
            route: route_id("empty.path"),
        },
    );
}

#[test]
fn workload_manifest_identity_changes_with_route_fabric_metadata() {
    let plain_topology = riscv_topology();
    let fabric_topology =
        WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 51).unwrap())
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
                WorkloadMemoryRoute::new(
                    route_id("cpu0.fetch"),
                    "cpu0.ifetch",
                    0,
                    "memory",
                    2,
                    2,
                    3,
                )
                .unwrap()
                .with_fabric(
                    WorkloadRouteFabric::new("mesh.cpu.mem", 8)
                        .unwrap()
                        .with_virtual_networks(1, 2),
                ),
            )
            .unwrap()
            .add_memory_route(
                WorkloadMemoryRoute::new(route_id("cpu0.data"), "cpu0.dmem", 0, "memory", 2, 2, 3)
                    .unwrap(),
            )
            .unwrap()
            .add_memory_route(
                WorkloadMemoryRoute::new(
                    route_id("cpu1.fetch"),
                    "cpu1.ifetch",
                    1,
                    "memory",
                    2,
                    2,
                    3,
                )
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
                    Address::new(0x8010),
                    "cpu1.ifetch",
                    route_id("cpu1.fetch"),
                )
                .unwrap()
                .with_data("cpu1.dmem", route_id("cpu1.data"))
                .unwrap(),
            )
            .unwrap();
    let plain = WorkloadManifest::builder(id("route-fabric-identity"), boot_image())
        .with_topology(plain_topology)
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let with_fabric = WorkloadManifest::builder(id("route-fabric-identity"), boot_image())
        .with_topology(fabric_topology)
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    assert_ne!(plain.identity(), with_fabric.identity());
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
    assert_eq!(topology.gpu_devices()[0].command_endpoint(), "gpu0.control");
    assert_eq!(topology.gpu_devices()[0].dma_endpoint(), "gpu0.dma");
    assert_eq!(topology.gpu_kernel_launches().len(), 1);
    assert_eq!(topology.gpu_kernel_launches()[0].device(), 12);
    assert_eq!(topology.gpu_kernel_launches()[0].kernel(), 90);
    assert_eq!(topology.gpu_kernel_launches()[0].workgroups(), 3);
    assert_eq!(topology.gpu_kernel_launches()[0].workgroup_latency(), 5);
}

#[test]
fn workload_topology_records_gpu_dma_copies() {
    let topology = riscv_topology()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("gpu0.dma"), "gpu0.dma", 3, "memory", 2, 3, 5)
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
        .unwrap();

    assert_eq!(topology.gpu_dma_copies().len(), 1);
    assert_eq!(topology.gpu_dma_copies()[0].device(), 12);
    assert_eq!(topology.gpu_dma_copies()[0].transfer(), 200);
    assert_eq!(topology.gpu_dma_copies()[0].route(), &route_id("gpu0.dma"));
    assert_eq!(topology.gpu_dma_copies()[0].agent(), 77);
    assert_eq!(topology.gpu_dma_copies()[0].source(), Address::new(0x9024));
    assert_eq!(
        topology.gpu_dma_copies()[0].destination(),
        Address::new(0x9048)
    );
    assert_eq!(topology.gpu_dma_copies()[0].bytes(), 4);
}

#[test]
fn workload_topology_rejects_invalid_gpu_dma_copies() {
    let topology = riscv_topology()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("gpu0.dma"), "gpu0.dma", 3, "memory", 2, 3, 5)
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
        .unwrap();

    let missing_device = topology
        .clone()
        .add_gpu_dma_copy(
            WorkloadGpuDmaCopy::new(
                99,
                200,
                route_id("gpu0.dma"),
                77,
                Address::new(0x9024),
                Address::new(0x9048),
                4,
            )
            .unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        missing_device,
        WorkloadError::MissingGpuDevice { device: 99 }
    );

    let missing_route = topology
        .add_gpu_dma_copy(
            WorkloadGpuDmaCopy::new(
                12,
                201,
                route_id("gpu0.missing-dma"),
                77,
                Address::new(0x9024),
                Address::new(0x9048),
                4,
            )
            .unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        missing_route,
        WorkloadError::MissingGpuDmaRoute {
            device: 12,
            route: route_id("gpu0.missing-dma"),
        }
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
    assert_eq!(
        topology.accelerator_devices()[0].command_endpoint(),
        "accelerator0.control"
    );
    assert_eq!(
        topology.accelerator_devices()[0].dma_endpoint(),
        "accelerator0.dma"
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
fn workload_topology_records_accelerator_dma_copies() {
    let topology = WorkloadTopology::new(5, 2, 2, WorkloadHostPlacement::new(4, 2, 11).unwrap())
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
                2,
                4,
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
        .unwrap();

    assert_eq!(topology.accelerator_dma_copies().len(), 1);
    assert_eq!(topology.accelerator_dma_copies()[0].engine(), 22);
    assert_eq!(topology.accelerator_dma_copies()[0].transfer(), 300);
    assert_eq!(
        topology.accelerator_dma_copies()[0].route(),
        &route_id("accelerator0.dma")
    );
    assert_eq!(topology.accelerator_dma_copies()[0].agent(), 88);
    assert_eq!(
        topology.accelerator_dma_copies()[0].source(),
        Address::new(0x9024)
    );
    assert_eq!(
        topology.accelerator_dma_copies()[0].destination(),
        Address::new(0x9048)
    );
    assert_eq!(topology.accelerator_dma_copies()[0].bytes(), 4);
}

#[test]
fn workload_topology_rejects_invalid_accelerator_dma_copies() {
    assert_eq!(
        WorkloadAcceleratorDmaCopy::new(
            22,
            300,
            route_id("accelerator0.dma"),
            88,
            Address::new(0x9024),
            Address::new(0x9048),
            0,
        )
        .unwrap_err(),
        WorkloadError::ZeroAcceleratorDmaCopyBytes {
            engine: 22,
            transfer: 300
        }
    );

    let topology = WorkloadTopology::new(5, 2, 2, WorkloadHostPlacement::new(4, 2, 11).unwrap())
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
                1,
                "memory",
                2,
                2,
                4,
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
        .unwrap();

    assert_eq!(
        topology
            .clone()
            .add_accelerator_dma_copy(
                WorkloadAcceleratorDmaCopy::new(
                    22,
                    300,
                    route_id("missing.dma"),
                    88,
                    Address::new(0x9024),
                    Address::new(0x9048),
                    4,
                )
                .unwrap(),
            )
            .unwrap_err(),
        WorkloadError::MissingAcceleratorDmaRoute {
            engine: 22,
            route: route_id("missing.dma"),
        }
    );

    assert_eq!(
        topology
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
            .unwrap_err(),
        WorkloadError::AcceleratorDmaRouteSourceMismatch {
            engine: 22,
            route: route_id("accelerator0.dma"),
            expected: 3,
            actual: 1,
        }
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
fn workload_manifest_records_resource_acquisition_provenance() {
    let acquisition = WorkloadResourceAcquisition::new(
        WorkloadResourceAcquisitionKind::RemoteUri,
        "https://resources.example/kernel.elf",
    )
    .unwrap()
    .with_tool("resource-cache")
    .unwrap()
    .with_revision("sha256:resource-index")
    .unwrap();
    let resource = kernel_resource().with_acquisition(acquisition.clone());
    let manifest = WorkloadManifest::builder(id("resource-provenance"), boot_image())
        .add_resource(resource.clone())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    assert_eq!(
        manifest.resource(resource.id()).unwrap().acquisition(),
        Some(&acquisition)
    );
    assert_eq!(
        acquisition.kind(),
        WorkloadResourceAcquisitionKind::RemoteUri
    );
    assert_eq!(
        acquisition.locator(),
        "https://resources.example/kernel.elf"
    );
    assert_eq!(acquisition.tool(), Some("resource-cache"));
    assert_eq!(acquisition.revision(), Some("sha256:resource-index"));

    let without_acquisition = WorkloadManifest::builder(id("resource-provenance"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let different_revision = WorkloadManifest::builder(id("resource-provenance"), boot_image())
        .add_resource(
            kernel_resource().with_acquisition(
                WorkloadResourceAcquisition::new(
                    WorkloadResourceAcquisitionKind::RemoteUri,
                    "https://resources.example/kernel.elf",
                )
                .unwrap()
                .with_tool("resource-cache")
                .unwrap()
                .with_revision("sha256:different-resource-index")
                .unwrap(),
            ),
        )
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    assert_ne!(manifest.identity(), without_acquisition.identity());
    assert_ne!(manifest.identity(), different_revision.identity());
    assert_eq!(
        WorkloadResourceAcquisition::new(WorkloadResourceAcquisitionKind::LocalFile, "")
            .unwrap_err(),
        WorkloadError::EmptyResourceAcquisitionField {
            field: WorkloadResourceAcquisitionField::Locator,
        }
    );
    assert_eq!(
        WorkloadResourceAcquisition::new(WorkloadResourceAcquisitionKind::Generated, "build:dtb")
            .unwrap()
            .with_tool("")
            .unwrap_err(),
        WorkloadError::EmptyResourceAcquisitionField {
            field: WorkloadResourceAcquisitionField::Tool,
        }
    );
}

#[test]
fn workload_manifest_records_disk_image_construction_provenance() {
    let construction = WorkloadDiskImageConstruction::new("raw", 8 * 1024 * 1024)
        .unwrap()
        .with_step(
            WorkloadDiskImageConstructionStep::new("mkfs.ext4", "format", "rootfs.ext4")
                .unwrap()
                .with_argument("-O ^metadata_csum")
                .unwrap(),
        );
    let disk = disk_resource()
        .with_disk_image_construction(construction.clone())
        .unwrap();
    let manifest = WorkloadManifest::builder(id("disk-image-construction"), boot_image())
        .add_resource(disk.clone())
        .unwrap()
        .add_required_resource(resource_id("disk"))
        .build()
        .unwrap();

    assert_eq!(
        manifest
            .resource(disk.id())
            .unwrap()
            .disk_image_construction(),
        Some(&construction)
    );
    assert_eq!(construction.image_format(), "raw");
    assert_eq!(construction.virtual_size_bytes(), 8 * 1024 * 1024);
    assert_eq!(construction.steps().len(), 1);
    assert_eq!(construction.steps()[0].tool(), "mkfs.ext4");
    assert_eq!(construction.steps()[0].operation(), "format");
    assert_eq!(construction.steps()[0].input(), "rootfs.ext4");
    assert_eq!(construction.steps()[0].arguments(), &["-O ^metadata_csum"]);

    let without_construction =
        WorkloadManifest::builder(id("disk-image-construction"), boot_image())
            .add_resource(disk_resource())
            .unwrap()
            .add_required_resource(resource_id("disk"))
            .build()
            .unwrap();
    let different_argument = WorkloadManifest::builder(id("disk-image-construction"), boot_image())
        .add_resource(
            disk_resource()
                .with_disk_image_construction(
                    WorkloadDiskImageConstruction::new("raw", 8 * 1024 * 1024)
                        .unwrap()
                        .with_step(
                            WorkloadDiskImageConstructionStep::new(
                                "mkfs.ext4",
                                "format",
                                "rootfs.ext4",
                            )
                            .unwrap()
                            .with_argument("-O metadata_csum")
                            .unwrap(),
                        ),
                )
                .unwrap(),
        )
        .unwrap()
        .add_required_resource(resource_id("disk"))
        .build()
        .unwrap();

    assert_ne!(manifest.identity(), without_construction.identity());
    assert_ne!(manifest.identity(), different_argument.identity());
    assert_eq!(
        kernel_resource()
            .with_disk_image_construction(construction)
            .unwrap_err(),
        WorkloadError::ResourceKindFieldMismatch {
            resource: resource_id("kernel"),
            field: WorkloadResourceKindField::DiskImageConstruction,
            expected: WorkloadResourceKind::DiskImage,
            actual: WorkloadResourceKind::Kernel,
        }
    );
    assert_eq!(
        WorkloadDiskImageConstruction::new("", 4096).unwrap_err(),
        WorkloadError::EmptyResourceConstructionField {
            field: WorkloadResourceConstructionField::ImageFormat,
        }
    );
    assert_eq!(
        WorkloadDiskImageConstruction::new("raw", 0).unwrap_err(),
        WorkloadError::ZeroDiskImageVirtualSizeBytes
    );
    assert_eq!(
        WorkloadDiskImageConstructionStep::new("", "format", "rootfs.ext4").unwrap_err(),
        WorkloadError::EmptyResourceConstructionField {
            field: WorkloadResourceConstructionField::Tool,
        }
    );
}

#[test]
fn workload_manifest_identity_changes_with_checkpoint_component_summary_expectations() {
    let manifest_with = |capture_component: &str, restore_payload_bytes: usize| {
        WorkloadManifest::builder(id("checkpoint-component-identity"), boot_image())
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
                40,
                HostEventIntent::RestoreCheckpoint {
                    label: "warm".to_string(),
                },
            ))
            .add_expected_checkpoint_component_summary(
                WorkloadExpectedCheckpointComponentSummary::new("warm", capture_component, 2, 8),
            )
            .unwrap()
            .add_expected_checkpoint_restore_component_summary(
                WorkloadExpectedCheckpointComponentSummary::new(
                    "warm",
                    "memory0",
                    1,
                    restore_payload_bytes,
                ),
            )
            .unwrap()
            .build()
            .unwrap()
    };

    let first = manifest_with("cpu0", 16);
    let different_capture_component = manifest_with("cpu1", 16);
    let different_restore_payload = manifest_with("cpu0", 32);

    assert_ne!(first.identity(), different_capture_component.identity());
    assert_ne!(first.identity(), different_restore_payload.identity());
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
        .with_checkpoint_label("after-roi")
        .with_restored_checkpoint_label("after-roi")
        .with_execution_mode_switch(40, "cpu0", WorkloadExecutionMode::Functional);

    assert_eq!(result.manifest_identity(), manifest.identity());
    assert_eq!(result.final_tick(), 96);
    assert_eq!(result.stop_reason(), Some("host-stop"));
    assert_eq!(result.stats_snapshot(), Some(&snapshot));
    assert_eq!(result.checkpoint_labels(), &["after-roi".to_string()]);
    assert_eq!(
        result.restored_checkpoint_labels(),
        &["after-roi".to_string()]
    );
    assert_eq!(
        result.execution_mode_switches(),
        &[WorkloadExecutionModeSwitch::new(
            40,
            "cpu0",
            WorkloadExecutionMode::Functional,
        )]
    );
}

#[test]
fn workload_result_records_execution_mode_stats_scope() {
    let manifest = WorkloadManifest::builder(id("mode-scope-result"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    let result = WorkloadResult::new(manifest.identity(), 96)
        .with_execution_mode_switch_stats_scope(52, "cpu0", WorkloadExecutionMode::Timing, 3, 40);

    assert_eq!(
        result.execution_mode_switches(),
        &[
            WorkloadExecutionModeSwitch::new(52, "cpu0", WorkloadExecutionMode::Timing)
                .with_stats_scope(3, 40)
        ]
    );
    assert_eq!(
        result.execution_mode_switches()[0].stats_scope(),
        Some(&WorkloadStatsScope::new(3, 40))
    );
}

#[test]
fn workload_result_records_host_action_summary() {
    let manifest = WorkloadManifest::builder(id("host-summary-result"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let mut summary = WorkloadHostActionSummary::default();
    summary.record_stats_reset();
    summary.record_stats_dump();
    summary.record_checkpoint();
    summary.record_execution_mode_switch();
    summary.record_guest_host_call();
    summary.record_stop();

    let result =
        WorkloadResult::new(manifest.identity(), 96).with_host_action_summary(summary.clone());

    assert_eq!(result.host_action_summary(), Some(&summary));
    assert_eq!(summary.total_action_count(), 6);
    assert_eq!(summary.stats_reset_count(), 1);
    assert_eq!(summary.stats_dump_count(), 1);
    assert_eq!(summary.checkpoint_count(), 1);
    assert_eq!(summary.execution_mode_switch_count(), 1);
    assert_eq!(summary.guest_host_call_count(), 1);
    assert_eq!(summary.stop_count(), 1);
    assert!(summary.has_host_actions());
}

#[test]
fn workload_manifest_records_guest_host_call_events() {
    let manifest = WorkloadManifest::builder(id("guest-host-call-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            32,
            HostEventIntent::GuestHostCall {
                selector: 0x42,
                arguments: vec![7, 9],
                payload: vec![1, 3, 5],
                response: Some(WorkloadGuestHostCallResponse::ok(vec![11, 13], vec![2, 4])),
            },
        ))
        .build()
        .unwrap();
    let without_response = WorkloadManifest::builder(id("guest-host-call-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            32,
            HostEventIntent::GuestHostCall {
                selector: 0x42,
                arguments: vec![7, 9],
                payload: vec![1, 3, 5],
                response: None,
            },
        ))
        .build()
        .unwrap();

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    assert_ne!(manifest.identity(), without_response.identity());
    assert_eq!(plan.host_events().len(), 1);
    assert_eq!(plan.host_events()[0].tick(), 32);
    assert_eq!(
        plan.host_events()[0].intent(),
        &HostEventIntent::GuestHostCall {
            selector: 0x42,
            arguments: vec![7, 9],
            payload: vec![1, 3, 5],
            response: Some(WorkloadGuestHostCallResponse::ok(vec![11, 13], vec![2, 4],)),
        },
    );
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
        .add_host_event(WorkloadHostEvent::new(
            40,
            HostEventIntent::SwitchExecutionMode {
                target: "cpu0".to_string(),
                mode: WorkloadExecutionMode::Functional,
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
    assert_eq!(plan.host_events().len(), 3);
    assert_eq!(plan.host_events()[0].tick(), 20);
    assert_eq!(
        plan.host_events()[1].intent(),
        &HostEventIntent::SwitchExecutionMode {
            target: "cpu0".to_string(),
            mode: WorkloadExecutionMode::Functional,
        },
    );
    assert_eq!(plan.host_events()[2].tick(), 80);
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
    let profiled_target = |profile| {
        WorkloadMemoryTarget::new(
            0,
            16,
            rem6_memory::AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap())
                .unwrap(),
        )
        .unwrap()
        .with_external_memory_profile(profile)
        .unwrap()
    };

    let hbm_two_by_two = profiled_target(hbm_profile(0));
    let hbm_one_by_four = profiled_target(
        ExternalMemoryProfile::hbm(
            memory_target(0),
            layout(),
            1,
            4,
            dram_geometry(),
            dram_timing(),
        )
        .unwrap(),
    );
    let nvm_two_by_eight = profiled_target(nvm_profile(0));
    let nvm_two_by_eight_with_media = profiled_target(
        nvm_profile(0)
            .with_nvm_media_timing(NvmMediaTiming::new(30, 50, 6, 4, 1).unwrap())
            .unwrap(),
    );

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
    let third = WorkloadManifest::builder(id("profiled-topology"), boot_image())
        .with_topology(
            WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 41).unwrap())
                .unwrap()
                .add_memory_target(nvm_two_by_eight)
                .unwrap(),
        )
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let fourth = WorkloadManifest::builder(id("profiled-topology"), boot_image())
        .with_topology(
            WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 41).unwrap())
                .unwrap()
                .add_memory_target(nvm_two_by_eight_with_media)
                .unwrap(),
        )
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    assert_ne!(first.identity(), second.identity());
    assert_ne!(first.identity(), third.identity());
    assert_ne!(third.identity(), fourth.identity());
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
fn workload_replay_plan_validates_checkpoint_manifest_summary_requirements() {
    let manifest = WorkloadManifest::builder(id("checkpoint-summary-run"), boot_image())
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
        .add_expected_checkpoint_manifest_summary(WorkloadExpectedCheckpointManifestSummary::new(
            "warm", 2, 3, 16,
        ))
        .unwrap()
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_label("warm");
    let error = plan.verify_result(&missing).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointManifestSummary {
            label: "warm".to_string(),
        }
    );

    let undercovered = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 20, 1, 3, 12,
        ));
    let error = plan.verify_result(&undercovered).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::CheckpointManifestSummaryBelowMinimum {
            label: "warm".to_string(),
            minimum_component_count: 2,
            actual_component_count: 1,
            minimum_chunk_count: 3,
            actual_chunk_count: 3,
            minimum_payload_bytes: 16,
            actual_payload_bytes: 12,
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 20, 2, 3, 16,
        ));
    plan.verify_result(&matched).unwrap();
}

#[test]
fn workload_replay_plan_validates_checkpoint_component_summary_requirements() {
    let manifest = WorkloadManifest::builder(id("checkpoint-component-summary-run"), boot_image())
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
        .add_expected_checkpoint_component_summary(WorkloadExpectedCheckpointComponentSummary::new(
            "warm", "cpu0", 2, 8,
        ))
        .unwrap()
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                20,
                [WorkloadCheckpointComponentSummary::new("memory0", 1, 16)],
            ),
        );
    let error = plan.verify_result(&missing).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointComponentSummary {
            label: "warm".to_string(),
            component: "cpu0".to_string(),
        }
    );

    let undercovered = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                20,
                [WorkloadCheckpointComponentSummary::new("cpu0", 1, 4)],
            ),
        );
    let error = plan.verify_result(&undercovered).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::CheckpointComponentSummaryBelowMinimum {
            label: "warm".to_string(),
            component: "cpu0".to_string(),
            minimum_chunk_count: 2,
            actual_chunk_count: 1,
            minimum_payload_bytes: 8,
            actual_payload_bytes: 4,
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 80)
        .with_stop_reason("done")
        .with_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                20,
                [WorkloadCheckpointComponentSummary::new("cpu0", 2, 8)],
            ),
        );
    plan.verify_result(&matched).unwrap();
}

#[test]
fn workload_replay_plan_validates_planned_checkpoint_restores() {
    let manifest = WorkloadManifest::builder(id("checkpoint-restore-run"), boot_image())
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
            40,
            HostEventIntent::RestoreCheckpoint {
                label: "warm".to_string(),
            },
        ))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    assert_eq!(plan.planned_checkpoint_labels(), &["warm".to_string()]);
    assert_eq!(
        plan.planned_checkpoint_restore_labels(),
        &["warm".to_string()]
    );

    let missing_restore =
        WorkloadResult::new(plan.manifest_identity(), 40).with_checkpoint_label("warm");
    let error = plan.verify_result(&missing_restore).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointRestoreLabel {
            label: "warm".to_string(),
        }
    );

    let unexpected_restore = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_label("cold");
    let error = plan.verify_result(&unexpected_restore).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::UnexpectedCheckpointRestoreLabel {
            label: "cold".to_string(),
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_label("warm");
    plan.verify_result(&matched).unwrap();
}

#[test]
fn workload_replay_plan_validates_checkpoint_restore_manifest_summary_requirements() {
    let manifest = WorkloadManifest::builder(id("checkpoint-restore-summary-run"), boot_image())
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
            40,
            HostEventIntent::RestoreCheckpoint {
                label: "warm".to_string(),
            },
        ))
        .add_expected_checkpoint_restore_manifest_summary(
            WorkloadExpectedCheckpointManifestSummary::new("warm", 2, 3, 16),
        )
        .unwrap()
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_label("warm");
    let error = plan.verify_result(&missing).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointRestoreManifestSummary {
            label: "warm".to_string(),
        }
    );

    let undercovered = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 20, 2, 2, 8,
        ));
    let error = plan.verify_result(&undercovered).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::CheckpointRestoreManifestSummaryBelowMinimum {
            label: "warm".to_string(),
            minimum_component_count: 2,
            actual_component_count: 2,
            minimum_chunk_count: 3,
            actual_chunk_count: 2,
            minimum_payload_bytes: 16,
            actual_payload_bytes: 8,
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(WorkloadCheckpointManifestSummary::new(
            "warm", 20, 2, 3, 16,
        ));
    plan.verify_result(&matched).unwrap();
}

#[test]
fn workload_replay_plan_validates_checkpoint_restore_component_summary_requirements() {
    let manifest =
        WorkloadManifest::builder(id("checkpoint-restore-component-summary-run"), boot_image())
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
                40,
                HostEventIntent::RestoreCheckpoint {
                    label: "warm".to_string(),
                },
            ))
            .add_expected_checkpoint_restore_component_summary(
                WorkloadExpectedCheckpointComponentSummary::new("warm", "memory0", 1, 16),
            )
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                20,
                [WorkloadCheckpointComponentSummary::new("cpu0", 2, 8)],
            ),
        );
    let error = plan.verify_result(&missing).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingCheckpointRestoreComponentSummary {
            label: "warm".to_string(),
            component: "memory0".to_string(),
        }
    );

    let undercovered = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                20,
                [WorkloadCheckpointComponentSummary::new("memory0", 1, 8)],
            ),
        );
    let error = plan.verify_result(&undercovered).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::CheckpointRestoreComponentSummaryBelowMinimum {
            label: "warm".to_string(),
            component: "memory0".to_string(),
            minimum_chunk_count: 1,
            actual_chunk_count: 1,
            minimum_payload_bytes: 16,
            actual_payload_bytes: 8,
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_checkpoint_label("warm")
        .with_restored_checkpoint_manifest_summary(
            WorkloadCheckpointManifestSummary::with_component_summaries(
                "warm",
                20,
                [WorkloadCheckpointComponentSummary::new("memory0", 1, 16)],
            ),
        );
    plan.verify_result(&matched).unwrap();
}

#[test]
fn workload_replay_plan_validates_planned_execution_mode_switches() {
    let manifest = WorkloadManifest::builder(id("mode-switch-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_host_event(WorkloadHostEvent::new(
            40,
            HostEventIntent::SwitchExecutionMode {
                target: "cpu0".to_string(),
                mode: WorkloadExecutionMode::Functional,
            },
        ))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let missing = WorkloadResult::new(plan.manifest_identity(), 40);
    let error = plan.verify_result(&missing).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::MissingExecutionModeSwitch {
            tick: 40,
            target: "cpu0".to_string(),
            mode: WorkloadExecutionMode::Functional,
        }
    );

    let unexpected = WorkloadResult::new(plan.manifest_identity(), 40).with_execution_mode_switch(
        40,
        "cpu1",
        WorkloadExecutionMode::Detailed,
    );
    let error = plan.verify_result(&unexpected).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::UnexpectedExecutionModeSwitch {
            tick: 40,
            target: "cpu1".to_string(),
            mode: WorkloadExecutionMode::Detailed,
        }
    );

    let matched = WorkloadResult::new(plan.manifest_identity(), 40).with_execution_mode_switch(
        40,
        "cpu0",
        WorkloadExecutionMode::Functional,
    );
    plan.verify_result(&matched).unwrap();

    let scoped = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_execution_mode_switch_stats_scope(
            40,
            "cpu0",
            WorkloadExecutionMode::Functional,
            1,
            20,
        );
    plan.verify_result(&scoped).unwrap();
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

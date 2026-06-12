use rem6_cache::MshrQueueConfig;
use rem6_coherence::MsiBankDirectoryHarness;
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_gpu::{
    GpuComputeConfig, GpuDeviceId, GpuDmaCompletion, GpuDmaCopy, GpuDmaId, GpuError, GpuKernelId,
    GpuKernelLaunch, GpuTopologyConfig, GpuTraceEvent, GpuTraceKind, GpuWorkgroupCompletion,
    GpuWorkgroupId,
};
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryTargetId,
    PartitionedMemoryStore, ResponseStatus,
};
use rem6_system::{RiscvTopologySystem, RiscvTopologySystemError};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, PortDirection, PortName, Topology,
    TopologyBuilder,
};
use rem6_transport::{MemoryTrace, MemoryTraceEvent, MemoryTraceKind, TransportEndpointId};

fn component(name: &str) -> ComponentId {
    ComponentId::new(name).unwrap()
}

fn kind(name: &str) -> ComponentKind {
    ComponentKind::new(name).unwrap()
}

fn port(name: &str) -> PortName {
    PortName::new(name).unwrap()
}

fn endpoint(component_name: &str, port_name: &str) -> Endpoint {
    Endpoint::new(component(component_name), port(port_name))
}

fn transport_endpoint(component_name: &str, port_name: &str) -> TransportEndpointId {
    TransportEndpointId::from_topology_endpoint(&endpoint(component_name, port_name)).unwrap()
}

fn clock(period: u64) -> ClockDomain {
    ClockDomain::new(period).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn topology_with_gpu() -> Topology {
    TopologyBuilder::new(3)
        .add_component(
            ComponentSpec::new(
                component("cpu0"),
                kind("cpu"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("ifetch"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("dmem"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("gpu"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("gpu0"),
                kind("gpu"),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(port("control"), PortDirection::Target)
            .unwrap()
            .add_port(port("dma"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mem0"),
                kind("dram"),
                PartitionId::new(2),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(
            endpoint("cpu0", "ifetch"),
            endpoint("mem0", "requests"),
            2,
            2,
        )
        .unwrap()
        .connect_with_latencies(endpoint("cpu0", "dmem"), endpoint("mem0", "requests"), 2, 2)
        .unwrap()
        .connect_with_latencies(endpoint("cpu0", "gpu"), endpoint("gpu0", "control"), 4, 1)
        .unwrap()
        .connect_with_latencies(endpoint("gpu0", "dma"), endpoint("mem0", "requests"), 3, 5)
        .unwrap()
        .build()
        .unwrap()
}

fn core_config() -> RiscvCoreTopologyConfig {
    RiscvCoreTopologyConfig::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(7),
            Address::new(0x8000),
        ),
        endpoint("cpu0", "ifetch"),
        endpoint("mem0", "requests"),
        layout(),
        AccessSize::new(4).unwrap(),
    )
    .with_data(
        endpoint("cpu0", "dmem"),
        endpoint("mem0", "requests"),
        layout(),
    )
}

fn gpu_config(device: GpuDeviceId) -> GpuTopologyConfig {
    GpuTopologyConfig::new(
        GpuComputeConfig::new(device, PartitionId::new(1), 2, 1).unwrap(),
        endpoint("cpu0", "gpu"),
        endpoint("gpu0", "control"),
    )
}

fn gpu_config_with_memory(device: GpuDeviceId) -> GpuTopologyConfig {
    gpu_config(device).with_memory(endpoint("gpu0", "dma"), endpoint("mem0", "requests"))
}

fn memory_request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(77), sequence)
}

fn memory_read(sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        memory_request(sequence),
        Address::new(address),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap()
}

fn gpu_dma_copy(route: rem6_transport::MemoryRouteId, transfer: u64) -> GpuDmaCopy {
    GpuDmaCopy::new(
        GpuDmaId::new(transfer),
        route,
        memory_read(transfer * 2, 0x1004),
        route,
        memory_request(transfer * 2 + 1),
        Address::new(0x3008),
    )
    .unwrap()
}

fn memory_store() -> PartitionedMemoryStore {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x1000),
            AccessSize::new(0x3000).unwrap(),
        )
        .unwrap();

    let mut source_line = vec![0; 16];
    source_line[4..8].copy_from_slice(&[0x3a, 0x4b, 0x5c, 0x6d]);
    store
        .insert_line(target, Address::new(0x1000), source_line)
        .unwrap();
    store
        .insert_line(target, Address::new(0x3000), vec![0; 16])
        .unwrap();
    store
}

fn gpu_bank_data_harness() -> MsiBankDirectoryHarness {
    let mut harness = MsiBankDirectoryHarness::new_with_mshr(
        layout(),
        [AgentId::new(77)],
        MshrQueueConfig::new(2, 3, 0).unwrap(),
    )
    .unwrap();

    let mut cached_source = vec![0; 16];
    cached_source[4..8].copy_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]);
    harness
        .insert_backing_line(Address::new(0x1000), cached_source)
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x3000), vec![0; 16])
        .unwrap();
    harness
}

fn unregistered_gpu_bank_data_harness() -> MsiBankDirectoryHarness {
    let mut harness = MsiBankDirectoryHarness::new_with_mshr(
        layout(),
        [AgentId::new(7)],
        MshrQueueConfig::new(2, 3, 0).unwrap(),
    )
    .unwrap();

    let mut cached_source = vec![0; 16];
    cached_source[4..8].copy_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]);
    harness
        .insert_backing_line(Address::new(0x1000), cached_source)
        .unwrap();
    harness
}

#[test]
fn topology_system_attaches_gpu_and_submits_kernel_over_control_path() {
    let gpu_id = GpuDeviceId::new(30);
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        topology_with_gpu(),
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap()
    .with_gpu(gpu_config(gpu_id))
    .unwrap();
    let launch = GpuKernelLaunch::new(GpuKernelId::new(90), 3, 5).unwrap();

    assert_eq!(system.gpu(gpu_id).unwrap().command_path().latency(), 4);
    assert_eq!(
        system
            .gpus()
            .map(|(device, gpu)| (device, gpu.gpu().partition()))
            .collect::<Vec<_>>(),
        vec![(gpu_id, PartitionId::new(1))],
    );
    system
        .submit_gpu_kernel_parallel(gpu_id, launch.clone())
        .unwrap();
    system.scheduler_mut().run_until_idle_parallel().unwrap();

    let gpu = system.gpu(gpu_id).unwrap().gpu();
    assert_eq!(
        gpu.completions(),
        vec![
            GpuWorkgroupCompletion::new(GpuKernelId::new(90), GpuWorkgroupId::new(0), 0, 0, 4, 9,),
            GpuWorkgroupCompletion::new(GpuKernelId::new(90), GpuWorkgroupId::new(1), 1, 0, 4, 9,),
            GpuWorkgroupCompletion::new(GpuKernelId::new(90), GpuWorkgroupId::new(2), 0, 0, 9, 14,),
        ],
    );
    assert_eq!(
        gpu.trace(),
        vec![
            GpuTraceEvent::new(
                0,
                GpuTraceKind::LaunchSubmitted {
                    kernel: GpuKernelId::new(90),
                    source: PartitionId::new(0),
                    target: PartitionId::new(1),
                },
            ),
            GpuTraceEvent::new(
                4,
                GpuTraceKind::LaunchAccepted {
                    kernel: GpuKernelId::new(90),
                    workgroups: 3,
                },
            ),
            GpuTraceEvent::new(
                4,
                GpuTraceKind::WorkgroupStarted {
                    kernel: GpuKernelId::new(90),
                    workgroup: GpuWorkgroupId::new(0),
                    compute_unit: 0,
                    slot: 0,
                    complete_at: 9,
                },
            ),
            GpuTraceEvent::new(
                4,
                GpuTraceKind::WorkgroupStarted {
                    kernel: GpuKernelId::new(90),
                    workgroup: GpuWorkgroupId::new(1),
                    compute_unit: 1,
                    slot: 0,
                    complete_at: 9,
                },
            ),
            GpuTraceEvent::new(
                9,
                GpuTraceKind::WorkgroupCompleted {
                    kernel: GpuKernelId::new(90),
                    workgroup: GpuWorkgroupId::new(0),
                    compute_unit: 0,
                    slot: 0,
                },
            ),
            GpuTraceEvent::new(
                9,
                GpuTraceKind::WorkgroupCompleted {
                    kernel: GpuKernelId::new(90),
                    workgroup: GpuWorkgroupId::new(1),
                    compute_unit: 1,
                    slot: 0,
                },
            ),
            GpuTraceEvent::new(
                9,
                GpuTraceKind::WorkgroupStarted {
                    kernel: GpuKernelId::new(90),
                    workgroup: GpuWorkgroupId::new(2),
                    compute_unit: 0,
                    slot: 0,
                    complete_at: 14,
                },
            ),
            GpuTraceEvent::new(
                14,
                GpuTraceKind::WorkgroupCompleted {
                    kernel: GpuKernelId::new(90),
                    workgroup: GpuWorkgroupId::new(2),
                    compute_unit: 0,
                    slot: 0,
                },
            ),
        ],
    );
}

#[test]
fn topology_system_runs_gpu_dma_copy_on_parallel_memory_backend() {
    let gpu_id = GpuDeviceId::new(33);
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        topology_with_gpu(),
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap()
    .with_memory_store(memory_store())
    .unwrap()
    .with_gpu(gpu_config_with_memory(gpu_id))
    .unwrap();
    let route = system.gpu(gpu_id).unwrap().memory_route().unwrap();
    let trace = MemoryTrace::new();
    let copy = GpuDmaCopy::new(
        GpuDmaId::new(120),
        route,
        memory_read(240, 0x1004),
        route,
        memory_request(241),
        Address::new(0x3008),
    )
    .unwrap();

    system
        .run_gpu_dma_copy_parallel(gpu_id, copy, trace.clone())
        .unwrap();

    let destination = system
        .memory_store()
        .unwrap()
        .lock()
        .unwrap()
        .line_data(MemoryTargetId::new(0), Address::new(0x3000))
        .unwrap();
    assert_eq!(&destination[8..12], &[0x3a, 0x4b, 0x5c, 0x6d]);
    assert_eq!(
        system.gpu(gpu_id).unwrap().gpu().dma_completions(),
        vec![GpuDmaCompletion::new(
            GpuDmaId::new(120),
            memory_request(240),
            memory_request(241),
            8,
            16,
        )],
    );
    assert_eq!(
        system.gpu(gpu_id).unwrap().gpu().trace(),
        vec![
            GpuTraceEvent::new(
                0,
                GpuTraceKind::DmaReadIssued {
                    transfer: GpuDmaId::new(120),
                    request: memory_request(240),
                },
            ),
            GpuTraceEvent::new(
                8,
                GpuTraceKind::DmaReadCompleted {
                    transfer: GpuDmaId::new(120),
                    request: memory_request(240),
                    bytes: 4,
                },
            ),
            GpuTraceEvent::new(
                8,
                GpuTraceKind::DmaWriteIssued {
                    transfer: GpuDmaId::new(120),
                    request: memory_request(241),
                },
            ),
            GpuTraceEvent::new(
                16,
                GpuTraceKind::DmaWriteCompleted {
                    transfer: GpuDmaId::new(120),
                    request: memory_request(241),
                },
            ),
        ],
    );
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                route,
                transport_endpoint("gpu0", "dma"),
                MemoryTraceKind::RequestSent,
                memory_request(240),
            ),
            MemoryTraceEvent::request(
                3,
                route,
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                memory_request(240),
            ),
            MemoryTraceEvent::response(
                8,
                route,
                transport_endpoint("gpu0", "dma"),
                memory_request(240),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::request(
                8,
                route,
                transport_endpoint("gpu0", "dma"),
                MemoryTraceKind::RequestSent,
                memory_request(241),
            ),
            MemoryTraceEvent::request(
                11,
                route,
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                memory_request(241),
            ),
            MemoryTraceEvent::response(
                16,
                route,
                transport_endpoint("gpu0", "dma"),
                memory_request(241),
                ResponseStatus::Completed,
            ),
        ],
    );
}

#[test]
fn topology_system_routes_gpu_dma_copy_through_msi_bank_data_cache() {
    let gpu_id = GpuDeviceId::new(34);
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        topology_with_gpu(),
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap()
    .with_memory_store(memory_store())
    .unwrap()
    .with_gpu(gpu_config_with_memory(gpu_id))
    .unwrap()
    .with_msi_bank_data_cache(gpu_bank_data_harness())
    .unwrap();
    let route = system.gpu(gpu_id).unwrap().memory_route().unwrap();

    let summary = system
        .run_gpu_dma_copy_parallel_recorded(gpu_id, gpu_dma_copy(route, 121), MemoryTrace::new())
        .unwrap();

    let harness = system.msi_bank_data_cache().unwrap();
    let harness = harness.lock().unwrap();
    let destination = harness
        .cache_data(AgentId::new(77), Address::new(0x3000))
        .unwrap()
        .unwrap();
    assert_eq!(&destination[8..12], &[0xaa, 0xbb, 0xcc, 0xdd]);
    let history = harness.parallel_cycle_history();
    assert_eq!(history.cycle_count(), 2);
    assert_eq!(history.total_accepted(), 2);
    assert_eq!(history.accepted_by_agent(AgentId::new(77)), 2);
    assert_eq!(history.accepted_by_line(Address::new(0x1000)), 1);
    assert_eq!(history.accepted_by_line(Address::new(0x3000)), 1);
    assert_eq!(system.msi_bank_data_cache_runs().len(), 2);
    assert_eq!(summary.read().dram_access_count(), 0);
    assert_eq!(summary.write().dram_access_count(), 0);
}

#[test]
fn topology_system_keeps_unregistered_gpu_dma_off_msi_bank_data_cache() {
    let gpu_id = GpuDeviceId::new(35);
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        topology_with_gpu(),
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap()
    .with_memory_store(memory_store())
    .unwrap()
    .with_gpu(gpu_config_with_memory(gpu_id))
    .unwrap()
    .with_msi_bank_data_cache(unregistered_gpu_bank_data_harness())
    .unwrap();
    let route = system.gpu(gpu_id).unwrap().memory_route().unwrap();

    system
        .run_gpu_dma_copy_parallel_recorded(gpu_id, gpu_dma_copy(route, 122), MemoryTrace::new())
        .unwrap();

    let destination = system
        .memory_store()
        .unwrap()
        .lock()
        .unwrap()
        .line_data(MemoryTargetId::new(0), Address::new(0x3000))
        .unwrap();
    assert_eq!(&destination[8..12], &[0x3a, 0x4b, 0x5c, 0x6d]);
    let harness = system.msi_bank_data_cache().unwrap();
    let harness = harness.lock().unwrap();
    assert!(harness.parallel_cycle_history().is_empty());
    assert!(system.msi_bank_data_cache_runs().is_empty());
}

#[test]
fn topology_system_rejects_duplicate_and_unknown_gpu_without_running_scheduler() {
    let gpu_id = GpuDeviceId::new(31);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology_with_gpu(),
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap()
    .with_gpu(gpu_config(gpu_id))
    .unwrap();

    let error = match system.with_gpu(gpu_config(gpu_id)) {
        Ok(_) => panic!("duplicate GPU attach unexpectedly succeeded"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        RiscvTopologySystemError::DuplicateGpu { device: gpu_id }
    );

    let mut system = RiscvTopologySystem::with_min_remote_delay(
        topology_with_gpu(),
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap();
    let error = system
        .submit_gpu_kernel_parallel(
            GpuDeviceId::new(404),
            GpuKernelLaunch::new(GpuKernelId::new(91), 1, 3).unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        error,
        RiscvTopologySystemError::UnknownGpu {
            device: GpuDeviceId::new(404),
        },
    );
    assert_eq!(system.scheduler().now(), 0);
}

#[test]
fn topology_system_maps_gpu_topology_errors() {
    let error = match RiscvTopologySystem::with_min_remote_delay(
        topology_with_gpu(),
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap()
    .with_gpu(GpuTopologyConfig::new(
        GpuComputeConfig::new(GpuDeviceId::new(32), PartitionId::new(0), 1, 1).unwrap(),
        endpoint("cpu0", "gpu"),
        endpoint("gpu0", "control"),
    )) {
        Ok(_) => panic!("GPU topology attach unexpectedly succeeded"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        RiscvTopologySystemError::Gpu(GpuError::CommandTargetPartitionMismatch {
            endpoint: endpoint("gpu0", "control"),
            expected: PartitionId::new(0),
            actual: PartitionId::new(1),
        }),
    );
}

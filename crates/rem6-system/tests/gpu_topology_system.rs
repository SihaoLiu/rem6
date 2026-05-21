use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_gpu::{
    GpuComputeConfig, GpuDeviceId, GpuError, GpuKernelId, GpuKernelLaunch, GpuTopologyConfig,
    GpuTraceEvent, GpuTraceKind, GpuWorkgroupCompletion, GpuWorkgroupId,
};
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_system::{RiscvTopologySystem, RiscvTopologySystemError};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, PortDirection, PortName, Topology,
    TopologyBuilder,
};

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

use rem6_gpu::{
    GpuComputeConfig, GpuDeviceId, GpuError, GpuKernelId, GpuKernelLaunch, GpuTopologyConfig,
    GpuTopologyDevice, GpuTraceEvent, GpuTraceKind, GpuWorkgroupCompletion, GpuWorkgroupId,
};
use rem6_kernel::{ClockDomain, PartitionId, PartitionedScheduler};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, PortDirection, PortName, Topology,
    TopologyBuilder, TopologyError,
};
use rem6_transport::TopologyRouteError;

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

fn gpu_topology() -> Topology {
    TopologyBuilder::new(2)
        .add_component(
            ComponentSpec::new(
                component("cpu0"),
                kind("cpu"),
                PartitionId::new(0),
                clock(1),
            )
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
        .connect_with_latencies(endpoint("cpu0", "gpu"), endpoint("gpu0", "control"), 3, 1)
        .unwrap()
        .build()
        .unwrap()
}

fn gpu_config(device: GpuDeviceId) -> GpuTopologyConfig {
    GpuTopologyConfig::new(
        GpuComputeConfig::new(device, PartitionId::new(1), 2, 1).unwrap(),
        endpoint("cpu0", "gpu"),
        endpoint("gpu0", "control"),
    )
}

#[test]
fn gpu_topology_device_submits_kernel_through_declared_control_path() {
    let topology = gpu_topology();
    let device =
        GpuTopologyDevice::from_topology(&topology, gpu_config(GpuDeviceId::new(7))).unwrap();
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let launch = GpuKernelLaunch::new(GpuKernelId::new(20), 3, 4).unwrap();

    assert_eq!(device.command_path().source(), &endpoint("cpu0", "gpu"));
    assert_eq!(device.command_path().target(), &endpoint("gpu0", "control"));
    assert_eq!(
        device.command_path().source_partition(),
        PartitionId::new(0)
    );
    assert_eq!(
        device.command_path().target_partition(),
        PartitionId::new(1)
    );
    assert_eq!(device.command_path().latency(), 3);

    device
        .submit_kernel(&mut scheduler, launch.clone())
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        device.gpu().completions(),
        vec![
            GpuWorkgroupCompletion::new(GpuKernelId::new(20), GpuWorkgroupId::new(0), 0, 0, 3, 7,),
            GpuWorkgroupCompletion::new(GpuKernelId::new(20), GpuWorkgroupId::new(1), 1, 0, 3, 7,),
            GpuWorkgroupCompletion::new(GpuKernelId::new(20), GpuWorkgroupId::new(2), 0, 0, 7, 11,),
        ],
    );
    assert_eq!(
        device.gpu().trace(),
        vec![
            GpuTraceEvent::new(
                0,
                GpuTraceKind::LaunchSubmitted {
                    kernel: GpuKernelId::new(20),
                    source: PartitionId::new(0),
                    target: PartitionId::new(1),
                },
            ),
            GpuTraceEvent::new(
                3,
                GpuTraceKind::LaunchAccepted {
                    kernel: GpuKernelId::new(20),
                    workgroups: 3,
                },
            ),
            GpuTraceEvent::new(
                3,
                GpuTraceKind::WorkgroupStarted {
                    kernel: GpuKernelId::new(20),
                    workgroup: GpuWorkgroupId::new(0),
                    compute_unit: 0,
                    slot: 0,
                    complete_at: 7,
                },
            ),
            GpuTraceEvent::new(
                3,
                GpuTraceKind::WorkgroupStarted {
                    kernel: GpuKernelId::new(20),
                    workgroup: GpuWorkgroupId::new(1),
                    compute_unit: 1,
                    slot: 0,
                    complete_at: 7,
                },
            ),
            GpuTraceEvent::new(
                7,
                GpuTraceKind::WorkgroupCompleted {
                    kernel: GpuKernelId::new(20),
                    workgroup: GpuWorkgroupId::new(0),
                    compute_unit: 0,
                    slot: 0,
                },
            ),
            GpuTraceEvent::new(
                7,
                GpuTraceKind::WorkgroupCompleted {
                    kernel: GpuKernelId::new(20),
                    workgroup: GpuWorkgroupId::new(1),
                    compute_unit: 1,
                    slot: 0,
                },
            ),
            GpuTraceEvent::new(
                7,
                GpuTraceKind::WorkgroupStarted {
                    kernel: GpuKernelId::new(20),
                    workgroup: GpuWorkgroupId::new(2),
                    compute_unit: 0,
                    slot: 0,
                    complete_at: 11,
                },
            ),
            GpuTraceEvent::new(
                11,
                GpuTraceKind::WorkgroupCompleted {
                    kernel: GpuKernelId::new(20),
                    workgroup: GpuWorkgroupId::new(2),
                    compute_unit: 0,
                    slot: 0,
                },
            ),
        ],
    );
}

#[test]
fn gpu_topology_rejects_bad_control_path_without_device_mutation() {
    let topology = gpu_topology();

    let error = GpuTopologyDevice::from_topology(
        &topology,
        GpuTopologyConfig::new(
            GpuComputeConfig::new(GpuDeviceId::new(8), PartitionId::new(1), 1, 1).unwrap(),
            endpoint("missing_cpu", "gpu"),
            endpoint("gpu0", "control"),
        ),
    )
    .unwrap_err();
    assert_eq!(
        error,
        GpuError::Topology(TopologyError::UnknownComponent {
            component: component("missing_cpu"),
        }),
    );

    let error = GpuTopologyDevice::from_topology(
        &topology,
        GpuTopologyConfig::new(
            GpuComputeConfig::new(GpuDeviceId::new(9), PartitionId::new(0), 1, 1).unwrap(),
            endpoint("cpu0", "gpu"),
            endpoint("gpu0", "control"),
        ),
    )
    .unwrap_err();
    assert_eq!(
        error,
        GpuError::CommandTargetPartitionMismatch {
            endpoint: endpoint("gpu0", "control"),
            expected: PartitionId::new(0),
            actual: PartitionId::new(1),
        },
    );

    let no_link_topology = TopologyBuilder::new(2)
        .add_component(
            ComponentSpec::new(
                component("cpu0"),
                kind("cpu"),
                PartitionId::new(0),
                clock(1),
            )
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
        .build()
        .unwrap();
    let error =
        GpuTopologyDevice::from_topology(&no_link_topology, gpu_config(GpuDeviceId::new(10)))
            .unwrap_err();
    assert_eq!(
        error,
        GpuError::TopologyRoute(TopologyRouteError::MissingTopologyConnection {
            from: endpoint("cpu0", "gpu"),
            to: endpoint("gpu0", "control"),
        }),
    );
}

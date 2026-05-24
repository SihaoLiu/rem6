use rem6_memory::{AccessSize, Address};
use rem6_workload::{
    WorkloadAcceleratorCommand, WorkloadAcceleratorCommandKind, WorkloadAcceleratorDevice,
    WorkloadError, WorkloadGpuDevice, WorkloadGpuKernelLaunch, WorkloadHostPlacement,
    WorkloadMemoryRoute, WorkloadMemoryTarget, WorkloadRiscvCore, WorkloadRouteId,
    WorkloadTopology,
};

fn route_id(value: &str) -> WorkloadRouteId {
    WorkloadRouteId::new(value).unwrap()
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
fn workload_topology_rejects_invalid_gpu_declarations() {
    let missing_route = riscv_topology()
        .add_gpu_device(
            WorkloadGpuDevice::new(12, 3, 2, 1, "gpu0.control", route_id("gpu0.command")).unwrap(),
        )
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
        .add_gpu_device(
            WorkloadGpuDevice::new(12, 3, 2, 1, "gpu0.control", route_id("gpu0.command")).unwrap(),
        )
        .unwrap();
    let duplicate = topology
        .clone()
        .add_gpu_device(
            WorkloadGpuDevice::new(12, 3, 2, 1, "gpu0.control", route_id("gpu0.command")).unwrap(),
        )
        .unwrap_err();
    assert_eq!(duplicate, WorkloadError::DuplicateGpuDevice { device: 12 });

    let endpoint_mismatch = riscv_topology()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("gpu0.command"),
                "cpu0.gpu",
                0,
                "gpu0.other-control",
                3,
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .add_gpu_device(
            WorkloadGpuDevice::new(12, 3, 2, 1, "gpu0.control", route_id("gpu0.command")).unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        endpoint_mismatch,
        WorkloadError::GpuCommandRouteEndpointMismatch {
            device: 12,
            route: route_id("gpu0.command"),
            expected: "gpu0.control".to_string(),
            actual: "gpu0.other-control".to_string(),
        }
    );

    let missing_device = topology
        .add_gpu_kernel_launch(WorkloadGpuKernelLaunch::new(99, 90, 3, 5).unwrap())
        .unwrap_err();
    assert_eq!(
        missing_device,
        WorkloadError::MissingGpuDevice { device: 99 }
    );
}

#[test]
fn workload_topology_rejects_invalid_accelerator_declarations() {
    let missing_route = riscv_topology()
        .add_accelerator_device(
            WorkloadAcceleratorDevice::new(
                22,
                3,
                2,
                "accelerator0.control",
                route_id("accelerator0.command"),
            )
            .unwrap(),
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
            WorkloadAcceleratorDevice::new(
                22,
                3,
                2,
                "accelerator0.control",
                route_id("accelerator0.command"),
            )
            .unwrap(),
        )
        .unwrap();
    let duplicate = topology
        .clone()
        .add_accelerator_device(
            WorkloadAcceleratorDevice::new(
                22,
                3,
                2,
                "accelerator0.control",
                route_id("accelerator0.command"),
            )
            .unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateAcceleratorDevice { engine: 22 }
    );

    let endpoint_mismatch = riscv_topology()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("accelerator0.command"),
                "cpu0.accelerator",
                0,
                "accelerator0.other-control",
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
                route_id("accelerator0.command"),
            )
            .unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        endpoint_mismatch,
        WorkloadError::AcceleratorCommandRouteEndpointMismatch {
            engine: 22,
            route: route_id("accelerator0.command"),
            expected: "accelerator0.control".to_string(),
            actual: "accelerator0.other-control".to_string(),
        }
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

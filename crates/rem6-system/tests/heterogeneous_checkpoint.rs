use rem6_accelerator::{
    AcceleratorCommand, AcceleratorCommandId, AcceleratorCommandKind, AcceleratorEngine,
    AcceleratorEngineConfig, AcceleratorEngineId, AcceleratorTopologyConfig,
};
use rem6_checkpoint::CheckpointComponentId;
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_gpu::{
    GpuComputeConfig, GpuDevice, GpuDeviceId, GpuKernelId, GpuKernelLaunch, GpuTopologyConfig,
};
use rem6_kernel::{ClockDomain, PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_stats::StatsRegistry;
use rem6_system::{
    AcceleratorCheckpointBank, AcceleratorCheckpointPort, GpuCheckpointBank, GpuCheckpointPort,
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, RiscvTopologyHostConfig,
    RiscvTopologySystem, SystemActionExecutor, SystemActionOutcome,
};
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

fn heterogeneous_topology() -> Topology {
    TopologyBuilder::new(4)
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
            .add_port(port("accel"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("gpu"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("accelerator0"),
                kind("accelerator"),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(port("dma"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("control"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("gpu0"),
                kind("gpu"),
                PartitionId::new(2),
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
                PartitionId::new(3),
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
        .connect_with_latencies(
            endpoint("cpu0", "accel"),
            endpoint("accelerator0", "control"),
            4,
            1,
        )
        .unwrap()
        .connect_with_latencies(endpoint("cpu0", "gpu"), endpoint("gpu0", "control"), 4, 1)
        .unwrap()
        .connect_with_latencies(
            endpoint("accelerator0", "dma"),
            endpoint("mem0", "requests"),
            3,
            5,
        )
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

fn accelerator_config(engine: AcceleratorEngineId) -> AcceleratorTopologyConfig {
    AcceleratorTopologyConfig::new(
        AcceleratorEngineConfig::new(engine, PartitionId::new(1), 1).unwrap(),
        endpoint("accelerator0", "dma"),
        endpoint("mem0", "requests"),
    )
    .with_command_submission(
        endpoint("cpu0", "accel"),
        endpoint("accelerator0", "control"),
    )
}

fn gpu_config(device: GpuDeviceId) -> GpuTopologyConfig {
    GpuTopologyConfig::new(
        GpuComputeConfig::new(device, PartitionId::new(2), 2, 1).unwrap(),
        endpoint("cpu0", "gpu"),
        endpoint("gpu0", "control"),
    )
    .with_memory(endpoint("gpu0", "dma"), endpoint("mem0", "requests"))
}

fn checkpoint_record(source: GuestSourceId) -> HostActionRecord {
    HostActionRecord::new(
        24,
        PartitionId::new(0),
        PartitionId::new(0),
        GuestEventId::new(300),
        source,
        HostAction::Checkpoint {
            label: "heterogeneous-devices".to_string(),
        },
    )
}

fn restore_record(source: GuestSourceId, outcome: &SystemActionOutcome) -> HostActionRecord {
    let SystemActionOutcome::Checkpoint { manifest, .. } = outcome else {
        panic!("checkpoint outcome expected");
    };
    HostActionRecord::new(
        40,
        PartitionId::new(0),
        PartitionId::new(0),
        GuestEventId::new(301),
        source,
        HostAction::RestoreCheckpoint {
            manifest: manifest.clone(),
        },
    )
}

#[test]
fn host_checkpoint_captures_and_restores_heterogeneous_devices() {
    let cpu_partition = PartitionId::new(0);
    let accelerator_partition = PartitionId::new(1);
    let gpu_partition = PartitionId::new(2);
    let accelerator = AcceleratorEngine::new(
        AcceleratorEngineConfig::new(AcceleratorEngineId::new(7), accelerator_partition, 1)
            .unwrap(),
    );
    let gpu =
        GpuDevice::new(GpuComputeConfig::new(GpuDeviceId::new(8), gpu_partition, 2, 1).unwrap());
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    accelerator
        .submit_from_partition(
            &mut scheduler,
            cpu_partition,
            2,
            AcceleratorCommand::new(
                AcceleratorCommandId::new(50),
                AcceleratorCommandKind::NpuInference { tiles: 5 },
                6,
            )
            .unwrap(),
        )
        .unwrap();
    gpu.submit_kernel_from_partition(
        &mut scheduler,
        cpu_partition,
        2,
        GpuKernelLaunch::new(GpuKernelId::new(60), 3, 4).unwrap(),
    )
    .unwrap();
    scheduler.run_until_idle_parallel().unwrap();
    let accelerator_snapshot = accelerator.snapshot();
    let gpu_snapshot = gpu.snapshot();

    let accelerator_component = CheckpointComponentId::new("accelerator7").unwrap();
    let gpu_component = CheckpointComponentId::new("gpu8").unwrap();
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());
    executor
        .attach_accelerator_checkpoint_bank(
            AcceleratorCheckpointBank::new([AcceleratorCheckpointPort::new(
                accelerator_component.clone(),
                accelerator.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
    executor
        .attach_gpu_checkpoint_bank(
            GpuCheckpointBank::new([GpuCheckpointPort::new(gpu_component.clone(), gpu.clone())])
                .unwrap(),
        )
        .unwrap();
    let source = GuestSourceId::new(70);
    let checkpoint = executor.apply(&checkpoint_record(source)).unwrap();

    let SystemActionOutcome::Checkpoint { manifest, .. } = &checkpoint else {
        panic!("checkpoint outcome expected");
    };
    assert_eq!(
        manifest
            .states()
            .iter()
            .map(|state| state.component().clone())
            .collect::<Vec<_>>(),
        vec![accelerator_component.clone(), gpu_component.clone()],
    );
    assert!(
        executor
            .checkpoints()
            .chunk(&accelerator_component, "accelerator")
            .unwrap()
            .len()
            > 64
    );
    assert!(
        executor
            .checkpoints()
            .chunk(&gpu_component, "gpu")
            .unwrap()
            .len()
            > 96
    );

    let mut mutation_scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    accelerator
        .submit_from_partition(
            &mut mutation_scheduler,
            cpu_partition,
            2,
            AcceleratorCommand::new(
                AcceleratorCommandId::new(51),
                AcceleratorCommandKind::GpuKernel { workgroups: 1 },
                3,
            )
            .unwrap(),
        )
        .unwrap();
    gpu.submit_kernel_from_partition(
        &mut mutation_scheduler,
        cpu_partition,
        2,
        GpuKernelLaunch::new(GpuKernelId::new(61), 1, 3).unwrap(),
    )
    .unwrap();
    mutation_scheduler.run_until_idle_parallel().unwrap();
    assert_ne!(accelerator.snapshot(), accelerator_snapshot);
    assert_ne!(gpu.snapshot(), gpu_snapshot);

    let restore = restore_record(source, &checkpoint);
    let restored = executor.apply(&restore).unwrap();

    assert_eq!(
        restored,
        SystemActionOutcome::CheckpointRestored {
            tick: 40,
            event: GuestEventId::new(301),
            source,
            manifest: manifest.clone(),
        },
    );
    assert_eq!(accelerator.snapshot(), accelerator_snapshot);
    assert_eq!(gpu.snapshot(), gpu_snapshot);
}

#[test]
fn topology_host_controller_checkpoints_attached_heterogeneous_devices() {
    let accelerator_id = AcceleratorEngineId::new(9);
    let gpu_id = GpuDeviceId::new(10);
    let source = GuestSourceId::new(71);
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        heterogeneous_topology(),
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap()
    .with_accelerator(accelerator_config(accelerator_id))
    .unwrap()
    .with_gpu(gpu_config(gpu_id))
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(3), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    let host = system.host_controller().unwrap();
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .accelerator_checkpoint_bank()
        .is_some());
    assert!(host
        .lock()
        .unwrap()
        .executor()
        .gpu_checkpoint_bank()
        .is_some());

    let accelerator = system.accelerator(accelerator_id).unwrap().clone();
    let gpu = system.gpu(gpu_id).unwrap().clone();
    accelerator
        .submit_command(
            system.scheduler_mut(),
            AcceleratorCommand::new(
                AcceleratorCommandId::new(52),
                AcceleratorCommandKind::NpuInference { tiles: 2 },
                3,
            )
            .unwrap(),
        )
        .unwrap();
    system.scheduler_mut().run_until_idle_parallel().unwrap();
    let accelerator_snapshot = system
        .accelerator(accelerator_id)
        .unwrap()
        .engine()
        .snapshot();
    let gpu_snapshot = system.gpu(gpu_id).unwrap().gpu().snapshot();

    let checkpoint = host
        .lock()
        .unwrap()
        .executor_mut()
        .apply(&checkpoint_record(source))
        .unwrap();
    let SystemActionOutcome::Checkpoint { manifest, .. } = &checkpoint else {
        panic!("checkpoint outcome expected");
    };

    assert_eq!(
        manifest
            .states()
            .iter()
            .map(|state| state.component().clone())
            .collect::<Vec<_>>(),
        vec![
            CheckpointComponentId::new("accelerator9").unwrap(),
            CheckpointComponentId::new("cpu0").unwrap(),
            CheckpointComponentId::new("gpu10").unwrap(),
        ],
    );

    accelerator
        .submit_command(
            system.scheduler_mut(),
            AcceleratorCommand::new(
                AcceleratorCommandId::new(53),
                AcceleratorCommandKind::GpuKernel { workgroups: 1 },
                2,
            )
            .unwrap(),
        )
        .unwrap();
    gpu.submit_kernel(
        system.scheduler_mut(),
        GpuKernelLaunch::new(GpuKernelId::new(62), 1, 3).unwrap(),
    )
    .unwrap();
    system.scheduler_mut().run_until_idle_parallel().unwrap();
    assert_ne!(
        system
            .accelerator(accelerator_id)
            .unwrap()
            .engine()
            .snapshot(),
        accelerator_snapshot
    );
    assert_ne!(system.gpu(gpu_id).unwrap().gpu().snapshot(), gpu_snapshot);

    let restore = restore_record(source, &checkpoint);
    host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(
        system
            .accelerator(accelerator_id)
            .unwrap()
            .engine()
            .snapshot(),
        accelerator_snapshot
    );
    assert_eq!(system.gpu(gpu_id).unwrap().gpu().snapshot(), gpu_snapshot);
}

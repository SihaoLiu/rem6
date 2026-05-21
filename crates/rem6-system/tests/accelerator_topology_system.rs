use rem6_accelerator::{
    AcceleratorCommand, AcceleratorCommandId, AcceleratorCommandKind, AcceleratorCompletion,
    AcceleratorDmaCompletion, AcceleratorDmaCopy, AcceleratorEngineConfig, AcceleratorEngineId,
    AcceleratorError, AcceleratorTopologyConfig, AcceleratorTraceEvent, AcceleratorTraceKind,
};
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
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

fn memory_request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(44), sequence)
}

fn topology_with_accelerator() -> Topology {
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
            .add_port(port("accel"), PortDirection::Initiator)
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
        .connect_with_latencies(
            endpoint("cpu0", "accel"),
            endpoint("accelerator0", "control"),
            4,
            1,
        )
        .unwrap()
        .connect_with_latencies(
            endpoint("accelerator0", "dma"),
            endpoint("mem0", "requests"),
            3,
            5,
        )
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
    source_line[4..8].copy_from_slice(&[0x11, 0x22, 0x33, 0x44]);
    store
        .insert_line(target, Address::new(0x1000), source_line)
        .unwrap();
    store
        .insert_line(target, Address::new(0x3000), vec![0; 16])
        .unwrap();
    store
}

fn accelerator_config(engine: AcceleratorEngineId) -> AcceleratorTopologyConfig {
    AcceleratorTopologyConfig::new(
        AcceleratorEngineConfig::new(engine, PartitionId::new(1), 2).unwrap(),
        endpoint("accelerator0", "dma"),
        endpoint("mem0", "requests"),
    )
}

fn accelerator_config_with_command(engine: AcceleratorEngineId) -> AcceleratorTopologyConfig {
    accelerator_config(engine).with_command_submission(
        endpoint("cpu0", "accel"),
        endpoint("accelerator0", "control"),
    )
}

fn dma_copy(route: rem6_transport::MemoryRouteId, command: u64) -> AcceleratorDmaCopy {
    AcceleratorDmaCopy::new(
        AcceleratorCommandId::new(command),
        route,
        MemoryRequest::read_shared(
            memory_request(command * 2),
            Address::new(0x1004),
            AccessSize::new(4).unwrap(),
            layout(),
        )
        .unwrap(),
        route,
        memory_request(command * 2 + 1),
        Address::new(0x3008),
    )
    .unwrap()
}

#[test]
fn topology_system_runs_accelerator_dma_copy_on_parallel_memory_backend() {
    let accelerator_id = AcceleratorEngineId::new(40);
    let topology = topology_with_accelerator();
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        topology,
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap()
    .with_memory_store(memory_store())
    .unwrap()
    .with_accelerator(accelerator_config(accelerator_id))
    .unwrap();
    let route = system.accelerator(accelerator_id).unwrap().dma_route();
    let trace = MemoryTrace::new();
    let read_request = MemoryRequest::read_shared(
        memory_request(1),
        Address::new(0x1004),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap();
    let copy = AcceleratorDmaCopy::new(
        AcceleratorCommandId::new(90),
        route,
        read_request.clone(),
        route,
        memory_request(2),
        Address::new(0x3008),
    )
    .unwrap();

    system
        .run_accelerator_dma_copy_parallel(accelerator_id, copy, trace.clone())
        .unwrap();

    let destination = system
        .memory_store()
        .unwrap()
        .lock()
        .unwrap()
        .line_data(MemoryTargetId::new(0), Address::new(0x3000))
        .unwrap();
    assert_eq!(&destination[8..12], &[0x11, 0x22, 0x33, 0x44]);
    assert_eq!(
        system
            .accelerator(accelerator_id)
            .unwrap()
            .engine()
            .dma_completions(),
        vec![AcceleratorDmaCompletion::new(
            AcceleratorCommandId::new(90),
            memory_request(1),
            memory_request(2),
            8,
            16,
        )],
    );
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                route,
                transport_endpoint("accelerator0", "dma"),
                MemoryTraceKind::RequestSent,
                memory_request(1),
            ),
            MemoryTraceEvent::request(
                3,
                route,
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                memory_request(1),
            ),
            MemoryTraceEvent::response(
                8,
                route,
                transport_endpoint("accelerator0", "dma"),
                memory_request(1),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::request(
                8,
                route,
                transport_endpoint("accelerator0", "dma"),
                MemoryTraceKind::RequestSent,
                memory_request(2),
            ),
            MemoryTraceEvent::request(
                11,
                route,
                transport_endpoint("mem0", "requests"),
                MemoryTraceKind::RequestArrived,
                memory_request(2),
            ),
            MemoryTraceEvent::response(
                16,
                route,
                transport_endpoint("accelerator0", "dma"),
                memory_request(2),
                ResponseStatus::Completed,
            ),
        ],
    );
}

#[test]
fn topology_system_rejects_duplicate_accelerator_without_route_mutation() {
    let accelerator_id = AcceleratorEngineId::new(41);
    let config = accelerator_config(accelerator_id);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology_with_accelerator(),
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap()
    .with_accelerator(config.clone())
    .unwrap();
    let route_count = system.transport().route_count();

    let error = match system.with_accelerator(config) {
        Ok(_) => panic!("duplicate accelerator attach unexpectedly succeeded"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        RiscvTopologySystemError::DuplicateAccelerator {
            engine: accelerator_id,
        },
    );
    assert_eq!(route_count, 3);
}

#[test]
fn topology_system_exposes_attached_accelerators_by_engine_id() {
    let accelerator_id = AcceleratorEngineId::new(43);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology_with_accelerator(),
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap()
    .with_accelerator(accelerator_config(accelerator_id))
    .unwrap();
    let route = system.accelerator(accelerator_id).unwrap().dma_route();

    let attached = system
        .accelerators()
        .map(|(engine, device)| (engine, device.engine().partition(), device.dma_route()))
        .collect::<Vec<_>>();

    assert_eq!(attached, vec![(accelerator_id, PartitionId::new(1), route)],);
    assert_eq!(system.transport().route_count(), 3);
    assert!(system.accelerator(AcceleratorEngineId::new(404)).is_none());
}

#[test]
fn topology_system_submits_accelerator_commands_through_declared_control_path() {
    let accelerator_id = AcceleratorEngineId::new(44);
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        topology_with_accelerator(),
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap()
    .with_accelerator(accelerator_config_with_command(accelerator_id))
    .unwrap();
    let gpu = AcceleratorCommand::new(
        AcceleratorCommandId::new(200),
        AcceleratorCommandKind::GpuKernel { workgroups: 8 },
        6,
    )
    .unwrap();
    let npu = AcceleratorCommand::new(
        AcceleratorCommandId::new(201),
        AcceleratorCommandKind::NpuInference { tiles: 5 },
        3,
    )
    .unwrap();

    assert_eq!(
        system
            .accelerator(accelerator_id)
            .unwrap()
            .command_path()
            .unwrap()
            .latency(),
        4,
    );
    system
        .submit_accelerator_command_parallel(accelerator_id, gpu.clone())
        .unwrap();
    system
        .submit_accelerator_command_parallel(accelerator_id, npu.clone())
        .unwrap();
    system.scheduler_mut().run_until_idle_parallel().unwrap();

    let engine = system.accelerator(accelerator_id).unwrap().engine();
    assert_eq!(
        engine.completed(),
        vec![
            AcceleratorCompletion::new(AcceleratorCommandId::new(201), npu.kind().clone(), 1, 4, 7,),
            AcceleratorCompletion::new(
                AcceleratorCommandId::new(200),
                gpu.kind().clone(),
                0,
                4,
                10,
            ),
        ],
    );
    assert_eq!(
        engine.trace(),
        vec![
            AcceleratorTraceEvent::new(
                0,
                AcceleratorTraceKind::Submitted {
                    command: AcceleratorCommandId::new(200),
                    source: PartitionId::new(0),
                    target: PartitionId::new(1),
                },
            ),
            AcceleratorTraceEvent::new(
                0,
                AcceleratorTraceKind::Submitted {
                    command: AcceleratorCommandId::new(201),
                    source: PartitionId::new(0),
                    target: PartitionId::new(1),
                },
            ),
            AcceleratorTraceEvent::new(
                4,
                AcceleratorTraceKind::Started {
                    command: AcceleratorCommandId::new(200),
                    lane: 0,
                    complete_at: 10,
                },
            ),
            AcceleratorTraceEvent::new(
                4,
                AcceleratorTraceKind::Started {
                    command: AcceleratorCommandId::new(201),
                    lane: 1,
                    complete_at: 7,
                },
            ),
            AcceleratorTraceEvent::new(
                7,
                AcceleratorTraceKind::Completed {
                    command: AcceleratorCommandId::new(201),
                    lane: 1,
                },
            ),
            AcceleratorTraceEvent::new(
                10,
                AcceleratorTraceKind::Completed {
                    command: AcceleratorCommandId::new(200),
                    lane: 0,
                },
            ),
        ],
    );
}

#[test]
fn topology_system_rejects_accelerator_command_without_control_path() {
    let accelerator_id = AcceleratorEngineId::new(45);
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        topology_with_accelerator(),
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap()
    .with_accelerator(accelerator_config(accelerator_id))
    .unwrap();
    let command = AcceleratorCommand::new(
        AcceleratorCommandId::new(202),
        AcceleratorCommandKind::GpuKernel { workgroups: 1 },
        2,
    )
    .unwrap();

    let error = system
        .submit_accelerator_command_parallel(accelerator_id, command)
        .unwrap_err();

    assert_eq!(
        error,
        RiscvTopologySystemError::Accelerator(AcceleratorError::MissingCommandSubmission {
            engine: accelerator_id,
        }),
    );
    assert_eq!(system.scheduler().now(), 0);
}

#[test]
fn topology_system_requires_memory_before_running_accelerator_dma() {
    let accelerator_id = AcceleratorEngineId::new(42);
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        topology_with_accelerator(),
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap()
    .with_accelerator(accelerator_config(accelerator_id))
    .unwrap();
    let route = system.accelerator(accelerator_id).unwrap().dma_route();

    let error = system
        .run_accelerator_dma_copy_parallel(accelerator_id, dma_copy(route, 92), MemoryTrace::new())
        .unwrap_err();

    assert_eq!(error, RiscvTopologySystemError::MissingMemoryStore);
    assert_eq!(system.scheduler().now(), 0);
}

#[test]
fn topology_system_rejects_unknown_accelerator_dma_without_running_scheduler() {
    let mut system = RiscvTopologySystem::with_min_remote_delay(
        topology_with_accelerator(),
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap()
    .with_memory_store(memory_store())
    .unwrap();
    let copy = dma_copy(rem6_transport::MemoryRouteId::new(0), 91);

    let error = system
        .run_accelerator_dma_copy_parallel(AcceleratorEngineId::new(99), copy, MemoryTrace::new())
        .unwrap_err();

    assert_eq!(
        error,
        RiscvTopologySystemError::UnknownAccelerator {
            engine: AcceleratorEngineId::new(99),
        },
    );
    assert_eq!(system.scheduler().now(), 0);
}

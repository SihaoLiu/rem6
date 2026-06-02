use std::sync::{Arc, Mutex};

use rem6_checkpoint::{
    CheckpointChunk, CheckpointComponentId, CheckpointManifest, CheckpointState,
};
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_interrupt::{InterruptController, InterruptLineId, InterruptSourceId, InterruptTargetId};
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{AccessSize, Address, AgentId, ByteMask, CacheLineLayout};
use rem6_mmio::{MmioCompletion, MmioRequest, MmioRequestId, MmioResponse, MmioRoute};
use rem6_net::{
    SinicFifoDevice, SinicInterrupts, SinicPciEndpointSpec, SinicRegisterBlock,
    SinicRegisterOffset, SinicRegisterParams,
};
use rem6_pci::{
    PciConfigAperture, PciFunctionAddress, PciHostAddressBases, PciHostBridge,
    PciLegacyInterruptMapper, PciLegacyInterruptPolicy, PciLegacyInterruptRouter,
    PciLegacyInterruptRoutingTable,
};
use rem6_platform::PlatformBuilder;
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, RiscvTopologyHostConfig,
    RiscvTopologySinicPciDeviceConfig, RiscvTopologySystem, RiscvTopologySystemError,
    SinicFifoCheckpointPort, SystemActionOutcome,
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

fn topology() -> Topology {
    TopologyBuilder::new(2)
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
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mem0"),
                kind("memory"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("pci0"),
                kind("pci-host"),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(port("bar"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(
            endpoint("cpu0", "ifetch"),
            endpoint("mem0", "requests"),
            1,
            1,
        )
        .unwrap()
        .connect_with_latencies(endpoint("cpu0", "dmem"), endpoint("mem0", "requests"), 1, 1)
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

fn base_system() -> RiscvTopologySystem {
    RiscvTopologySystem::with_min_remote_delay(
        topology(),
        RiscvClusterTopologyConfig::new([core_config()]),
        1,
    )
    .unwrap()
}

fn host_config(source: GuestSourceId) -> RiscvTopologyHostConfig {
    RiscvTopologyHostConfig::new(PartitionId::new(1), 1, source)
}

fn pci_function() -> PciFunctionAddress {
    PciFunctionAddress::new(0, 9, 0).unwrap()
}

fn pci_host() -> Arc<Mutex<PciHostBridge>> {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 1).unwrap();
    let bases = PciHostAddressBases::new(
        Address::new(0x1000_0000),
        Address::new(0x8000_0000),
        Address::new(0x9000_0000),
    );
    Arc::new(Mutex::new(PciHostBridge::with_address_bases(
        aperture, bases,
    )))
}

fn legacy_interrupt_router() -> Arc<Mutex<PciLegacyInterruptRouter>> {
    let mapper = PciLegacyInterruptMapper::new(
        InterruptLineId::new(40),
        4,
        PciLegacyInterruptPolicy::DeviceModulo,
    )
    .unwrap();
    Arc::new(Mutex::new(
        PciLegacyInterruptRouter::new(
            PciLegacyInterruptRoutingTable::new(mapper),
            InterruptTargetId::new(3),
            PartitionId::new(0),
            2,
            Arc::new(Mutex::new(InterruptController::new())),
        )
        .unwrap(),
    ))
}

fn read_request(id: u64, address: u64, bytes: u64) -> MmioRequest {
    MmioRequest::read(
        MmioRequestId::new(id),
        Address::new(address),
        AccessSize::new(bytes).unwrap(),
    )
    .unwrap()
}

fn write_request(id: u64, address: u64, data: Vec<u8>) -> MmioRequest {
    MmioRequest::write(
        MmioRequestId::new(id),
        Address::new(address),
        data.clone(),
        ByteMask::full(AccessSize::new(data.len() as u64).unwrap()).unwrap(),
    )
    .unwrap()
}

fn response_for(
    completions: &[MmioCompletion],
    request: MmioRequestId,
) -> &Result<MmioResponse, rem6_mmio::MmioError> {
    completions
        .iter()
        .find(|completion| match completion.response() {
            Ok(response) => response.request() == request,
            Err(_) => false,
        })
        .unwrap()
        .response()
}

fn corrupt_register_config_chunk(
    manifest: CheckpointManifest,
    component: &CheckpointComponentId,
) -> CheckpointManifest {
    let states = manifest
        .states()
        .iter()
        .map(|state| {
            let chunks = state
                .chunks()
                .iter()
                .map(|chunk| {
                    let mut payload = chunk.payload().to_vec();
                    if state.component() == component && chunk.name() == "sinic-register" {
                        payload[68..72].copy_from_slice(&0_u32.to_le_bytes());
                    }
                    CheckpointChunk::new(chunk.name().to_string(), payload)
                })
                .collect();
            CheckpointState::new(state.component().clone(), chunks)
        })
        .collect();
    CheckpointManifest::new(manifest.label().to_string(), manifest.tick(), states)
}

#[test]
fn topology_system_assembles_sinic_pci_device_into_platform_bus_and_checkpoints() {
    let function = pci_function();
    let pci_host = pci_host();
    let router = legacy_interrupt_router();
    let register_component = CheckpointComponentId::new("net.sinic0.registers").unwrap();
    let fifo_component = CheckpointComponentId::new("net.sinic0.fifo").unwrap();
    let pci_host_component = CheckpointComponentId::new("pci.host0").unwrap();
    let router_component = CheckpointComponentId::new("pci.intx-router0").unwrap();
    let bar_base = Address::new(0x8000_0000);
    let config_bits = SinicRegisterBlock::CONFIG_INT_EN | SinicRegisterBlock::CONFIG_RX_EN;
    let source = GuestSourceId::new(131);

    let platform = PlatformBuilder::new(2).build().unwrap();
    let device_config = RiscvTopologySinicPciDeviceConfig::new(
        SinicPciEndpointSpec::new(function),
        Arc::clone(&pci_host),
        Arc::clone(&router),
        bar_base,
        MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 1).unwrap(),
        InterruptSourceId::new(0x1293),
        SinicRegisterParams::default()
            .with_interrupt_mask(SinicInterrupts::SOFT)
            .with_hardware_address(0x00aa_bbcc_ddee),
    )
    .with_register_checkpoint_component(register_component.clone())
    .with_fifo_checkpoint_component(fifo_component.clone())
    .with_pci_host_checkpoint_component(pci_host_component.clone())
    .with_pci_legacy_interrupt_router_checkpoint_component(router_component.clone());

    let system = base_system()
        .with_platform(platform)
        .unwrap()
        .with_sinic_pci_device(device_config)
        .unwrap()
        .with_host_controller(host_config(source), StatsRegistry::new())
        .unwrap();

    assert_eq!(
        system.sinic_register_checkpoint_components(),
        vec![register_component.clone()]
    );
    assert_eq!(
        system.sinic_fifo_checkpoint_components(),
        vec![fifo_component.clone()]
    );
    assert_eq!(
        system.pci_host_checkpoint_components(),
        vec![pci_host_component.clone()]
    );
    assert_eq!(
        system.pci_legacy_interrupt_router_checkpoint_components(),
        vec![router_component.clone()]
    );
    assert!(pci_host.lock().unwrap().endpoint(function).is_some());
    assert_eq!(
        pci_host.lock().unwrap().active_host_bar_ranges().unwrap()[0]
            .host_range()
            .start(),
        bar_base
    );

    let completions = Arc::new(Mutex::new(Vec::new()));
    let bus = system.platform_bus().unwrap().clone();
    let completed = Arc::clone(&completions);
    let mut scheduler = system.scheduler_mut();
    scheduler
        .schedule_parallel_at(PartitionId::new(0), 5, move |context| {
            bus.submit_parallel(
                context,
                write_request(
                    1,
                    bar_base.get() + SinicRegisterOffset::CONFIG.addr() as u64,
                    config_bits.to_le_bytes().to_vec(),
                ),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();
    drop(scheduler);
    assert_eq!(
        response_for(&completions.lock().unwrap(), MmioRequestId::new(1)),
        &Ok(MmioResponse::completed(MmioRequestId::new(1), None))
    );

    let host = system.host_controller().unwrap();
    let checkpoint = HostActionRecord::new(
        20,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(420),
        source,
        HostAction::Checkpoint {
            label: "sinic-pci-full-system".to_string(),
        },
    );
    let manifest = match host
        .lock()
        .unwrap()
        .executor_mut()
        .apply(&checkpoint)
        .unwrap()
    {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };

    assert!(manifest.states().iter().any(|state| {
        state.component() == &register_component
            && state
                .chunks()
                .iter()
                .any(|chunk| chunk.name() == "sinic-register")
    }));
    assert!(manifest.states().iter().any(|state| {
        state.component() == &fifo_component
            && state
                .chunks()
                .iter()
                .any(|chunk| chunk.name() == "sinic-fifo")
    }));
    assert!(manifest.states().iter().any(|state| {
        state.component() == &pci_host_component
            && ["host-topology", "host-endpoint-config-space"]
                .into_iter()
                .all(|name| state.chunks().iter().any(|chunk| chunk.name() == name))
    }));
    assert!(manifest.states().iter().any(|state| {
        state.component() == &router_component
            && state
                .chunks()
                .iter()
                .any(|chunk| chunk.name() == "legacy-intx-router")
    }));

    let reset_bus = system.platform_bus().unwrap().clone();
    let mut scheduler = system.scheduler_mut();
    scheduler
        .schedule_parallel_at(PartitionId::new(0), 12, move |context| {
            reset_bus
                .submit_parallel(
                    context,
                    write_request(
                        2,
                        bar_base.get() + SinicRegisterOffset::CONFIG.addr() as u64,
                        0_u32.to_le_bytes().to_vec(),
                    ),
                    |_| {},
                )
                .unwrap();
        })
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();
    drop(scheduler);

    let restore = HostActionRecord::new(
        30,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(421),
        source,
        HostAction::RestoreCheckpoint { manifest },
    );
    host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    let read_completions = Arc::new(Mutex::new(Vec::new()));
    let read_bus = system.platform_bus().unwrap().clone();
    let read_completed = Arc::clone(&read_completions);
    let mut scheduler = system.scheduler_mut();
    scheduler
        .schedule_parallel_at(PartitionId::new(0), 20, move |context| {
            read_bus
                .submit_parallel(
                    context,
                    read_request(
                        3,
                        bar_base.get() + SinicRegisterOffset::CONFIG.addr() as u64,
                        4,
                    ),
                    move |completion| read_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        response_for(&read_completions.lock().unwrap(), MmioRequestId::new(3)),
        &Ok(MmioResponse::completed(
            MmioRequestId::new(3),
            Some(config_bits.to_le_bytes().to_vec()),
        ))
    );
}

#[test]
fn topology_system_rejects_misaligned_sinic_pci_bar_base_without_host_mutation() {
    let function = pci_function();
    let pci_host = pci_host();
    let router = legacy_interrupt_router();
    let bar_base = Address::new(0x8000_0001);
    let platform = PlatformBuilder::new(2).build().unwrap();
    let device_config = RiscvTopologySinicPciDeviceConfig::new(
        SinicPciEndpointSpec::new(function),
        Arc::clone(&pci_host),
        router,
        bar_base,
        MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 1).unwrap(),
        InterruptSourceId::new(0x1293),
        SinicRegisterParams::default(),
    );

    let error = match base_system()
        .with_platform(platform)
        .unwrap()
        .with_sinic_pci_device(device_config)
    {
        Ok(_) => panic!("misaligned SINIC PCI BAR base succeeded"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        RiscvTopologySystemError::SinicPciBarAddressMisaligned {
            address,
            alignment_bytes,
        } if address == bar_base && alignment_bytes == 0x1_0000
    ));
    assert!(pci_host.lock().unwrap().endpoint(function).is_none());
}

#[test]
fn topology_system_rejects_duplicate_sinic_checkpoint_before_host_mutation() {
    let function = pci_function();
    let pci_host = pci_host();
    let router = legacy_interrupt_router();
    let fifo_component = CheckpointComponentId::new("net.sinic0.fifo").unwrap();
    let platform = PlatformBuilder::new(2).build().unwrap();
    let existing_device = Arc::new(Mutex::new(
        SinicFifoDevice::new(SinicRegisterParams::default()).unwrap(),
    ));
    let device_config = RiscvTopologySinicPciDeviceConfig::new(
        SinicPciEndpointSpec::new(function),
        Arc::clone(&pci_host),
        router,
        Address::new(0x8000_0000),
        MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 1).unwrap(),
        InterruptSourceId::new(0x1293),
        SinicRegisterParams::default(),
    )
    .with_fifo_checkpoint_component(fifo_component.clone());

    let error = match base_system()
        .with_platform(platform)
        .unwrap()
        .with_sinic_fifo_checkpoint_port(SinicFifoCheckpointPort::new(
            fifo_component,
            existing_device,
        ))
        .unwrap()
        .with_sinic_pci_device(device_config)
    {
        Ok(_) => panic!("duplicate SINIC FIFO checkpoint component succeeded"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        RiscvTopologySystemError::DuplicateSinicPciCheckpointComponent { .. }
    ));
    assert!(pci_host.lock().unwrap().endpoint(function).is_none());
}

#[test]
fn topology_system_rejects_mismatched_sinic_register_and_fifo_checkpoint_chunks() {
    let function = pci_function();
    let pci_host = pci_host();
    let router = legacy_interrupt_router();
    let register_component = CheckpointComponentId::new("net.sinic0.registers").unwrap();
    let fifo_component = CheckpointComponentId::new("net.sinic0.fifo").unwrap();
    let bar_base = Address::new(0x8000_0000);
    let config_bits = SinicRegisterBlock::CONFIG_INT_EN | SinicRegisterBlock::CONFIG_RX_EN;
    let source = GuestSourceId::new(132);

    let platform = PlatformBuilder::new(2).build().unwrap();
    let device_config = RiscvTopologySinicPciDeviceConfig::new(
        SinicPciEndpointSpec::new(function),
        Arc::clone(&pci_host),
        router,
        bar_base,
        MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 1).unwrap(),
        InterruptSourceId::new(0x1293),
        SinicRegisterParams::default(),
    )
    .with_register_checkpoint_component(register_component.clone())
    .with_fifo_checkpoint_component(fifo_component);
    let system = base_system()
        .with_platform(platform)
        .unwrap()
        .with_sinic_pci_device(device_config)
        .unwrap()
        .with_host_controller(host_config(source), StatsRegistry::new())
        .unwrap();

    let bus = system.platform_bus().unwrap().clone();
    let mut scheduler = system.scheduler_mut();
    scheduler
        .schedule_parallel_at(PartitionId::new(0), 5, move |context| {
            bus.submit_parallel(
                context,
                write_request(
                    1,
                    bar_base.get() + SinicRegisterOffset::CONFIG.addr() as u64,
                    config_bits.to_le_bytes().to_vec(),
                ),
                |_| {},
            )
            .unwrap();
        })
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();
    drop(scheduler);

    let host = system.host_controller().unwrap();
    let checkpoint = HostActionRecord::new(
        20,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(422),
        source,
        HostAction::Checkpoint {
            label: "sinic-pci-conflict".to_string(),
        },
    );
    let manifest = match host
        .lock()
        .unwrap()
        .executor_mut()
        .apply(&checkpoint)
        .unwrap()
    {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let manifest = corrupt_register_config_chunk(manifest, &register_component);

    let restore = HostActionRecord::new(
        30,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(423),
        source,
        HostAction::RestoreCheckpoint { manifest },
    );
    assert!(host.lock().unwrap().executor_mut().apply(&restore).is_err());
}

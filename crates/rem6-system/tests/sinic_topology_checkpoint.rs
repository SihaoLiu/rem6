use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError};
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_net::{
    EthernetPacket, SinicDataDescriptor, SinicFifoDevice, SinicInterrupts, SinicRegisterBlock,
    SinicRegisterParams,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, RiscvTopologyHostConfig,
    RiscvTopologySystem, RiscvTopologySystemError, SinicFifoCheckpointPort,
    SinicRegisterCheckpointPort, SystemActionOutcome, SystemError,
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

fn packet(bytes: &[u8]) -> EthernetPacket {
    EthernetPacket::new(bytes.to_vec()).unwrap()
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

fn register_block() -> Arc<Mutex<SinicRegisterBlock>> {
    let params = SinicRegisterParams::default()
        .with_interrupt_mask(SinicInterrupts::SOFT | SinicInterrupts::RX_PACKET)
        .with_hardware_address(0x00aa_bbcc_ddee);
    let mut registers = SinicRegisterBlock::new(params).unwrap();
    registers
        .change_config(
            SinicRegisterBlock::CONFIG_RX_EN | SinicRegisterBlock::CONFIG_INT_EN,
            10,
        )
        .unwrap();
    registers
        .post_interrupt(SinicInterrupts::RX_PACKET, 12, 5)
        .unwrap();
    Arc::new(Mutex::new(registers))
}

fn clear_register_block(registers: &Arc<Mutex<SinicRegisterBlock>>) {
    let mut registers = registers.lock().unwrap();
    registers
        .clear_interrupts(SinicInterrupts::SOFT | SinicInterrupts::RX_PACKET)
        .unwrap();
    let params = registers.params();
    *registers = SinicRegisterBlock::new(params).unwrap();
}

fn fifo_device() -> Arc<Mutex<SinicFifoDevice>> {
    let params = SinicRegisterParams::default()
        .with_zero_copy(true)
        .with_fifo_limits(48, 48, 4, 4, 16, 16)
        .with_interrupt_mask(
            SinicInterrupts::RX_PACKET
                | SinicInterrupts::RX_DMA
                | SinicInterrupts::TX_DMA
                | SinicInterrupts::TX_FULL,
        );
    let mut device = SinicFifoDevice::new(params).unwrap();
    device
        .registers_mut()
        .change_config(
            SinicRegisterBlock::CONFIG_INT_EN
                | SinicRegisterBlock::CONFIG_RX_EN
                | SinicRegisterBlock::CONFIG_TX_EN
                | SinicRegisterBlock::CONFIG_ZERO_COPY,
            1,
        )
        .unwrap();
    device
        .receive_from_wire(packet(&[1, 2, 3, 4, 5, 6, 7, 8]), 2, 3)
        .unwrap();
    device
        .begin_rx_dma_copy(SinicDataDescriptor::new(0x1000, 4).unwrap())
        .unwrap()
        .expect("pending receive DMA copy");
    device.complete_rx_dma_copy(3, 4).unwrap();
    device
        .begin_rx_dma_copy(SinicDataDescriptor::new(0x2000, 4).unwrap())
        .unwrap()
        .expect("pending second receive DMA copy");
    device
        .begin_tx_dma_copy(SinicDataDescriptor::new(0x3000, 3).unwrap().with_more(true))
        .unwrap();
    device.complete_tx_dma_copy(&[9, 8, 7], 4, 5).unwrap();
    device
        .begin_tx_dma_copy(SinicDataDescriptor::new(0x4000, 2).unwrap())
        .unwrap();
    Arc::new(Mutex::new(device))
}

fn clear_fifo_device(device: &Arc<Mutex<SinicFifoDevice>>) {
    let params = device.lock().unwrap().registers().params();
    *device.lock().unwrap() = SinicFifoDevice::new(params).unwrap();
}

#[test]
fn topology_host_controller_attaches_existing_sinic_register_checkpoint_port() {
    let source = GuestSourceId::new(111);
    let component = CheckpointComponentId::new("net.sinic0.registers").unwrap();
    let registers = register_block();
    let expected = registers.lock().unwrap().snapshot();
    let system = base_system()
        .with_sinic_register_checkpoint_port(SinicRegisterCheckpointPort::new(
            component.clone(),
            Arc::clone(&registers),
        ))
        .unwrap()
        .with_host_controller(host_config(source), StatsRegistry::new())
        .unwrap();
    let host = system.host_controller().unwrap();

    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .sinic_register_checkpoint_bank()
            .unwrap()
            .components(),
        vec![component.clone()]
    );

    let checkpoint = HostActionRecord::new(
        19,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(400),
        source,
        HostAction::Checkpoint {
            label: "topology-sinic-register".to_string(),
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
        state.component() == &component
            && state
                .chunks()
                .iter()
                .any(|chunk| chunk.name() == "sinic-register")
    }));

    clear_register_block(&registers);

    let restore = HostActionRecord::new(
        29,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(401),
        source,
        HostAction::RestoreCheckpoint { manifest },
    );
    host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(registers.lock().unwrap().snapshot(), expected);
}

#[test]
fn topology_host_controller_attaches_existing_sinic_fifo_checkpoint_port() {
    let source = GuestSourceId::new(112);
    let component = CheckpointComponentId::new("net.sinic0.fifo").unwrap();
    let device = fifo_device();
    let expected = device.lock().unwrap().snapshot();
    let system = base_system()
        .with_sinic_fifo_checkpoint_port(SinicFifoCheckpointPort::new(
            component.clone(),
            Arc::clone(&device),
        ))
        .unwrap()
        .with_host_controller(host_config(source), StatsRegistry::new())
        .unwrap();
    let host = system.host_controller().unwrap();

    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .sinic_fifo_checkpoint_bank()
            .unwrap()
            .components(),
        vec![component.clone()]
    );

    let checkpoint = HostActionRecord::new(
        20,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(402),
        source,
        HostAction::Checkpoint {
            label: "topology-sinic-fifo".to_string(),
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
        state.component() == &component
            && state
                .chunks()
                .iter()
                .any(|chunk| chunk.name() == "sinic-fifo")
    }));

    clear_fifo_device(&device);

    let restore = HostActionRecord::new(
        30,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(403),
        source,
        HostAction::RestoreCheckpoint { manifest },
    );
    host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(device.lock().unwrap().snapshot(), expected);
}

#[test]
fn topology_host_controller_attaches_late_sinic_checkpoint_ports() {
    let source = GuestSourceId::new(113);
    let register_component = CheckpointComponentId::new("net.sinic1.registers").unwrap();
    let fifo_component = CheckpointComponentId::new("net.sinic1.fifo").unwrap();
    let registers = register_block();
    let device = fifo_device();
    let system = base_system()
        .with_host_controller(host_config(source), StatsRegistry::new())
        .unwrap()
        .with_sinic_register_checkpoint_port(SinicRegisterCheckpointPort::new(
            register_component.clone(),
            registers,
        ))
        .unwrap()
        .with_sinic_fifo_checkpoint_port(SinicFifoCheckpointPort::new(
            fifo_component.clone(),
            device,
        ))
        .unwrap();
    let host = system.host_controller().unwrap();

    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .sinic_register_checkpoint_bank()
            .unwrap()
            .components(),
        vec![register_component]
    );
    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .sinic_fifo_checkpoint_bank()
            .unwrap()
            .components(),
        vec![fifo_component]
    );
}

#[test]
fn topology_rejects_duplicate_sinic_register_checkpoint_component() {
    let component = CheckpointComponentId::new("net.sinic2.registers").unwrap();
    let first_registers = register_block();
    let second_registers = register_block();
    let error = match base_system()
        .with_sinic_register_checkpoint_port(SinicRegisterCheckpointPort::new(
            component.clone(),
            first_registers,
        ))
        .unwrap()
        .with_sinic_register_checkpoint_port(SinicRegisterCheckpointPort::new(
            component.clone(),
            second_registers,
        )) {
        Ok(_) => panic!("duplicate SINIC register checkpoint component succeeded"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        RiscvTopologySystemError::System(SystemError::Checkpoint(
            CheckpointError::DuplicateComponent { component: duplicate }
        )) if duplicate == component
    ));
}

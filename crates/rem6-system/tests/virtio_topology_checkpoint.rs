use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError};
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{AccessSize, Address, AgentId, ByteMask, CacheLineLayout};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostAction, HostActionRecord, RiscvTopologyHostConfig,
    RiscvTopologySystem, RiscvTopologySystemError, SystemActionOutcome, SystemError,
    VirtioPciCommonCheckpointPort, VirtioPciDeviceConfigCheckpointPort, VirtioPciIsrCheckpointPort,
    VirtioPciNotifyCheckpointPort, VirtioSplitQueueCheckpointPort,
};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, PortDirection, PortName, Topology,
    TopologyBuilder,
};
use rem6_virtio::{
    VirtioPciCommonConfigDevice, VirtioPciDeviceConfigDevice, VirtioPciDeviceConfigSpec,
    VirtioPciIsrDevice, VirtioPciNotifyDevice, VirtioQueueIndex, VirtioQueueNotifySpec,
    VirtioQueueSpec, VirtioSplitQueue, VIRTIO_PCI_DEVICE_STATUS_OFFSET,
    VIRTIO_PCI_QUEUE_SELECT_OFFSET, VIRTIO_PCI_QUEUE_SIZE_OFFSET, VIRTIO_STATUS_ACKNOWLEDGE,
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

fn common_device() -> VirtioPciCommonConfigDevice {
    VirtioPciCommonConfigDevice::new([(0, 0x0000_0005)], [VirtioQueueSpec::available(64, 0)])
        .unwrap()
}

fn notify_device() -> VirtioPciNotifyDevice {
    VirtioPciNotifyDevice::new(
        4,
        [
            VirtioQueueNotifySpec::new(VirtioQueueIndex::new(0).unwrap(), 0),
            VirtioQueueNotifySpec::new(VirtioQueueIndex::new(1).unwrap(), 3),
        ],
    )
    .unwrap()
}

fn device_config() -> VirtioPciDeviceConfigDevice {
    VirtioPciDeviceConfigDevice::new(
        VirtioPciDeviceConfigSpec::new(
            vec![0x11, 0x22, 0x33],
            ByteMask::full(AccessSize::new(3).unwrap()).unwrap(),
        )
        .unwrap(),
    )
}

fn split_queue(last_available_index: u16, event_index: bool) -> VirtioSplitQueue {
    VirtioSplitQueue::new(
        8,
        Address::new(0x1000),
        Address::new(0x2000),
        Address::new(0x3000),
        last_available_index,
    )
    .unwrap()
    .with_event_index(event_index)
}

fn write_common_u8(device: &VirtioPciCommonConfigDevice, offset: u64, value: u8) {
    device
        .write_local(
            Address::new(offset),
            vec![value],
            ByteMask::from_bits(vec![true]).unwrap(),
        )
        .unwrap();
}

fn write_common_u16(device: &VirtioPciCommonConfigDevice, offset: u64, value: u16) {
    device
        .write_local(
            Address::new(offset),
            value.to_le_bytes().to_vec(),
            ByteMask::from_bits(vec![true, true]).unwrap(),
        )
        .unwrap();
}

fn notify_queue(notify: &VirtioPciNotifyDevice, address: u64, value: u16, tick: u64) {
    notify
        .write_local(
            Address::new(address),
            value.to_le_bytes().to_vec(),
            ByteMask::from_bits(vec![true, true]).unwrap(),
            tick,
        )
        .unwrap();
}

fn mutate_config(config: &VirtioPciDeviceConfigDevice, address: u64, value: u8) {
    config
        .write_local(
            Address::new(address),
            vec![value],
            ByteMask::from_bits(vec![true]).unwrap(),
        )
        .unwrap();
}

fn clear_isr(isr: &VirtioPciIsrDevice) {
    isr.read_local(Address::new(0), AccessSize::new(1).unwrap())
        .unwrap();
}

#[test]
fn topology_host_controller_attaches_existing_virtio_split_queue_checkpoint_port() {
    let source = GuestSourceId::new(100);
    let component = CheckpointComponentId::new("virtio.block0.queue0").unwrap();
    let queue = Arc::new(Mutex::new(split_queue(5, true)));
    let expected = queue.lock().unwrap().snapshot();
    let system = base_system()
        .with_virtio_split_queue_checkpoint_port(VirtioSplitQueueCheckpointPort::new(
            component.clone(),
            Arc::clone(&queue),
        ))
        .unwrap()
        .with_host_controller(host_config(source), StatsRegistry::new())
        .unwrap();
    let host = system.host_controller().unwrap();

    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .virtio_split_queue_checkpoint_bank()
            .unwrap()
            .components(),
        vec![component.clone()]
    );

    let checkpoint = HostActionRecord::new(
        18,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(300),
        source,
        HostAction::Checkpoint {
            label: "topology-virtio-queue".to_string(),
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
                .any(|chunk| chunk.name() == "split-queue")
    }));

    *queue.lock().unwrap() = split_queue(0, false);

    let restore = HostActionRecord::new(
        28,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(303),
        source,
        HostAction::RestoreCheckpoint { manifest },
    );
    host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(queue.lock().unwrap().snapshot(), expected);
}

#[test]
fn topology_host_controller_attaches_existing_virtio_pci_checkpoint_ports() {
    let source = GuestSourceId::new(101);
    let common_component = CheckpointComponentId::new("virtio.block0.pci-common").unwrap();
    let notify_component = CheckpointComponentId::new("virtio.block0.pci-notify").unwrap();
    let isr_component = CheckpointComponentId::new("virtio.block0.pci-isr").unwrap();
    let config_component = CheckpointComponentId::new("virtio.block0.device-config").unwrap();
    let common = common_device();
    let notify = notify_device();
    let isr = VirtioPciIsrDevice::new();
    let config = device_config();

    write_common_u8(
        &common,
        VIRTIO_PCI_DEVICE_STATUS_OFFSET,
        VIRTIO_STATUS_ACKNOWLEDGE,
    );
    write_common_u16(&common, VIRTIO_PCI_QUEUE_SELECT_OFFSET, 0);
    write_common_u16(&common, VIRTIO_PCI_QUEUE_SIZE_OFFSET, 32);
    notify_queue(&notify, 0, 0, 11);
    isr.raise_queue_interrupt(12);
    mutate_config(&config, 1, 0xaa);
    let expected_common = common.snapshot();
    let expected_notify = notify.snapshot();
    let expected_isr = isr.snapshot();
    let expected_config = config.snapshot();

    let system = base_system()
        .with_virtio_pci_common_checkpoint_port(VirtioPciCommonCheckpointPort::new(
            common_component.clone(),
            common.clone(),
        ))
        .unwrap()
        .with_virtio_pci_notify_checkpoint_port(VirtioPciNotifyCheckpointPort::new(
            notify_component.clone(),
            notify.clone(),
        ))
        .unwrap()
        .with_virtio_pci_isr_checkpoint_port(VirtioPciIsrCheckpointPort::new(
            isr_component.clone(),
            isr.clone(),
        ))
        .unwrap()
        .with_virtio_pci_device_config_checkpoint_port(VirtioPciDeviceConfigCheckpointPort::new(
            config_component.clone(),
            config.clone(),
        ))
        .unwrap()
        .with_host_controller(host_config(source), StatsRegistry::new())
        .unwrap();
    let host = system.host_controller().unwrap();

    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .virtio_pci_common_checkpoint_bank()
            .unwrap()
            .components(),
        vec![common_component.clone()]
    );
    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .virtio_pci_notify_checkpoint_bank()
            .unwrap()
            .components(),
        vec![notify_component.clone()]
    );
    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .virtio_pci_isr_checkpoint_bank()
            .unwrap()
            .components(),
        vec![isr_component.clone()]
    );
    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .virtio_pci_device_config_checkpoint_bank()
            .unwrap()
            .components(),
        vec![config_component.clone()]
    );

    let checkpoint = HostActionRecord::new(
        20,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(301),
        source,
        HostAction::Checkpoint {
            label: "topology-virtio".to_string(),
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
        state.component() == &common_component
            && state
                .chunks()
                .iter()
                .any(|chunk| chunk.name() == "pci-common")
    }));
    assert!(manifest.states().iter().any(|state| {
        state.component() == &notify_component
            && state
                .chunks()
                .iter()
                .any(|chunk| chunk.name() == "pci-notify")
    }));
    assert!(manifest.states().iter().any(|state| {
        state.component() == &isr_component
            && state.chunks().iter().any(|chunk| chunk.name() == "pci-isr")
    }));
    assert!(manifest.states().iter().any(|state| {
        state.component() == &config_component
            && state
                .chunks()
                .iter()
                .any(|chunk| chunk.name() == "device-config")
    }));

    write_common_u8(&common, VIRTIO_PCI_DEVICE_STATUS_OFFSET, 0);
    notify_queue(&notify, 12, 1, 17);
    clear_isr(&isr);
    mutate_config(&config, 2, 0xbb);

    let restore = HostActionRecord::new(
        30,
        PartitionId::new(1),
        PartitionId::new(1),
        GuestEventId::new(302),
        source,
        HostAction::RestoreCheckpoint { manifest },
    );
    host.lock().unwrap().executor_mut().apply(&restore).unwrap();

    assert_eq!(common.snapshot(), expected_common);
    assert_eq!(notify.snapshot(), expected_notify);
    assert_eq!(isr.snapshot(), expected_isr);
    assert_eq!(config.snapshot(), expected_config);
}

#[test]
fn topology_host_controller_attaches_late_virtio_split_queue_checkpoint_port() {
    let source = GuestSourceId::new(99);
    let component = CheckpointComponentId::new("virtio.net0.queue0").unwrap();
    let queue = Arc::new(Mutex::new(split_queue(0, false)));
    let system = base_system()
        .with_host_controller(host_config(source), StatsRegistry::new())
        .unwrap()
        .with_virtio_split_queue_checkpoint_port(VirtioSplitQueueCheckpointPort::new(
            component.clone(),
            queue,
        ))
        .unwrap();
    let host = system.host_controller().unwrap();

    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .virtio_split_queue_checkpoint_bank()
            .unwrap()
            .components(),
        vec![component]
    );
}

#[test]
fn topology_rejects_duplicate_virtio_split_queue_checkpoint_component() {
    let component = CheckpointComponentId::new("virtio.block1.queue0").unwrap();
    let first_queue = Arc::new(Mutex::new(split_queue(0, false)));
    let second_queue = Arc::new(Mutex::new(split_queue(1, true)));
    let error = match base_system()
        .with_virtio_split_queue_checkpoint_port(VirtioSplitQueueCheckpointPort::new(
            component.clone(),
            first_queue,
        ))
        .unwrap()
        .with_virtio_split_queue_checkpoint_port(VirtioSplitQueueCheckpointPort::new(
            component.clone(),
            second_queue,
        )) {
        Ok(_) => panic!("duplicate VirtIO split queue checkpoint component succeeded"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        RiscvTopologySystemError::System(SystemError::Checkpoint(
            CheckpointError::DuplicateComponent { component: duplicate }
        )) if duplicate == component
    ));
}

#[test]
fn topology_host_controller_attaches_late_virtio_pci_checkpoint_port() {
    let source = GuestSourceId::new(102);
    let component = CheckpointComponentId::new("virtio.net0.pci-isr").unwrap();
    let isr = VirtioPciIsrDevice::new();
    let system = base_system()
        .with_host_controller(host_config(source), StatsRegistry::new())
        .unwrap()
        .with_virtio_pci_isr_checkpoint_port(VirtioPciIsrCheckpointPort::new(
            component.clone(),
            isr,
        ))
        .unwrap();
    let host = system.host_controller().unwrap();

    assert_eq!(
        host.lock()
            .unwrap()
            .executor()
            .virtio_pci_isr_checkpoint_bank()
            .unwrap()
            .components(),
        vec![component]
    );
}

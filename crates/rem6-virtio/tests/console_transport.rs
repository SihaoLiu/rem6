use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{MmioBus, MmioCompletion, MmioRequest, MmioRequestId, MmioResponse, MmioRoute};
use rem6_pci::{PciBarKind, PciClassCode, PciConfigOffset, PciDeviceIdentity, PciFunctionAddress};
use rem6_virtio::{
    VirtioConsoleConfig, VirtioConsoleDevice, VirtioPciBarIndex, VirtioPciCapabilityOffset,
    VirtioPciModernTransportDevices, VirtioPciModernTransportSpec, VirtioPciNotifyRegion,
    VirtioPciTransportBarSpec, VirtioPciTransportEndpointSpec, VirtioPciTransportRegion,
    VirtioQueueIndex, VIRTIO_CONSOLE_CONFIG_SIZE, VIRTIO_CONSOLE_DEVICE_ID, VIRTIO_CONSOLE_F_SIZE,
    VIRTIO_PCI_DEVICE_FEATURE_OFFSET, VIRTIO_PCI_ISR_STATUS_SIZE, VIRTIO_PCI_NUM_QUEUES_OFFSET,
    VIRTIO_PCI_QUEUE_NOTIFY_OFF_OFFSET, VIRTIO_PCI_QUEUE_SELECT_OFFSET,
    VIRTIO_PCI_QUEUE_SIZE_OFFSET,
};

fn bar(index: u8) -> VirtioPciBarIndex {
    VirtioPciBarIndex::new(index).unwrap()
}

fn memory_bar(index: u8, size: u64) -> VirtioPciTransportBarSpec {
    VirtioPciTransportBarSpec::new(
        bar(index),
        PciBarKind::Memory32 {
            prefetchable: false,
        },
        AccessSize::new(size).unwrap(),
    )
}

fn region(index: u8, offset: u64, length: u32) -> VirtioPciTransportRegion {
    VirtioPciTransportRegion::new(bar(index), offset, length)
}

fn endpoint_spec() -> VirtioPciTransportEndpointSpec {
    VirtioPciTransportEndpointSpec::new(
        PciFunctionAddress::new(0, 13, 0).unwrap(),
        PciDeviceIdentity::new(0x1af4, 0x1043),
        PciClassCode::new(0x07, 0x00, 0x00, 0x00),
    )
}

fn read_u16(device: &rem6_virtio::VirtioPciCommonConfigDevice, offset: u64) -> u16 {
    u16::from_le_bytes(
        device
            .read_local(Address::new(offset), AccessSize::new(2).unwrap())
            .unwrap()
            .try_into()
            .unwrap(),
    )
}

fn read_u32(device: &rem6_virtio::VirtioPciCommonConfigDevice, offset: u64) -> u32 {
    u32::from_le_bytes(
        device
            .read_local(Address::new(offset), AccessSize::new(4).unwrap())
            .unwrap()
            .try_into()
            .unwrap(),
    )
}

fn response_for(
    completions: &[MmioCompletion],
    request: MmioRequestId,
) -> &Result<MmioResponse, rem6_mmio::MmioError> {
    completions
        .iter()
        .find_map(|completion| match completion.response() {
            Ok(response) if response.request() == request => Some(completion.response()),
            _ => None,
        })
        .expect("completion for request")
}

fn completed_data(completions: &[MmioCompletion], request: MmioRequestId) -> Vec<u8> {
    response_for(completions, request)
        .as_ref()
        .expect("successful MMIO response")
        .data()
        .expect("read response data")
        .to_vec()
}

#[test]
fn virtio_console_builds_read_only_device_config_and_queue_devices() {
    let device = VirtioConsoleDevice::with_config(VirtioConsoleConfig::new(100, 40).unwrap());
    let config_spec = device.device_config_spec().unwrap();

    assert_eq!(config_spec.bytes(), &[100, 0, 40, 0]);
    assert_eq!(config_spec.writable().bits(), &[false, false, false, false]);

    let device_config = device.build_device_config().unwrap();
    assert_eq!(
        device_config.range(),
        AddressRange::new(
            Address::new(0),
            AccessSize::new(VIRTIO_CONSOLE_CONFIG_SIZE).unwrap()
        )
        .unwrap()
    );
    assert_eq!(
        device_config
            .read_local(Address::new(0), AccessSize::new(4).unwrap())
            .unwrap(),
        vec![100, 0, 40, 0]
    );
    assert!(device_config
        .write_local(
            Address::new(0),
            120_u16.to_le_bytes().to_vec(),
            ByteMask::from_bits(vec![true, true]).unwrap(),
        )
        .is_err());

    let common = device.build_common_config().unwrap();
    assert_eq!(read_u16(&common, VIRTIO_PCI_NUM_QUEUES_OFFSET), 2);
    assert_eq!(
        read_u32(&common, VIRTIO_PCI_DEVICE_FEATURE_OFFSET),
        VIRTIO_CONSOLE_F_SIZE
    );
    assert_eq!(read_u16(&common, VIRTIO_PCI_QUEUE_SIZE_OFFSET), 16);
    assert_eq!(read_u16(&common, VIRTIO_PCI_QUEUE_NOTIFY_OFF_OFFSET), 0);
    common
        .write_local(
            Address::new(VIRTIO_PCI_QUEUE_SELECT_OFFSET),
            1_u16.to_le_bytes().to_vec(),
            ByteMask::from_bits(vec![true, true]).unwrap(),
        )
        .unwrap();
    assert_eq!(read_u16(&common, VIRTIO_PCI_QUEUE_SIZE_OFFSET), 16);
    assert_eq!(read_u16(&common, VIRTIO_PCI_QUEUE_NOTIFY_OFF_OFFSET), 1);

    let notify = device.build_notify_device(4).unwrap();
    notify
        .write_local(
            Address::new(0),
            0_u16.to_le_bytes().to_vec(),
            ByteMask::from_bits(vec![true, true]).unwrap(),
            76,
        )
        .unwrap();
    notify
        .write_local(
            Address::new(4),
            1_u16.to_le_bytes().to_vec(),
            ByteMask::from_bits(vec![true, true]).unwrap(),
            77,
        )
        .unwrap();
    assert_eq!(notify.notifications().len(), 2);
    assert_eq!(
        notify.notifications()[0].queue(),
        VirtioQueueIndex::new(0).unwrap()
    );
    assert_eq!(notify.notifications()[0].address(), Address::new(0));
    assert_eq!(notify.notifications()[0].tick(), 76);
    assert_eq!(
        notify.notifications()[1].queue(),
        VirtioQueueIndex::new(1).unwrap()
    );
    assert_eq!(notify.notifications()[1].address(), Address::new(4));
    assert_eq!(notify.notifications()[1].tick(), 77);
}

#[test]
fn virtio_console_attaches_to_modern_pci_transport_runtime() {
    let device = VirtioConsoleDevice::new();
    let common = device.build_common_config().unwrap();
    let notify = device.build_notify_device(4).unwrap();
    let isr = rem6_virtio::VirtioPciIsrDevice::new();
    let device_config = device.build_device_config().unwrap();
    let transport = VirtioPciModernTransportSpec::new(
        endpoint_spec(),
        VirtioPciCapabilityOffset::new(0x70).unwrap(),
        [memory_bar(0, 0x1000)],
        region(0, 0x000, 0x40),
        VirtioPciNotifyRegion::new(region(0, 0x100, 0x100), 4).unwrap(),
        region(0, 0x200, VIRTIO_PCI_ISR_STATUS_SIZE as u32),
    )
    .with_device_config(region(0, 0x300, VIRTIO_CONSOLE_CONFIG_SIZE as u32));

    let endpoint = transport.build_endpoint().unwrap();
    assert_eq!(endpoint.identity().vendor_id(), 0x1af4);
    assert_eq!(endpoint.identity().device_id(), 0x1043);
    assert_eq!(
        endpoint
            .read_config(
                PciConfigOffset::new(0x34).unwrap(),
                AccessSize::new(1).unwrap(),
            )
            .unwrap(),
        vec![0x70]
    );

    let runtime = transport
        .build_bar_runtime(
            bar(0),
            VirtioPciModernTransportDevices::new(common, notify.clone(), isr)
                .with_device_config(device_config),
        )
        .unwrap();
    assert_eq!(runtime.device_count(), 4);

    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let route = MmioRoute::new(cpu, pci, 1, 1).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(runtime.range(), route, runtime).unwrap();
    let bus = Arc::new(bus);
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();

    let config_bus = Arc::clone(&bus);
    let config_completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 5, move |context| {
            config_bus
                .submit_parallel(
                    context,
                    MmioRequest::read(
                        MmioRequestId::new(1),
                        Address::new(0x300),
                        AccessSize::new(4).unwrap(),
                    )
                    .unwrap(),
                    move |completion| config_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    let notify_bus = Arc::clone(&bus);
    let notify_completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 5, move |context| {
            notify_bus
                .submit_parallel(
                    context,
                    MmioRequest::write(
                        MmioRequestId::new(2),
                        Address::new(0x100 + 4),
                        1_u16.to_le_bytes().to_vec(),
                        ByteMask::from_bits(vec![true, true]).unwrap(),
                    )
                    .unwrap(),
                    move |completion| notify_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    let completions = completions.lock().unwrap();
    assert_eq!(
        completed_data(&completions, MmioRequestId::new(1)),
        vec![80, 0, 24, 0]
    );
    assert!(response_for(&completions, MmioRequestId::new(2))
        .as_ref()
        .unwrap()
        .data()
        .is_none());
    drop(completions);

    assert_eq!(notify.notifications().len(), 1);
    assert_eq!(
        notify.notifications()[0].queue(),
        VirtioQueueIndex::new(1).unwrap()
    );
    assert_eq!(notify.notifications()[0].address(), Address::new(4));
}

#[test]
fn virtio_console_reports_gem5_device_id_for_transport_identity() {
    assert_eq!(VIRTIO_CONSOLE_DEVICE_ID, 3);
}

use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{MmioBus, MmioCompletion, MmioRequest, MmioRequestId, MmioResponse, MmioRoute};
use rem6_pci::{
    PciBarKind, PciClassCode, PciConfigOffset, PciDeviceIdentity, PciEndpointConfig,
    PciFunctionAddress,
};
use rem6_virtio::{
    VirtioPciBarIndex, VirtioPciCapabilityKind, VirtioPciCapabilityOffset,
    VirtioPciModernTransportDevices, VirtioPciModernTransportSpec, VirtioPciNotifyRegion,
    VirtioPciTransportBarSpec, VirtioPciTransportEndpointSpec, VirtioPciTransportRegion,
    VirtioQueueIndex, VirtioRngByteSource, VirtioRngDevice, VIRTIO_PCI_DEVICE_FEATURE_OFFSET,
    VIRTIO_PCI_ISR_STATUS_SIZE, VIRTIO_PCI_NUM_QUEUES_OFFSET, VIRTIO_PCI_QUEUE_NOTIFY_OFF_OFFSET,
    VIRTIO_PCI_QUEUE_SIZE_OFFSET, VIRTIO_RNG_DEFAULT_QUEUE_SIZE, VIRTIO_RNG_DEVICE_ID,
    VIRTIO_RNG_REQUEST_QUEUE_INDEX,
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
        PciFunctionAddress::new(0, 15, 0).unwrap(),
        PciDeviceIdentity::new(0x1af4, 0x1044),
        PciClassCode::new(0xff, 0x00, 0x00, 0x00),
    )
}

fn rng_device() -> VirtioRngDevice {
    VirtioRngDevice::new(VirtioRngByteSource::repeating(vec![0x11, 0x22]).unwrap())
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

fn capability_cfg_types(endpoint: &PciEndpointConfig) -> Vec<u8> {
    let mut offset = endpoint
        .read_config(
            PciConfigOffset::new(0x34).unwrap(),
            AccessSize::new(1).unwrap(),
        )
        .unwrap()[0];
    let mut cfg_types = Vec::new();
    for _ in 0..16 {
        if offset == 0 {
            break;
        }
        let header = endpoint
            .read_config(
                PciConfigOffset::new(u16::from(offset)).unwrap(),
                AccessSize::new(4).unwrap(),
            )
            .unwrap();
        cfg_types.push(header[3]);
        offset = header[1];
    }
    cfg_types
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
fn virtio_rng_builds_zero_config_request_queue_transport_devices() {
    let device = rng_device();

    assert!(device.feature_pages().is_empty());
    assert_eq!(device.config_size(), 0);

    let common = device.build_common_config().unwrap();
    assert_eq!(read_u16(&common, VIRTIO_PCI_NUM_QUEUES_OFFSET), 1);
    assert_eq!(read_u32(&common, VIRTIO_PCI_DEVICE_FEATURE_OFFSET), 0);
    assert_eq!(
        read_u16(&common, VIRTIO_PCI_QUEUE_SIZE_OFFSET),
        VIRTIO_RNG_DEFAULT_QUEUE_SIZE
    );
    assert_eq!(read_u16(&common, VIRTIO_PCI_QUEUE_NOTIFY_OFF_OFFSET), 0);

    let notify = device.build_notify_device(4).unwrap();
    notify
        .write_local(
            Address::new(0),
            VIRTIO_RNG_REQUEST_QUEUE_INDEX.to_le_bytes().to_vec(),
            ByteMask::from_bits(vec![true, true]).unwrap(),
            101,
        )
        .unwrap();
    assert_eq!(notify.notifications().len(), 1);
    assert_eq!(
        notify.notifications()[0].queue(),
        VirtioQueueIndex::new(VIRTIO_RNG_REQUEST_QUEUE_INDEX).unwrap()
    );
    assert_eq!(notify.notifications()[0].address(), Address::new(0));
    assert_eq!(notify.notifications()[0].tick(), 101);
}

#[test]
fn virtio_rng_attaches_to_modern_pci_transport_without_device_config() {
    let device = rng_device();
    let common = device.build_common_config().unwrap();
    let notify = device.build_notify_device(4).unwrap();
    let isr = rem6_virtio::VirtioPciIsrDevice::new();
    let transport = VirtioPciModernTransportSpec::new(
        endpoint_spec(),
        VirtioPciCapabilityOffset::new(0x70).unwrap(),
        [memory_bar(0, 0x1000)],
        region(0, 0x000, 0x40),
        VirtioPciNotifyRegion::new(region(0, 0x100, 0x100), 4).unwrap(),
        region(0, 0x200, VIRTIO_PCI_ISR_STATUS_SIZE as u32),
    );

    let endpoint = transport.build_endpoint().unwrap();
    assert_eq!(endpoint.identity().vendor_id(), 0x1af4);
    assert_eq!(endpoint.identity().device_id(), 0x1044);
    assert_eq!(
        endpoint
            .read_config(
                PciConfigOffset::new(0x34).unwrap(),
                AccessSize::new(1).unwrap(),
            )
            .unwrap(),
        vec![0x70]
    );
    let cfg_types = capability_cfg_types(&endpoint);
    assert_eq!(cfg_types.len(), 3);
    assert!(!cfg_types.contains(&VirtioPciCapabilityKind::DeviceConfig.cfg_type()));

    let runtime = transport
        .build_bar_runtime(
            bar(0),
            VirtioPciModernTransportDevices::new(common, notify.clone(), isr),
        )
        .unwrap();
    assert_eq!(runtime.device_count(), 3);

    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let route = MmioRoute::new(cpu, pci, 1, 1).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(runtime.range(), route, runtime).unwrap();
    let bus = Arc::new(bus);
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();

    let common_bus = Arc::clone(&bus);
    let common_completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 5, move |context| {
            common_bus
                .submit_parallel(
                    context,
                    MmioRequest::read(
                        MmioRequestId::new(1),
                        Address::new(VIRTIO_PCI_NUM_QUEUES_OFFSET),
                        AccessSize::new(2).unwrap(),
                    )
                    .unwrap(),
                    move |completion| common_completed.lock().unwrap().push(completion),
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
                        Address::new(0x100),
                        VIRTIO_RNG_REQUEST_QUEUE_INDEX.to_le_bytes().to_vec(),
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
        1_u16.to_le_bytes()
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
        VirtioQueueIndex::new(VIRTIO_RNG_REQUEST_QUEUE_INDEX).unwrap()
    );
}

#[test]
fn virtio_rng_reports_gem5_device_id_for_transport_identity() {
    assert_eq!(VIRTIO_RNG_DEVICE_ID, 4);
}

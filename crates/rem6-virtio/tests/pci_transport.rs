use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{MmioBus, MmioCompletion, MmioRequest, MmioRequestId, MmioResponse, MmioRoute};
use rem6_pci::{
    PciBarKind, PciClassCode, PciConfigOffset, PciDeviceIdentity, PciEndpointConfig,
    PciFunctionAddress,
};
use rem6_virtio::{
    VirtioPciBarIndex, VirtioPciCapabilityOffset, VirtioPciCommonConfigDevice,
    VirtioPciDeviceConfigDevice, VirtioPciDeviceConfigSpec, VirtioPciIsrDevice,
    VirtioPciIsrEventKind, VirtioPciModernTransportDevices, VirtioPciModernTransportSpec,
    VirtioPciNotifyDevice, VirtioPciNotifyRegion, VirtioPciSharedMemoryId,
    VirtioPciSharedMemoryRegionSpec, VirtioPciSharedMemoryRegistry, VirtioPciTransportBarSpec,
    VirtioPciTransportEndpointSpec, VirtioPciTransportRegion, VirtioQueueIndex,
    VirtioQueueNotifySpec, VirtioQueueSpec, VIRTIO_PCI_DEVICE_FEATURE_OFFSET,
    VIRTIO_PCI_ISR_STATUS_SIZE,
};

fn bar(index: u8) -> VirtioPciBarIndex {
    VirtioPciBarIndex::new(index).unwrap()
}

fn function() -> PciFunctionAddress {
    PciFunctionAddress::new(0, 12, 0).unwrap()
}

fn identity() -> PciDeviceIdentity {
    PciDeviceIdentity::new(0x1af4, 0x1042)
}

fn class() -> PciClassCode {
    PciClassCode::new(0x02, 0x00, 0x00, 0x00)
}

fn endpoint_spec() -> VirtioPciTransportEndpointSpec {
    VirtioPciTransportEndpointSpec::new(function(), identity(), class())
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

fn base_transport(
    common: VirtioPciTransportRegion,
    notify: VirtioPciNotifyRegion,
    isr: VirtioPciTransportRegion,
) -> VirtioPciModernTransportSpec {
    VirtioPciModernTransportSpec::new(
        endpoint_spec(),
        VirtioPciCapabilityOffset::new(0x70).unwrap(),
        [memory_bar(0, 0x1000)],
        common,
        notify,
        isr,
    )
}

fn read4(endpoint: &PciEndpointConfig, offset: u16) -> Vec<u8> {
    endpoint
        .read_config(
            PciConfigOffset::new(offset).unwrap(),
            AccessSize::new(4).unwrap(),
        )
        .unwrap()
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
fn virtio_pci_modern_transport_builds_endpoint_bars_and_capabilities() {
    let shared = VirtioPciSharedMemoryRegistry::new(
        [(bar(0), AccessSize::new(0x1000).unwrap())],
        [VirtioPciSharedMemoryRegionSpec::new(
            VirtioPciSharedMemoryId::new(7),
            bar(0),
            0x800,
            0x100,
        )
        .unwrap()],
    )
    .unwrap();
    let transport = base_transport(
        region(0, 0x000, 0x40),
        VirtioPciNotifyRegion::new(region(0, 0x100, 0x100), 4).unwrap(),
        region(0, 0x200, VIRTIO_PCI_ISR_STATUS_SIZE as u32),
    )
    .with_device_config(region(0, 0x300, 0x20))
    .with_shared_memory(shared);

    let endpoint = transport.build_endpoint().unwrap();

    assert_eq!(endpoint.function(), function());
    assert_eq!(endpoint.identity(), identity());
    assert_eq!(endpoint.class(), class());
    assert_eq!(
        endpoint
            .read_config(
                PciConfigOffset::new(0x34).unwrap(),
                AccessSize::new(1).unwrap(),
            )
            .unwrap(),
        vec![0x70]
    );
    assert_eq!(read4(&endpoint, 0x70), vec![0x09, 0x80, 0x10, 0x01]);
    assert_eq!(read4(&endpoint, 0x78), vec![0x00, 0x00, 0x00, 0x00]);
    assert_eq!(read4(&endpoint, 0x7c), vec![0x40, 0x00, 0x00, 0x00]);
    assert_eq!(read4(&endpoint, 0x80), vec![0x09, 0x94, 0x14, 0x02]);
    assert_eq!(read4(&endpoint, 0x88), vec![0x00, 0x01, 0x00, 0x00]);
    assert_eq!(read4(&endpoint, 0x8c), vec![0x00, 0x01, 0x00, 0x00]);
    assert_eq!(read4(&endpoint, 0x90), vec![0x04, 0x00, 0x00, 0x00]);
    assert_eq!(read4(&endpoint, 0x94), vec![0x09, 0xa4, 0x10, 0x03]);
    assert_eq!(read4(&endpoint, 0xa4), vec![0x09, 0xb4, 0x10, 0x04]);
    assert_eq!(read4(&endpoint, 0xb4), vec![0x09, 0x00, 0x18, 0x08]);
    assert_eq!(read4(&endpoint, 0xb8), vec![0x00, 0x07, 0x00, 0x00]);
    assert_eq!(read4(&endpoint, 0xbc), vec![0x00, 0x08, 0x00, 0x00]);
    assert_eq!(read4(&endpoint, 0xc0), vec![0x00, 0x01, 0x00, 0x00]);
}

#[test]
fn virtio_pci_modern_transport_rejects_invalid_bar_regions() {
    let missing = VirtioPciModernTransportSpec::new(
        endpoint_spec(),
        VirtioPciCapabilityOffset::new(0x70).unwrap(),
        [memory_bar(0, 0x1000)],
        region(1, 0x000, 0x40),
        VirtioPciNotifyRegion::new(region(0, 0x100, 0x100), 4).unwrap(),
        region(0, 0x200, VIRTIO_PCI_ISR_STATUS_SIZE as u32),
    );
    assert!(matches!(
        missing.build_endpoint(),
        Err(error) if error.to_string().contains("undeclared BAR")
    ));

    let outside = base_transport(
        region(0, 0xf80, 0x100),
        VirtioPciNotifyRegion::new(region(0, 0x100, 0x100), 4).unwrap(),
        region(0, 0x200, VIRTIO_PCI_ISR_STATUS_SIZE as u32),
    );
    assert!(matches!(
        outside.build_endpoint(),
        Err(error) if error.to_string().contains("contained within BAR")
    ));

    let overlapping = base_transport(
        region(0, 0x000, 0x80),
        VirtioPciNotifyRegion::new(region(0, 0x040, 0x100), 4).unwrap(),
        region(0, 0x200, VIRTIO_PCI_ISR_STATUS_SIZE as u32),
    );
    assert!(matches!(
        overlapping.build_endpoint(),
        Err(error) if error.to_string().contains("overlaps")
    ));
}

#[test]
fn virtio_pci_modern_transport_routes_parallel_bar_runtime_devices() {
    let transport = base_transport(
        region(0, 0x000, 0x40),
        VirtioPciNotifyRegion::new(region(0, 0x100, 0x100), 4).unwrap(),
        region(0, 0x200, VIRTIO_PCI_ISR_STATUS_SIZE as u32),
    )
    .with_device_config(region(0, 0x300, 0x10));
    let common = VirtioPciCommonConfigDevice::new(
        [(0, 0x5a5a_0101)],
        [
            VirtioQueueSpec::available(8, 0),
            VirtioQueueSpec::available(16, 3),
        ],
    )
    .unwrap();
    let notify = VirtioPciNotifyDevice::new(
        4,
        [
            VirtioQueueNotifySpec::new(VirtioQueueIndex::new(0).unwrap(), 0),
            VirtioQueueNotifySpec::new(VirtioQueueIndex::new(1).unwrap(), 3),
        ],
    )
    .unwrap();
    let isr = VirtioPciIsrDevice::new();
    let device_config = VirtioPciDeviceConfigDevice::new(
        VirtioPciDeviceConfigSpec::new(
            vec![0x11, 0x22, 0x33, 0x44],
            ByteMask::from_bits(vec![false, true, true, false]).unwrap(),
        )
        .unwrap(),
    );
    isr.raise_queue_interrupt(4);

    let runtime = transport
        .build_bar_runtime(
            bar(0),
            VirtioPciModernTransportDevices::new(common, notify.clone(), isr.clone())
                .with_device_config(device_config.clone()),
        )
        .unwrap();
    assert_eq!(runtime.bar(), bar(0));
    assert_eq!(runtime.device_count(), 4);

    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let route = MmioRoute::new(cpu, pci, 1, 1).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(runtime.range(), route, runtime).unwrap();
    let bus = Arc::new(bus);
    let completions = Arc::new(Mutex::new(Vec::new()));
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();

    let read_bus = Arc::clone(&bus);
    let read_completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 5, move |context| {
            read_bus
                .submit_parallel(
                    context,
                    MmioRequest::read(
                        MmioRequestId::new(1),
                        Address::new(VIRTIO_PCI_DEVICE_FEATURE_OFFSET),
                        AccessSize::new(4).unwrap(),
                    )
                    .unwrap(),
                    move |completion| read_completed.lock().unwrap().push(completion),
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
                        Address::new(0x100 + 3 * 4),
                        1_u16.to_le_bytes().to_vec(),
                        ByteMask::from_bits(vec![true, true]).unwrap(),
                    )
                    .unwrap(),
                    move |completion| notify_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    let isr_bus = Arc::clone(&bus);
    let isr_completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 5, move |context| {
            isr_bus
                .submit_parallel(
                    context,
                    MmioRequest::read(
                        MmioRequestId::new(3),
                        Address::new(0x200),
                        AccessSize::new(1).unwrap(),
                    )
                    .unwrap(),
                    move |completion| isr_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    let config_bus = Arc::clone(&bus);
    let config_completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 5, move |context| {
            config_bus
                .submit_parallel(
                    context,
                    MmioRequest::write(
                        MmioRequestId::new(4),
                        Address::new(0x301),
                        vec![0xaa, 0xbb],
                        ByteMask::from_bits(vec![true, true]).unwrap(),
                    )
                    .unwrap(),
                    move |completion| config_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    let completions = completions.lock().unwrap();
    assert_eq!(
        completed_data(&completions, MmioRequestId::new(1)),
        0x5a5a_0101_u32.to_le_bytes()
    );
    assert!(response_for(&completions, MmioRequestId::new(2))
        .as_ref()
        .unwrap()
        .data()
        .is_none());
    assert_eq!(
        completed_data(&completions, MmioRequestId::new(3)),
        vec![0x01]
    );
    assert!(response_for(&completions, MmioRequestId::new(4))
        .as_ref()
        .unwrap()
        .data()
        .is_none());
    drop(completions);

    assert_eq!(notify.notifications().len(), 1);
    assert_eq!(notify.notifications()[0].tick(), 6);
    assert_eq!(
        notify.notifications()[0].queue(),
        VirtioQueueIndex::new(1).unwrap()
    );
    assert_eq!(notify.notifications()[0].address(), Address::new(12));
    let isr_events = isr.events();
    assert_eq!(isr_events.len(), 2);
    assert_eq!(isr_events[1].kind(), VirtioPciIsrEventKind::DriverReadClear);
    assert_eq!(isr.status().bits(), 0);
    assert_eq!(device_config.bytes(), vec![0x11, 0xaa, 0xbb, 0x44]);
}

#[test]
fn virtio_pci_modern_transport_rejects_runtime_shape_mismatches() {
    let short_common = base_transport(
        region(0, 0x000, 0x10),
        VirtioPciNotifyRegion::new(region(0, 0x100, 0x100), 4).unwrap(),
        region(0, 0x200, VIRTIO_PCI_ISR_STATUS_SIZE as u32),
    );
    let common = VirtioPciCommonConfigDevice::new([], [VirtioQueueSpec::available(8, 0)]).unwrap();
    let notify = VirtioPciNotifyDevice::new(
        4,
        [VirtioQueueNotifySpec::new(
            VirtioQueueIndex::new(0).unwrap(),
            0,
        )],
    )
    .unwrap();
    let isr = VirtioPciIsrDevice::new();
    assert!(matches!(
        short_common.build_bar_runtime(
            bar(0),
            VirtioPciModernTransportDevices::new(common.clone(), notify.clone(), isr.clone()),
        ),
        Err(error) if error.to_string().contains("does not fit declared")
    ));

    let needs_config = base_transport(
        region(0, 0x000, 0x40),
        VirtioPciNotifyRegion::new(region(0, 0x100, 0x100), 4).unwrap(),
        region(0, 0x200, VIRTIO_PCI_ISR_STATUS_SIZE as u32),
    )
    .with_device_config(region(0, 0x300, 0x10));
    assert!(matches!(
        needs_config.build_bar_runtime(
            bar(0),
            VirtioPciModernTransportDevices::new(common, notify, isr),
        ),
        Err(error) if error.to_string().contains("device-specific config")
    ));
}

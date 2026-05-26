use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{MmioAccess, MmioError, MmioOperation};
use rem6_virtio::{
    VirtioPciCommonConfigDevice, VirtioQueueIndex, VirtioQueueSpec, VIRTIO_PCI_COMMON_CONFIG_SIZE,
    VIRTIO_PCI_DEVICE_FEATURE_OFFSET, VIRTIO_PCI_DEVICE_FEATURE_SELECT_OFFSET,
    VIRTIO_PCI_DEVICE_STATUS_OFFSET, VIRTIO_PCI_DRIVER_FEATURE_OFFSET,
    VIRTIO_PCI_DRIVER_FEATURE_SELECT_OFFSET, VIRTIO_PCI_NUM_QUEUES_OFFSET,
    VIRTIO_PCI_QUEUE_DESC_OFFSET, VIRTIO_PCI_QUEUE_DEVICE_OFFSET, VIRTIO_PCI_QUEUE_DRIVER_OFFSET,
    VIRTIO_PCI_QUEUE_ENABLE_OFFSET, VIRTIO_PCI_QUEUE_NOTIFY_OFF_OFFSET,
    VIRTIO_PCI_QUEUE_SELECT_OFFSET, VIRTIO_PCI_QUEUE_SIZE_OFFSET, VIRTIO_STATUS_ACKNOWLEDGE,
    VIRTIO_STATUS_DRIVER, VIRTIO_STATUS_FEATURES_OK,
};

fn common_device() -> VirtioPciCommonConfigDevice {
    VirtioPciCommonConfigDevice::new(
        [(0, 0x0000_0005), (1, 0x0000_0002)],
        [
            VirtioQueueSpec::available(256, 0),
            VirtioQueueSpec::available(128, 1),
        ],
    )
    .unwrap()
}

fn read_u8(device: &VirtioPciCommonConfigDevice, offset: u64) -> u8 {
    device
        .read_local(Address::new(offset), AccessSize::new(1).unwrap())
        .unwrap()[0]
}

fn read_u16(device: &VirtioPciCommonConfigDevice, offset: u64) -> u16 {
    u16::from_le_bytes(
        device
            .read_local(Address::new(offset), AccessSize::new(2).unwrap())
            .unwrap()
            .try_into()
            .unwrap(),
    )
}

fn read_u32(device: &VirtioPciCommonConfigDevice, offset: u64) -> u32 {
    u32::from_le_bytes(
        device
            .read_local(Address::new(offset), AccessSize::new(4).unwrap())
            .unwrap()
            .try_into()
            .unwrap(),
    )
}

fn write_u8(device: &VirtioPciCommonConfigDevice, offset: u64, value: u8) -> Result<(), MmioError> {
    device.write_local(
        Address::new(offset),
        vec![value],
        ByteMask::from_bits(vec![true]).unwrap(),
    )
}

fn write_u16(
    device: &VirtioPciCommonConfigDevice,
    offset: u64,
    value: u16,
) -> Result<(), MmioError> {
    device.write_local(
        Address::new(offset),
        value.to_le_bytes().to_vec(),
        ByteMask::from_bits(vec![true, true]).unwrap(),
    )
}

fn write_u32(
    device: &VirtioPciCommonConfigDevice,
    offset: u64,
    value: u32,
) -> Result<(), MmioError> {
    device.write_local(
        Address::new(offset),
        value.to_le_bytes().to_vec(),
        ByteMask::from_bits(vec![true, true, true, true]).unwrap(),
    )
}

fn write_u64(
    device: &VirtioPciCommonConfigDevice,
    offset: u64,
    value: u64,
) -> Result<(), MmioError> {
    device.write_local(
        Address::new(offset),
        value.to_le_bytes().to_vec(),
        ByteMask::from_bits(vec![true, true, true, true, true, true, true, true]).unwrap(),
    )
}

#[test]
fn virtio_pci_common_config_tracks_feature_pages_and_selected_queue_state() {
    let device = common_device();
    assert_eq!(
        device.range(),
        AddressRange::new(
            Address::new(0),
            AccessSize::new(VIRTIO_PCI_COMMON_CONFIG_SIZE).unwrap()
        )
        .unwrap()
    );
    assert_eq!(read_u16(&device, VIRTIO_PCI_NUM_QUEUES_OFFSET), 2);
    assert_eq!(
        read_u32(&device, VIRTIO_PCI_DEVICE_FEATURE_OFFSET),
        0x0000_0005
    );

    write_u32(&device, VIRTIO_PCI_DEVICE_FEATURE_SELECT_OFFSET, 1).unwrap();
    assert_eq!(
        read_u32(&device, VIRTIO_PCI_DEVICE_FEATURE_OFFSET),
        0x0000_0002
    );
    write_u32(&device, VIRTIO_PCI_DRIVER_FEATURE_SELECT_OFFSET, 1).unwrap();
    write_u32(&device, VIRTIO_PCI_DRIVER_FEATURE_OFFSET, 0x0000_0002).unwrap();

    write_u16(&device, VIRTIO_PCI_QUEUE_SELECT_OFFSET, 1).unwrap();
    assert_eq!(read_u16(&device, VIRTIO_PCI_QUEUE_SIZE_OFFSET), 128);
    assert_eq!(read_u16(&device, VIRTIO_PCI_QUEUE_NOTIFY_OFF_OFFSET), 1);
    write_u16(&device, VIRTIO_PCI_QUEUE_SIZE_OFFSET, 64).unwrap();
    write_u64(&device, VIRTIO_PCI_QUEUE_DESC_OFFSET, 0x0000_1000).unwrap();
    write_u64(&device, VIRTIO_PCI_QUEUE_DRIVER_OFFSET, 0x0000_2000).unwrap();
    write_u64(&device, VIRTIO_PCI_QUEUE_DEVICE_OFFSET, 0x0000_3000).unwrap();
    write_u16(&device, VIRTIO_PCI_QUEUE_ENABLE_OFFSET, 1).unwrap();
    write_u8(
        &device,
        VIRTIO_PCI_DEVICE_STATUS_OFFSET,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_FEATURES_OK,
    )
    .unwrap();

    let snapshot = device.snapshot();
    assert_eq!(snapshot.device_status(), 0x0b);
    assert_eq!(snapshot.driver_feature_page(1), 0x0000_0002);
    let queue = snapshot
        .queue(VirtioQueueIndex::new(1).unwrap())
        .expect("queue 1 snapshot");
    assert_eq!(queue.size(), 64);
    assert!(queue.enabled());
    assert_eq!(queue.desc_address(), 0x0000_1000);
    assert_eq!(queue.driver_address(), 0x0000_2000);
    assert_eq!(queue.device_address(), 0x0000_3000);

    write_u8(&device, VIRTIO_PCI_DEVICE_STATUS_OFFSET, 0).unwrap();
    assert_eq!(read_u8(&device, VIRTIO_PCI_DEVICE_STATUS_OFFSET), 0);
    let reset = device.snapshot();
    assert_eq!(reset.driver_feature_page(1), 0);
    let queue = reset
        .queue(VirtioQueueIndex::new(1).unwrap())
        .expect("queue 1 snapshot");
    assert_eq!(queue.size(), 128);
    assert!(!queue.enabled());
    assert_eq!(queue.desc_address(), 0);
    assert_eq!(queue.driver_address(), 0);
    assert_eq!(queue.device_address(), 0);

    device.restore(&snapshot);
    assert_eq!(read_u8(&device, VIRTIO_PCI_DEVICE_STATUS_OFFSET), 0x0b);
    assert_eq!(
        device
            .snapshot()
            .queue(VirtioQueueIndex::new(1).unwrap())
            .unwrap()
            .size(),
        64
    );
}

#[test]
fn virtio_pci_common_config_rejects_read_only_and_invalid_queue_writes() {
    let device = common_device();
    assert!(matches!(
        write_u32(&device, VIRTIO_PCI_DEVICE_FEATURE_OFFSET, 0xffff_ffff),
        Err(MmioError::AccessDenied {
            operation: MmioOperation::Write,
            access: MmioAccess::ReadOnly,
            ..
        })
    ));

    write_u16(&device, VIRTIO_PCI_QUEUE_SELECT_OFFSET, 1).unwrap();
    let invalid_size = write_u16(&device, VIRTIO_PCI_QUEUE_SIZE_OFFSET, 3);
    assert!(matches!(
        invalid_size,
        Err(MmioError::DeviceError { message, .. }) if message.contains("power of two")
    ));
    assert_eq!(read_u16(&device, VIRTIO_PCI_QUEUE_SIZE_OFFSET), 128);

    let invalid_enable = write_u16(&device, VIRTIO_PCI_QUEUE_ENABLE_OFFSET, 0);
    assert!(matches!(
        invalid_enable,
        Err(MmioError::DeviceError { message, .. }) if message.contains("queue_enable")
    ));

    write_u16(&device, VIRTIO_PCI_QUEUE_SELECT_OFFSET, 5).unwrap();
    assert_eq!(read_u16(&device, VIRTIO_PCI_QUEUE_SIZE_OFFSET), 0);
    let invalid_address = write_u64(&device, VIRTIO_PCI_QUEUE_DESC_OFFSET, 0x4000);
    assert!(matches!(
        invalid_address,
        Err(MmioError::DeviceError { message, .. }) if message.contains("unavailable queue")
    ));
}

use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::{MmioError, MmioOperation};
use rem6_virtio::{
    VirtioBlockConfigSpec, VirtioMmioTransportDevice, VirtioQueueIndex, VirtioQueueSpec,
    VIRTIO_BLOCK_CONFIG_CAPACITY_OFFSET, VIRTIO_BLOCK_DEVICE_ID, VIRTIO_BLOCK_F_FLUSH,
    VIRTIO_BLOCK_F_RO, VIRTIO_MMIO_CONFIG_OFFSET, VIRTIO_MMIO_DEVICE_FEATURES_OFFSET,
    VIRTIO_MMIO_DEVICE_FEATURES_SELECT_OFFSET, VIRTIO_MMIO_DEVICE_ID_OFFSET,
    VIRTIO_MMIO_DRIVER_FEATURES_OFFSET, VIRTIO_MMIO_DRIVER_FEATURES_SELECT_OFFSET,
    VIRTIO_MMIO_GUEST_PAGE_SIZE_OFFSET, VIRTIO_MMIO_INTERRUPT_ACK_OFFSET,
    VIRTIO_MMIO_INTERRUPT_STATUS_OFFSET, VIRTIO_MMIO_INTERRUPT_USED_RING, VIRTIO_MMIO_MAGIC_OFFSET,
    VIRTIO_MMIO_MAGIC_VALUE, VIRTIO_MMIO_QUEUE_ALIGN_OFFSET, VIRTIO_MMIO_QUEUE_NOTIFY_OFFSET,
    VIRTIO_MMIO_QUEUE_NUM_MAX_OFFSET, VIRTIO_MMIO_QUEUE_NUM_OFFSET, VIRTIO_MMIO_QUEUE_PFN_OFFSET,
    VIRTIO_MMIO_QUEUE_SELECT_OFFSET, VIRTIO_MMIO_STATUS_OFFSET, VIRTIO_MMIO_VENDOR_ID,
    VIRTIO_MMIO_VENDOR_ID_OFFSET, VIRTIO_MMIO_VERSION, VIRTIO_MMIO_VERSION_OFFSET,
    VIRTIO_STATUS_ACKNOWLEDGE, VIRTIO_STATUS_DRIVER, VIRTIO_STATUS_FEATURES_OK,
};

fn mmio_device() -> VirtioMmioTransportDevice {
    let config = VirtioBlockConfigSpec::new(8)
        .with_read_only(true)
        .with_flush(true)
        .build_device_config()
        .unwrap();
    VirtioMmioTransportDevice::new(
        VIRTIO_BLOCK_DEVICE_ID,
        [
            (0, VIRTIO_BLOCK_F_RO as u32),
            (1, VIRTIO_BLOCK_F_FLUSH as u32),
        ],
        [
            VirtioQueueSpec::available(256, 0),
            VirtioQueueSpec::available(128, 1),
        ],
        Some(config),
    )
    .unwrap()
}

fn read_u32(device: &VirtioMmioTransportDevice, offset: u64) -> u32 {
    u32::from_le_bytes(
        device
            .read_local(Address::new(offset), AccessSize::new(4).unwrap())
            .unwrap()
            .try_into()
            .unwrap(),
    )
}

fn read_u64(device: &VirtioMmioTransportDevice, offset: u64) -> u64 {
    u64::from_le_bytes(
        device
            .read_local(Address::new(offset), AccessSize::new(8).unwrap())
            .unwrap()
            .try_into()
            .unwrap(),
    )
}

fn write_u32(device: &VirtioMmioTransportDevice, offset: u64, value: u32) {
    device
        .write_local(
            Address::new(offset),
            value.to_le_bytes().to_vec(),
            ByteMask::from_bits(vec![true; 4]).unwrap(),
            11,
        )
        .unwrap();
}

#[test]
fn virtio_mmio_transport_tracks_legacy_registers_and_split_queue_layout() {
    let device = mmio_device();

    assert_eq!(
        read_u32(&device, VIRTIO_MMIO_MAGIC_OFFSET),
        VIRTIO_MMIO_MAGIC_VALUE
    );
    assert_eq!(
        read_u32(&device, VIRTIO_MMIO_VERSION_OFFSET),
        VIRTIO_MMIO_VERSION
    );
    assert_eq!(
        read_u32(&device, VIRTIO_MMIO_DEVICE_ID_OFFSET),
        u32::from(VIRTIO_BLOCK_DEVICE_ID)
    );
    assert_eq!(
        read_u32(&device, VIRTIO_MMIO_VENDOR_ID_OFFSET),
        VIRTIO_MMIO_VENDOR_ID
    );
    assert_eq!(
        read_u32(&device, VIRTIO_MMIO_DEVICE_FEATURES_OFFSET),
        VIRTIO_BLOCK_F_RO as u32
    );

    write_u32(&device, VIRTIO_MMIO_DEVICE_FEATURES_SELECT_OFFSET, 1);
    assert_eq!(
        read_u32(&device, VIRTIO_MMIO_DEVICE_FEATURES_OFFSET),
        VIRTIO_BLOCK_F_FLUSH as u32
    );
    write_u32(&device, VIRTIO_MMIO_DRIVER_FEATURES_SELECT_OFFSET, 1);
    write_u32(
        &device,
        VIRTIO_MMIO_DRIVER_FEATURES_OFFSET,
        VIRTIO_BLOCK_F_FLUSH as u32,
    );

    assert_eq!(read_u32(&device, VIRTIO_MMIO_QUEUE_NUM_MAX_OFFSET), 256);
    write_u32(&device, VIRTIO_MMIO_QUEUE_SELECT_OFFSET, 1);
    assert_eq!(read_u32(&device, VIRTIO_MMIO_QUEUE_NUM_MAX_OFFSET), 128);
    write_u32(&device, VIRTIO_MMIO_QUEUE_NUM_OFFSET, 64);
    write_u32(&device, VIRTIO_MMIO_GUEST_PAGE_SIZE_OFFSET, 4096);
    write_u32(&device, VIRTIO_MMIO_QUEUE_ALIGN_OFFSET, 4096);
    write_u32(&device, VIRTIO_MMIO_QUEUE_PFN_OFFSET, 0x20);
    write_u32(
        &device,
        VIRTIO_MMIO_STATUS_OFFSET,
        u32::from(VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_FEATURES_OK),
    );

    let snapshot = device.snapshot();
    assert_eq!(snapshot.driver_feature_page(1), VIRTIO_BLOCK_F_FLUSH as u32);
    assert_eq!(snapshot.device_status(), 0x0b);
    let queue = snapshot.queue(VirtioQueueIndex::new(1).unwrap()).unwrap();
    assert_eq!(queue.size(), 64);
    assert_eq!(queue.pfn(), 0x20);

    let split = device
        .split_queue(VirtioQueueIndex::new(1).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(split.descriptor_table(), Address::new(0x0002_0000));
    assert_eq!(split.available_ring(), Address::new(0x0002_0400));
    assert_eq!(split.used_ring(), Address::new(0x0002_1000));
}

#[test]
fn virtio_mmio_transport_forwards_device_config_and_records_interrupts() {
    let device = mmio_device();

    assert_eq!(
        read_u64(
            &device,
            VIRTIO_MMIO_CONFIG_OFFSET + VIRTIO_BLOCK_CONFIG_CAPACITY_OFFSET
        ),
        8
    );

    write_u32(&device, VIRTIO_MMIO_QUEUE_NOTIFY_OFFSET, 1);
    assert_eq!(device.notifications().len(), 1);
    assert_eq!(
        device.notifications()[0].queue(),
        VirtioQueueIndex::new(1).unwrap()
    );
    assert_eq!(device.notifications()[0].tick(), 11);

    device.raise_used_ring_interrupt();
    assert_eq!(
        read_u32(&device, VIRTIO_MMIO_INTERRUPT_STATUS_OFFSET),
        VIRTIO_MMIO_INTERRUPT_USED_RING
    );
    write_u32(
        &device,
        VIRTIO_MMIO_INTERRUPT_ACK_OFFSET,
        VIRTIO_MMIO_INTERRUPT_USED_RING,
    );
    assert_eq!(read_u32(&device, VIRTIO_MMIO_INTERRUPT_STATUS_OFFSET), 0);
}

#[test]
fn virtio_mmio_transport_replaces_gem5_panic_paths_with_typed_errors() {
    let device = mmio_device();

    let unsupported_feature = device.write_local(
        Address::new(VIRTIO_MMIO_DRIVER_FEATURES_OFFSET),
        (VIRTIO_BLOCK_F_FLUSH as u32).to_le_bytes().to_vec(),
        ByteMask::from_bits(vec![true; 4]).unwrap(),
        1,
    );
    assert!(matches!(
        unsupported_feature,
        Err(MmioError::DeviceError { message, .. }) if message.contains("unsupported")
    ));

    let bad_page = device.write_local(
        Address::new(VIRTIO_MMIO_GUEST_PAGE_SIZE_OFFSET),
        8192_u32.to_le_bytes().to_vec(),
        ByteMask::from_bits(vec![true; 4]).unwrap(),
        2,
    );
    assert!(matches!(
        bad_page,
        Err(MmioError::DeviceError { message, .. }) if message.contains("page size")
    ));

    let bad_align = device.write_local(
        Address::new(VIRTIO_MMIO_QUEUE_ALIGN_OFFSET),
        8192_u32.to_le_bytes().to_vec(),
        ByteMask::from_bits(vec![true; 4]).unwrap(),
        3,
    );
    assert!(matches!(
        bad_align,
        Err(MmioError::DeviceError { message, .. }) if message.contains("queue align")
    ));

    let bad_queue_size = device.write_local(
        Address::new(VIRTIO_MMIO_QUEUE_NUM_OFFSET),
        65_u32.to_le_bytes().to_vec(),
        ByteMask::from_bits(vec![true; 4]).unwrap(),
        4,
    );
    assert!(matches!(
        bad_queue_size,
        Err(MmioError::DeviceError { message, .. }) if message.contains("nonzero power of two")
    ));

    let unavailable_queue_notify = device.write_local(
        Address::new(VIRTIO_MMIO_QUEUE_NOTIFY_OFFSET),
        9_u32.to_le_bytes().to_vec(),
        ByteMask::from_bits(vec![true; 4]).unwrap(),
        5,
    );
    assert!(matches!(
        unavailable_queue_notify,
        Err(MmioError::DeviceError { message, .. }) if message.contains("unavailable queue")
    ));

    let short_register_read = device.read_local(
        Address::new(VIRTIO_MMIO_MAGIC_OFFSET),
        AccessSize::new(2).unwrap(),
    );
    assert!(matches!(
        short_register_read,
        Err(MmioError::AccessSizeMismatch {
            expected: 4,
            actual: 2,
            ..
        })
    ));

    let read_only_write = device.write_local(
        Address::new(VIRTIO_MMIO_MAGIC_OFFSET),
        VIRTIO_MMIO_MAGIC_VALUE.to_le_bytes().to_vec(),
        ByteMask::from_bits(vec![true; 4]).unwrap(),
        3,
    );
    assert!(matches!(
        read_only_write,
        Err(MmioError::AccessDenied {
            operation: MmioOperation::Write,
            ..
        })
    ));
}

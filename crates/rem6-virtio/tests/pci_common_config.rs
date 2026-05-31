use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{MmioAccess, MmioError, MmioOperation};
use rem6_virtio::{
    VirtioError, VirtioPciCommonConfigDevice, VirtioPciCommonSnapshot, VirtioQueueIndex,
    VirtioQueueSpec, VIRTIO_PCI_COMMON_CONFIG_SIZE, VIRTIO_PCI_CONFIG_MSIX_VECTOR_OFFSET,
    VIRTIO_PCI_DEVICE_FEATURE_OFFSET, VIRTIO_PCI_DEVICE_FEATURE_SELECT_OFFSET,
    VIRTIO_PCI_DEVICE_STATUS_OFFSET, VIRTIO_PCI_DRIVER_FEATURE_OFFSET,
    VIRTIO_PCI_DRIVER_FEATURE_SELECT_OFFSET, VIRTIO_PCI_NUM_QUEUES_OFFSET,
    VIRTIO_PCI_QUEUE_DESC_OFFSET, VIRTIO_PCI_QUEUE_DEVICE_OFFSET, VIRTIO_PCI_QUEUE_DRIVER_OFFSET,
    VIRTIO_PCI_QUEUE_ENABLE_OFFSET, VIRTIO_PCI_QUEUE_MSIX_VECTOR_OFFSET,
    VIRTIO_PCI_QUEUE_NOTIFY_OFF_OFFSET, VIRTIO_PCI_QUEUE_SELECT_OFFSET,
    VIRTIO_PCI_QUEUE_SIZE_OFFSET, VIRTIO_STATUS_ACKNOWLEDGE, VIRTIO_STATUS_DRIVER,
    VIRTIO_STATUS_FEATURES_OK,
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

fn read_u64(device: &VirtioPciCommonConfigDevice, offset: u64) -> u64 {
    u64::from_le_bytes(
        device
            .read_local(Address::new(offset), AccessSize::new(8).unwrap())
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
fn virtio_pci_common_config_snapshot_bytes_round_trip_and_restore() {
    let device = common_device();
    write_u32(&device, VIRTIO_PCI_DEVICE_FEATURE_SELECT_OFFSET, 1).unwrap();
    write_u32(&device, VIRTIO_PCI_DRIVER_FEATURE_SELECT_OFFSET, 1).unwrap();
    write_u32(&device, VIRTIO_PCI_DRIVER_FEATURE_OFFSET, 0x0000_0002).unwrap();
    write_u16(&device, VIRTIO_PCI_CONFIG_MSIX_VECTOR_OFFSET, 7).unwrap();
    write_u16(&device, VIRTIO_PCI_QUEUE_SELECT_OFFSET, 1).unwrap();
    write_u16(&device, VIRTIO_PCI_QUEUE_MSIX_VECTOR_OFFSET, 9).unwrap();
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
    let payload = snapshot.to_bytes();

    assert_eq!(&payload[0..8], b"VIOCOMM1");
    assert_eq!(u16::from_le_bytes(payload[8..10].try_into().unwrap()), 1);
    assert_eq!(u64::from_le_bytes(payload[10..18].try_into().unwrap()), 2);
    assert_eq!(u32::from_le_bytes(payload[18..22].try_into().unwrap()), 0);
    assert_eq!(
        u32::from_le_bytes(payload[22..26].try_into().unwrap()),
        0x0000_0005
    );
    assert_eq!(u32::from_le_bytes(payload[26..30].try_into().unwrap()), 1);
    assert_eq!(
        u32::from_le_bytes(payload[30..34].try_into().unwrap()),
        0x0000_0002
    );
    assert_eq!(u64::from_le_bytes(payload[34..42].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(payload[42..46].try_into().unwrap()), 1);
    assert_eq!(
        u32::from_le_bytes(payload[46..50].try_into().unwrap()),
        0x0000_0002
    );
    assert_eq!(u32::from_le_bytes(payload[50..54].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(payload[54..58].try_into().unwrap()), 1);
    assert_eq!(u16::from_le_bytes(payload[58..60].try_into().unwrap()), 7);
    assert_eq!(payload[60], 0x0b);
    assert_eq!(payload[61], 0);
    assert_eq!(u16::from_le_bytes(payload[62..64].try_into().unwrap()), 1);
    assert_eq!(u16::from_le_bytes(payload[64..66].try_into().unwrap()), 0);
    assert_eq!(u16::from_le_bytes(payload[66..68].try_into().unwrap()), 0);
    assert_eq!(u64::from_le_bytes(payload[68..76].try_into().unwrap()), 2);
    let queue1 = 111;
    assert_eq!(
        u16::from_le_bytes(payload[queue1..queue1 + 2].try_into().unwrap()),
        128
    );
    assert_eq!(
        u16::from_le_bytes(payload[queue1 + 2..queue1 + 4].try_into().unwrap()),
        64
    );
    assert_eq!(
        u16::from_le_bytes(payload[queue1 + 4..queue1 + 6].try_into().unwrap()),
        1
    );
    assert_eq!(
        u16::from_le_bytes(payload[queue1 + 6..queue1 + 8].try_into().unwrap()),
        1
    );
    assert_eq!(
        u16::from_le_bytes(payload[queue1 + 8..queue1 + 10].try_into().unwrap()),
        9
    );
    assert_eq!(payload[queue1 + 10], 1);
    assert_eq!(
        u64::from_le_bytes(payload[queue1 + 11..queue1 + 19].try_into().unwrap()),
        0x0000_1000
    );
    assert_eq!(
        u64::from_le_bytes(payload[queue1 + 19..queue1 + 27].try_into().unwrap()),
        0x0000_2000
    );
    assert_eq!(
        u64::from_le_bytes(payload[queue1 + 27..queue1 + 35].try_into().unwrap()),
        0x0000_3000
    );

    let decoded = VirtioPciCommonSnapshot::from_bytes(&payload).unwrap();

    assert_eq!(decoded, snapshot);
    assert_eq!(decoded.device_status(), 0x0b);
    assert_eq!(decoded.driver_feature_page(1), 0x0000_0002);
    assert_eq!(
        decoded
            .queue(VirtioQueueIndex::new(1).unwrap())
            .unwrap()
            .size(),
        64
    );

    let restored = VirtioPciCommonConfigDevice::new([], []).unwrap();
    restored.restore(&decoded);
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(read_u16(&restored, VIRTIO_PCI_QUEUE_MSIX_VECTOR_OFFSET), 9);
    assert_eq!(
        read_u64(&restored, VIRTIO_PCI_QUEUE_DESC_OFFSET),
        0x0000_1000
    );
}

#[test]
fn virtio_pci_common_config_snapshot_bytes_reject_malformed_payloads() {
    let device = common_device();
    write_u32(&device, VIRTIO_PCI_DRIVER_FEATURE_SELECT_OFFSET, 1).unwrap();
    write_u32(&device, VIRTIO_PCI_DRIVER_FEATURE_OFFSET, 0x0000_0002).unwrap();
    write_u16(&device, VIRTIO_PCI_QUEUE_SELECT_OFFSET, 1).unwrap();
    write_u16(&device, VIRTIO_PCI_QUEUE_SIZE_OFFSET, 64).unwrap();
    let payload = device.snapshot().to_bytes();

    assert_eq!(
        VirtioPciCommonSnapshot::from_bytes(&payload[..payload.len() - 1]),
        Err(VirtioError::InvalidCommonConfigSnapshot)
    );

    let mut invalid_magic = payload.clone();
    invalid_magic[0] ^= 0xff;
    assert_eq!(
        VirtioPciCommonSnapshot::from_bytes(&invalid_magic),
        Err(VirtioError::InvalidCommonConfigSnapshot)
    );

    let mut invalid_version = payload.clone();
    invalid_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        VirtioPciCommonSnapshot::from_bytes(&invalid_version),
        Err(VirtioError::InvalidCommonConfigSnapshot)
    );

    let mut invalid_device_feature_count = payload.clone();
    invalid_device_feature_count[10..18].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        VirtioPciCommonSnapshot::from_bytes(&invalid_device_feature_count),
        Err(VirtioError::InvalidCommonConfigSnapshot)
    );

    let mut duplicate_device_feature = payload.clone();
    duplicate_device_feature[26..30].copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        VirtioPciCommonSnapshot::from_bytes(&duplicate_device_feature),
        Err(VirtioError::InvalidCommonConfigSnapshot)
    );

    let mut invalid_queue_count = payload.clone();
    invalid_queue_count[68..76].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        VirtioPciCommonSnapshot::from_bytes(&invalid_queue_count),
        Err(VirtioError::InvalidCommonConfigSnapshot)
    );

    let mut invalid_queue_max_size = payload.clone();
    invalid_queue_max_size[76..78].copy_from_slice(&0_u16.to_le_bytes());
    assert_eq!(
        VirtioPciCommonSnapshot::from_bytes(&invalid_queue_max_size),
        Err(VirtioError::InvalidCommonConfigSnapshot)
    );

    let queue1 = 111;
    let mut invalid_queue_size = payload.clone();
    invalid_queue_size[queue1 + 2..queue1 + 4].copy_from_slice(&3_u16.to_le_bytes());
    assert_eq!(
        VirtioPciCommonSnapshot::from_bytes(&invalid_queue_size),
        Err(VirtioError::InvalidCommonConfigSnapshot)
    );

    let mut invalid_queue_enabled = payload.clone();
    invalid_queue_enabled[queue1 + 10] = 2;
    assert_eq!(
        VirtioPciCommonSnapshot::from_bytes(&invalid_queue_enabled),
        Err(VirtioError::InvalidCommonConfigSnapshot)
    );

    let mut trailing = payload.clone();
    trailing.push(0);
    assert_eq!(
        VirtioPciCommonSnapshot::from_bytes(&trailing),
        Err(VirtioError::InvalidCommonConfigSnapshot)
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

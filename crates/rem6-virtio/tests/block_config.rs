use rem6_memory::{AccessSize, Address, ByteMask};
use rem6_mmio::MmioError;
use rem6_virtio::{
    VirtioBlockCacheMode, VirtioBlockConfigSpec, VirtioBlockDiscardLimits, VirtioBlockGeometry,
    VirtioBlockSecureEraseLimits, VirtioBlockTopology, VirtioBlockWriteZeroesLimits,
    VirtioPciDeviceConfigDevice, VIRTIO_BLOCK_CONFIG_ALIGNMENT_OFFSET_OFFSET,
    VIRTIO_BLOCK_CONFIG_BLK_SIZE_OFFSET, VIRTIO_BLOCK_CONFIG_CAPACITY_OFFSET,
    VIRTIO_BLOCK_CONFIG_CYLINDERS_OFFSET, VIRTIO_BLOCK_CONFIG_DISCARD_ALIGNMENT_OFFSET,
    VIRTIO_BLOCK_CONFIG_HEADS_OFFSET, VIRTIO_BLOCK_CONFIG_MAX_DISCARD_SECTORS_OFFSET,
    VIRTIO_BLOCK_CONFIG_MAX_DISCARD_SEG_OFFSET,
    VIRTIO_BLOCK_CONFIG_MAX_SECURE_ERASE_SECTORS_OFFSET,
    VIRTIO_BLOCK_CONFIG_MAX_SECURE_ERASE_SEG_OFFSET,
    VIRTIO_BLOCK_CONFIG_MAX_WRITE_ZEROES_SECTORS_OFFSET,
    VIRTIO_BLOCK_CONFIG_MAX_WRITE_ZEROES_SEG_OFFSET, VIRTIO_BLOCK_CONFIG_NUM_QUEUES_OFFSET,
    VIRTIO_BLOCK_CONFIG_OPT_IO_SIZE_OFFSET, VIRTIO_BLOCK_CONFIG_PHYSICAL_BLOCK_EXP_OFFSET,
    VIRTIO_BLOCK_CONFIG_SECTORS_OFFSET, VIRTIO_BLOCK_CONFIG_SECURE_ERASE_ALIGNMENT_OFFSET,
    VIRTIO_BLOCK_CONFIG_SEG_MAX_OFFSET, VIRTIO_BLOCK_CONFIG_SIZE,
    VIRTIO_BLOCK_CONFIG_SIZE_MAX_OFFSET, VIRTIO_BLOCK_CONFIG_UNUSED0_OFFSET,
    VIRTIO_BLOCK_CONFIG_UNUSED1_OFFSET, VIRTIO_BLOCK_CONFIG_WRITEBACK_OFFSET,
    VIRTIO_BLOCK_CONFIG_WRITE_ZEROES_MAY_UNMAP_OFFSET, VIRTIO_BLOCK_F_BLK_SIZE,
    VIRTIO_BLOCK_F_CONFIG_WCE, VIRTIO_BLOCK_F_DISCARD, VIRTIO_BLOCK_F_FLUSH,
    VIRTIO_BLOCK_F_GEOMETRY, VIRTIO_BLOCK_F_MQ, VIRTIO_BLOCK_F_RO, VIRTIO_BLOCK_F_SECURE_ERASE,
    VIRTIO_BLOCK_F_SEG_MAX, VIRTIO_BLOCK_F_SIZE_MAX, VIRTIO_BLOCK_F_TOPOLOGY,
    VIRTIO_BLOCK_F_WRITE_ZEROES,
};

fn le_u16(bytes: &[u8], offset: u64) -> u16 {
    u16::from_le_bytes(
        bytes[offset as usize..offset as usize + 2]
            .try_into()
            .unwrap(),
    )
}

fn le_u32(bytes: &[u8], offset: u64) -> u32 {
    u32::from_le_bytes(
        bytes[offset as usize..offset as usize + 4]
            .try_into()
            .unwrap(),
    )
}

fn le_u64(bytes: &[u8], offset: u64) -> u64 {
    u64::from_le_bytes(
        bytes[offset as usize..offset as usize + 8]
            .try_into()
            .unwrap(),
    )
}

#[test]
fn virtio_block_config_exports_typed_modern_layout_and_features() {
    let spec = VirtioBlockConfigSpec::new(0x0123_4567_89ab_cdef)
        .with_size_max(0x1000)
        .with_seg_max(0x40)
        .with_geometry(VirtioBlockGeometry::new(0x1234, 16, 63).unwrap())
        .with_read_only(true)
        .with_block_size(4096)
        .with_topology(VirtioBlockTopology::new(3, 2, 8, 128).unwrap())
        .with_flush(true)
        .with_writeback(VirtioBlockCacheMode::WriteBack)
        .with_queues(4)
        .with_discard(VirtioBlockDiscardLimits::new(2048, 4, 8).unwrap())
        .with_write_zeroes(VirtioBlockWriteZeroesLimits::new(4096, 3, true).unwrap())
        .with_secure_erase(VirtioBlockSecureEraseLimits::new(8192, 2, 16).unwrap());

    let expected_features = VIRTIO_BLOCK_F_SIZE_MAX
        | VIRTIO_BLOCK_F_SEG_MAX
        | VIRTIO_BLOCK_F_GEOMETRY
        | VIRTIO_BLOCK_F_RO
        | VIRTIO_BLOCK_F_BLK_SIZE
        | VIRTIO_BLOCK_F_FLUSH
        | VIRTIO_BLOCK_F_TOPOLOGY
        | VIRTIO_BLOCK_F_CONFIG_WCE
        | VIRTIO_BLOCK_F_MQ
        | VIRTIO_BLOCK_F_DISCARD
        | VIRTIO_BLOCK_F_WRITE_ZEROES
        | VIRTIO_BLOCK_F_SECURE_ERASE;
    assert_eq!(spec.feature_bits(), expected_features);
    assert_eq!(
        spec.feature_pages(),
        vec![(0, (expected_features & u64::from(u32::MAX)) as u32)]
    );

    let config = spec.device_config_spec().unwrap();
    let bytes = config.bytes();
    assert_eq!(bytes.len(), VIRTIO_BLOCK_CONFIG_SIZE as usize);
    assert_eq!(config.writable().len(), VIRTIO_BLOCK_CONFIG_SIZE);
    assert!(config.writable().bits()[VIRTIO_BLOCK_CONFIG_WRITEBACK_OFFSET as usize]);
    assert_eq!(
        config
            .writable()
            .bits()
            .iter()
            .filter(|enabled| **enabled)
            .count(),
        1
    );

    assert_eq!(
        le_u64(bytes, VIRTIO_BLOCK_CONFIG_CAPACITY_OFFSET),
        0x0123_4567_89ab_cdef
    );
    assert_eq!(le_u32(bytes, VIRTIO_BLOCK_CONFIG_SIZE_MAX_OFFSET), 0x1000);
    assert_eq!(le_u32(bytes, VIRTIO_BLOCK_CONFIG_SEG_MAX_OFFSET), 0x40);
    assert_eq!(le_u16(bytes, VIRTIO_BLOCK_CONFIG_CYLINDERS_OFFSET), 0x1234);
    assert_eq!(bytes[VIRTIO_BLOCK_CONFIG_HEADS_OFFSET as usize], 16);
    assert_eq!(bytes[VIRTIO_BLOCK_CONFIG_SECTORS_OFFSET as usize], 63);
    assert_eq!(le_u32(bytes, VIRTIO_BLOCK_CONFIG_BLK_SIZE_OFFSET), 4096);
    assert_eq!(
        bytes[VIRTIO_BLOCK_CONFIG_PHYSICAL_BLOCK_EXP_OFFSET as usize],
        3
    );
    assert_eq!(
        bytes[VIRTIO_BLOCK_CONFIG_ALIGNMENT_OFFSET_OFFSET as usize],
        2
    );
    assert_eq!(le_u16(bytes, VIRTIO_BLOCK_CONFIG_OPT_IO_SIZE_OFFSET - 2), 8);
    assert_eq!(le_u32(bytes, VIRTIO_BLOCK_CONFIG_OPT_IO_SIZE_OFFSET), 128);
    assert_eq!(bytes[VIRTIO_BLOCK_CONFIG_WRITEBACK_OFFSET as usize], 1);
    assert_eq!(bytes[VIRTIO_BLOCK_CONFIG_UNUSED0_OFFSET as usize], 0);
    assert_eq!(le_u16(bytes, VIRTIO_BLOCK_CONFIG_NUM_QUEUES_OFFSET), 4);
    assert_eq!(
        le_u32(bytes, VIRTIO_BLOCK_CONFIG_MAX_DISCARD_SECTORS_OFFSET),
        2048
    );
    assert_eq!(le_u32(bytes, VIRTIO_BLOCK_CONFIG_MAX_DISCARD_SEG_OFFSET), 4);
    assert_eq!(
        le_u32(bytes, VIRTIO_BLOCK_CONFIG_DISCARD_ALIGNMENT_OFFSET),
        8
    );
    assert_eq!(
        le_u32(bytes, VIRTIO_BLOCK_CONFIG_MAX_WRITE_ZEROES_SECTORS_OFFSET),
        4096
    );
    assert_eq!(
        le_u32(bytes, VIRTIO_BLOCK_CONFIG_MAX_WRITE_ZEROES_SEG_OFFSET),
        3
    );
    assert_eq!(
        bytes[VIRTIO_BLOCK_CONFIG_WRITE_ZEROES_MAY_UNMAP_OFFSET as usize],
        1
    );
    assert_eq!(
        &bytes[VIRTIO_BLOCK_CONFIG_UNUSED1_OFFSET as usize
            ..VIRTIO_BLOCK_CONFIG_UNUSED1_OFFSET as usize + 3],
        &[0, 0, 0]
    );
    assert_eq!(
        le_u32(bytes, VIRTIO_BLOCK_CONFIG_MAX_SECURE_ERASE_SECTORS_OFFSET),
        8192
    );
    assert_eq!(
        le_u32(bytes, VIRTIO_BLOCK_CONFIG_MAX_SECURE_ERASE_SEG_OFFSET),
        2
    );
    assert_eq!(
        le_u32(bytes, VIRTIO_BLOCK_CONFIG_SECURE_ERASE_ALIGNMENT_OFFSET),
        16
    );

    let device = VirtioPciDeviceConfigDevice::new(config);
    device
        .write_local(
            Address::new(VIRTIO_BLOCK_CONFIG_WRITEBACK_OFFSET),
            vec![0],
            ByteMask::from_bits(vec![true]).unwrap(),
        )
        .unwrap();
    assert_eq!(
        device.bytes()[VIRTIO_BLOCK_CONFIG_WRITEBACK_OFFSET as usize],
        0
    );
    assert!(matches!(
        device.write_local(
            Address::new(VIRTIO_BLOCK_CONFIG_CAPACITY_OFFSET),
            vec![0],
            ByteMask::from_bits(vec![true]).unwrap(),
        ),
        Err(MmioError::DeviceError { message, .. }) if message.contains("read-only")
    ));
    assert_eq!(
        device
            .read_local(
                Address::new(VIRTIO_BLOCK_CONFIG_CAPACITY_OFFSET),
                AccessSize::new(8).unwrap(),
            )
            .unwrap(),
        0x0123_4567_89ab_cdef_u64.to_le_bytes()
    );
}

#[test]
fn virtio_block_config_rejects_inconsistent_shapes() {
    assert!(matches!(
        VirtioBlockConfigSpec::new(0).device_config_spec(),
        Err(error) if error.to_string().contains("capacity")
    ));
    assert!(matches!(
        VirtioBlockConfigSpec::new(1)
            .with_block_size(1000)
            .device_config_spec(),
        Err(error) if error.to_string().contains("block size")
    ));
    assert!(matches!(
        VirtioBlockConfigSpec::new(1)
            .with_writeback(VirtioBlockCacheMode::WriteThrough)
            .device_config_spec(),
        Err(error) if error.to_string().contains("flush")
    ));
    assert!(matches!(
        VirtioBlockConfigSpec::new(1)
            .with_queues(0)
            .device_config_spec(),
        Err(error) if error.to_string().contains("queue")
    ));
    assert!(matches!(
        VirtioBlockGeometry::new(0, 16, 63),
        Err(error) if error.to_string().contains("geometry")
    ));
    assert!(matches!(
        VirtioBlockTopology::new(2, 4, 1, 1),
        Err(error) if error.to_string().contains("topology")
    ));
    assert!(matches!(
        VirtioBlockDiscardLimits::new(0, 1, 0),
        Err(error) if error.to_string().contains("discard")
    ));
    assert!(matches!(
        VirtioBlockWriteZeroesLimits::new(1, 0, false),
        Err(error) if error.to_string().contains("write zeroes")
    ));
    assert!(matches!(
        VirtioBlockSecureEraseLimits::new(1, 0, 0),
        Err(error) if error.to_string().contains("secure erase")
    ));
}

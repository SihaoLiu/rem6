use rem6_memory::{AccessSize, Address, AddressRange};
use rem6_virtio::{
    VirtioPciBarIndex, VirtioPciCapabilityOffset, VirtioPciSharedMemoryCap64Fields,
    VirtioPciSharedMemoryCapabilities, VirtioPciSharedMemoryCapabilityEntry,
    VirtioPciSharedMemoryId, VirtioPciSharedMemoryRegion, VirtioPciSharedMemoryRegionSpec,
    VirtioPciSharedMemoryRegistry,
};

fn bar(index: u8) -> VirtioPciBarIndex {
    VirtioPciBarIndex::new(index).unwrap()
}

fn region_id(value: u8) -> VirtioPciSharedMemoryId {
    VirtioPciSharedMemoryId::new(value)
}

#[test]
fn virtio_pci_shared_memory_registry_exports_checked_cap64_regions() {
    let region_a =
        VirtioPciSharedMemoryRegionSpec::new(region_id(7), bar(2), 0x100, 0x1000).unwrap();
    let region_b =
        VirtioPciSharedMemoryRegionSpec::new(region_id(9), bar(4), 0x1_0000_0000, 0x2000).unwrap();

    let registry = VirtioPciSharedMemoryRegistry::new(
        [
            (bar(2), AccessSize::new(0x4000).unwrap()),
            (bar(4), AccessSize::new(0x1_0000_4000).unwrap()),
        ],
        [region_b, region_a],
    )
    .unwrap();

    assert_eq!(
        registry.regions(),
        &[
            VirtioPciSharedMemoryRegion::new(region_id(7), bar(2), 0x100, 0x1000).unwrap(),
            VirtioPciSharedMemoryRegion::new(region_id(9), bar(4), 0x1_0000_0000, 0x2000).unwrap(),
        ]
    );
    assert_eq!(
        registry.region(region_id(7)).unwrap().range(),
        AddressRange::new(Address::new(0x100), AccessSize::new(0x1000).unwrap()).unwrap()
    );
    assert_eq!(
        registry.regions_for_bar(bar(2)),
        vec![VirtioPciSharedMemoryRegion::new(region_id(7), bar(2), 0x100, 0x1000).unwrap()]
    );
    assert_eq!(registry.regions_for_bar(bar(1)), Vec::new());
    assert_eq!(
        registry.cap64_fields(region_id(9)).unwrap(),
        VirtioPciSharedMemoryCap64Fields::new(bar(4), region_id(9), 0, 1, 0x2000, 0)
    );
}

#[test]
fn virtio_pci_shared_memory_capabilities_export_cap64_bytes() {
    let registry = VirtioPciSharedMemoryRegistry::new(
        [
            (bar(2), AccessSize::new(0x1000).unwrap()),
            (bar(4), AccessSize::new(0x3_0000_0100).unwrap()),
        ],
        [
            VirtioPciSharedMemoryRegionSpec::new(
                region_id(7),
                bar(4),
                0x1_0000_0008,
                0x2_0000_0010,
            )
            .unwrap(),
            VirtioPciSharedMemoryRegionSpec::new(region_id(3), bar(2), 0x10, 0x20).unwrap(),
        ],
    )
    .unwrap();

    let capabilities = VirtioPciSharedMemoryCapabilities::new(
        VirtioPciCapabilityOffset::new(0x40).unwrap(),
        &registry,
    )
    .unwrap();

    assert_eq!(
        capabilities.first_offset(),
        Some(VirtioPciCapabilityOffset::new(0x40).unwrap())
    );
    assert_eq!(
        capabilities.entries(),
        &[
            VirtioPciSharedMemoryCapabilityEntry::new(
                VirtioPciCapabilityOffset::new(0x40).unwrap(),
                Some(VirtioPciCapabilityOffset::new(0x58).unwrap()),
                registry.region(region_id(3)).copied().unwrap(),
            ),
            VirtioPciSharedMemoryCapabilityEntry::new(
                VirtioPciCapabilityOffset::new(0x58).unwrap(),
                None,
                registry.region(region_id(7)).copied().unwrap(),
            ),
        ]
    );
    assert_eq!(
        capabilities.entries()[0].bytes(),
        [
            0x09, 0x58, 0x18, 0x08, 0x02, 0x03, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x20, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]
    );
    assert_eq!(
        capabilities.entries()[1].bytes(),
        [
            0x09, 0x00, 0x18, 0x08, 0x04, 0x07, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x10, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
        ]
    );
    assert_eq!(
        capabilities
            .entry_for_region(region_id(7))
            .unwrap()
            .offset(),
        VirtioPciCapabilityOffset::new(0x58).unwrap()
    );
    assert_eq!(
        capabilities
            .entry_at(VirtioPciCapabilityOffset::new(0x40).unwrap())
            .unwrap()
            .region()
            .id(),
        region_id(3)
    );

    let mut config = [0xaa; 0x100];
    capabilities.write_into_config(&mut config).unwrap();
    assert_eq!(&config[0x40..0x58], &capabilities.entries()[0].bytes());
    assert_eq!(&config[0x58..0x70], &capabilities.entries()[1].bytes());
    assert_eq!(config[0x3f], 0xaa);
    assert_eq!(config[0x70], 0xaa);

    let image = capabilities.config_image();
    assert_eq!(&image[0x40..0x58], &capabilities.entries()[0].bytes());
    assert_eq!(&image[0x58..0x70], &capabilities.entries()[1].bytes());
    assert_eq!(image[0x3f], 0);
    assert_eq!(image[0x70], 0);
}

#[test]
fn virtio_pci_shared_memory_registry_rejects_invalid_regions() {
    assert!(VirtioPciBarIndex::new(6).is_none());
    assert!(matches!(
        VirtioPciSharedMemoryRegionSpec::new(region_id(1), bar(0), 0, 0),
        Err(error) if error.to_string().contains("length")
    ));
    assert!(matches!(
        VirtioPciSharedMemoryRegion::new(region_id(2), bar(1), u64::MAX - 1, 8),
        Err(error) if error.to_string().contains("overflows")
    ));
    assert!(matches!(
        VirtioPciSharedMemoryRegistry::new(
            [(bar(1), AccessSize::new(0x1000).unwrap())],
            [VirtioPciSharedMemoryRegionSpec::new(region_id(1), bar(2), 0, 0x100).unwrap()],
        ),
        Err(error) if error.to_string().contains("BAR 2")
    ));
    assert!(matches!(
        VirtioPciSharedMemoryRegistry::new(
            [
                (bar(2), AccessSize::new(0x1000).unwrap()),
                (bar(2), AccessSize::new(0x2000).unwrap()),
            ],
            [VirtioPciSharedMemoryRegionSpec::new(region_id(1), bar(2), 0, 0x100).unwrap()],
        ),
        Err(error) if error.to_string().contains("declared more than once")
    ));
    assert!(matches!(
        VirtioPciSharedMemoryRegistry::new(
            [(bar(2), AccessSize::new(0x1000).unwrap())],
            [
                VirtioPciSharedMemoryRegionSpec::new(region_id(3), bar(2), 0, 0x100).unwrap(),
                VirtioPciSharedMemoryRegionSpec::new(region_id(3), bar(2), 0x200, 0x100)
                    .unwrap(),
            ],
        ),
        Err(error) if error.to_string().contains("id 3")
    ));
    assert!(matches!(
        VirtioPciSharedMemoryRegistry::new(
            [(bar(2), AccessSize::new(0x1000).unwrap())],
            [VirtioPciSharedMemoryRegionSpec::new(region_id(4), bar(2), 0x800, 0x900)
                .unwrap()],
        ),
        Err(error) if error.to_string().contains("contained within BAR")
    ));
    assert!(matches!(
        VirtioPciSharedMemoryRegistry::new(
            [(bar(2), AccessSize::new(0x2000).unwrap())],
            [
                VirtioPciSharedMemoryRegionSpec::new(region_id(4), bar(2), 0x800, 0x400)
                    .unwrap(),
                VirtioPciSharedMemoryRegionSpec::new(region_id(5), bar(2), 0xa00, 0x400)
                    .unwrap(),
            ],
        ),
        Err(error) if error.to_string().contains("overlaps")
    ));
}

#[test]
fn virtio_pci_shared_memory_capabilities_reject_invalid_config_placement() {
    assert!(VirtioPciCapabilityOffset::new(0x3c).is_none());
    assert!(VirtioPciCapabilityOffset::new(0x42).is_none());

    let registry = VirtioPciSharedMemoryRegistry::new(
        [(bar(2), AccessSize::new(0x1000).unwrap())],
        [VirtioPciSharedMemoryRegionSpec::new(region_id(1), bar(2), 0, 0x100).unwrap()],
    )
    .unwrap();
    assert!(matches!(
        VirtioPciSharedMemoryCapabilities::new(
            VirtioPciCapabilityOffset::new(0xec).unwrap(),
            &registry,
        ),
        Err(error) if error.to_string().contains("configuration space")
    ));

    let crowded = VirtioPciSharedMemoryRegistry::new(
        [(bar(2), AccessSize::new(0x1000).unwrap())],
        (0..9).map(|index| {
            VirtioPciSharedMemoryRegionSpec::new(
                region_id(index),
                bar(2),
                u64::from(index) * 0x100,
                0x80,
            )
            .unwrap()
        }),
    )
    .unwrap();
    assert!(matches!(
        VirtioPciSharedMemoryCapabilities::new(
            VirtioPciCapabilityOffset::new(0x40).unwrap(),
            &crowded,
        ),
        Err(error) if error.to_string().contains("configuration space")
    ));
    assert!(matches!(
        VirtioPciSharedMemoryCapabilities::new(
            VirtioPciCapabilityOffset::new(0x40).unwrap(),
            &registry,
        )
        .unwrap()
        .write_into_config(&mut [0; 0x57]),
        Err(error) if error.to_string().contains("configuration buffer")
    ));
}

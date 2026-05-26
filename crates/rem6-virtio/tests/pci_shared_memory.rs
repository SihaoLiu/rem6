use rem6_memory::{AccessSize, Address, AddressRange};
use rem6_virtio::{
    VirtioPciBarIndex, VirtioPciSharedMemoryCap64Fields, VirtioPciSharedMemoryId,
    VirtioPciSharedMemoryRegion, VirtioPciSharedMemoryRegionSpec, VirtioPciSharedMemoryRegistry,
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

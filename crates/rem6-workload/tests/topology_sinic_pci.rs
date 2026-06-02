use rem6_boot::BootImage;
use rem6_memory::{AccessSize, Address};
use rem6_workload::{
    WorkloadError, WorkloadHostPlacement, WorkloadId, WorkloadManifest, WorkloadMemoryRoute,
    WorkloadMemoryTarget, WorkloadResource, WorkloadResourceId, WorkloadResourceKind,
    WorkloadRouteId, WorkloadSinicPciDevice, WorkloadSinicPciTopologyError, WorkloadTopology,
};

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

fn resource_id(value: &str) -> WorkloadResourceId {
    WorkloadResourceId::new(value).unwrap()
}

fn route_id(value: &str) -> WorkloadRouteId {
    WorkloadRouteId::new(value).unwrap()
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), vec![0x13, 0x05, 0x00, 0x00])
        .unwrap()
}

fn kernel_resource() -> WorkloadResource {
    WorkloadResource::new(
        resource_id("kernel"),
        WorkloadResourceKind::Kernel,
        "sha256:kernel",
        "resources/kernel.elf",
    )
    .unwrap()
}

fn base_topology() -> WorkloadTopology {
    WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 41).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                16,
                rem6_memory::AddressRange::new(
                    Address::new(0x8000),
                    AccessSize::new(0x2000).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("sinic0.mmio"),
                "cpu0.dmem",
                0,
                "sinic0.mmio",
                2,
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("sinic1.mmio"),
                "cpu1.dmem",
                1,
                "sinic1.mmio",
                2,
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap()
}

fn sinic_device(
    nic: u32,
    pci_device: u8,
    bar_base: u64,
    endpoint: &str,
    route: &str,
) -> WorkloadSinicPciDevice {
    WorkloadSinicPciDevice::new(
        nic,
        2,
        0,
        pci_device,
        0,
        Address::new(bar_base),
        endpoint,
        route_id(route),
        0x1293 + nic,
    )
    .unwrap()
}

#[test]
fn workload_topology_records_sinic_pci_devices_in_stable_order() {
    let topology = base_topology()
        .add_sinic_pci_device(sinic_device(1, 2, 0x30000, "sinic1.mmio", "sinic1.mmio"))
        .unwrap()
        .add_sinic_pci_device(sinic_device(0, 1, 0x20000, "sinic0.mmio", "sinic0.mmio"))
        .unwrap();

    let devices = topology.sinic_pci_devices();
    assert_eq!(
        devices
            .iter()
            .map(|device| device.nic())
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(devices[0].partition(), 2);
    assert_eq!(devices[0].pci_bus(), 0);
    assert_eq!(devices[0].pci_device(), 1);
    assert_eq!(devices[0].pci_function(), 0);
    assert_eq!(devices[0].bar_base(), Address::new(0x20000));
    assert_eq!(devices[0].mmio_endpoint(), "sinic0.mmio");
    assert_eq!(devices[0].mmio_route(), &route_id("sinic0.mmio"));
    assert_eq!(devices[0].interrupt_source(), 0x1293);
}

#[test]
fn workload_topology_rejects_invalid_sinic_pci_declarations() {
    let missing_route = base_topology()
        .add_sinic_pci_device(sinic_device(0, 1, 0x20000, "sinic0.mmio", "missing"))
        .unwrap_err();
    assert_eq!(
        missing_route,
        WorkloadError::SinicPciTopology(WorkloadSinicPciTopologyError::MissingMmioRoute {
            nic: 0,
            route: route_id("missing"),
        })
    );

    let wrong_partition = base_topology()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("sinic0.wrong-partition"),
                "cpu0.dmem",
                0,
                "sinic0.mmio",
                1,
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap()
        .add_sinic_pci_device(sinic_device(
            0,
            1,
            0x20000,
            "sinic0.mmio",
            "sinic0.wrong-partition",
        ))
        .unwrap_err();
    assert_eq!(
        wrong_partition,
        WorkloadError::SinicPciTopology(WorkloadSinicPciTopologyError::MmioRouteTargetMismatch {
            nic: 0,
            route: route_id("sinic0.wrong-partition"),
            expected: 2,
            actual: 1,
        })
    );

    let wrong_endpoint = base_topology()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("sinic0.wrong-endpoint"),
                "cpu0.dmem",
                0,
                "sinic0.other-mmio",
                2,
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap()
        .add_sinic_pci_device(sinic_device(
            0,
            1,
            0x20000,
            "sinic0.mmio",
            "sinic0.wrong-endpoint",
        ))
        .unwrap_err();
    assert_eq!(
        wrong_endpoint,
        WorkloadError::SinicPciTopology(WorkloadSinicPciTopologyError::MmioRouteEndpointMismatch {
            nic: 0,
            route: route_id("sinic0.wrong-endpoint"),
            expected: "sinic0.mmio".to_string(),
            actual: "sinic0.other-mmio".to_string(),
        })
    );

    let topology = base_topology()
        .add_sinic_pci_device(sinic_device(0, 1, 0x20000, "sinic0.mmio", "sinic0.mmio"))
        .unwrap();
    let duplicate_nic = topology
        .clone()
        .add_sinic_pci_device(sinic_device(0, 2, 0x30000, "sinic1.mmio", "sinic1.mmio"))
        .unwrap_err();
    assert_eq!(
        duplicate_nic,
        WorkloadError::SinicPciTopology(WorkloadSinicPciTopologyError::DuplicateDevice { nic: 0 })
    );

    let duplicate_function = topology
        .add_sinic_pci_device(sinic_device(1, 1, 0x30000, "sinic1.mmio", "sinic1.mmio"))
        .unwrap_err();
    assert_eq!(
        duplicate_function,
        WorkloadError::SinicPciTopology(WorkloadSinicPciTopologyError::DuplicateFunction {
            nic: 1,
            existing_nic: 0,
            bus: 0,
            device: 1,
            function: 0,
        })
    );

    let misaligned_bar = base_topology()
        .add_sinic_pci_device(sinic_device(0, 1, 0x20008, "sinic0.mmio", "sinic0.mmio"))
        .unwrap_err();
    assert_eq!(
        misaligned_bar,
        WorkloadError::SinicPciTopology(WorkloadSinicPciTopologyError::BarBaseMisaligned {
            nic: 0,
            bar_base: Address::new(0x20008),
            alignment_bytes: WorkloadSinicPciDevice::BAR_BYTES,
        })
    );
}

#[test]
fn workload_topology_rejects_overlapping_sinic_pci_bar_ranges() {
    let duplicate_bar = base_topology()
        .add_sinic_pci_device(sinic_device(0, 1, 0x20000, "sinic0.mmio", "sinic0.mmio"))
        .unwrap()
        .add_sinic_pci_device(sinic_device(1, 2, 0x20000, "sinic1.mmio", "sinic1.mmio"))
        .unwrap_err();

    assert_eq!(
        duplicate_bar,
        WorkloadError::SinicPciTopology(WorkloadSinicPciTopologyError::DuplicateBarBase {
            nic: 1,
            existing_nic: 0,
            bar_base: Address::new(0x20000),
        })
    );
}

#[test]
fn workload_manifest_identity_changes_with_sinic_pci_device_declarations() {
    let first = WorkloadManifest::builder(id("sinic-pci-identity"), boot_image())
        .with_topology(
            base_topology()
                .add_sinic_pci_device(sinic_device(0, 1, 0x20000, "sinic0.mmio", "sinic0.mmio"))
                .unwrap(),
        )
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let second = WorkloadManifest::builder(id("sinic-pci-identity"), boot_image())
        .with_topology(
            base_topology()
                .add_sinic_pci_device(sinic_device(0, 1, 0x30000, "sinic0.mmio", "sinic0.mmio"))
                .unwrap(),
        )
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    assert_ne!(first.identity(), second.identity());
}

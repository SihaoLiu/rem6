use rem6_boot::BootImage;
use rem6_memory::{AccessSize, Address};
use rem6_workload::{
    WorkloadDataCacheProtocol, WorkloadHostPlacement, WorkloadId, WorkloadManifest,
    WorkloadMemoryRoute, WorkloadMemoryTarget, WorkloadResource, WorkloadResourceId,
    WorkloadResourceKind, WorkloadRiscvCore, WorkloadRiscvDataCache, WorkloadRouteId,
    WorkloadTopology,
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

fn topology_with_data_cache(backing_route: WorkloadRouteId) -> WorkloadTopology {
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
            WorkloadMemoryRoute::new(route_id("cpu0.fetch"), "cpu0.ifetch", 0, "memory", 2, 2, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu0.data"), "cpu0.dmem", 0, "memory", 2, 2, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("dcache.backing.a"),
                "dcache.dir",
                2,
                "memory",
                2,
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("dcache.backing.b"),
                "dcache.dir",
                2,
                "memory",
                2,
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap()
        .add_riscv_core(
            WorkloadRiscvCore::new(
                0,
                0,
                7,
                Address::new(0x8000),
                "cpu0.ifetch",
                route_id("cpu0.fetch"),
            )
            .unwrap()
            .with_data("cpu0.dmem", route_id("cpu0.data"))
            .unwrap(),
        )
        .unwrap()
        .with_riscv_data_cache(
            WorkloadRiscvDataCache::new(
                WorkloadDataCacheProtocol::Msi,
                0,
                Address::new(0x9000),
                2,
                "dcache.dir",
                backing_route,
            )
            .unwrap(),
        )
        .unwrap()
}

#[test]
fn workload_manifest_identity_changes_with_data_cache_backing_route() {
    let first = WorkloadManifest::builder(id("cache-backing-identity"), boot_image())
        .with_topology(topology_with_data_cache(route_id("dcache.backing.a")))
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let second = WorkloadManifest::builder(id("cache-backing-identity"), boot_image())
        .with_topology(topology_with_data_cache(route_id("dcache.backing.b")))
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();

    assert_ne!(first.identity(), second.identity());
}

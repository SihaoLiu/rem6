use rem6_memory::{AccessSize, Address};
use rem6_workload::{
    WorkloadError, WorkloadHostPlacement, WorkloadMemoryRoute, WorkloadMemoryTarget,
    WorkloadRiscvCore, WorkloadRiscvDataCache, WorkloadRouteId, WorkloadTopology,
};

fn route_id(value: &str) -> WorkloadRouteId {
    WorkloadRouteId::new(value).unwrap()
}

fn topology_with_mismatched_core_routes() -> WorkloadTopology {
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
            WorkloadMemoryRoute::new(route_id("cpu1.data"), "cpu1.dmem", 1, "memory", 2, 2, 3)
                .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("cpu1.fetch"), "cpu1.ifetch", 1, "memory", 2, 2, 3)
                .unwrap(),
        )
        .unwrap()
}

fn topology_with_same_partition_mismatched_endpoints() -> WorkloadTopology {
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
}

#[test]
fn workload_topology_rejects_riscv_core_routes_from_different_partition() {
    let topology = topology_with_mismatched_core_routes();

    assert_eq!(
        topology
            .clone()
            .add_riscv_core(
                WorkloadRiscvCore::new(
                    1,
                    1,
                    8,
                    Address::new(0x9000),
                    "cpu1.ifetch",
                    route_id("cpu0.fetch"),
                )
                .unwrap(),
            )
            .unwrap_err(),
        WorkloadError::CoreFetchRouteSourceMismatch {
            cpu: 1,
            route: route_id("cpu0.fetch"),
            expected: 1,
            actual: 0,
        }
    );

    assert_eq!(
        topology
            .add_riscv_core(
                WorkloadRiscvCore::new(
                    1,
                    1,
                    8,
                    Address::new(0x9000),
                    "cpu1.ifetch",
                    route_id("cpu1.fetch"),
                )
                .unwrap()
                .with_data("cpu1.dmem", route_id("cpu0.fetch"))
                .unwrap(),
            )
            .unwrap_err(),
        WorkloadError::CoreDataRouteSourceMismatch {
            cpu: 1,
            route: route_id("cpu0.fetch"),
            expected: 1,
            actual: 0,
        }
    );
}

#[test]
fn workload_topology_rejects_riscv_core_routes_from_different_endpoint() {
    let topology = topology_with_same_partition_mismatched_endpoints();

    assert_eq!(
        topology
            .clone()
            .add_riscv_core(
                WorkloadRiscvCore::new(
                    0,
                    0,
                    7,
                    Address::new(0x8000),
                    "cpu0.wrong-ifetch",
                    route_id("cpu0.fetch"),
                )
                .unwrap(),
            )
            .unwrap_err(),
        WorkloadError::CoreFetchRouteEndpointMismatch {
            cpu: 0,
            route: route_id("cpu0.fetch"),
            expected: "cpu0.wrong-ifetch".to_string(),
            actual: "cpu0.ifetch".to_string(),
        }
    );

    assert_eq!(
        topology
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
                .with_data("cpu0.wrong-dmem", route_id("cpu0.data"))
                .unwrap(),
            )
            .unwrap_err(),
        WorkloadError::CoreDataRouteEndpointMismatch {
            cpu: 0,
            route: route_id("cpu0.data"),
            expected: "cpu0.wrong-dmem".to_string(),
            actual: "cpu0.dmem".to_string(),
        }
    );
}

#[test]
fn workload_topology_rejects_riscv_data_cache_backing_route_mismatch() {
    let topology = topology_with_same_partition_mismatched_endpoints()
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
        .unwrap();

    assert_eq!(
        topology
            .clone()
            .with_riscv_data_cache(
                WorkloadRiscvDataCache::new(
                    rem6_workload::WorkloadDataCacheProtocol::Msi,
                    0,
                    Address::new(0x9000),
                    2,
                    "dcache.dir",
                    route_id("dcache.missing"),
                )
                .unwrap(),
            )
            .unwrap_err(),
        WorkloadError::MissingDataCacheBackingRoute {
            route: route_id("dcache.missing"),
        }
    );

    let wrong_partition = topology
        .clone()
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("dcache.backing"),
                "dcache.dir",
                1,
                "memory",
                2,
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .with_riscv_data_cache(
            WorkloadRiscvDataCache::new(
                rem6_workload::WorkloadDataCacheProtocol::Msi,
                0,
                Address::new(0x9000),
                2,
                "dcache.dir",
                route_id("dcache.backing"),
            )
            .unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        wrong_partition,
        WorkloadError::DataCacheBackingRouteSourceMismatch {
            route: route_id("dcache.backing"),
            expected: 2,
            actual: 1,
        }
    );

    let wrong_endpoint = topology
        .add_memory_route(
            WorkloadMemoryRoute::new(
                route_id("dcache.backing"),
                "dcache.other-dir",
                2,
                "memory",
                2,
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap()
        .with_riscv_data_cache(
            WorkloadRiscvDataCache::new(
                rem6_workload::WorkloadDataCacheProtocol::Msi,
                0,
                Address::new(0x9000),
                2,
                "dcache.dir",
                route_id("dcache.backing"),
            )
            .unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        wrong_endpoint,
        WorkloadError::DataCacheBackingRouteEndpointMismatch {
            route: route_id("dcache.backing"),
            expected: "dcache.dir".to_string(),
            actual: "dcache.other-dir".to_string(),
        }
    );
}

use rem6_memory::{AccessSize, Address};
use rem6_workload::{
    WorkloadError, WorkloadHostPlacement, WorkloadMemoryRoute, WorkloadMemoryTarget,
    WorkloadRiscvCore, WorkloadRouteId, WorkloadTopology,
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
                    route_id("cpu1.data"),
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

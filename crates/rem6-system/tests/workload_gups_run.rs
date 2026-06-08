use rem6_boot::BootImage;
use rem6_memory::{AccessSize, Address, AddressRange};
use rem6_system::run_workload_gups_plan;
use rem6_workload::{
    WorkloadExpectedGupsRunSummary, WorkloadGupsRun, WorkloadGupsRunSummary, WorkloadHostPlacement,
    WorkloadId, WorkloadManifest, WorkloadMemoryRoute, WorkloadMemoryTarget, WorkloadReplayPlan,
    WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadRouteHop, WorkloadRouteId,
    WorkloadTopology,
};

fn workload_id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

fn route_id(value: &str) -> WorkloadRouteId {
    WorkloadRouteId::new(value).unwrap()
}

fn resource_id(value: &str) -> WorkloadResourceId {
    WorkloadResourceId::new(value).unwrap()
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

fn topology() -> WorkloadTopology {
    WorkloadTopology::new(2, 2, 2, WorkloadHostPlacement::new(0, 2, 41).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                16,
                AddressRange::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new(route_id("gups.data"), "gups", 0, "memory", 1, 2, 2).unwrap(),
        )
        .unwrap()
}

fn manifest() -> WorkloadManifest {
    let run = WorkloadGupsRun::new(route_id("gups.data"), 0, Address::new(0x1000), 8, 2)
        .unwrap()
        .with_rng_state(0)
        .with_maximum_final_tick(20);
    let expected = WorkloadExpectedGupsRunSummary::new(route_id("gups.data"))
        .with_maximum_final_tick(20)
        .with_minimum_scheduled_count(4)
        .with_minimum_response_count(4)
        .with_minimum_completed_response_count(4)
        .with_minimum_read_response_count(2)
        .with_minimum_write_response_count(2)
        .with_minimum_response_data_byte_count(16)
        .with_minimum_memory_trace_event_count(12);

    WorkloadManifest::builder(workload_id("workload-gups"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .with_topology(topology())
        .add_gups_run(run)
        .unwrap()
        .add_expected_gups_run_summary(expected)
        .unwrap()
        .build()
        .unwrap()
}

fn multihop_topology() -> WorkloadTopology {
    WorkloadTopology::new(3, 2, 3, WorkloadHostPlacement::new(0, 2, 41).unwrap())
        .unwrap()
        .add_memory_target(
            WorkloadMemoryTarget::new(
                0,
                16,
                AddressRange::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
        .add_memory_route(
            WorkloadMemoryRoute::new_path(
                route_id("gups.multihop"),
                "gups",
                0,
                [
                    WorkloadRouteHop::new("crossbar", 1, 2, 2).unwrap(),
                    WorkloadRouteHop::new("memory", 2, 3, 3).unwrap(),
                ],
            )
            .unwrap(),
        )
        .unwrap()
}

fn multihop_manifest() -> WorkloadManifest {
    let run = WorkloadGupsRun::new(route_id("gups.multihop"), 0, Address::new(0x1000), 8, 2)
        .unwrap()
        .with_rng_state(0)
        .with_maximum_final_tick(44);
    let expected = WorkloadExpectedGupsRunSummary::new(route_id("gups.multihop"))
        .with_maximum_final_tick(44)
        .with_minimum_scheduled_count(4)
        .with_minimum_response_count(4)
        .with_minimum_completed_response_count(4)
        .with_minimum_read_response_count(2)
        .with_minimum_write_response_count(2)
        .with_minimum_response_data_byte_count(16)
        .with_minimum_memory_trace_event_count(20);

    WorkloadManifest::builder(workload_id("workload-gups-multihop"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .with_topology(multihop_topology())
        .add_gups_run(run)
        .unwrap()
        .add_expected_gups_run_summary(expected)
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn workload_gups_run_declaration_executes_controller_transport() {
    let plan = WorkloadReplayPlan::from_manifest(&manifest()).unwrap();
    let result = run_workload_gups_plan(&plan).unwrap();

    assert_eq!(result.final_tick(), 20);
    assert_eq!(
        result.gups_run_summaries(),
        &[WorkloadGupsRunSummary::new(route_id("gups.data"), 20)
            .with_scheduled_count(4)
            .with_response_count(4)
            .with_completed_response_count(4)
            .with_read_response_count(2)
            .with_write_response_count(2)
            .with_response_data_byte_count(16)
            .with_memory_trace_event_count(12)],
    );
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_gups_run_declaration_preserves_multihop_topology_route() {
    let plan = WorkloadReplayPlan::from_manifest(&multihop_manifest()).unwrap();
    let result = run_workload_gups_plan(&plan).unwrap();

    assert_eq!(result.final_tick(), 44);
    assert_eq!(
        result.gups_run_summaries(),
        &[WorkloadGupsRunSummary::new(route_id("gups.multihop"), 44)
            .with_scheduled_count(4)
            .with_response_count(4)
            .with_completed_response_count(4)
            .with_read_response_count(2)
            .with_write_response_count(2)
            .with_response_data_byte_count(16)
            .with_memory_trace_event_count(20)],
    );
    plan.verify_result(&result).unwrap();
}

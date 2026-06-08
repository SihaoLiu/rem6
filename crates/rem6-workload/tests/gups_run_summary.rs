use rem6_boot::BootImage;
use rem6_memory::{AccessSize, Address, AddressRange};
use rem6_workload::{
    WorkloadError, WorkloadExpectedGupsRunSummary, WorkloadGupsRun, WorkloadGupsRunSummary,
    WorkloadGupsRunSummaryExpectationError, WorkloadHostPlacement, WorkloadId, WorkloadMemoryRoute,
    WorkloadMemoryTarget, WorkloadReplayPlan, WorkloadResource, WorkloadResourceId,
    WorkloadResourceKind, WorkloadResult, WorkloadRouteId, WorkloadTopology,
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

fn expected_gups_summary(route: &str) -> WorkloadExpectedGupsRunSummary {
    WorkloadExpectedGupsRunSummary::new(route_id(route))
        .with_maximum_final_tick(32)
        .with_minimum_scheduled_count(4)
        .with_minimum_response_count(4)
        .with_minimum_completed_response_count(4)
        .with_minimum_read_response_count(2)
        .with_minimum_write_response_count(2)
        .with_minimum_response_data_byte_count(16)
        .with_minimum_memory_trace_event_count(8)
}

fn actual_gups_summary(route: &str) -> WorkloadGupsRunSummary {
    WorkloadGupsRunSummary::new(route_id(route), 24)
        .with_scheduled_count(4)
        .with_response_count(4)
        .with_completed_response_count(4)
        .with_read_response_count(2)
        .with_write_response_count(2)
        .with_response_data_byte_count(16)
        .with_memory_trace_event_count(8)
}

fn gups_run(route: &str, memory_start: u64, updates: u64) -> WorkloadGupsRun {
    WorkloadGupsRun::new(route_id(route), 0, Address::new(memory_start), 8, updates)
        .unwrap()
        .with_rng_state(0)
        .with_maximum_final_tick(32)
}

fn gups_topology(route: &str) -> WorkloadTopology {
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
            WorkloadMemoryRoute::new(route_id(route), "gups", 0, "memory", 1, 3, 5).unwrap(),
        )
        .unwrap()
}

fn manifest_with_gups_summary(
    expected: WorkloadExpectedGupsRunSummary,
) -> rem6_workload::WorkloadManifest {
    rem6_workload::WorkloadManifest::builder(id("manifest-gups-identity"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_expected_gups_run_summary(expected)
        .unwrap()
        .build()
        .unwrap()
}

fn manifest_with_gups_run(run: WorkloadGupsRun) -> rem6_workload::WorkloadManifest {
    rem6_workload::WorkloadManifest::builder(id("manifest-gups-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .with_topology(gups_topology(run.route().as_str()))
        .add_gups_run(run)
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn workload_manifest_records_gups_run_declarations() {
    let gups_a = gups_run("gups.a", 0x1000, 2);
    let gups_b = gups_run("gups.b", 0x1010, 1);
    let manifest = rem6_workload::WorkloadManifest::builder(id("manifest-gups-run"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .with_topology(gups_topology("gups.a"))
        .add_gups_run(gups_b.clone())
        .unwrap()
        .add_gups_run(gups_a.clone())
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(manifest.gups_runs(), &[gups_a.clone(), gups_b.clone()]);
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(plan.gups_runs(), manifest.gups_runs());
}

#[test]
fn workload_manifest_identity_changes_with_gups_run_declarations() {
    let base = manifest_with_gups_run(gups_run("gups.identity", 0x1000, 2));
    let route_changed = manifest_with_gups_run(gups_run("gups.identity.alt", 0x1000, 2));
    let start_changed = manifest_with_gups_run(gups_run("gups.identity", 0x1010, 2));
    let updates_changed = manifest_with_gups_run(gups_run("gups.identity", 0x1000, 3));

    assert_ne!(base.identity(), route_changed.identity());
    assert_ne!(base.identity(), start_changed.identity());
    assert_ne!(base.identity(), updates_changed.identity());
}

#[test]
fn workload_manifest_records_gups_run_summary_expectations() {
    let gups_a = expected_gups_summary("gups.a");
    let gups_b = expected_gups_summary("gups.b")
        .with_minimum_retry_response_count(1)
        .with_minimum_completed_response_count(3);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-gups-summary"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_gups_run_summary(gups_b.clone())
            .unwrap()
            .add_expected_gups_run_summary(gups_a.clone())
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_gups_run_summaries(),
        &[gups_a.clone(), gups_b.clone()],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_gups_run_summaries(),
        manifest.expected_gups_run_summaries(),
    );

    let result = WorkloadResult::new(plan.manifest_identity(), 40)
        .with_gups_run_summary(
            actual_gups_summary("gups.b")
                .with_retry_response_count(1)
                .with_completed_response_count(3),
        )
        .with_gups_run_summary(actual_gups_summary("gups.a"));
    assert_eq!(
        result.gups_run_summaries(),
        &[
            actual_gups_summary("gups.a"),
            actual_gups_summary("gups.b")
                .with_retry_response_count(1)
                .with_completed_response_count(3),
        ],
    );

    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_gups_run_summary_expectations() {
    let base = manifest_with_gups_summary(expected_gups_summary("gups.identity"));
    let route_changed = manifest_with_gups_summary(expected_gups_summary("gups.identity.alt"));
    let scheduled_changed = manifest_with_gups_summary(
        expected_gups_summary("gups.identity").with_minimum_scheduled_count(5),
    );
    let maximum_tick_changed = manifest_with_gups_summary(
        expected_gups_summary("gups.identity").with_maximum_final_tick(33),
    );
    let maximum_tick_omitted = manifest_with_gups_summary(
        WorkloadExpectedGupsRunSummary::new(route_id("gups.identity"))
            .with_minimum_scheduled_count(4)
            .with_minimum_response_count(4)
            .with_minimum_completed_response_count(4)
            .with_minimum_read_response_count(2)
            .with_minimum_write_response_count(2)
            .with_minimum_response_data_byte_count(16)
            .with_minimum_memory_trace_event_count(8),
    );
    let maximum_tick_unbounded = manifest_with_gups_summary(
        WorkloadExpectedGupsRunSummary::new(route_id("gups.identity"))
            .with_maximum_final_tick(u64::MAX)
            .with_minimum_scheduled_count(4)
            .with_minimum_response_count(4)
            .with_minimum_completed_response_count(4)
            .with_minimum_read_response_count(2)
            .with_minimum_write_response_count(2)
            .with_minimum_response_data_byte_count(16)
            .with_minimum_memory_trace_event_count(8),
    );

    assert_ne!(base.identity(), route_changed.identity());
    assert_ne!(base.identity(), scheduled_changed.identity());
    assert_ne!(base.identity(), maximum_tick_changed.identity());
    assert_ne!(
        maximum_tick_omitted.identity(),
        maximum_tick_unbounded.identity()
    );
}

#[test]
fn workload_manifest_rejects_duplicate_gups_run_summary_expectations() {
    let expected = expected_gups_summary("gups.duplicate");
    let error =
        rem6_workload::WorkloadManifest::builder(id("manifest-gups-duplicate"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_gups_run_summary(expected.clone())
            .unwrap()
            .add_expected_gups_run_summary(expected)
            .unwrap_err();

    assert_eq!(
        error,
        WorkloadError::GupsRunSummaryExpectation(Box::new(
            WorkloadGupsRunSummaryExpectationError::DuplicateExpected {
                route: route_id("gups.duplicate"),
            },
        )),
    );
}

#[test]
fn workload_replay_plan_rejects_missing_gups_run_summary() {
    let expected = expected_gups_summary("gups.missing");
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-gups-missing"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_gups_run_summary(expected.clone())
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let result = WorkloadResult::new(plan.manifest_identity(), 40);

    let error = plan.verify_result(&result).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::GupsRunSummaryExpectation(Box::new(
            WorkloadGupsRunSummaryExpectationError::Missing(expected),
        )),
    );
}

#[test]
fn workload_replay_plan_rejects_gups_summary_after_result_final_tick() {
    let expected = expected_gups_summary("gups.after-final");
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-gups-after-final"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_gups_run_summary(expected)
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let actual = actual_gups_summary("gups.after-final");
    let result =
        WorkloadResult::new(plan.manifest_identity(), 12).with_gups_run_summary(actual.clone());

    let error = plan.verify_result(&result).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::GupsRunSummaryExpectation(Box::new(
            WorkloadGupsRunSummaryExpectationError::AfterResultFinalTick {
                actual,
                result_final_tick: 12,
            },
        )),
    );
}

#[test]
fn workload_replay_plan_rejects_underreported_gups_run_summary() {
    let expected = expected_gups_summary("gups.underreported");
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-gups-underreported"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_gups_run_summary(expected.clone())
            .unwrap()
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let actual = actual_gups_summary("gups.underreported")
        .with_scheduled_count(3)
        .with_response_data_byte_count(8);
    let result = WorkloadResult::new(plan.manifest_identity(), 40).with_gups_run_summary(actual);

    let error = plan.verify_result(&result).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::GupsRunSummaryExpectation(Box::new(
            WorkloadGupsRunSummaryExpectationError::OutsideBounds {
                expected,
                actual: actual_gups_summary("gups.underreported")
                    .with_scheduled_count(3)
                    .with_response_data_byte_count(8),
            },
        )),
    );
}

use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedTrafficTraceReplaySummary, WorkloadId, WorkloadReplayPlan,
    WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadResult, WorkloadRouteId,
    WorkloadTrafficTraceReplaySummary, WorkloadTrafficTraceReplaySummaryExpectationError,
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

fn manifest_with_expected(
    manifest_id: &str,
    expected: WorkloadExpectedTrafficTraceReplaySummary,
) -> rem6_workload::WorkloadManifest {
    rem6_workload::WorkloadManifest::builder(id(manifest_id), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_expected_traffic_trace_replay_summary(expected)
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn workload_manifest_records_response_invalidate_policy_expectations() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.policy"))
        .with_minimum_trace_invalidate_response_count(1)
        .with_minimum_trace_clean_response_count(1);
    let manifest = manifest_with_expected("manifest-response-policy", expected.clone());
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0].minimum_trace_invalidate_response_count(),
        1
    );
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0].minimum_trace_clean_response_count(),
        1
    );

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.policy"), 4)
        .with_trace_invalidate_response_count(1)
        .with_trace_clean_response_count(1);
    assert_eq!(actual.trace_invalidate_response_count(), 1);
    assert_eq!(actual.trace_clean_response_count(), 1);

    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_traffic_trace_replay_summary(actual);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_underreported_trace_response_invalidate_policy() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.policy"))
        .with_minimum_trace_invalidate_response_count(1)
        .with_minimum_trace_clean_response_count(1);
    let manifest = manifest_with_expected("trace-response-policy-mismatch", expected.clone());
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.policy"), 2);
    let underreported = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_traffic_trace_replay_summary(actual.clone());
    let error = plan.verify_result(&underreported).unwrap_err();
    assert_eq!(
        error,
        WorkloadError::TrafficTraceReplaySummaryExpectation(Box::new(
            WorkloadTrafficTraceReplaySummaryExpectationError::BelowMinimum { expected, actual },
        )),
    );
    assert!(
        error.to_string().contains("trace invalidate responses 0/1"),
        "{error}"
    );
    assert!(
        error.to_string().contains("trace clean responses 0/1"),
        "{error}"
    );
}

#[test]
fn workload_manifest_identity_changes_with_response_invalidate_policy_expectation() {
    let generic = manifest_with_expected(
        "identity-response-policy",
        WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.policy")),
    );
    let invalidate = manifest_with_expected(
        "identity-response-policy",
        WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.policy"))
            .with_minimum_trace_invalidate_response_count(1),
    );
    let clean = manifest_with_expected(
        "identity-response-policy",
        WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.policy"))
            .with_minimum_trace_clean_response_count(1),
    );

    assert_ne!(generic.identity(), invalidate.identity());
    assert_ne!(generic.identity(), clean.identity());
    assert_ne!(invalidate.identity(), clean.identity());
}

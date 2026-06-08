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
fn workload_manifest_records_trace_error_expectations() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.error"))
        .with_minimum_scheduled_count(1)
        .with_minimum_memory_failure_count(1)
        .with_minimum_trace_error_count(1)
        .with_minimum_trace_data_cache_error_count(1)
        .with_minimum_trace_data_cache_write_error_count(1)
        .with_minimum_memory_failure_write_count(1);
    let manifest = manifest_with_expected("manifest-trace-error", expected.clone());

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries(),
        std::slice::from_ref(&expected),
    );

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.error"), 1)
        .with_memory_failure_count(1)
        .with_trace_error_count(1)
        .with_trace_data_cache_error_count(1)
        .with_trace_data_cache_write_error_count(1)
        .with_memory_failure_write_count(1);
    assert_eq!(actual.trace_data_cache_write_error_count(), 1);
    let result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_traffic_trace_replay_summary(actual.clone());
    assert_eq!(result.traffic_trace_replay_summaries(), &[actual]);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_records_memory_failure_kind_expectations() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.error.kind"))
        .with_minimum_memory_failure_count(6)
        .with_minimum_memory_failure_invalid_destination_count(1)
        .with_minimum_memory_failure_bad_address_count(1)
        .with_minimum_memory_failure_read_count(1)
        .with_minimum_memory_failure_write_count(1)
        .with_minimum_memory_failure_functional_read_count(1)
        .with_minimum_memory_failure_functional_write_count(1);
    let manifest = manifest_with_expected("manifest-memory-failure-kinds", expected.clone());

    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0]
            .minimum_memory_failure_invalid_destination_count(),
        1
    );
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0]
            .minimum_memory_failure_bad_address_count(),
        1
    );
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0].minimum_memory_failure_read_count(),
        1
    );
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0].minimum_memory_failure_write_count(),
        1
    );
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0]
            .minimum_memory_failure_functional_read_count(),
        1
    );
    assert_eq!(
        plan.expected_traffic_trace_replay_summaries()[0]
            .minimum_memory_failure_functional_write_count(),
        1
    );

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.error.kind"), 6)
        .with_memory_failure_count(6)
        .with_memory_failure_invalid_destination_count(1)
        .with_memory_failure_bad_address_count(1)
        .with_memory_failure_read_count(1)
        .with_memory_failure_write_count(1)
        .with_memory_failure_functional_read_count(1)
        .with_memory_failure_functional_write_count(1);
    assert_eq!(actual.memory_failure_invalid_destination_count(), 1);
    assert_eq!(actual.memory_failure_bad_address_count(), 1);
    assert_eq!(actual.memory_failure_read_count(), 1);
    assert_eq!(actual.memory_failure_write_count(), 1);
    assert_eq!(actual.memory_failure_functional_read_count(), 1);
    assert_eq!(actual.memory_failure_functional_write_count(), 1);

    let result =
        WorkloadResult::new(plan.manifest_identity(), 32).with_traffic_trace_replay_summary(actual);
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_replay_plan_rejects_underreported_memory_failure_kind_summary() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.error.kind"))
        .with_minimum_memory_failure_count(2)
        .with_minimum_memory_failure_write_count(2);
    let manifest = manifest_with_expected("memory-failure-kind-mismatch", expected.clone());
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.error.kind"), 2)
        .with_memory_failure_count(2)
        .with_memory_failure_write_count(1);
    let underreported = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_traffic_trace_replay_summary(actual.clone());
    assert_eq!(
        plan.verify_result(&underreported).unwrap_err(),
        WorkloadError::TrafficTraceReplaySummaryExpectation(Box::new(
            WorkloadTrafficTraceReplaySummaryExpectationError::BelowMinimum { expected, actual },
        )),
    );
}

#[test]
fn workload_manifest_identity_changes_with_memory_failure_kind_expectations() {
    let generic = manifest_with_expected(
        "identity-memory-failure-kind",
        WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.error"))
            .with_minimum_memory_failure_count(1),
    );
    let read = manifest_with_expected(
        "identity-memory-failure-kind",
        WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.error"))
            .with_minimum_memory_failure_count(1)
            .with_minimum_memory_failure_read_count(1),
    );
    let write = manifest_with_expected(
        "identity-memory-failure-kind",
        WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.error"))
            .with_minimum_memory_failure_count(1)
            .with_minimum_memory_failure_write_count(1),
    );

    assert_ne!(generic.identity(), read.identity());
    assert_ne!(read.identity(), write.identity());
}

#[test]
fn workload_manifest_identity_changes_with_trace_data_cache_error_kind_expectations() {
    let generic = manifest_with_expected(
        "identity-trace-data-cache-error-kind",
        WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.error"))
            .with_minimum_trace_data_cache_error_count(1),
    );
    let write = manifest_with_expected(
        "identity-trace-data-cache-error-kind",
        WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.error"))
            .with_minimum_trace_data_cache_error_count(1)
            .with_minimum_trace_data_cache_write_error_count(1),
    );
    let functional_write = manifest_with_expected(
        "identity-trace-data-cache-error-kind",
        WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.error"))
            .with_minimum_trace_data_cache_error_count(1)
            .with_minimum_trace_data_cache_functional_write_error_count(1),
    );

    assert_ne!(generic.identity(), write.identity());
    assert_ne!(write.identity(), functional_write.identity());
}

#[test]
fn workload_replay_plan_rejects_underreported_trace_data_cache_error_kind_summary() {
    let expected =
        WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.cache.error.kind"))
            .with_minimum_trace_data_cache_error_count(1)
            .with_minimum_trace_data_cache_write_error_count(1)
            .with_minimum_memory_failure_count(1)
            .with_minimum_memory_failure_write_count(1);
    let manifest = manifest_with_expected("trace-data-cache-error-kind-mismatch", expected.clone());
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.cache.error.kind"), 1)
        .with_trace_data_cache_error_count(1)
        .with_memory_failure_count(1)
        .with_memory_failure_write_count(1);
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
        error
            .to_string()
            .contains("trace data-cache write errors 0/1"),
        "{error}"
    );
}

#[test]
fn workload_replay_plan_rejects_underreported_trace_data_cache_error_summary() {
    let expected = WorkloadExpectedTrafficTraceReplaySummary::new(route_id("trace.cache.error"))
        .with_minimum_trace_data_cache_error_count(2);
    let manifest = manifest_with_expected("trace-data-cache-error-mismatch", expected.clone());
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let actual = WorkloadTrafficTraceReplaySummary::new(route_id("trace.cache.error"), 1)
        .with_trace_data_cache_error_count(1);
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
        error.to_string().contains("trace data-cache errors 1/2"),
        "{error}"
    );
}
